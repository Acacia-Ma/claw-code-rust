use serde::Deserialize;
use serde::Serialize;

/// Experimental feature gates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentalConfig {
    /// Enables the built-in `code_search` tool.
    #[serde(
        default = "default_code_search",
        rename = "code-search",
        alias = "code_search",
        skip_serializing_if = "is_true"
    )]
    pub code_search: bool,
}

impl Default for ExperimentalConfig {
    fn default() -> Self {
        Self {
            code_search: default_code_search(),
        }
    }
}

fn default_code_search() -> bool {
    true
}

fn is_true(value: &bool) -> bool {
    *value
}
