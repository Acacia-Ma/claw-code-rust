//! Subagent coordination — spawn, lifecycle, mailboxes.
//!
//! Implements L3-BEH-SERVER-003. Provides AgentRegistry, agent tree,
//! inter-agent mailbox channels, and spawn/close tool handlers.

use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use devo_protocol::AgentInfo;
use devo_protocol::AgentMailboxMessage;
use devo_protocol::SessionId;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tokio::sync::Notify;

// ── Agent Registry ──────────────────────────────────────────────────

/// Per-root-session registry of all spawned subagents.
#[derive(Debug, Clone, Default)]
pub struct AgentRegistry {
    pub agents: HashMap<SessionId, SubagentMetadata>,
    pub parent_to_children: HashMap<SessionId, Vec<SessionId>>,
    pub child_to_parent: HashMap<SessionId, SessionId>,
}

impl AgentRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &mut self,
        parent_id: SessionId,
        child_id: SessionId,
        metadata: SubagentMetadata,
    ) {
        self.agents.insert(child_id, metadata);
        self.parent_to_children
            .entry(parent_id)
            .or_default()
            .push(child_id);
        self.child_to_parent.insert(child_id, parent_id);
    }

    pub fn unregister(&mut self, child_id: SessionId) {
        self.agents.remove(&child_id);
        if let Some(parent_id) = self.child_to_parent.remove(&child_id)
            && let Some(children) = self.parent_to_children.get_mut(&parent_id)
        {
            children.retain(|id| *id != child_id);
        }
    }

    pub fn get(&self, session_id: SessionId) -> Option<&SubagentMetadata> {
        self.agents.get(&session_id)
    }

    pub fn children_of(&self, parent_id: SessionId) -> Vec<SessionId> {
        self.parent_to_children
            .get(&parent_id)
            .cloned()
            .unwrap_or_default()
    }

    pub fn update_status(&mut self, child_id: SessionId, status: SubagentStatus) {
        if let Some(agent) = self.agents.get_mut(&child_id) {
            agent.status = status;
            if matches!(
                status,
                SubagentStatus::Completed
                    | SubagentStatus::Failed
                    | SubagentStatus::Interrupted
                    | SubagentStatus::Canceled
                    | SubagentStatus::Closed
            ) {
                agent.closed_at.get_or_insert_with(Utc::now);
            }
        }
    }

    pub fn find_child(&self, parent_id: SessionId, target: &str) -> Option<SessionId> {
        let target = target.trim();
        if let Ok(session_id) = target.parse::<SessionId>()
            && self.agents.contains_key(&session_id)
        {
            return Some(session_id);
        }
        self.children_of(parent_id).into_iter().find(|child_id| {
            self.agents.get(child_id).is_some_and(|meta| {
                meta.agent_path == target
                    || meta.nickname == target
                    || meta.session_id.to_string() == target
            })
        })
    }

    pub fn list_children(&self, parent_id: SessionId, path_prefix: Option<&str>) -> Vec<AgentInfo> {
        self.children_of(parent_id)
            .into_iter()
            .filter_map(|child_id| self.agents.get(&child_id))
            .filter(|meta| path_prefix.is_none_or(|prefix| meta.agent_path.starts_with(prefix)))
            .map(SubagentMetadata::to_agent_info)
            .collect()
    }
}

/// Per-subagent metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubagentMetadata {
    pub session_id: SessionId,
    pub parent_session_id: SessionId,
    pub agent_path: String,
    pub nickname: String,
    pub role: String,
    pub status: SubagentStatus,
    pub spawned_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    pub last_task_message: Option<String>,
    pub close_requested: bool,
}

impl SubagentMetadata {
    pub fn to_agent_info(&self) -> AgentInfo {
        AgentInfo {
            session_id: self.session_id,
            parent_session_id: Some(self.parent_session_id),
            agent_path: self.agent_path.clone(),
            agent_nickname: self.nickname.clone(),
            agent_role: self.role.clone(),
            status: self.status.as_str().to_string(),
            last_task_message: self.last_task_message.clone(),
        }
    }
}

/// Lifecycle status of a subagent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubagentStatus {
    Spawning,
    Running,
    WaitingForInput,
    Completed,
    Failed,
    Interrupted,
    Canceled,
    Closed,
}

impl SubagentStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Spawning => "spawning",
            Self::Running => "running",
            Self::WaitingForInput => "waiting_for_input",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Interrupted => "interrupted",
            Self::Canceled => "canceled",
            Self::Closed => "closed",
        }
    }
}

// ── Agent Path ──────────────────────────────────────────────────────

/// Canonical agent path: `<parent>/<child>/...`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentPath(pub String);

impl AgentPath {
    pub fn new(path: impl Into<String>) -> Self {
        Self(path.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn parent(&self) -> Option<AgentPath> {
        let s = &self.0;
        s.rfind('/').map(|pos| AgentPath(s[..pos].to_string()))
    }

    pub fn join(&self, name: &str) -> AgentPath {
        AgentPath(format!("{}/{}", self.0, name))
    }
}

// ── Inter-Agent Mailbox ─────────────────────────────────────────────

/// Mailbox queue for ordered inter-agent communication.
#[derive(Debug, Clone)]
pub struct SubagentMailbox {
    inner: Arc<Mutex<MailboxInner>>,
    notify: Arc<Notify>,
}

#[derive(Debug, Default)]
struct MailboxInner {
    next_sequence: u64,
    pending: VecDeque<AgentMailboxMessage>,
}

impl Default for SubagentMailbox {
    fn default() -> Self {
        Self::new()
    }
}

impl SubagentMailbox {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(MailboxInner {
                next_sequence: 1,
                pending: VecDeque::new(),
            })),
            notify: Arc::new(Notify::new()),
        }
    }

    pub async fn send(
        &self,
        mut msg: AgentMailboxMessage,
    ) -> Result<AgentMailboxMessage, SubagentError> {
        let mut inner = self.inner.lock().await;
        msg.sequence = inner.next_sequence;
        if msg.message_id.is_empty() {
            msg.message_id = format!("mail-{}", inner.next_sequence);
        }
        inner.next_sequence = inner.next_sequence.saturating_add(1);
        inner.pending.push_back(msg.clone());
        drop(inner);
        self.notify.notify_waiters();
        Ok(msg)
    }

    pub async fn drain(&self) -> Vec<AgentMailboxMessage> {
        let mut inner = self.inner.lock().await;
        inner.pending.drain(..).collect()
    }

    pub async fn wait(&self, timeout: Duration) -> (Vec<AgentMailboxMessage>, bool) {
        let messages = self.drain().await;
        if !messages.is_empty() {
            return (messages, false);
        }
        if tokio::time::timeout(timeout, self.notify.notified())
            .await
            .is_err()
        {
            return (Vec::new(), true);
        }
        (self.drain().await, false)
    }
}

// ── Errors ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, thiserror::Error)]
pub enum SubagentError {
    #[error("agent not found: {0}")]
    AgentNotFound(SessionId),
    #[error("agent registry full")]
    RegistryFull,
    #[error("max depth exceeded")]
    MaxDepthExceeded,
    #[error("max agents per root exceeded")]
    MaxAgentsExceeded,
    #[error("mailbox closed")]
    MailboxClosed,
    #[error("spawn failed: {0}")]
    SpawnFailed(String),
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_registry_register_and_lookup() {
        let mut registry = AgentRegistry::new();
        let parent = SessionId::new();
        let child = SessionId::new();
        let meta = SubagentMetadata {
            session_id: child,
            parent_session_id: parent,
            agent_path: "root/code-reviewer".into(),
            nickname: "code-reviewer".into(),
            role: "reviewer".into(),
            status: SubagentStatus::Running,
            spawned_at: Utc::now(),
            closed_at: None,
            last_task_message: Some("review this".into()),
            close_requested: false,
        };
        registry.register(parent, child, meta.clone());
        assert_eq!(registry.get(child), Some(&meta));
        assert_eq!(registry.children_of(parent), vec![child]);
    }

    #[test]
    fn agent_registry_unregister() {
        let mut registry = AgentRegistry::new();
        let parent = SessionId::new();
        let child = SessionId::new();
        registry.register(
            parent,
            child,
            SubagentMetadata {
                session_id: child,
                parent_session_id: parent,
                agent_path: "root/test".into(),
                nickname: "test".into(),
                role: "tester".into(),
                status: SubagentStatus::Completed,
                spawned_at: Utc::now(),
                closed_at: None,
                last_task_message: None,
                close_requested: false,
            },
        );
        registry.unregister(child);
        assert!(registry.get(child).is_none());
        assert!(registry.children_of(parent).is_empty());
    }

    #[tokio::test]
    async fn mailbox_drains_waits_and_keeps_late_messages() {
        let mailbox = SubagentMailbox::new();
        let parent = SessionId::new();
        let child = SessionId::new();
        let message = AgentMailboxMessage {
            message_id: String::new(),
            from_session_id: child,
            to_session_id: parent,
            from_agent_path: "root/test".into(),
            to_agent_path: "root".into(),
            content: "done".into(),
            sequence: 0,
            created_at: Utc::now(),
        };
        let delivered = mailbox.send(message.clone()).await.expect("send message");
        let messages = mailbox.drain().await;
        assert_eq!(messages, vec![delivered]);

        let (messages, timed_out) = mailbox.wait(Duration::from_millis(1)).await;
        assert!(messages.is_empty());
        assert!(timed_out);

        let mailbox_for_task = mailbox.clone();
        let expected = message;
        let task = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(5)).await;
            mailbox_for_task
                .send(expected)
                .await
                .expect("send delayed message");
        });
        let (messages, timed_out) = mailbox.wait(Duration::from_millis(100)).await;
        task.await.expect("delayed send joins");
        assert!(!timed_out);
        assert_eq!(messages.len(), 1);
    }

    #[test]
    fn agent_path_join_and_parent() {
        let root = AgentPath::new("root");
        let child = root.join("code-reviewer");
        assert_eq!(child.as_str(), "root/code-reviewer");
        assert_eq!(child.parent().unwrap().as_str(), "root");
    }

    #[tokio::test]
    async fn mailbox_send_receive() {
        let mailbox = SubagentMailbox::new();
        let msg = AgentMailboxMessage {
            message_id: "msg-1".into(),
            from_session_id: SessionId::new(),
            to_session_id: SessionId::new(),
            from_agent_path: "root/child".into(),
            to_agent_path: "root".into(),
            content: "hello".into(),
            sequence: 0,
            created_at: Utc::now(),
        };
        let delivered = mailbox.send(msg).await.expect("send");
        assert_eq!(mailbox.drain().await, vec![delivered]);
    }

    #[test]
    fn subagent_status_serde_roundtrip() {
        for status in &[
            SubagentStatus::Spawning,
            SubagentStatus::Running,
            SubagentStatus::WaitingForInput,
            SubagentStatus::Completed,
            SubagentStatus::Failed,
            SubagentStatus::Interrupted,
            SubagentStatus::Canceled,
            SubagentStatus::Closed,
        ] {
            let json = serde_json::to_string(status).expect("serialize");
            let restored: SubagentStatus = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(restored, *status);
        }
    }
}
