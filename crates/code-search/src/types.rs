//! Public request, response, and error types for the code-search crate.
//!
//! These types mirror the built-in tool schema: callers choose `search` or
//! `find_related`, a content filter, a bounded result count, and optional path or
//! language filters. The JSON-facing response shape is kept stable while the
//! internals can change cache layout, refresh strategy, or semantic backend.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub const DEFAULT_TOP_K: usize = 5;
pub const MAX_TOP_K: usize = 20;

/// High-level content partition selected by the tool input.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentFilter {
    #[default]
    Code,
    Docs,
    Config,
    All,
}

/// File classification produced during discovery.
///
/// Data files are recognized separately so `all` can still avoid indexing CSV or
/// similar table data that is usually noisy for code retrieval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentKind {
    Code,
    Docs,
    Config,
    Data,
}

impl ContentKind {
    /// Returns whether this content kind belongs in a requested index.
    pub fn is_selected_by(self, filter: ContentFilter) -> bool {
        match filter {
            ContentFilter::Code => self == Self::Code,
            ContentFilter::Docs => self == Self::Docs,
            ContentFilter::Config => self == Self::Config,
            ContentFilter::All => self != Self::Data,
        }
    }
}

/// Supported code-search operation names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeSearchOperation {
    Search,
    FindRelated,
}

/// Searchable source fragment with stable workspace-relative location.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Chunk {
    pub content: String,
    pub file_path: PathBuf,
    pub start_line: usize,
    pub end_line: usize,
    pub language: String,
}

impl Chunk {
    /// Formats the chunk location for deterministic tie-breaking and display.
    pub fn location(&self) -> String {
        format!(
            "{}:{}-{}",
            self.file_path.to_string_lossy().replace('\\', "/"),
            self.start_line,
            self.end_line
        )
    }
}

/// Ranked result returned by both search operations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SearchResult {
    pub score: f32,
    pub chunk: Chunk,
}

/// Index size summary included in every response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexStats {
    pub indexed_files: usize,
    pub total_chunks: usize,
}

/// Stable JSON output shape for the built-in tool.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SearchOutput {
    pub operation: CodeSearchOperation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    pub root: PathBuf,
    pub content: ContentFilter,
    pub results: Vec<SearchResult>,
    pub index_stats: IndexStats,
}

/// Optional path and language filters applied before final ranking.
///
/// Paths are normalized to `/` separators and matched as exact files or directory
/// prefixes. Languages are lowercased classification names such as `rust`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchFilters {
    pub paths: Vec<String>,
    pub languages: Vec<String>,
}

impl SearchFilters {
    /// Creates an unfiltered search scope.
    pub fn empty() -> Self {
        Self {
            paths: Vec::new(),
            languages: Vec::new(),
        }
    }

    /// Returns true when no path or language restriction is active.
    pub fn is_empty(&self) -> bool {
        self.paths.is_empty() && self.languages.is_empty()
    }

    /// Normalizes tool input filters into the form used by search.
    pub fn normalized(paths: Vec<String>, languages: Vec<String>) -> Self {
        Self {
            paths: paths
                .into_iter()
                .map(|path| path.replace('\\', "/").trim_matches('/').to_lowercase())
                .filter(|path| !path.is_empty())
                .collect(),
            languages: languages
                .into_iter()
                .map(|language| language.trim().to_lowercase())
                .filter(|language| !language.is_empty())
                .collect(),
        }
    }

    /// Checks whether a chunk survives both path and language filters.
    pub fn allows(&self, chunk: &Chunk) -> bool {
        let path = chunk
            .file_path
            .to_string_lossy()
            .replace('\\', "/")
            .to_lowercase();
        let path_allowed = self.paths.is_empty()
            || self
                .paths
                .iter()
                .any(|filter| path == *filter || path.starts_with(&format!("{filter}/")));
        let language_allowed = self.languages.is_empty()
            || self
                .languages
                .iter()
                .any(|language| language == &chunk.language.to_lowercase());
        path_allowed && language_allowed
    }
}

/// Internal request for the `search` operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchRequest {
    pub root: PathBuf,
    pub query: String,
    pub content: ContentFilter,
    pub top_k: usize,
    pub filters: SearchFilters,
}

/// Internal request for the `find_related` operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelatedRequest {
    pub root: PathBuf,
    pub file_path: PathBuf,
    pub line: usize,
    pub content: ContentFilter,
    pub top_k: usize,
    pub filters: SearchFilters,
}

/// Error categories surfaced by the code-search service.
///
/// Model failures are kept separate from index and input failures so the tool
/// runtime can report missing local model cache as a recoverable condition.
#[derive(Debug, Clone, thiserror::Error)]
pub enum CodeSearchError {
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("model unavailable: {0}")]
    ModelUnavailable(String),
    #[error("index error: {0}")]
    Index(String),
    #[error("I/O error: {0}")]
    Io(String),
}

impl From<std::io::Error> for CodeSearchError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

impl From<anyhow::Error> for CodeSearchError {
    fn from(error: anyhow::Error) -> Self {
        Self::Index(error.to_string())
    }
}

/// Validates and returns the requested result limit.
///
/// The max bound protects retrieval from accidentally building huge candidate
/// sets through the public tool schema.
pub fn validate_top_k(top_k: usize) -> Result<usize, CodeSearchError> {
    if top_k == 0 {
        return Err(CodeSearchError::InvalidInput(
            "`top_k` must be greater than zero".to_string(),
        ));
    }
    if top_k > MAX_TOP_K {
        return Err(CodeSearchError::InvalidInput(format!(
            "`top_k` must be less than or equal to {MAX_TOP_K}"
        )));
    }
    Ok(top_k)
}
