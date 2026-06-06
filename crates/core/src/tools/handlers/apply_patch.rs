use async_trait::async_trait;

use crate::apply_patch::exec_apply_patch;
use crate::contracts::ToolCallError;
use crate::contracts::ToolContext;
use crate::contracts::ToolProgressSender;
use crate::contracts::ToolResult;
use crate::contracts::ToolResultContent;
use crate::json_schema::JsonSchema;
use crate::tool_handler::ToolHandler;
use crate::tool_spec::ToolCapabilityTag;
use crate::tool_spec::ToolExecutionMode;
use crate::tool_spec::ToolOutputMode;
use crate::tool_spec::ToolSpec;

pub struct ApplyPatchHandler {
    spec: ToolSpec,
}

impl Default for ApplyPatchHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl ApplyPatchHandler {
    pub fn new() -> Self {
        Self {
            spec: ToolSpec {
                name: "apply_patch".into(),
                description: "Apply a unified diff patch to the filesystem.".into(),
                input_schema: JsonSchema::object(
                    std::collections::BTreeMap::from([(
                        "patchText".to_string(),
                        JsonSchema::string(Some(
                            "The full patch text that describes all changes to be made",
                        )),
                    )]),
                    Some(vec!["patchText".to_string()]),
                    Some(false),
                ),
                output_mode: ToolOutputMode::Text,
                execution_mode: ToolExecutionMode::Mutating,
                capability_tags: vec![ToolCapabilityTag::WriteFiles],
                supports_parallel: false,
                preparation_feedback: crate::tool_spec::ToolPreparationFeedback::None,
                display_name: None,
                supports_cancellation: None,
                supports_streaming: None,
            },
        }
    }
}

#[async_trait]
impl ToolHandler for ApplyPatchHandler {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    async fn handle(
        &self,
        ctx: ToolContext,
        input: serde_json::Value,
        _progress: Option<ToolProgressSender>,
    ) -> Result<ToolResult, ToolCallError> {
        let output = exec_apply_patch(&ctx.workspace_root, input)
            .await
            .map_err(|e| ToolCallError::ExecutionFailed(e.to_string()))?;

        let text = output.content.into_string();
        if output.is_error {
            Ok(ToolResult::error(
                ToolResultContent::Text(text.clone()),
                "Patch failed",
                ToolCallError::ExecutionFailed(text),
            ))
        } else {
            Ok(ToolResult::success(
                ToolResultContent::Text(text),
                "Patch applied",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn apply_patch_schema_matches_executor_input() {
        let handler = ApplyPatchHandler::new();
        let expected = JsonSchema::object(
            BTreeMap::from([(
                "patchText".to_string(),
                JsonSchema::string(Some(
                    "The full patch text that describes all changes to be made",
                )),
            )]),
            Some(vec!["patchText".to_string()]),
            Some(false),
        );

        assert_eq!(handler.spec().input_schema, expected);
    }
}
