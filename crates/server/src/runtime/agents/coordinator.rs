use super::*;

impl ServerRuntime {
    async fn send_message_inner(
        &self,
        params: devo_protocol::AgentMessageParams,
    ) -> Result<devo_protocol::AgentMessageResult, ToolCallError> {
        self.queue_agent_message(params.session_id, &params.target, params.message)
            .await?;
        Ok(devo_protocol::AgentMessageResult { delivered: true })
    }

    async fn followup_task_inner(
        self: &Arc<Self>,
        params: devo_protocol::AgentMessageParams,
    ) -> Result<devo_protocol::AgentMessageResult, ToolCallError> {
        let route = self
            .queue_agent_message(params.session_id, &params.target, params.message.clone())
            .await?;
        self.start_runtime_turn(route.to_session_id, params.message.clone(), params.message)
            .await?;
        self.set_agent_status(
            route.parent_session_id,
            route.to_session_id,
            SubagentStatus::Running,
        )
        .await;
        Ok(devo_protocol::AgentMessageResult { delivered: true })
    }

    async fn wait_agent_inner(
        &self,
        params: devo_protocol::WaitAgentParams,
    ) -> Result<devo_protocol::WaitAgentResult, ToolCallError> {
        let timeout = params
            .timeout_ms
            .map(Duration::from_millis)
            .unwrap_or(DEFAULT_WAIT_AGENT_TIMEOUT)
            .min(MAX_WAIT_AGENT_TIMEOUT);
        let mailbox = self.mailbox(params.session_id).await;
        let (messages, timed_out) = mailbox.wait(timeout).await;
        Ok(devo_protocol::WaitAgentResult {
            messages,
            timed_out,
        })
    }

    async fn list_agents_inner(
        &self,
        params: devo_protocol::AgentListParams,
    ) -> Result<Vec<devo_protocol::AgentInfo>, ToolCallError> {
        let registries = self.agent_registries.lock().await;
        Ok(registries
            .get(&params.session_id)
            .map(|registry| {
                registry.list_children(params.session_id, params.path_prefix.as_deref())
            })
            .unwrap_or_default())
    }

    async fn close_agent_inner(
        self: &Arc<Self>,
        params: devo_protocol::CloseAgentParams,
    ) -> Result<devo_protocol::CloseAgentResult, ToolCallError> {
        let child_session_id = self
            .resolve_child_agent(params.session_id, &params.target)
            .await?
            .session_id;
        let status = self
            .close_child_agent(params.session_id, child_session_id)
            .await?;
        Ok(devo_protocol::CloseAgentResult {
            closed: true,
            status,
        })
    }
}

#[async_trait::async_trait]
impl AgentToolCoordinator for ServerRuntime {
    async fn spawn_agent(
        self: Arc<Self>,
        params: devo_protocol::SpawnAgentParams,
    ) -> Result<devo_protocol::SpawnAgentResult, ToolCallError> {
        self.spawn_agent_inner(params).await
    }

    async fn send_message(
        self: Arc<Self>,
        params: devo_protocol::AgentMessageParams,
    ) -> Result<devo_protocol::AgentMessageResult, ToolCallError> {
        self.send_message_inner(params).await
    }

    async fn followup_task(
        self: Arc<Self>,
        params: devo_protocol::AgentMessageParams,
    ) -> Result<devo_protocol::AgentMessageResult, ToolCallError> {
        self.followup_task_inner(params).await
    }

    async fn wait_agent(
        self: Arc<Self>,
        params: devo_protocol::WaitAgentParams,
    ) -> Result<devo_protocol::WaitAgentResult, ToolCallError> {
        self.wait_agent_inner(params).await
    }

    async fn list_agents(
        self: Arc<Self>,
        params: devo_protocol::AgentListParams,
    ) -> Result<Vec<devo_protocol::AgentInfo>, ToolCallError> {
        self.list_agents_inner(params).await
    }

    async fn close_agent(
        self: Arc<Self>,
        params: devo_protocol::CloseAgentParams,
    ) -> Result<devo_protocol::CloseAgentResult, ToolCallError> {
        self.close_agent_inner(params).await
    }
}
