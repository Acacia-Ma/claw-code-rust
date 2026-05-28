use std::sync::Arc;
use std::sync::Mutex;

use async_trait::async_trait;
use devo_protocol::ToolDefinition;

use crate::deferred_loading::DeferredLoadingConfig;
use crate::deferred_loading::LoadedDeferredTools;
use crate::deferred_loading::execute_tool_search;
use crate::errors::ToolExecutionError;
use crate::events::ToolProgressSender;
use crate::handler_kind::ToolHandlerKind;
use crate::invocation::{FunctionToolOutput, ToolInvocation, ToolOutput};
use crate::tool_handler::ToolHandler;

pub struct ToolSearchHandler {
    definitions: Vec<ToolDefinition>,
    loaded_tools: Arc<Mutex<LoadedDeferredTools>>,
    config: DeferredLoadingConfig,
}

impl ToolSearchHandler {
    pub fn new(
        definitions: Vec<ToolDefinition>,
        loaded_tools: Arc<Mutex<LoadedDeferredTools>>,
        config: DeferredLoadingConfig,
    ) -> Self {
        Self {
            definitions,
            loaded_tools,
            config,
        }
    }
}

#[async_trait]
impl ToolHandler for ToolSearchHandler {
    fn tool_kind(&self) -> ToolHandlerKind {
        ToolHandlerKind::ToolSearch
    }

    async fn handle(
        &self,
        invocation: ToolInvocation,
        _progress: Option<ToolProgressSender>,
    ) -> Result<Box<dyn ToolOutput>, ToolExecutionError> {
        let query = invocation.input["query"].as_str().ok_or_else(|| {
            ToolExecutionError::ExecutionFailed {
                message: "Expected query format: select:<name>[,<name>...]".to_string(),
            }
        })?;

        let mut loaded_tools =
            self.loaded_tools
                .lock()
                .map_err(|_| ToolExecutionError::Internal {
                    message: "loaded deferred tool state lock poisoned".to_string(),
                })?;
        let result = execute_tool_search(
            query,
            &self.definitions,
            &mut loaded_tools,
            &invocation.session_id,
            &self.config,
        )
        .map_err(|message| ToolExecutionError::ExecutionFailed { message })?;

        Ok(Box::new(FunctionToolOutput::success(result.summary())))
    }
}
