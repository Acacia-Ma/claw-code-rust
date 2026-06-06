use std::collections::HashMap;
use std::time::Duration;

use super::*;

mod coordinator;
mod handlers;

const DEFAULT_WAIT_AGENT_TIMEOUT: Duration = Duration::from_secs(300);
const MAX_WAIT_AGENT_TIMEOUT: Duration = Duration::from_secs(900);
const RESULT_EXCERPT_LIMIT: usize = 600;

impl ServerRuntime {
    async fn spawn_agent_inner(
        self: &Arc<Self>,
        params: devo_protocol::SpawnAgentParams,
    ) -> Result<devo_protocol::SpawnAgentResult, ToolCallError> {
        let parent_session_id = params.session_id;
        let child_session_id = SessionId::new();
        let now = Utc::now();
        let fork_turns = params.fork_turns.as_deref().unwrap_or("all");
        if !matches!(fork_turns, "none" | "all") {
            return Err(ToolCallError::InvalidInput(
                "fork_turns must be \"none\" or \"all\"".to_string(),
            ));
        }

        let parent_arc = self.session_arc(parent_session_id).await?;
        let parent_snapshot = {
            let parent = parent_arc.lock().await;
            let parent_core = parent.core_session.lock().await;
            let stable_items = if fork_turns == "all" {
                let active_turn_id = parent.active_turn.as_ref().map(|turn| turn.turn_id);
                parent
                    .persisted_turn_items
                    .iter()
                    .filter(|item| active_turn_id.is_none_or(|turn_id| item.turn_id != turn_id))
                    .cloned()
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            };
            (
                parent.summary.clone(),
                parent_core.config.clone(),
                stable_items,
                parent.latest_turn.clone(),
            )
        };
        let (parent_summary, parent_config, stable_items, parent_latest_turn) = parent_snapshot;

        let nickname = sanitize_agent_name(&params.task_name);
        let role = params.agent_type.unwrap_or_else(|| "default".to_string());
        let parent_path = parent_summary
            .agent_path
            .clone()
            .unwrap_or_else(|| "root".to_string());
        let agent_path = AgentPath::new(parent_path).join(&nickname).0;
        let model = params.model.or_else(|| parent_summary.model.clone());
        let thinking = params.thinking.or_else(|| parent_summary.thinking.clone());

        let mut record = self.rollout_store.create_session_record(
            child_session_id,
            now,
            parent_summary.cwd.clone(),
            Some(nickname.clone()),
            model.clone(),
            thinking.clone(),
            self.deps.provider.name().to_string(),
            Some(parent_session_id),
        );
        record.agent_path = Some(agent_path.clone());
        record.agent_nickname = Some(nickname.clone());
        record.agent_role = Some(role.clone());
        record.first_user_message = Some(params.message.clone());
        self.rollout_store
            .append_session_meta(&record)
            .map_err(|error| ToolCallError::InternalError(error.to_string()))?;

        let mut core_session = self
            .deps
            .new_session_state(child_session_id, parent_summary.cwd.clone());
        core_session.config = parent_config;
        let mut rebuilt_history_items = Vec::new();
        let mut rebuilt_messages = Vec::new();
        let mut tool_names_by_id = HashMap::new();
        for item in &stable_items {
            crate::persistence::apply_turn_item(
                &mut rebuilt_messages,
                &mut rebuilt_history_items,
                &mut tool_names_by_id,
                item.turn_item.clone(),
            );
        }
        core_session.messages = rebuilt_messages;
        core_session.turn_count = stable_items
            .iter()
            .filter(|item| matches!(item.turn_item, TurnItem::UserMessage(_)))
            .count();
        let pending_turn_queue = Arc::clone(&core_session.pending_turn_queue);
        let btw_input_queue = Arc::clone(&core_session.btw_input_queue);
        let latest_turn = if stable_items.is_empty() {
            None
        } else {
            parent_latest_turn.map(|mut turn| {
                turn.session_id = child_session_id;
                turn
            })
        };
        let summary = SessionMetadata {
            session_id: child_session_id,
            cwd: parent_summary.cwd.clone(),
            created_at: now,
            updated_at: now,
            title: Some(nickname.clone()),
            title_state: SessionTitleState::Final(SessionTitleFinalSource::ExplicitCreate),
            parent_session_id: Some(parent_session_id),
            agent_path: Some(agent_path.clone()),
            agent_nickname: Some(nickname.clone()),
            agent_role: Some(role.clone()),
            ephemeral: false,
            model: model.clone(),
            thinking,
            reasoning_effort: None,
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_cache_creation_tokens: 0,
            total_cache_read_tokens: 0,
            prompt_token_estimate: core_session.prompt_token_estimate,
            last_query_total_tokens: 0,
            status: SessionRuntimeStatus::Idle,
        };
        let child_session = RuntimeSession {
            record: Some(record),
            summary: summary.clone(),
            core_session: Arc::new(Mutex::new(core_session)),
            active_turn: None,
            latest_turn,
            loaded_item_count: u64::try_from(stable_items.len()).unwrap_or(u64::MAX),
            history_items: rebuilt_history_items,
            persisted_turn_items: stable_items,
            latest_compaction_snapshot: None,
            pending_turn_queue,
            btw_input_queue,
            deferred_assistant: None,
            deferred_reasoning: None,
            next_item_seq: 1,
            first_user_input: Some(params.message.clone()),
            pending_approvals: HashMap::new(),
            session_approval_cache: crate::execution::ApprovalGrantCache::default(),
            turn_approval_cache: crate::execution::ApprovalGrantCache::default(),
        };
        self.sessions
            .lock()
            .await
            .insert(child_session_id, child_session.shared());
        self.agent_mailboxes
            .lock()
            .await
            .entry(parent_session_id)
            .or_default();
        self.agent_mailboxes
            .lock()
            .await
            .entry(child_session_id)
            .or_default();
        self.register_child_agent(
            parent_session_id,
            child_session_id,
            SubagentMetadata {
                session_id: child_session_id,
                parent_session_id,
                agent_path: agent_path.clone(),
                nickname: nickname.clone(),
                role: role.clone(),
                status: SubagentStatus::Spawning,
                spawned_at: now,
                closed_at: None,
                last_task_message: Some(params.message.clone()),
                close_requested: false,
            },
        )
        .await;
        if let Err(error) = self.deps.db.upsert_session(&summary) {
            tracing::warn!(
                session_id = %child_session_id,
                error = %error,
                "failed to persist child session metadata to database"
            );
        }
        self.broadcast_event(ServerEvent::SessionStarted(SessionEventPayload {
            session: summary,
        }))
        .await;
        self.start_runtime_turn(child_session_id, params.message.clone(), params.message)
            .await?;
        self.set_agent_status(parent_session_id, child_session_id, SubagentStatus::Running)
            .await;

        Ok(devo_protocol::SpawnAgentResult {
            child_session_id,
            agent_path,
            agent_nickname: nickname,
            status: SubagentStatus::Running.as_str().to_string(),
        })
    }

    async fn session_arc(
        &self,
        session_id: SessionId,
    ) -> Result<Arc<Mutex<RuntimeSession>>, ToolCallError> {
        self.sessions
            .lock()
            .await
            .get(&session_id)
            .cloned()
            .ok_or_else(|| ToolCallError::InvalidInput(format!("session not found: {session_id}")))
    }

    async fn mailbox(&self, session_id: SessionId) -> SubagentMailbox {
        self.agent_mailboxes
            .lock()
            .await
            .entry(session_id)
            .or_default()
            .clone()
    }

    async fn register_child_agent(
        &self,
        parent_session_id: SessionId,
        child_session_id: SessionId,
        metadata: SubagentMetadata,
    ) {
        self.agent_registries
            .lock()
            .await
            .entry(parent_session_id)
            .or_insert_with(AgentRegistry::new)
            .register(parent_session_id, child_session_id, metadata);
    }

    async fn set_agent_status(
        &self,
        parent_session_id: SessionId,
        child_session_id: SessionId,
        status: SubagentStatus,
    ) {
        if let Some(registry) = self
            .agent_registries
            .lock()
            .await
            .get_mut(&parent_session_id)
        {
            registry.update_status(child_session_id, status);
        }
    }

    async fn start_runtime_turn(
        self: &Arc<Self>,
        session_id: SessionId,
        display_input: String,
        input_text: String,
    ) -> Result<TurnMetadata, ToolCallError> {
        let Some(session_arc) = self.sessions.lock().await.get(&session_id).cloned() else {
            return Err(ToolCallError::InvalidInput(format!(
                "session not found: {session_id}"
            )));
        };
        let queued_active_turn = {
            let session = session_arc.lock().await;
            session.active_turn.as_ref().map(|turn| {
                (
                    turn.clone(),
                    Arc::clone(&session.pending_turn_queue),
                    session.summary.ephemeral,
                )
            })
        };
        if let Some((active_turn, pending_turn_queue, is_ephemeral)) = queued_active_turn {
            let item = devo_core::PendingInputItem {
                kind: devo_core::PendingInputKind::UserText { text: input_text },
                metadata: None,
                created_at: Utc::now(),
            };
            pending_turn_queue
                .lock()
                .expect("pending turn queue mutex should not be poisoned")
                .push_back(item.clone());
            if !is_ephemeral
                && let Err(error) = self
                    .deps
                    .db
                    .push_pending(&session_id, QueueType::Turn, &item)
            {
                tracing::warn!(
                    session_id = %session_id,
                    error = %error,
                    "failed to persist agent follow-up pending message"
                );
            }
            self.broadcast_updated_queue(session_id).await;
            return Ok(active_turn);
        }

        let (turn_config, resolved_request) = {
            let session = session_arc.lock().await;
            let turn_config = self.deps.resolve_turn_config(
                session.summary.model.as_deref(),
                session.summary.thinking.clone(),
            );
            let resolved_request = turn_config
                .model
                .resolve_thinking_selection(turn_config.thinking_selection.as_deref());
            (turn_config, resolved_request)
        };
        let request_model = turn_config.provider_request_model(&resolved_request.request_model);
        let now = Utc::now();
        let turn = {
            let mut session = session_arc.lock().await;
            let turn = TurnMetadata {
                turn_id: TurnId::new(),
                session_id,
                sequence: session
                    .latest_turn
                    .as_ref()
                    .map_or(1, |turn| turn.sequence + 1),
                status: TurnStatus::Running,
                kind: devo_core::TurnKind::Regular,
                model: turn_config.model.slug.clone(),
                thinking: turn_config.thinking_selection.clone(),
                reasoning_effort: resolved_request.effective_reasoning_effort,
                request_model,
                request_thinking: resolved_request.request_thinking.clone(),
                started_at: now,
                completed_at: None,
                usage: None,
            };
            session.summary.status = SessionRuntimeStatus::ActiveTurn;
            session.summary.updated_at = now;
            session.summary.model = Some(turn_config.model.slug.clone());
            session.summary.thinking = turn_config.thinking_selection.clone();
            session.active_turn = Some(turn.clone());
            turn
        };
        self.append_turn_start(session_id, &turn).await?;
        self.broadcast_event(ServerEvent::SessionStatusChanged(
            SessionStatusChangedPayload {
                session_id,
                status: SessionRuntimeStatus::ActiveTurn,
            },
        ))
        .await;
        self.broadcast_event(ServerEvent::TurnStarted(TurnEventPayload {
            session_id,
            turn: turn.clone(),
        }))
        .await;
        let runtime = Arc::clone(self);
        let turn_for_task = turn.clone();
        let turn_config_for_task = turn_config.clone();
        let task = tokio::spawn(async move {
            runtime
                .execute_turn(
                    session_id,
                    turn_for_task,
                    turn_config_for_task,
                    display_input,
                    input_text,
                )
                .await;
        });
        self.active_tasks
            .lock()
            .await
            .insert(session_id, task.abort_handle());
        Ok(turn)
    }

    async fn append_turn_start(
        &self,
        session_id: SessionId,
        turn: &TurnMetadata,
    ) -> Result<(), ToolCallError> {
        let session_arc = self
            .sessions
            .lock()
            .await
            .get(&session_id)
            .cloned()
            .ok_or_else(|| {
                ToolCallError::InvalidInput(format!("session not found: {session_id}"))
            })?;
        let (record, session_context, turn_context) = {
            let session = session_arc.lock().await;
            let core_session = session.core_session.lock().await;
            (
                session.record.clone(),
                core_session.session_context.clone(),
                core_session.latest_turn_context.clone(),
            )
        };
        if let Some(record) = record {
            self.rollout_store
                .append_turn(
                    &record,
                    build_turn_record(turn, session_context, turn_context),
                )
                .map_err(|error| ToolCallError::InternalError(error.to_string()))?;
        }
        Ok(())
    }

    async fn queue_agent_message(
        &self,
        from_session_id: SessionId,
        target: &str,
        content: String,
    ) -> Result<AgentRoute, ToolCallError> {
        let route = self.resolve_agent_route(from_session_id, target).await?;
        let message = devo_protocol::AgentMailboxMessage {
            message_id: String::new(),
            from_session_id,
            to_session_id: route.to_session_id,
            from_agent_path: route.from_agent_path.clone(),
            to_agent_path: route.to_agent_path.clone(),
            content,
            sequence: 0,
            created_at: Utc::now(),
        };
        self.mailbox(route.to_session_id)
            .await
            .send(message)
            .await
            .map_err(|error| ToolCallError::InternalError(error.to_string()))?;
        Ok(route)
    }

    async fn resolve_child_agent(
        &self,
        parent_session_id: SessionId,
        target: &str,
    ) -> Result<SubagentMetadata, ToolCallError> {
        let registries = self.agent_registries.lock().await;
        let Some(registry) = registries.get(&parent_session_id) else {
            return Err(ToolCallError::InvalidInput(format!(
                "agent not found: {target}"
            )));
        };
        let Some(child_session_id) = registry.find_child(parent_session_id, target) else {
            return Err(ToolCallError::InvalidInput(format!(
                "agent not found: {target}"
            )));
        };
        registry
            .get(child_session_id)
            .cloned()
            .ok_or_else(|| ToolCallError::InvalidInput(format!("agent not found: {target}")))
    }

    async fn agent_info(
        &self,
        parent_session_id: SessionId,
        target: &str,
    ) -> Result<devo_protocol::AgentInfo, ToolCallError> {
        Ok(self
            .resolve_child_agent(parent_session_id, target)
            .await?
            .to_agent_info())
    }

    async fn resolve_agent_route(
        &self,
        from_session_id: SessionId,
        target: &str,
    ) -> Result<AgentRoute, ToolCallError> {
        if let Ok(child) = self.resolve_child_agent(from_session_id, target).await {
            let from_path = self.session_agent_path(from_session_id).await;
            return Ok(AgentRoute {
                parent_session_id: from_session_id,
                to_session_id: child.session_id,
                from_agent_path: from_path,
                to_agent_path: child.agent_path,
            });
        }

        let registries = self.agent_registries.lock().await;
        for registry in registries.values() {
            if let Some(parent_session_id) = registry.child_to_parent.get(&from_session_id)
                && is_parent_target(*parent_session_id, target)
            {
                let from_path = registry
                    .get(from_session_id)
                    .map(|meta| meta.agent_path.clone())
                    .unwrap_or_else(|| "root".to_string());
                return Ok(AgentRoute {
                    parent_session_id: *parent_session_id,
                    to_session_id: *parent_session_id,
                    from_agent_path: from_path,
                    to_agent_path: self.session_agent_path(*parent_session_id).await,
                });
            }
        }
        Err(ToolCallError::InvalidInput(format!(
            "agent not found: {target}"
        )))
    }

    async fn session_agent_path(&self, session_id: SessionId) -> String {
        let Some(session_arc) = self.sessions.lock().await.get(&session_id).cloned() else {
            return "root".to_string();
        };
        let session = session_arc.lock().await;
        session
            .summary
            .agent_path
            .clone()
            .unwrap_or_else(|| "root".to_string())
    }

    pub(super) async fn handle_subagent_turn_completed(
        &self,
        child_session_id: SessionId,
        turn: &TurnMetadata,
    ) {
        let Some((parent_session_id, agent_path)) =
            self.child_parent_and_path(child_session_id).await
        else {
            return;
        };
        let status = match turn.status {
            TurnStatus::Completed => SubagentStatus::Completed,
            TurnStatus::Interrupted => SubagentStatus::Interrupted,
            TurnStatus::Failed => SubagentStatus::Failed,
            TurnStatus::Pending | TurnStatus::Running | TurnStatus::WaitingApproval => {
                SubagentStatus::Running
            }
        };
        let status = if self
            .agent_close_requested(parent_session_id, child_session_id)
            .await
        {
            SubagentStatus::Closed
        } else {
            status
        };
        self.set_agent_status(parent_session_id, child_session_id, status)
            .await;
        let result_excerpt = self.child_result_excerpt(child_session_id).await;
        let notification = devo_protocol::AgentCompletionNotification {
            child_session_id,
            parent_session_id,
            agent_path: agent_path.clone(),
            status: status.as_str().to_string(),
            turn_id: turn.turn_id,
            result_excerpt: (!result_excerpt.is_empty()).then_some(result_excerpt),
        };
        let content = serde_json::to_string(&notification)
            .unwrap_or_else(|error| format!("failed to serialize agent notification: {error}"));
        let message = devo_protocol::AgentMailboxMessage {
            message_id: String::new(),
            from_session_id: child_session_id,
            to_session_id: parent_session_id,
            from_agent_path: agent_path,
            to_agent_path: self.session_agent_path(parent_session_id).await,
            content,
            sequence: 0,
            created_at: Utc::now(),
        };
        if let Err(error) = self.mailbox(parent_session_id).await.send(message).await {
            tracing::warn!(
                parent_session_id = %parent_session_id,
                child_session_id = %child_session_id,
                error = %error,
                "failed to notify parent mailbox about child completion"
            );
        }
    }

    async fn child_parent_and_path(
        &self,
        child_session_id: SessionId,
    ) -> Option<(SessionId, String)> {
        let registries = self.agent_registries.lock().await;
        registries.values().find_map(|registry| {
            let parent_session_id = registry.child_to_parent.get(&child_session_id).copied()?;
            let agent_path = registry.get(child_session_id)?.agent_path.clone();
            Some((parent_session_id, agent_path))
        })
    }

    async fn agent_close_requested(
        &self,
        parent_session_id: SessionId,
        child_session_id: SessionId,
    ) -> bool {
        self.agent_registries
            .lock()
            .await
            .get(&parent_session_id)
            .and_then(|registry| registry.get(child_session_id))
            .is_some_and(|metadata| metadata.close_requested)
    }

    async fn child_result_excerpt(&self, child_session_id: SessionId) -> String {
        let Some(session_arc) = self.sessions.lock().await.get(&child_session_id).cloned() else {
            return String::new();
        };
        let session = session_arc.lock().await;
        let excerpt = session
            .persisted_turn_items
            .iter()
            .rev()
            .find_map(|item| match &item.turn_item {
                TurnItem::AgentMessage(TextItem { text }) => Some(text.clone()),
                TurnItem::ToolResult(ToolResultItem {
                    display_content: Some(text),
                    ..
                }) => Some(text.clone()),
                _ => None,
            })
            .unwrap_or_default();
        truncate_chars(&excerpt, RESULT_EXCERPT_LIMIT)
    }

    async fn close_child_agent(
        self: &Arc<Self>,
        parent_session_id: SessionId,
        child_session_id: SessionId,
    ) -> Result<String, ToolCallError> {
        let already_terminal = {
            let mut registries = self.agent_registries.lock().await;
            let Some(registry) = registries.get_mut(&parent_session_id) else {
                return Err(ToolCallError::InvalidInput(format!(
                    "agent not found: {child_session_id}"
                )));
            };
            let Some(metadata) = registry.agents.get_mut(&child_session_id) else {
                return Err(ToolCallError::InvalidInput(format!(
                    "agent not found: {child_session_id}"
                )));
            };
            let terminal = matches!(
                metadata.status,
                SubagentStatus::Completed
                    | SubagentStatus::Failed
                    | SubagentStatus::Interrupted
                    | SubagentStatus::Canceled
                    | SubagentStatus::Closed
            );
            metadata.close_requested = true;
            if !terminal {
                metadata.status = SubagentStatus::Closed;
                metadata.closed_at = Some(Utc::now());
            }
            terminal
        };
        if already_terminal {
            let status = self
                .resolve_child_agent(parent_session_id, &child_session_id.to_string())
                .await?
                .status
                .as_str()
                .to_string();
            return Ok(status);
        }

        if let Some(task) = self.active_tasks.lock().await.remove(&child_session_id) {
            task.abort();
        }
        let interrupted_turn = {
            let Some(session_arc) = self.sessions.lock().await.get(&child_session_id).cloned()
            else {
                return Ok(SubagentStatus::Closed.as_str().to_string());
            };
            let mut session = session_arc.lock().await;
            session.summary.status = SessionRuntimeStatus::Idle;
            session.summary.updated_at = Utc::now();
            session.active_turn.take().map(|mut turn| {
                turn.status = TurnStatus::Interrupted;
                turn.completed_at = Some(Utc::now());
                session.latest_turn = Some(turn.clone());
                turn
            })
        };
        if let Some(turn) = interrupted_turn {
            self.broadcast_event(ServerEvent::TurnInterrupted(TurnEventPayload {
                session_id: child_session_id,
                turn: turn.clone(),
            }))
            .await;
            self.broadcast_event(ServerEvent::TurnCompleted(TurnEventPayload {
                session_id: child_session_id,
                turn: turn.clone(),
            }))
            .await;
            self.handle_subagent_turn_completed(child_session_id, &turn)
                .await;
        } else {
            self.send_closed_notification(parent_session_id, child_session_id)
                .await;
        }
        self.broadcast_event(ServerEvent::SessionStatusChanged(
            SessionStatusChangedPayload {
                session_id: child_session_id,
                status: SessionRuntimeStatus::Idle,
            },
        ))
        .await;
        Ok(SubagentStatus::Closed.as_str().to_string())
    }

    async fn send_closed_notification(
        &self,
        parent_session_id: SessionId,
        child_session_id: SessionId,
    ) {
        let agent_path = self.session_agent_path(child_session_id).await;
        let notification = devo_protocol::AgentCompletionNotification {
            child_session_id,
            parent_session_id,
            agent_path: agent_path.clone(),
            status: SubagentStatus::Closed.as_str().to_string(),
            turn_id: TurnId::new(),
            result_excerpt: Some("agent closed".to_string()),
        };
        let content = serde_json::to_string(&notification)
            .unwrap_or_else(|error| format!("failed to serialize agent notification: {error}"));
        let message = devo_protocol::AgentMailboxMessage {
            message_id: String::new(),
            from_session_id: child_session_id,
            to_session_id: parent_session_id,
            from_agent_path: agent_path,
            to_agent_path: self.session_agent_path(parent_session_id).await,
            content,
            sequence: 0,
            created_at: Utc::now(),
        };
        if let Err(error) = self.mailbox(parent_session_id).await.send(message).await {
            tracing::warn!(
                parent_session_id = %parent_session_id,
                child_session_id = %child_session_id,
                error = %error,
                "failed to notify parent mailbox about closed child"
            );
        }
    }
}

struct AgentRoute {
    parent_session_id: SessionId,
    to_session_id: SessionId,
    from_agent_path: String,
    to_agent_path: String,
}

fn sanitize_agent_name(name: &str) -> String {
    let mut sanitized = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    while sanitized.contains("--") {
        sanitized = sanitized.replace("--", "-");
    }
    let sanitized = sanitized.trim_matches('-').to_string();
    if sanitized.is_empty() {
        "agent".to_string()
    } else {
        sanitized
    }
}

fn is_parent_target(parent_session_id: SessionId, target: &str) -> bool {
    target == "parent" || target == "root" || target == parent_session_id.to_string()
}

fn truncate_chars(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let mut truncated = text.chars().take(limit).collect::<String>();
    truncated.push_str("...");
    truncated
}
