use super::*;

impl ServerRuntime {
    // ── Agent Handlers ────────────────────────────────────────────────

    pub(in crate::runtime) async fn handle_agent_spawn(
        self: &Arc<Self>,
        request_id: serde_json::Value,
        params: serde_json::Value,
    ) -> serde_json::Value {
        match serde_json::from_value::<devo_protocol::SpawnAgentParams>(params) {
            Ok(params) => match Arc::clone(self).spawn_agent(params).await {
                Ok(result) => success_response(request_id, result),
                Err(error) => self.tool_error_response(request_id, error),
            },
            Err(error) => self.error_response(
                request_id,
                ProtocolErrorCode::InvalidParams,
                format!("invalid agent/spawn params: {error}"),
            ),
        }
    }

    pub(in crate::runtime) async fn handle_agent_send_message(
        self: &Arc<Self>,
        request_id: serde_json::Value,
        params: serde_json::Value,
    ) -> serde_json::Value {
        match serde_json::from_value::<devo_protocol::AgentMessageParams>(params) {
            Ok(params) => match Arc::clone(self).send_message(params).await {
                Ok(result) => success_response(request_id, result),
                Err(error) => self.tool_error_response(request_id, error),
            },
            Err(error) => self.error_response(
                request_id,
                ProtocolErrorCode::InvalidParams,
                format!("invalid agent/send_message params: {error}"),
            ),
        }
    }

    pub(in crate::runtime) async fn handle_agent_followup_task(
        self: &Arc<Self>,
        request_id: serde_json::Value,
        params: serde_json::Value,
    ) -> serde_json::Value {
        match serde_json::from_value::<devo_protocol::AgentMessageParams>(params) {
            Ok(params) => match Arc::clone(self).followup_task(params).await {
                Ok(result) => success_response(request_id, result),
                Err(error) => self.tool_error_response(request_id, error),
            },
            Err(error) => self.error_response(
                request_id,
                ProtocolErrorCode::InvalidParams,
                format!("invalid agent/followup_task params: {error}"),
            ),
        }
    }

    pub(in crate::runtime) async fn handle_agent_wait(
        self: &Arc<Self>,
        request_id: serde_json::Value,
        params: serde_json::Value,
    ) -> serde_json::Value {
        match serde_json::from_value::<devo_protocol::WaitAgentParams>(params) {
            Ok(params) => match Arc::clone(self).wait_agent(params).await {
                Ok(result) => success_response(request_id, result),
                Err(error) => self.tool_error_response(request_id, error),
            },
            Err(error) => self.error_response(
                request_id,
                ProtocolErrorCode::InvalidParams,
                format!("invalid agent/wait params: {error}"),
            ),
        }
    }

    pub(in crate::runtime) async fn handle_agent_list(
        self: &Arc<Self>,
        request_id: serde_json::Value,
        params: serde_json::Value,
    ) -> serde_json::Value {
        match serde_json::from_value::<devo_protocol::AgentListParams>(params) {
            Ok(params) => match Arc::clone(self).list_agents(params).await {
                Ok(agents) => {
                    success_response(request_id, devo_protocol::AgentListResult { agents })
                }
                Err(error) => self.tool_error_response(request_id, error),
            },
            Err(error) => self.error_response(
                request_id,
                ProtocolErrorCode::InvalidParams,
                format!("invalid agent/list params: {error}"),
            ),
        }
    }

    pub(in crate::runtime) async fn handle_agent_status(
        self: &Arc<Self>,
        request_id: serde_json::Value,
        params: serde_json::Value,
    ) -> serde_json::Value {
        match serde_json::from_value::<devo_protocol::AgentStatusParams>(params) {
            Ok(params) => match self.agent_info(params.session_id, &params.target).await {
                Ok(agent) => success_response(request_id, agent),
                Err(error) => self.tool_error_response(request_id, error),
            },
            Err(error) => self.error_response(
                request_id,
                ProtocolErrorCode::InvalidParams,
                format!("invalid agent/status params: {error}"),
            ),
        }
    }

    pub(in crate::runtime) async fn handle_agent_close(
        self: &Arc<Self>,
        request_id: serde_json::Value,
        params: serde_json::Value,
    ) -> serde_json::Value {
        match serde_json::from_value::<devo_protocol::CloseAgentParams>(params) {
            Ok(params) => match Arc::clone(self).close_agent(params).await {
                Ok(result) => success_response(request_id, result),
                Err(error) => self.tool_error_response(request_id, error),
            },
            Err(error) => self.error_response(
                request_id,
                ProtocolErrorCode::InvalidParams,
                format!("invalid agent/close params: {error}"),
            ),
        }
    }

    fn tool_error_response(
        &self,
        request_id: serde_json::Value,
        error: ToolCallError,
    ) -> serde_json::Value {
        let code = match error {
            ToolCallError::InvalidInput(_) => ProtocolErrorCode::InvalidParams,
            ToolCallError::Denied(_) => ProtocolErrorCode::PermissionDenied,
            ToolCallError::Cancelled => ProtocolErrorCode::AlreadyResolved,
            _ => ProtocolErrorCode::InternalError,
        };
        self.error_response(request_id, code, error.to_string())
    }
}

fn success_response<T: serde::Serialize>(
    request_id: serde_json::Value,
    result: T,
) -> serde_json::Value {
    serde_json::to_value(SuccessResponse {
        id: request_id,
        result,
    })
    .expect("serialize agent response")
}
