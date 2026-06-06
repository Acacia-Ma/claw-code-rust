use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use devo_protocol::{
    AgentListParams, AgentMessageParams, CloseAgentParams, SessionId, SpawnAgentParams,
    WaitAgentParams,
};

use crate::contracts::{
    ToolCallError, ToolContext, ToolProgress, ToolProgressSender, ToolResult, ToolResultContent,
};
use crate::json_schema::JsonSchema;
use crate::registry::ToolExposure;
use crate::registry::ToolRegistryBuilder;
use crate::tool_handler::ToolHandler;
use crate::tool_spec::{ToolExecutionMode, ToolOutputMode, ToolPreparationFeedback, ToolSpec};

#[derive(Clone, Copy)]
enum AgentToolKind {
    Spawn,
    SendMessage,
    FollowupTask,
    Wait,
    List,
    Close,
}

pub struct AgentToolHandler {
    spec: ToolSpec,
    kind: AgentToolKind,
}

impl AgentToolHandler {
    fn new(spec: ToolSpec, kind: AgentToolKind) -> Self {
        Self { spec, kind }
    }
}

pub fn register_agent_tools(builder: &mut ToolRegistryBuilder) {
    let spawn = Arc::new(AgentToolHandler::new(spawn_spec(), AgentToolKind::Spawn));
    let send = Arc::new(AgentToolHandler::new(
        send_message_spec(),
        AgentToolKind::SendMessage,
    ));
    let followup = Arc::new(AgentToolHandler::new(
        followup_task_spec(),
        AgentToolKind::FollowupTask,
    ));
    let wait = Arc::new(AgentToolHandler::new(
        wait_agent_spec(),
        AgentToolKind::Wait,
    ));
    let list = Arc::new(AgentToolHandler::new(
        list_agents_spec(),
        AgentToolKind::List,
    ));
    let close = Arc::new(AgentToolHandler::new(
        close_agent_spec(),
        AgentToolKind::Close,
    ));

    register(builder, spawn, &["spawn_subagent", "subagent", "delegate"]);
    register(builder, send, &[]);
    register(builder, followup, &[]);
    register(builder, wait, &["subagent_result"]);
    register(builder, list, &["subagent_status"]);
    register(builder, close, &[]);
}

fn register(builder: &mut ToolRegistryBuilder, handler: Arc<AgentToolHandler>, aliases: &[&str]) {
    builder.push_spec_with_exposure(handler.spec().clone(), ToolExposure::Deferred);
    let handler: Arc<dyn ToolHandler> = handler;
    let name = handler.spec().name.clone();
    builder.register_handler(&name, Arc::clone(&handler));
    for alias in aliases {
        builder.register_handler(alias, Arc::clone(&handler));
    }
}

#[async_trait]
impl ToolHandler for AgentToolHandler {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    async fn handle(
        &self,
        ctx: ToolContext,
        input: serde_json::Value,
        progress: Option<ToolProgressSender>,
    ) -> Result<ToolResult, ToolCallError> {
        let Some(coordinator) = ctx.agent_coordinator.clone() else {
            return Err(ToolCallError::NeedsConfiguration(
                "child agent coordination is not configured".to_string(),
            ));
        };
        let session_id = current_session_id(&ctx)?;
        match self.kind {
            AgentToolKind::Spawn => {
                let input: SpawnAgentInput = parse_input(input)?;
                let result = coordinator
                    .spawn_agent(SpawnAgentParams {
                        session_id,
                        task_name: input.task_name,
                        message: input.message,
                        agent_type: input.agent_type,
                        model: input.model,
                        thinking: input.thinking,
                        fork_turns: input.fork_turns,
                    })
                    .await?;
                json_result(result, "agent spawned")
            }
            AgentToolKind::SendMessage => {
                let input: AgentMessageInput = parse_input(input)?;
                let result = coordinator
                    .send_message(AgentMessageParams {
                        session_id,
                        target: input.target,
                        message: input.message,
                    })
                    .await?;
                json_result(result, "message delivered")
            }
            AgentToolKind::FollowupTask => {
                let input: AgentMessageInput = parse_input(input)?;
                let result = coordinator
                    .followup_task(AgentMessageParams {
                        session_id,
                        target: input.target,
                        message: input.message,
                    })
                    .await?;
                json_result(result, "follow-up task sent")
            }
            AgentToolKind::Wait => {
                if let Some(progress) = progress {
                    let _ = progress.send(ToolProgress::StatusUpdate {
                        message: "Waiting for subagent messages...".to_string(),
                        percent: None,
                    });
                }
                let input: WaitAgentInput = parse_input(input)?;
                let result = coordinator
                    .wait_agent(WaitAgentParams {
                        session_id,
                        timeout_ms: input.timeout_ms,
                    })
                    .await?;
                json_result(result, "wait completed")
            }
            AgentToolKind::List => {
                let input: ListAgentsInput = parse_input(input)?;
                let agents = coordinator
                    .list_agents(AgentListParams {
                        session_id,
                        path_prefix: input.path_prefix,
                    })
                    .await?;
                json_result(serde_json::json!({ "agents": agents }), "agents listed")
            }
            AgentToolKind::Close => {
                let input: CloseAgentInput = parse_input(input)?;
                let result = coordinator
                    .close_agent(CloseAgentParams {
                        session_id,
                        target: input.target,
                    })
                    .await?;
                json_result(result, "agent closed")
            }
        }
    }
}

#[derive(serde::Deserialize)]
struct SpawnAgentInput {
    task_name: String,
    message: String,
    #[serde(default)]
    agent_type: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    thinking: Option<String>,
    #[serde(default)]
    fork_turns: Option<String>,
}

#[derive(serde::Deserialize)]
struct AgentMessageInput {
    target: String,
    message: String,
}

#[derive(serde::Deserialize)]
struct WaitAgentInput {
    #[serde(default)]
    timeout_ms: Option<u64>,
}

#[derive(serde::Deserialize)]
struct ListAgentsInput {
    #[serde(default)]
    path_prefix: Option<String>,
}

#[derive(serde::Deserialize)]
struct CloseAgentInput {
    target: String,
}

fn current_session_id(ctx: &ToolContext) -> Result<SessionId, ToolCallError> {
    SessionId::try_from(ctx.session_id.clone()).map_err(|error| {
        ToolCallError::InvalidInput(format!("invalid current session id: {error}"))
    })
}

fn parse_input<T: serde::de::DeserializeOwned>(
    input: serde_json::Value,
) -> Result<T, ToolCallError> {
    serde_json::from_value(input).map_err(|error| ToolCallError::InvalidInput(error.to_string()))
}

fn json_result(
    value: impl serde::Serialize,
    summary: impl Into<String>,
) -> Result<ToolResult, ToolCallError> {
    let value = serde_json::to_value(value)
        .map_err(|error| ToolCallError::InternalError(error.to_string()))?;
    Ok(ToolResult::success(ToolResultContent::Json(value), summary))
}

fn spec(name: &str, description: &str, schema: JsonSchema) -> ToolSpec {
    ToolSpec {
        name: name.to_string(),
        description: description.to_string(),
        input_schema: schema,
        output_mode: ToolOutputMode::StructuredJson,
        execution_mode: ToolExecutionMode::Mutating,
        capability_tags: vec![],
        supports_parallel: false,
        preparation_feedback: ToolPreparationFeedback::None,
        display_name: None,
        supports_cancellation: None,
        supports_streaming: None,
    }
}

fn spawn_spec() -> ToolSpec {
    spec(
        "spawn_agent",
        "Create a child agent for a bounded delegated task.",
        JsonSchema::object(
            BTreeMap::from([
                (
                    "task_name".to_string(),
                    JsonSchema::string(Some("Unique child agent name under the current session")),
                ),
                (
                    "message".to_string(),
                    JsonSchema::string(Some("Initial task message for the child agent")),
                ),
                (
                    "agent_type".to_string(),
                    JsonSchema::string(Some("Optional agent role such as default or explorer")),
                ),
                (
                    "model".to_string(),
                    JsonSchema::string(Some("Optional model override")),
                ),
                (
                    "thinking".to_string(),
                    JsonSchema::string(Some("Optional thinking selection override")),
                ),
                (
                    "fork_turns".to_string(),
                    JsonSchema::string(Some("History fork mode: none or all")),
                ),
            ]),
            Some(vec!["task_name".to_string(), "message".to_string()]),
            Some(false),
        ),
    )
}

fn send_message_spec() -> ToolSpec {
    spec(
        "send_message",
        "Send a queued message to an existing child agent without starting a turn.",
        message_schema(),
    )
}

fn followup_task_spec() -> ToolSpec {
    spec(
        "followup_task",
        "Send a message to an existing child agent and start or queue a turn.",
        message_schema(),
    )
}

fn wait_agent_spec() -> ToolSpec {
    spec(
        "wait_agent",
        "Wait for subagent mailbox messages or completion notifications.",
        JsonSchema::object(
            BTreeMap::from([(
                "timeout_ms".to_string(),
                JsonSchema::integer(Some("Optional wait timeout in milliseconds")),
            )]),
            None,
            Some(false),
        ),
    )
}

fn list_agents_spec() -> ToolSpec {
    spec(
        "list_agents",
        "List child agents for the current session.",
        JsonSchema::object(
            BTreeMap::from([(
                "path_prefix".to_string(),
                JsonSchema::string(Some("Optional path prefix filter")),
            )]),
            None,
            Some(false),
        ),
    )
}

fn close_agent_spec() -> ToolSpec {
    spec(
        "close_agent",
        "Close an existing child agent and notify the parent.",
        JsonSchema::object(
            BTreeMap::from([(
                "target".to_string(),
                JsonSchema::string(Some("Target child agent path or session id")),
            )]),
            Some(vec!["target".to_string()]),
            Some(false),
        ),
    )
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use pretty_assertions::assert_eq;
    use tokio::sync::Mutex;
    use tokio_util::sync::CancellationToken;

    use super::*;
    use crate::contracts::ToolBudgets;

    #[derive(Debug, Default)]
    struct FakeAgentCoordinator {
        spawned: Mutex<Vec<SpawnAgentParams>>,
    }

    #[async_trait]
    impl devo_tools::AgentToolCoordinator for FakeAgentCoordinator {
        async fn spawn_agent(
            self: Arc<Self>,
            params: SpawnAgentParams,
        ) -> Result<devo_protocol::SpawnAgentResult, ToolCallError> {
            self.spawned.lock().await.push(params);
            Ok(devo_protocol::SpawnAgentResult {
                child_session_id: SessionId::new(),
                agent_path: "root/reviewer".to_string(),
                agent_nickname: "reviewer".to_string(),
                status: "running".to_string(),
            })
        }

        async fn send_message(
            self: Arc<Self>,
            _params: AgentMessageParams,
        ) -> Result<devo_protocol::AgentMessageResult, ToolCallError> {
            Ok(devo_protocol::AgentMessageResult { delivered: true })
        }

        async fn followup_task(
            self: Arc<Self>,
            _params: AgentMessageParams,
        ) -> Result<devo_protocol::AgentMessageResult, ToolCallError> {
            Ok(devo_protocol::AgentMessageResult { delivered: true })
        }

        async fn wait_agent(
            self: Arc<Self>,
            _params: devo_protocol::WaitAgentParams,
        ) -> Result<devo_protocol::WaitAgentResult, ToolCallError> {
            Ok(devo_protocol::WaitAgentResult {
                messages: Vec::new(),
                timed_out: false,
            })
        }

        async fn list_agents(
            self: Arc<Self>,
            _params: AgentListParams,
        ) -> Result<Vec<devo_protocol::AgentInfo>, ToolCallError> {
            Ok(Vec::new())
        }

        async fn close_agent(
            self: Arc<Self>,
            _params: CloseAgentParams,
        ) -> Result<devo_protocol::CloseAgentResult, ToolCallError> {
            Ok(devo_protocol::CloseAgentResult {
                closed: true,
                status: "closed".to_string(),
            })
        }
    }

    #[tokio::test]
    async fn spawn_handler_delegates_to_coordinator() {
        let session_id = SessionId::new();
        let coordinator = Arc::new(FakeAgentCoordinator::default());
        let handler = AgentToolHandler::new(spawn_spec(), AgentToolKind::Spawn);
        let result = handler
            .handle(
                tool_context(
                    session_id,
                    Some(coordinator.clone() as Arc<dyn devo_tools::AgentToolCoordinator>),
                ),
                serde_json::json!({
                    "task_name": "reviewer",
                    "message": "review this",
                    "fork_turns": "all"
                }),
                None,
            )
            .await
            .expect("spawn succeeds");

        assert_eq!(result.result_summary, "agent spawned");
        assert_eq!(
            coordinator.spawned.lock().await.as_slice(),
            &[SpawnAgentParams {
                session_id,
                task_name: "reviewer".to_string(),
                message: "review this".to_string(),
                agent_type: None,
                model: None,
                thinking: None,
                fork_turns: Some("all".to_string()),
            }]
        );
    }

    #[tokio::test]
    async fn agent_handler_requires_configured_coordinator() {
        let handler = AgentToolHandler::new(spawn_spec(), AgentToolKind::Spawn);
        let error = handler
            .handle(
                tool_context(SessionId::new(), None),
                serde_json::json!({
                    "task_name": "reviewer",
                    "message": "review this"
                }),
                None,
            )
            .await
            .expect_err("missing coordinator should fail");

        assert!(matches!(
            error,
            ToolCallError::NeedsConfiguration(message)
                if message == "child agent coordination is not configured"
        ));
    }

    fn tool_context(
        session_id: SessionId,
        agent_coordinator: Option<Arc<dyn devo_tools::AgentToolCoordinator>>,
    ) -> ToolContext {
        ToolContext {
            tool_call_id: crate::invocation::ToolCallId("tool-call".to_string()),
            session_id: session_id.to_string(),
            turn_id: None,
            workspace_root: ".".into(),
            budgets: ToolBudgets {
                output_limit_bytes: 1024,
                wall_time_limit_ms: None,
            },
            cancel_token: CancellationToken::new(),
            agent_coordinator,
        }
    }
}

fn message_schema() -> JsonSchema {
    JsonSchema::object(
        BTreeMap::from([
            (
                "target".to_string(),
                JsonSchema::string(Some("Target child agent path or session id")),
            ),
            (
                "message".to_string(),
                JsonSchema::string(Some("Message content")),
            ),
        ]),
        Some(vec!["target".to_string(), "message".to_string()]),
        Some(false),
    )
}
