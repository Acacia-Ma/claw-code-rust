use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;

use crate::SessionId;
use crate::TurnId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpawnAgentParams {
    pub session_id: SessionId,
    pub task_name: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fork_turns: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpawnAgentResult {
    pub child_session_id: SessionId,
    pub agent_path: String,
    pub agent_nickname: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentMessageParams {
    pub session_id: SessionId,
    pub target: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentMessageResult {
    pub delivered: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaitAgentParams {
    pub session_id: SessionId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentMailboxMessage {
    pub message_id: String,
    pub from_session_id: SessionId,
    pub to_session_id: SessionId,
    pub from_agent_path: String,
    pub to_agent_path: String,
    pub content: String,
    pub sequence: u64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaitAgentResult {
    pub messages: Vec<AgentMailboxMessage>,
    pub timed_out: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentInfo {
    pub session_id: SessionId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_session_id: Option<SessionId>,
    pub agent_path: String,
    pub agent_nickname: String,
    pub agent_role: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_task_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentListParams {
    pub session_id: SessionId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path_prefix: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentListResult {
    pub agents: Vec<AgentInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentStatusParams {
    pub session_id: SessionId,
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloseAgentParams {
    pub session_id: SessionId,
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloseAgentResult {
    pub closed: bool,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentCompletionNotification {
    pub child_session_id: SessionId,
    pub parent_session_id: SessionId,
    pub agent_path: String,
    pub status: String,
    pub turn_id: TurnId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_excerpt: Option<String>,
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn agent_dtos_roundtrip_through_json() {
        let session_id = SessionId::new();
        let child_session_id = SessionId::new();
        let payloads = serde_json::json!({
            "spawn": SpawnAgentParams {
                session_id,
                task_name: "review".to_string(),
                message: "review this".to_string(),
                agent_type: Some("reviewer".to_string()),
                model: Some("model-a".to_string()),
                thinking: Some("medium".to_string()),
                fork_turns: Some("all".to_string()),
            },
            "result": SpawnAgentResult {
                child_session_id,
                agent_path: "root/review".to_string(),
                agent_nickname: "review".to_string(),
                status: "running".to_string(),
            },
            "wait": WaitAgentResult {
                messages: vec![AgentMailboxMessage {
                    message_id: "mail-1".to_string(),
                    from_session_id: child_session_id,
                    to_session_id: session_id,
                    from_agent_path: "root/review".to_string(),
                    to_agent_path: "root".to_string(),
                    content: "done".to_string(),
                    sequence: 1,
                    created_at: Utc::now(),
                }],
                timed_out: false,
            },
        });
        let json = serde_json::to_string(&payloads).expect("serialize agent payloads");
        let restored: serde_json::Value =
            serde_json::from_str(&json).expect("deserialize agent payloads");

        assert_eq!(restored, payloads);
    }
}
