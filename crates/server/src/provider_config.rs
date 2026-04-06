use std::{fs, path::Path};

use anyhow::{Context, Result};
use serde::Deserialize;

use clawcr_provider::{
    anthropic::AnthropicProvider, openai_compat::OpenAICompatProvider, ModelProvider,
};

/// Resolved provider bootstrap owned by the server runtime.
pub struct ResolvedServerProvider {
    /// Concrete provider used for model requests.
    pub provider: std::sync::Arc<dyn ModelProvider>,
    /// Default model slug used when a session or turn does not request one.
    pub default_model: String,
}

/// Legacy provider fields still stored in the user config file.
#[derive(Debug, Clone, Default, Deserialize)]
struct LegacyProviderConfig {
    provider: Option<String>,
    model: Option<String>,
    base_url: Option<String>,
    api_key: Option<String>,
}

/// Loads the server-side provider from config and environment variables.
pub fn load_server_provider(
    config_file: &Path,
    default_model: Option<&str>,
) -> Result<ResolvedServerProvider> {
    let file_config = read_legacy_provider_config(config_file).unwrap_or_default();
    let env_provider = env_non_empty("CLAWCR_PROVIDER");
    let env_model = env_non_empty("CLAWCR_MODEL");
    let env_base_url = env_non_empty("CLAWCR_BASE_URL");
    let env_api_key = env_non_empty("CLAWCR_API_KEY");

    let provider_name = env_provider
        .or(file_config.provider)
        .or_else(|| {
            if env_non_empty("ANTHROPIC_API_KEY").is_some()
                || env_non_empty("ANTHROPIC_AUTH_TOKEN").is_some()
            {
                Some("anthropic".to_string())
            } else if env_non_empty("OPENAI_API_KEY").is_some()
                || env_non_empty("OPENAI_BASE_URL").is_some()
            {
                Some("openai".to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "openai".to_string());

    let model = env_model
        .or(file_config.model)
        .or_else(|| default_model.map(ToOwned::to_owned));
    let base_url = env_base_url
        .or(file_config.base_url)
        .or_else(|| env_non_empty("ANTHROPIC_BASE_URL"))
        .or_else(|| env_non_empty("OPENAI_BASE_URL"));
    let api_key = env_api_key
        .or(file_config.api_key)
        .or_else(|| env_non_empty("ANTHROPIC_API_KEY"))
        .or_else(|| env_non_empty("ANTHROPIC_AUTH_TOKEN"))
        .or_else(|| env_non_empty("OPENAI_API_KEY"));

    match provider_name.as_str() {
        "anthropic" => {
            let api_key = api_key.context("anthropic provider requires an API key")?;
            let provider: std::sync::Arc<dyn ModelProvider> = if let Some(url) = base_url {
                std::sync::Arc::new(AnthropicProvider::new_with_url(api_key, url))
            } else {
                std::sync::Arc::new(AnthropicProvider::new(api_key))
            };
            Ok(ResolvedServerProvider {
                provider,
                default_model: model.unwrap_or_else(|| "claude-sonnet-4-20250514".to_string()),
            })
        }
        "ollama" => {
            let base_url =
                ensure_openai_v1(&base_url.unwrap_or_else(|| "http://localhost:11434".to_string()));
            let mut provider = OpenAICompatProvider::new(base_url);
            if let Some(api_key) = api_key {
                provider = provider.with_api_key(api_key);
            }
            Ok(ResolvedServerProvider {
                provider: std::sync::Arc::new(provider),
                default_model: model.unwrap_or_else(|| "qwen3.5:9b".to_string()),
            })
        }
        "openai" => {
            let base_url =
                ensure_openai_v1(&base_url.unwrap_or_else(|| "https://api.openai.com".to_string()));
            let mut provider = OpenAICompatProvider::new(base_url);
            if let Some(api_key) = api_key {
                provider = provider.with_api_key(api_key);
            }
            Ok(ResolvedServerProvider {
                provider: std::sync::Arc::new(provider),
                default_model: model.unwrap_or_else(|| "gpt-4o".to_string()),
            })
        }
        other => anyhow::bail!("unsupported provider '{other}'"),
    }
}

fn read_legacy_provider_config(config_file: &Path) -> Result<LegacyProviderConfig> {
    if !config_file.exists() {
        return Ok(LegacyProviderConfig::default());
    }
    let contents = fs::read_to_string(config_file)
        .with_context(|| format!("failed to read {}", config_file.display()))?;
    toml::from_str(&contents).with_context(|| format!("failed to parse {}", config_file.display()))
}

fn env_non_empty(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
}

fn ensure_openai_v1(url: &str) -> String {
    let trimmed = url.trim_end_matches('/');
    if trimmed.ends_with("/v1") {
        trimmed.to_string()
    } else {
        format!("{trimmed}/v1")
    }
}
