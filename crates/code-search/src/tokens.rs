//! Identifier-aware tokenization for sparse retrieval.
//!
//! Code search needs `parse_input`, `ParseInput`, and `parse input` to overlap.
//! These helpers split snake_case and camel/PascalCase identifiers, preserve the
//! raw lowercase identifier as a token, and enrich BM25 documents with file/path
//! terms so short symbol queries can still find the right module.

use std::collections::BTreeSet;
use std::path::Path;

use crate::types::Chunk;

/// Builds the BM25 document text for a chunk.
///
/// The file stem is repeated to emulate Semble's path-aware sparse weighting:
/// chunk content remains dominant, but filename/module matches get enough signal
/// to help identifier-heavy queries.
pub fn enrich_for_bm25(chunk: &Chunk) -> String {
    let mut parts = vec![chunk.content.clone()];
    if let Some(stem) = chunk.file_path.file_stem().and_then(|stem| stem.to_str()) {
        parts.push(stem.to_string());
        parts.push(stem.to_string());
    }
    let path_parts = chunk
        .file_path
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .collect::<Vec<_>>();
    let tail_start = path_parts.len().saturating_sub(4);
    parts.extend(
        path_parts[tail_start..path_parts.len().saturating_sub(1)]
            .iter()
            .map(|part| part.to_string()),
    );
    parts.join(" ")
}

/// Returns normalized query terms used by path reranking.
pub fn query_terms(query: &str) -> Vec<String> {
    split_identifier_tokens(query)
        .into_iter()
        .filter(|token| token.len() > 2)
        .collect()
}

/// Heuristically detects exact-symbol style queries.
///
/// Symbol queries use a lower semantic alpha because matching names and paths is
/// often more important than natural-language meaning for these inputs.
pub fn is_symbol_query(query: &str) -> bool {
    let trimmed = query.trim();
    if trimmed.is_empty() || trimmed.split_whitespace().count() != 1 {
        return false;
    }
    trimmed.contains("::")
        || trimmed.contains('_')
        || trimmed.contains('.')
        || trimmed.chars().any(char::is_uppercase)
        || trimmed
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | ':' | '.'))
}

/// Splits a file stem into terms for path-aware boosts.
pub fn file_stem_terms(path: &Path) -> BTreeSet<String> {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .map(split_identifier_tokens)
        .unwrap_or_default()
        .into_iter()
        .collect()
}

/// Splits arbitrary code/query text into stable lowercase identifier tokens.
///
/// The returned set includes both the full lowercase identifier and its
/// camel/snake pieces. Sorting and deduplication keep BM25 input deterministic.
pub fn split_identifier_tokens(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    for raw in input
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .filter(|part| !part.is_empty())
    {
        let lower = raw.to_lowercase();
        tokens.push(lower);
        tokens.extend(split_camel_and_snake(raw));
    }
    tokens.sort();
    tokens.dedup();
    tokens
}

/// Splits one identifier token on snake and camel/Pascal boundaries.
fn split_camel_and_snake(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    for part in input.split('_').filter(|part| !part.is_empty()) {
        let mut start = 0;
        let chars = part.char_indices().collect::<Vec<_>>();
        for idx in 1..chars.len() {
            let (_, current) = chars[idx];
            let (_, previous) = chars[idx - 1];
            let next_is_lower = chars
                .get(idx + 1)
                .map(|(_, next)| next.is_ascii_lowercase())
                .unwrap_or(false);
            let boundary = (current.is_ascii_uppercase() && previous.is_ascii_lowercase())
                || (current.is_ascii_uppercase() && previous.is_ascii_uppercase() && next_is_lower);
            if boundary {
                let byte_idx = chars[idx].0;
                push_part(&mut out, &part[start..byte_idx]);
                start = byte_idx;
            }
        }
        push_part(&mut out, &part[start..]);
    }
    out.sort();
    out.dedup();
    out
}

fn push_part(out: &mut Vec<String>, part: &str) {
    if !part.is_empty() {
        out.push(part.to_lowercase());
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use pretty_assertions::assert_eq;

    use super::*;

    /// Trace: L2-DES-TOOL-001
    /// Verifies: sparse code search tokenization splits identifiers into queryable terms.
    #[test]
    fn split_identifier_tokens_handles_camel_and_snake() {
        let tokens = split_identifier_tokens("CodeSearchHandler parse_JSONInput");
        let expected = vec![
            "code".to_string(),
            "codesearchhandler".to_string(),
            "handler".to_string(),
            "input".to_string(),
            "json".to_string(),
            "parse".to_string(),
            "parse_jsoninput".to_string(),
            "search".to_string(),
        ];
        assert_eq!(tokens, expected);
    }

    /// Trace: L2-DES-TOOL-001
    /// Verifies: BM25 enrichment includes repeated file-stem and path context.
    #[test]
    fn enrich_for_bm25_adds_path_terms() {
        let chunk = Chunk {
            content: "fn parse_input() {}".to_string(),
            file_path: PathBuf::from("crates/core/src/parser.rs"),
            start_line: 1,
            end_line: 1,
            language: "rust".to_string(),
        };
        assert_eq!(
            enrich_for_bm25(&chunk),
            "fn parse_input() {} parser parser crates core src"
        );
    }
}
