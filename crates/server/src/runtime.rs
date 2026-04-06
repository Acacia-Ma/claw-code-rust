use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

use chrono::Utc;
use tokio::sync::{mpsc, Mutex};

use clawcr_core::{
    query, ItemId, Message, QueryEvent, SessionId, SessionTitleFinalSource, SessionTitleState,
    TurnId, TurnStatus,
};
use clawcr_tools::ToolOrchestrator;

use crate::{
    execution::{RuntimeSession, ServerRuntimeDependencies},
    ClientTransportKind, ConnectionState, ErrorResponse, EventContext, EventsSubscribeParams,
    EventsSubscribeResult, InitializeParams, InitializeResult, ItemDeltaKind, ItemDeltaPayload,
    ItemEnvelope, ItemEventPayload, ItemKind, NotificationEnvelope, ProtocolError,
    ProtocolErrorCode, ServerCapabilities, ServerEvent, ServerRequestResolvedPayload,
    SessionEventPayload, SessionForkParams, SessionForkResult, SessionResumeParams,
    SessionResumeResult, SessionRuntimeStatus, SessionStartParams, SessionStartResult,
    SessionStatusChangedPayload, SteerInputRecord, SuccessResponse, TurnEventPayload,
    TurnInterruptParams, TurnInterruptResult, TurnStartParams, TurnStartResult, TurnSteerParams,
    TurnSteerResult, TurnSummary,
};

pub struct ServerRuntime {
    metadata: InitializeResult,
    deps: ServerRuntimeDependencies,
    sessions: Mutex<HashMap<SessionId, Arc<Mutex<RuntimeSession>>>>,
    connections: Mutex<HashMap<u64, ConnectionRuntime>>,
    active_tasks: Mutex<HashMap<SessionId, tokio::task::AbortHandle>>,
    next_connection_id: AtomicU64,
}

impl ServerRuntime {
    pub fn new(server_home: PathBuf, deps: ServerRuntimeDependencies) -> Arc<Self> {
        Arc::new(Self {
            metadata: InitializeResult {
                server_name: "clawcr-server".into(),
                server_version: env!("CARGO_PKG_VERSION").into(),
                platform_family: std::env::consts::FAMILY.into(),
                platform_os: std::env::consts::OS.into(),
                server_home,
                capabilities: ServerCapabilities {
                    session_resume: true,
                    session_fork: true,
                    turn_interrupt: true,
                    approval_requests: true,
                    event_streaming: true,
                },
            },
            deps,
            sessions: Mutex::new(HashMap::new()),
            connections: Mutex::new(HashMap::new()),
            active_tasks: Mutex::new(HashMap::new()),
            next_connection_id: AtomicU64::new(1),
        })
    }

    pub async fn register_connection(
        self: &Arc<Self>,
        transport: ClientTransportKind,
        sender: mpsc::UnboundedSender<serde_json::Value>,
    ) -> u64 {
        let connection_id = self.next_connection_id.fetch_add(1, Ordering::SeqCst);
        self.connections.lock().await.insert(
            connection_id,
            ConnectionRuntime {
                transport,
                state: ConnectionState::Connected,
                sender,
                opt_out_notification_methods: HashSet::new(),
                subscriptions: Vec::new(),
                next_event_seq: 1,
            },
        );
        connection_id
    }

    pub async fn unregister_connection(&self, connection_id: u64) {
        self.connections.lock().await.remove(&connection_id);
    }

    pub async fn handle_incoming(
        self: &Arc<Self>,
        connection_id: u64,
        message: serde_json::Value,
    ) -> Option<serde_json::Value> {
        let method = message.get("method")?.as_str()?.to_string();
        let id = message.get("id").cloned();
        let params = message
            .get("params")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));

        if method == "initialized" {
            if let Some(connection) = self.connections.lock().await.get_mut(&connection_id) {
                connection.state = ConnectionState::Ready;
            }
            return None;
        }
        if method == "initialize" {
            return Some(self.handle_initialize(connection_id, id, params).await);
        }
        if !self.connection_ready(connection_id).await {
            return id.map(|request_id| {
                self.error_response(
                    request_id,
                    ProtocolErrorCode::NotInitialized,
                    "connection has not completed initialize/initialized",
                )
            });
        }

        match method.as_str() {
            "session/start" => Some(self.handle_session_start(connection_id, id?, params).await),
            "session/resume" => Some(self.handle_session_resume(connection_id, id?, params).await),
            "session/fork" => Some(self.handle_session_fork(connection_id, id?, params).await),
            "turn/start" => Some(self.handle_turn_start(id?, params).await),
            "turn/interrupt" => Some(self.handle_turn_interrupt(id?, params).await),
            "turn/steer" => Some(self.handle_turn_steer(connection_id, id?, params).await),
            "approval/respond" => Some(self.error_response(
                id?,
                ProtocolErrorCode::ApprovalNotFound,
                "no pending approval request exists for this runtime",
            )),
            "events/subscribe" => Some(
                self.handle_events_subscribe(connection_id, id?, params)
                    .await,
            ),
            _ => Some(self.error_response(
                id?,
                ProtocolErrorCode::InvalidParams,
                format!("unknown method: {method}"),
            )),
        }
    }

    async fn handle_initialize(
        &self,
        connection_id: u64,
        id: Option<serde_json::Value>,
        params: serde_json::Value,
    ) -> serde_json::Value {
        let request_id = id.unwrap_or_else(|| serde_json::json!(null));
        match serde_json::from_value::<InitializeParams>(params) {
            Ok(params) => {
                if let Some(connection) = self.connections.lock().await.get_mut(&connection_id) {
                    connection.state = ConnectionState::Initializing;
                    connection.transport = params.transport;
                    connection.opt_out_notification_methods =
                        params.opt_out_notification_methods.into_iter().collect();
                }
                serde_json::to_value(SuccessResponse {
                    id: request_id,
                    result: self.metadata.clone(),
                })
                .expect("serialize initialize result")
            }
            Err(error) => self.error_response(
                request_id,
                ProtocolErrorCode::InvalidParams,
                format!("invalid initialize params: {error}"),
            ),
        }
    }

    async fn handle_session_start(
        &self,
        connection_id: u64,
        request_id: serde_json::Value,
        params: serde_json::Value,
    ) -> serde_json::Value {
        let params: SessionStartParams = match serde_json::from_value(params) {
            Ok(params) => params,
            Err(error) => {
                return self.error_response(
                    request_id,
                    ProtocolErrorCode::InvalidParams,
                    format!("invalid session/start params: {error}"),
                )
            }
        };

        let now = Utc::now();
        let session_id = SessionId::new();
        let resolved_model = params
            .model
            .clone()
            .unwrap_or_else(|| self.deps.default_model.clone());
        let summary = crate::SessionSummary {
            session_id,
            cwd: params.cwd.clone(),
            created_at: now,
            updated_at: now,
            title: params.title.clone(),
            title_state: params
                .title
                .as_ref()
                .map(|_| SessionTitleState::Final(SessionTitleFinalSource::ExplicitCreate))
                .unwrap_or(SessionTitleState::Unset),
            ephemeral: params.ephemeral,
            resolved_model: Some(resolved_model.clone()),
            status: SessionRuntimeStatus::Idle,
        };
        self.sessions.lock().await.insert(
            session_id,
            RuntimeSession {
                summary: summary.clone(),
                core_session: Arc::new(Mutex::new(self.deps.new_session_state(
                    session_id,
                    params.cwd.clone(),
                    Some(resolved_model.clone()),
                ))),
                active_turn: None,
                latest_turn: None,
                loaded_item_count: 0,
                steering_queue: std::collections::VecDeque::new(),
                active_task: None,
                next_item_seq: 1,
            }
            .shared(),
        );
        self.subscribe_connection_to_session(connection_id, session_id, None)
            .await;
        self.broadcast_event(ServerEvent::SessionStarted(SessionEventPayload {
            session: summary.clone(),
        }))
        .await;

        serde_json::to_value(SuccessResponse {
            id: request_id,
            result: SessionStartResult {
                session_id,
                created_at: now,
                cwd: params.cwd,
                ephemeral: params.ephemeral,
                resolved_model: Some(resolved_model),
            },
        })
        .expect("serialize session/start response")
    }

    async fn handle_session_resume(
        &self,
        connection_id: u64,
        request_id: serde_json::Value,
        params: serde_json::Value,
    ) -> serde_json::Value {
        let params: SessionResumeParams = match serde_json::from_value(params) {
            Ok(params) => params,
            Err(error) => {
                return self.error_response(
                    request_id,
                    ProtocolErrorCode::InvalidParams,
                    format!("invalid session/resume params: {error}"),
                )
            }
        };
        let Some(session_arc) = self.sessions.lock().await.get(&params.session_id).cloned() else {
            return self.error_response(
                request_id,
                ProtocolErrorCode::SessionNotFound,
                "session does not exist",
            );
        };
        let session = session_arc.lock().await;
        let session_summary = session.summary.clone();
        let latest_turn = session.latest_turn.clone();
        let loaded_item_count = session.loaded_item_count;
        drop(session);
        self.subscribe_connection_to_session(connection_id, params.session_id, None)
            .await;
        serde_json::to_value(SuccessResponse {
            id: request_id,
            result: SessionResumeResult {
                session: session_summary,
                latest_turn,
                loaded_item_count,
            },
        })
        .expect("serialize session/resume response")
    }

    async fn handle_session_fork(
        &self,
        connection_id: u64,
        request_id: serde_json::Value,
        params: serde_json::Value,
    ) -> serde_json::Value {
        let params: SessionForkParams = match serde_json::from_value(params) {
            Ok(params) => params,
            Err(error) => {
                return self.error_response(
                    request_id,
                    ProtocolErrorCode::InvalidParams,
                    format!("invalid session/fork params: {error}"),
                )
            }
        };
        let Some(source_arc) = self.sessions.lock().await.get(&params.session_id).cloned() else {
            return self.error_response(
                request_id,
                ProtocolErrorCode::SessionNotFound,
                "session does not exist",
            );
        };
        let source = source_arc.lock().await;
        let source_core_session = source.core_session.lock().await;
        let now = Utc::now();
        let forked_id = SessionId::new();
        let fork_cwd = params.cwd.unwrap_or_else(|| source.summary.cwd.clone());
        let fork_model = source
            .summary
            .resolved_model
            .clone()
            .unwrap_or_else(|| self.deps.default_model.clone());
        let summary = crate::SessionSummary {
            session_id: forked_id,
            cwd: fork_cwd.clone(),
            created_at: now,
            updated_at: now,
            title: params.title.or_else(|| source.summary.title.clone()),
            title_state: source.summary.title_state.clone(),
            ephemeral: source.summary.ephemeral,
            resolved_model: Some(fork_model.clone()),
            status: SessionRuntimeStatus::Idle,
        };
        let mut core_session = self
            .deps
            .new_session_state(forked_id, fork_cwd, Some(fork_model));
        core_session.messages = source_core_session.messages.clone();
        core_session.turn_count = source_core_session.turn_count;
        core_session.total_input_tokens = source_core_session.total_input_tokens;
        core_session.total_output_tokens = source_core_session.total_output_tokens;
        core_session.total_cache_creation_tokens = source_core_session.total_cache_creation_tokens;
        core_session.total_cache_read_tokens = source_core_session.total_cache_read_tokens;
        core_session.last_input_tokens = source_core_session.last_input_tokens;
        let latest_turn = source.latest_turn.clone();
        let loaded_item_count = source.loaded_item_count;
        drop(source_core_session);
        drop(source);
        self.sessions.lock().await.insert(
            forked_id,
            RuntimeSession {
                summary: summary.clone(),
                core_session: Arc::new(Mutex::new(core_session)),
                active_turn: None,
                latest_turn,
                loaded_item_count,
                steering_queue: std::collections::VecDeque::new(),
                active_task: None,
                next_item_seq: loaded_item_count + 1,
            }
            .shared(),
        );
        self.subscribe_connection_to_session(connection_id, forked_id, None)
            .await;
        self.broadcast_event(ServerEvent::SessionStarted(SessionEventPayload {
            session: summary.clone(),
        }))
        .await;
        serde_json::to_value(SuccessResponse {
            id: request_id,
            result: SessionForkResult {
                session: summary,
                forked_from_session_id: params.session_id,
            },
        })
        .expect("serialize session/fork response")
    }

    async fn handle_turn_start(
        self: &Arc<Self>,
        request_id: serde_json::Value,
        params: serde_json::Value,
    ) -> serde_json::Value {
        let params: TurnStartParams = match serde_json::from_value(params) {
            Ok(params) => params,
            Err(error) => {
                return self.error_response(
                    request_id,
                    ProtocolErrorCode::InvalidParams,
                    format!("invalid turn/start params: {error}"),
                )
            }
        };
        if params.input.is_empty() {
            return self.error_response(
                request_id,
                ProtocolErrorCode::EmptyInput,
                "turn input is empty",
            );
        }
        let Some(input_text) = render_input_items(&params.input) else {
            return self.error_response(
                request_id,
                ProtocolErrorCode::EmptyInput,
                "turn input is empty",
            );
        };
        let Some(session_arc) = self.sessions.lock().await.get(&params.session_id).cloned() else {
            return self.error_response(
                request_id,
                ProtocolErrorCode::SessionNotFound,
                "session does not exist",
            );
        };

        let now = Utc::now();
        let turn = {
            let mut session = session_arc.lock().await;
            if session.active_turn.is_some() {
                return self.error_response(
                    request_id,
                    ProtocolErrorCode::TurnAlreadyRunning,
                    "session already has an active turn",
                );
            }
            if let Some(cwd) = params.cwd.clone() {
                session.summary.cwd = cwd.clone();
                session.core_session.lock().await.cwd = cwd;
            }
            if let Some(model) = params.model.clone() {
                session.summary.resolved_model = Some(model.clone());
                session.core_session.lock().await.config.model = model;
            }
            let turn = TurnSummary {
                turn_id: TurnId::new(),
                session_id: params.session_id,
                sequence: session
                    .latest_turn
                    .as_ref()
                    .map_or(1, |turn| turn.sequence + 1),
                status: TurnStatus::Running,
                model_slug: session
                    .summary
                    .resolved_model
                    .clone()
                    .unwrap_or_else(|| self.deps.default_model.clone()),
                started_at: now,
                completed_at: None,
            };
            session.summary.status = SessionRuntimeStatus::ActiveTurn;
            session.summary.updated_at = now;
            session.active_turn = Some(turn.clone());
            session.steering_queue.clear();
            let runtime = Arc::clone(self);
            let turn_for_task = turn.clone();
            let input_for_task = input_text.clone();
            let task = tokio::spawn(async move {
                runtime
                    .execute_turn(params.session_id, turn_for_task, input_for_task)
                    .await;
            });
            self.active_tasks
                .lock()
                .await
                .insert(params.session_id, task.abort_handle());
            turn
        };

        self.broadcast_event(ServerEvent::SessionStatusChanged(
            SessionStatusChangedPayload {
                session_id: params.session_id,
                status: SessionRuntimeStatus::ActiveTurn,
            },
        ))
        .await;
        self.broadcast_event(ServerEvent::TurnStarted(TurnEventPayload {
            session_id: params.session_id,
            turn: turn.clone(),
        }))
        .await;

        serde_json::to_value(SuccessResponse {
            id: request_id,
            result: TurnStartResult {
                turn_id: turn.turn_id,
                status: turn.status.clone(),
                accepted_at: now,
            },
        })
        .expect("serialize turn/start response")
    }

    async fn handle_turn_interrupt(
        &self,
        request_id: serde_json::Value,
        params: serde_json::Value,
    ) -> serde_json::Value {
        let params: TurnInterruptParams = match serde_json::from_value(params) {
            Ok(params) => params,
            Err(error) => {
                return self.error_response(
                    request_id,
                    ProtocolErrorCode::InvalidParams,
                    format!("invalid turn/interrupt params: {error}"),
                )
            }
        };
        let Some(session_arc) = self.sessions.lock().await.get(&params.session_id).cloned() else {
            return self.error_response(
                request_id,
                ProtocolErrorCode::SessionNotFound,
                "session does not exist",
            );
        };
        if let Some(task) = self.active_tasks.lock().await.remove(&params.session_id) {
            task.abort();
        }
        let interrupted_turn = {
            let mut session = session_arc.lock().await;
            let Some(mut turn) = session.active_turn.take() else {
                return self.error_response(
                    request_id,
                    ProtocolErrorCode::TurnNotFound,
                    "turn is not active",
                );
            };
            if turn.turn_id != params.turn_id {
                session.active_turn = Some(turn);
                return self.error_response(
                    request_id,
                    ProtocolErrorCode::TurnNotFound,
                    "turn does not exist",
                );
            }
            turn.status = TurnStatus::Interrupted;
            turn.completed_at = Some(Utc::now());
            session.latest_turn = Some(turn.clone());
            session.summary.status = SessionRuntimeStatus::Idle;
            session.summary.updated_at = Utc::now();
            turn
        };

        self.broadcast_event(ServerEvent::TurnInterrupted(TurnEventPayload {
            session_id: params.session_id,
            turn: interrupted_turn.clone(),
        }))
        .await;
        self.broadcast_event(ServerEvent::TurnCompleted(TurnEventPayload {
            session_id: params.session_id,
            turn: interrupted_turn.clone(),
        }))
        .await;
        self.broadcast_event(ServerEvent::SessionStatusChanged(
            SessionStatusChangedPayload {
                session_id: params.session_id,
                status: SessionRuntimeStatus::Idle,
            },
        ))
        .await;

        serde_json::to_value(SuccessResponse {
            id: request_id,
            result: TurnInterruptResult {
                turn_id: interrupted_turn.turn_id,
                status: interrupted_turn.status,
            },
        })
        .expect("serialize turn/interrupt response")
    }

    async fn handle_turn_steer(
        &self,
        connection_id: u64,
        request_id: serde_json::Value,
        params: serde_json::Value,
    ) -> serde_json::Value {
        let params: TurnSteerParams = match serde_json::from_value(params) {
            Ok(params) => params,
            Err(error) => {
                return self.error_response(
                    request_id,
                    ProtocolErrorCode::InvalidParams,
                    format!("invalid turn/steer params: {error}"),
                )
            }
        };
        if params.input.is_empty() {
            return self.error_response(
                request_id,
                ProtocolErrorCode::EmptyInput,
                "turn steer input is empty",
            );
        }
        let Some(session_arc) = self.sessions.lock().await.get(&params.session_id).cloned() else {
            return self.error_response(
                request_id,
                ProtocolErrorCode::SessionNotFound,
                "session does not exist",
            );
        };
        let turn_id = {
            let mut session = session_arc.lock().await;
            let Some(turn_id) = session.active_turn.as_ref().map(|turn| turn.turn_id) else {
                return self.error_response(
                    request_id,
                    ProtocolErrorCode::NoActiveTurn,
                    "no active turn exists",
                );
            };
            if turn_id != params.expected_turn_id {
                return self.error_response(
                    request_id,
                    ProtocolErrorCode::ExpectedTurnMismatch,
                    "active turn did not match expectedTurnId",
                );
            }
            session.steering_queue.push_back(SteerInputRecord {
                item_id: ItemId::new(),
                received_at: Utc::now(),
                input: params.input,
            });
            turn_id
        };

        self.emit_to_connection(
            connection_id,
            "serverRequest/resolved",
            ServerEvent::ServerRequestResolved(ServerRequestResolvedPayload {
                session_id: params.session_id,
                request_id: "steer-accepted".into(),
                turn_id: Some(turn_id),
            }),
        )
        .await;
        serde_json::to_value(SuccessResponse {
            id: request_id,
            result: TurnSteerResult { turn_id },
        })
        .expect("serialize turn/steer response")
    }

    async fn handle_events_subscribe(
        &self,
        connection_id: u64,
        request_id: serde_json::Value,
        params: serde_json::Value,
    ) -> serde_json::Value {
        let params: EventsSubscribeParams = match serde_json::from_value(params) {
            Ok(params) => params,
            Err(error) => {
                return self.error_response(
                    request_id,
                    ProtocolErrorCode::InvalidParams,
                    format!("invalid events/subscribe params: {error}"),
                )
            }
        };
        if let Some(connection) = self.connections.lock().await.get_mut(&connection_id) {
            connection.subscriptions.push(SubscriptionFilter {
                session_id: params.session_id,
                event_types: params.event_types.unwrap_or_default().into_iter().collect(),
            });
        }
        serde_json::to_value(SuccessResponse {
            id: request_id,
            result: EventsSubscribeResult {
                subscription_id: format!("sub-{connection_id}-1").into(),
            },
        })
        .expect("serialize events/subscribe response")
    }

    async fn execute_turn(
        self: Arc<Self>,
        session_id: SessionId,
        turn: TurnSummary,
        input: String,
    ) {
        self.emit_text_item(
            session_id,
            turn.turn_id,
            ItemKind::UserMessage,
            "You",
            input.clone(),
        )
        .await;

        let Some(session_arc) = self.sessions.lock().await.get(&session_id).cloned() else {
            return;
        };
        let (event_tx, mut event_rx) = mpsc::unbounded_channel::<QueryEvent>();
        let runtime = Arc::clone(&self);
        let turn_for_events = turn.clone();
        let event_task = tokio::spawn(async move {
            let mut assistant_item_id = None;
            let mut assistant_text = String::new();
            while let Some(event) = event_rx.recv().await {
                match event {
                    QueryEvent::TextDelta(text) => {
                        let item_id = match assistant_item_id {
                            Some(item_id) => item_id,
                            None => {
                                let item_id = ItemId::new();
                                runtime
                                    .emit_item_started(
                                        session_id,
                                        turn_for_events.turn_id,
                                        item_id,
                                        ItemKind::AgentMessage,
                                        serde_json::json!({ "title": "Assistant", "text": "" }),
                                    )
                                    .await;
                                assistant_item_id = Some(item_id);
                                item_id
                            }
                        };
                        assistant_text.push_str(&text);
                        runtime
                            .broadcast_event(ServerEvent::ItemDelta {
                                delta_kind: ItemDeltaKind::AgentMessageDelta,
                                payload: ItemDeltaPayload {
                                    context: EventContext {
                                        session_id,
                                        turn_id: Some(turn_for_events.turn_id),
                                        item_id: Some(item_id),
                                        seq: 0,
                                    },
                                    delta: text,
                                    stream_index: None,
                                    channel: None,
                                },
                            })
                            .await;
                    }
                    QueryEvent::ToolUseStart { id, name, input } => {
                        runtime
                            .emit_item_pair(
                                session_id,
                                turn_for_events.turn_id,
                                ItemKind::ToolCall,
                                serde_json::json!({
                                    "tool_use_id": id,
                                    "tool_name": name,
                                    "input": input,
                                }),
                            )
                            .await;
                    }
                    QueryEvent::ToolResult {
                        tool_use_id,
                        content,
                        is_error,
                    } => {
                        runtime
                            .emit_item_pair(
                                session_id,
                                turn_for_events.turn_id,
                                ItemKind::ToolResult,
                                serde_json::json!({
                                    "tool_use_id": tool_use_id,
                                    "content": content,
                                    "is_error": is_error,
                                }),
                            )
                            .await;
                    }
                    QueryEvent::Usage { .. } | QueryEvent::TurnComplete { .. } => {}
                }
            }
            if let Some(item_id) = assistant_item_id {
                runtime
                    .emit_item_completed(
                        session_id,
                        turn_for_events.turn_id,
                        item_id,
                        ItemKind::AgentMessage,
                        serde_json::json!({ "title": "Assistant", "text": assistant_text }),
                    )
                    .await;
            }
        });

        let result = {
            let core_session = {
                let session = session_arc.lock().await;
                Arc::clone(&session.core_session)
            };
            let mut core_session = core_session.lock().await;
            core_session.push_message(Message::user(input.clone()));
            let event_callback_tx = event_tx.clone();
            let callback = std::sync::Arc::new(move |event: QueryEvent| {
                let _ = event_callback_tx.send(event);
            });
            let registry = Arc::clone(&self.deps.registry);
            let orchestrator = ToolOrchestrator::new(Arc::clone(&registry));
            query(
                &mut core_session,
                self.deps.provider.as_ref(),
                registry,
                &orchestrator,
                Some(callback),
            )
            .await
        };
        drop(event_tx);
        let _ = event_task.await;
        self.active_tasks.lock().await.remove(&session_id);

        let final_turn = {
            let mut session = session_arc.lock().await;
            let mut final_turn = turn.clone();
            final_turn.completed_at = Some(Utc::now());
            final_turn.status = if result.is_ok() {
                TurnStatus::Completed
            } else {
                TurnStatus::Failed
            };
            session.latest_turn = Some(final_turn.clone());
            session.active_turn = None;
            session.active_task = None;
            session.summary.status = SessionRuntimeStatus::Idle;
            session.summary.updated_at = Utc::now();
            final_turn
        };

        if let Err(error) = result {
            self.emit_text_item(
                session_id,
                final_turn.turn_id,
                ItemKind::AgentMessage,
                "Error",
                error.to_string(),
            )
            .await;
            self.broadcast_event(ServerEvent::TurnFailed(TurnEventPayload {
                session_id,
                turn: final_turn.clone(),
            }))
            .await;
        }
        self.broadcast_event(ServerEvent::TurnCompleted(TurnEventPayload {
            session_id,
            turn: final_turn,
        }))
        .await;
        self.broadcast_event(ServerEvent::SessionStatusChanged(
            SessionStatusChangedPayload {
                session_id,
                status: SessionRuntimeStatus::Idle,
            },
        ))
        .await;
    }

    async fn emit_text_item(
        &self,
        session_id: SessionId,
        turn_id: TurnId,
        item_kind: ItemKind,
        title: impl Into<String>,
        text: String,
    ) {
        self.emit_item_pair(
            session_id,
            turn_id,
            item_kind,
            serde_json::json!({ "title": title.into(), "text": text }),
        )
        .await;
    }

    async fn emit_item_pair(
        &self,
        session_id: SessionId,
        turn_id: TurnId,
        item_kind: ItemKind,
        payload: serde_json::Value,
    ) {
        let item_id = ItemId::new();
        self.emit_item_started(
            session_id,
            turn_id,
            item_id,
            item_kind.clone(),
            payload.clone(),
        )
        .await;
        self.emit_item_completed(session_id, turn_id, item_id, item_kind, payload)
            .await;
    }

    async fn emit_item_started(
        &self,
        session_id: SessionId,
        turn_id: TurnId,
        item_id: ItemId,
        item_kind: ItemKind,
        payload: serde_json::Value,
    ) {
        self.bump_session_item_count(session_id).await;
        self.broadcast_event(ServerEvent::ItemStarted(ItemEventPayload {
            context: EventContext {
                session_id,
                turn_id: Some(turn_id),
                item_id: Some(item_id),
                seq: 0,
            },
            item: ItemEnvelope {
                item_id,
                item_kind,
                payload,
            },
        }))
        .await;
    }

    async fn emit_item_completed(
        &self,
        session_id: SessionId,
        turn_id: TurnId,
        item_id: ItemId,
        item_kind: ItemKind,
        payload: serde_json::Value,
    ) {
        self.broadcast_event(ServerEvent::ItemCompleted(ItemEventPayload {
            context: EventContext {
                session_id,
                turn_id: Some(turn_id),
                item_id: Some(item_id),
                seq: 0,
            },
            item: ItemEnvelope {
                item_id,
                item_kind,
                payload,
            },
        }))
        .await;
    }

    async fn bump_session_item_count(&self, session_id: SessionId) {
        if let Some(session_arc) = self.sessions.lock().await.get(&session_id).cloned() {
            let mut session = session_arc.lock().await;
            session.loaded_item_count += 1;
            session.next_item_seq += 1;
        }
    }

    async fn subscribe_connection_to_session(
        &self,
        connection_id: u64,
        session_id: SessionId,
        event_types: Option<HashSet<String>>,
    ) {
        if let Some(connection) = self.connections.lock().await.get_mut(&connection_id) {
            let desired = event_types.unwrap_or_default();
            let already = connection.subscriptions.iter().any(|subscription| {
                subscription.session_id == Some(session_id) && subscription.event_types == desired
            });
            if already {
                return;
            }
            connection.subscriptions.push(SubscriptionFilter {
                session_id: Some(session_id),
                event_types: desired,
            });
        }
    }

    async fn connection_ready(&self, connection_id: u64) -> bool {
        self.connections
            .lock()
            .await
            .get(&connection_id)
            .is_some_and(|connection| connection.state == ConnectionState::Ready)
    }

    async fn emit_to_connection(&self, connection_id: u64, method: &str, event: ServerEvent) {
        let session_id = event.session_id();
        let mut connections = self.connections.lock().await;
        if let Some(connection) = connections.get_mut(&connection_id) {
            if !connection.should_deliver(method, session_id) {
                return;
            }
            let value = serde_json::to_value(NotificationEnvelope {
                method: method.to_string(),
                params: event.with_seq(connection.next_seq()),
            })
            .expect("serialize notification");
            let _ = connection.sender.send(value);
        }
    }

    async fn broadcast_event(&self, event: ServerEvent) {
        let method = event.method_name();
        let session_id = event.session_id();
        let mut connections = self.connections.lock().await;
        for connection in connections.values_mut() {
            if !connection.should_deliver(method, session_id) {
                continue;
            }
            let value = serde_json::to_value(NotificationEnvelope {
                method: method.to_string(),
                params: event.clone().with_seq(connection.next_seq()),
            })
            .expect("serialize notification");
            let _ = connection.sender.send(value);
        }
    }

    fn error_response(
        &self,
        request_id: serde_json::Value,
        code: ProtocolErrorCode,
        message: impl Into<String>,
    ) -> serde_json::Value {
        serde_json::to_value(ErrorResponse {
            id: request_id,
            error: ProtocolError {
                code,
                message: message.into(),
                data: serde_json::json!({}),
            },
        })
        .expect("serialize error response")
    }
}

struct ConnectionRuntime {
    transport: ClientTransportKind,
    state: ConnectionState,
    sender: mpsc::UnboundedSender<serde_json::Value>,
    opt_out_notification_methods: HashSet<String>,
    subscriptions: Vec<SubscriptionFilter>,
    next_event_seq: u64,
}

impl ConnectionRuntime {
    fn should_deliver(&self, method: &str, session_id: Option<SessionId>) -> bool {
        if self.opt_out_notification_methods.contains(method) {
            return false;
        }
        if self.transport == ClientTransportKind::Stdio {
            return true;
        }
        if self.subscriptions.is_empty() {
            return false;
        }
        self.subscriptions.iter().any(|subscription| {
            let session_matches = subscription
                .session_id
                .is_none_or(|expected| session_id == Some(expected));
            let event_matches =
                subscription.event_types.is_empty() || subscription.event_types.contains(method);
            session_matches && event_matches
        })
    }

    fn next_seq(&mut self) -> u64 {
        let seq = self.next_event_seq;
        self.next_event_seq += 1;
        seq
    }
}

struct SubscriptionFilter {
    session_id: Option<SessionId>,
    event_types: HashSet<String>,
}

impl ServerEvent {
    fn session_id(&self) -> Option<SessionId> {
        match self {
            ServerEvent::SessionStarted(payload)
            | ServerEvent::SessionArchived(payload)
            | ServerEvent::SessionUnarchived(payload)
            | ServerEvent::SessionClosed(payload) => Some(payload.session.session_id),
            ServerEvent::SessionStatusChanged(payload) => Some(payload.session_id),
            ServerEvent::TurnStarted(payload)
            | ServerEvent::TurnCompleted(payload)
            | ServerEvent::TurnInterrupted(payload)
            | ServerEvent::TurnFailed(payload)
            | ServerEvent::TurnPlanUpdated(payload)
            | ServerEvent::TurnDiffUpdated(payload) => Some(payload.session_id),
            ServerEvent::ItemStarted(payload) | ServerEvent::ItemCompleted(payload) => {
                Some(payload.context.session_id)
            }
            ServerEvent::ItemDelta { payload, .. } => Some(payload.context.session_id),
            ServerEvent::ServerRequestResolved(payload) => Some(payload.session_id),
        }
    }

    fn method_name(&self) -> &'static str {
        match self {
            ServerEvent::SessionStarted(_) => "session/started",
            ServerEvent::SessionStatusChanged(_) => "session/status/changed",
            ServerEvent::SessionArchived(_) => "session/archived",
            ServerEvent::SessionUnarchived(_) => "session/unarchived",
            ServerEvent::SessionClosed(_) => "session/closed",
            ServerEvent::TurnStarted(_) => "turn/started",
            ServerEvent::TurnCompleted(_) => "turn/completed",
            ServerEvent::TurnInterrupted(_) => "turn/interrupted",
            ServerEvent::TurnFailed(_) => "turn/failed",
            ServerEvent::TurnPlanUpdated(_) => "turn/plan/updated",
            ServerEvent::TurnDiffUpdated(_) => "turn/diff/updated",
            ServerEvent::ItemStarted(_) => "item/started",
            ServerEvent::ItemCompleted(_) => "item/completed",
            ServerEvent::ItemDelta { delta_kind, .. } => match delta_kind {
                ItemDeltaKind::AgentMessageDelta => "item/agentMessage/delta",
                ItemDeltaKind::ReasoningSummaryTextDelta => "item/reasoning/summaryTextDelta",
                ItemDeltaKind::ReasoningTextDelta => "item/reasoning/textDelta",
                ItemDeltaKind::CommandExecutionOutputDelta => "item/commandExecution/outputDelta",
                ItemDeltaKind::FileChangeOutputDelta => "item/fileChange/outputDelta",
                ItemDeltaKind::PlanDelta => "item/plan/delta",
            },
            ServerEvent::ServerRequestResolved(_) => "serverRequest/resolved",
        }
    }

    fn with_seq(mut self, seq: u64) -> Self {
        match &mut self {
            ServerEvent::ItemStarted(payload) | ServerEvent::ItemCompleted(payload) => {
                payload.context.seq = seq;
            }
            ServerEvent::ItemDelta { payload, .. } => payload.context.seq = seq,
            _ => {}
        }
        self
    }
}

fn render_input_items(input: &[crate::InputItem]) -> Option<String> {
    let parts = input
        .iter()
        .filter_map(|item| match item {
            crate::InputItem::Text { text } => Some(text.trim().to_string()),
            crate::InputItem::Skill { id } => Some(format!("[skill:{id}]")),
            crate::InputItem::LocalImage { path } => Some(format!("[image:{}]", path.display())),
            crate::InputItem::Mention { path, name } => Some(format!(
                "[mention:{}]",
                name.as_deref().unwrap_or(path.as_str())
            )),
        })
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>();
    (!parts.is_empty()).then(|| parts.join("\n"))
}
