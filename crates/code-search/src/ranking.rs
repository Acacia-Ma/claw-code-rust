//! Hybrid ranking and code-aware reranking.
//!
//! Retrieval starts with dense semantic candidates and sparse BM25 candidates,
//! fuses their ranks with reciprocal rank fusion, then applies Semble-style
//! code-specific adjustments. Symbol-like queries lean more on sparse matching
//! because exact identifiers matter; natural-language queries keep semantic and
//! sparse signals balanced. The final penalties bias results toward production
//! code while still allowing tests/docs/examples to appear when their relevance
//! is strong.

use std::collections::HashMap;
use std::path::Path;

use crate::index::SearchIndex;
use crate::tokens::{file_stem_terms, is_symbol_query, query_terms};
use crate::types::{SearchFilters, SearchResult};

const RRF_K: f32 = 60.0;
const CANDIDATE_MULTIPLIER: usize = 5;

/// Runs hybrid ranking for a query and returns the final top_k chunks.
///
/// Both retrieval channels intentionally over-fetch so reranking can correct for
/// path relevance, symbol definitions, and repeated file hits before truncation.
pub fn rank_search(
    index: &SearchIndex,
    query: &str,
    query_embedding: &[f32],
    top_k: usize,
    filters: &SearchFilters,
) -> Vec<SearchResult> {
    let candidate_limit = top_k.saturating_mul(CANDIDATE_MULTIPLIER).max(top_k);
    let semantic = index.semantic_search(query_embedding, candidate_limit, filters);
    let sparse = index.sparse_search(query, candidate_limit, filters);
    let alpha = resolve_alpha(query);
    let mut scores = HashMap::<usize, f32>::new();

    for (rank, (chunk_id, _)) in semantic.into_iter().enumerate() {
        *scores.entry(chunk_id).or_default() += alpha * reciprocal_rank(rank);
    }
    for (rank, (chunk_id, _)) in sparse.into_iter().enumerate() {
        *scores.entry(chunk_id).or_default() += (1.0 - alpha) * reciprocal_rank(rank);
    }

    rerank(index, query, scores, top_k)
}

/// Chooses the dense/sparse mixture for the query shape.
fn resolve_alpha(query: &str) -> f32 {
    if is_symbol_query(query) { 0.3 } else { 0.5 }
}

/// Standard RRF contribution for a zero-based rank.
fn reciprocal_rank(rank: usize) -> f32 {
    1.0 / (RRF_K + rank as f32 + 1.0)
}

/// Applies code-aware boosts and penalties after dense/sparse fusion.
///
/// This stage is multiplicative so BM25 and semantic order still dominate, but
/// obvious code-search signals can break close ties: symbol definitions, file
/// stem/path matches, and production-code preference.
fn rerank(
    index: &SearchIndex,
    query: &str,
    scores: HashMap<usize, f32>,
    top_k: usize,
) -> Vec<SearchResult> {
    let mut file_counts = HashMap::<String, usize>::new();
    for chunk_id in scores.keys() {
        if let Some(chunk) = index.chunk(*chunk_id) {
            *file_counts
                .entry(chunk.file_path.to_string_lossy().replace('\\', "/"))
                .or_default() += 1;
        }
    }

    let symbol_query = is_symbol_query(query);
    let symbol = query
        .trim()
        .trim_end_matches("()")
        .rsplit("::")
        .next()
        .unwrap_or(query)
        .to_string();
    let terms = query_terms(query);
    let mut candidates = scores
        .into_iter()
        .filter_map(|(chunk_id, base_score)| {
            let chunk = index.chunk(chunk_id)?;
            let path = chunk.file_path.to_string_lossy().replace('\\', "/");
            let mut score = base_score;
            score *= multi_chunk_file_boost(file_counts.get(&path).copied().unwrap_or(1));
            if symbol_query && contains_symbol_definition(&chunk.content, &symbol) {
                score *= 1.35;
            }
            score *= path_keyword_boost(&chunk.file_path, &path, &terms);
            score *= path_penalty(&path);
            Some((chunk_id, score))
        })
        .collect::<Vec<_>>();

    candidates.sort_by(|left, right| {
        right
            .1
            .total_cmp(&left.1)
            .then_with(|| left.0.cmp(&right.0))
    });

    let mut seen_per_file = HashMap::<String, usize>::new();
    // Saturation prevents one file with many adjacent chunks from crowding out
    // other relevant files while preserving deterministic ordering within ties.
    let mut saturated = candidates
        .into_iter()
        .filter_map(|(chunk_id, score)| {
            let chunk = index.chunk(chunk_id)?;
            let path = chunk.file_path.to_string_lossy().replace('\\', "/");
            let seen = seen_per_file.entry(path).or_default();
            let saturated_score = score / (1.0 + 0.2 * *seen as f32);
            *seen += 1;
            Some(SearchResult {
                score: saturated_score,
                chunk: chunk.clone(),
            })
        })
        .collect::<Vec<_>>();

    saturated.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| left.chunk.location().cmp(&right.chunk.location()))
    });
    saturated.truncate(top_k);
    saturated
}

/// Gives a small lift when independent chunks from the same file agree.
fn multi_chunk_file_boost(count: usize) -> f32 {
    1.0 + count.saturating_sub(1).min(3) as f32 * 0.08
}

/// Detects simple language-agnostic definition patterns for symbol queries.
fn contains_symbol_definition(content: &str, symbol: &str) -> bool {
    if symbol.is_empty() {
        return false;
    }
    [
        "fn", "struct", "enum", "trait", "impl", "mod", "const", "static", "class", "def",
        "function",
    ]
    .iter()
    .any(|keyword| content.contains(&format!("{keyword} {symbol}")))
}

/// Rewards query terms that appear in the file stem or recent path context.
fn path_keyword_boost(file_path: &Path, normalized_path: &str, terms: &[String]) -> f32 {
    if terms.is_empty() {
        return 1.0;
    }
    let stem_terms = file_stem_terms(file_path);
    let mut boost: f32 = 1.0;
    for term in terms {
        if stem_terms.contains(term) || normalized_path.to_lowercase().contains(term) {
            boost += 0.05;
        }
    }
    boost.min(1.25)
}

/// Penalizes paths that are usually less central than production source.
///
/// The penalties are soft so a test/doc/compat chunk can still win when the
/// retrieval signals are much stronger than nearby production-code candidates.
fn path_penalty(normalized_path: &str) -> f32 {
    let path = normalized_path.to_lowercase();
    let mut penalty = 1.0;
    if path.starts_with("tests/")
        || path.contains("/tests/")
        || path.starts_with("test/")
        || path.contains("/test/")
        || path.contains("_test.")
        || path.starts_with("spec/")
        || path.contains("/spec/")
        || path.contains("_spec.")
    {
        penalty *= 0.82;
    }
    if path.starts_with("examples/")
        || path.contains("/examples/")
        || path.starts_with("docs/")
        || path.contains("/docs/")
    {
        penalty *= 0.88;
    }
    if path.starts_with("legacy/")
        || path.contains("/legacy/")
        || path.starts_with("compat/")
        || path.contains("/compat/")
    {
        penalty *= 0.78;
    }
    if path.ends_with("__init__.py")
        || path.ends_with("package-info.java")
        || path.ends_with(".d.ts")
    {
        penalty *= 0.75;
    }
    penalty
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use pretty_assertions::assert_eq;

    use crate::cache::{CachedFileRecord, CachedIndex, CachedIndexPayloadV4, content_hash};
    use crate::files::FileManifestEntry;
    use crate::index::SearchIndex;
    use crate::matrix::EmbeddingMatrix;
    use crate::types::{Chunk, ContentFilter};

    use super::*;

    fn index_with_chunks(chunks: Vec<Chunk>) -> SearchIndex {
        let vectors: Vec<Vec<f32>> = chunks
            .iter()
            .map(|chunk| {
                if chunk.content.contains("parser") {
                    vec![1.0, 0.0]
                } else {
                    vec![0.0, 1.0]
                }
            })
            .collect();
        let embeddings = EmbeddingMatrix::from_vectors(vectors).expect("matrix");
        let records = chunks
            .into_iter()
            .enumerate()
            .map(|(idx, chunk)| {
                CachedFileRecord::new(
                    FileManifestEntry {
                        path: chunk.file_path.clone(),
                        size: chunk.content.len() as u64,
                        modified_unix_nanos: 1,
                    },
                    content_hash(&chunk.content),
                    vec![chunk],
                    idx,
                    1,
                )
            })
            .collect();
        let payload = CachedIndexPayloadV4::new(
            PathBuf::from("/repo"),
            ContentFilter::Code,
            "test".to_string(),
            &embeddings,
            records,
        );
        SearchIndex::from_cached(CachedIndex {
            payload,
            embeddings,
        })
        .expect("index")
    }

    /// Trace: L2-DES-TOOL-001
    /// Verifies: hybrid ranking applies BM25 and semantic RRF to return relevant chunks.
    #[test]
    fn rank_search_returns_relevant_chunk() {
        let index = index_with_chunks(vec![
            Chunk {
                content: "fn parse_input() { parser(); }".to_string(),
                file_path: PathBuf::from("src/parser.rs"),
                start_line: 1,
                end_line: 1,
                language: "rust".to_string(),
            },
            Chunk {
                content: "fn render_output() {}".to_string(),
                file_path: PathBuf::from("src/render.rs"),
                start_line: 1,
                end_line: 1,
                language: "rust".to_string(),
            },
        ]);

        let results = rank_search(
            &index,
            "parse input",
            &[1.0, 0.0],
            1,
            &SearchFilters::empty(),
        );

        assert_eq!(results[0].chunk.file_path, PathBuf::from("src/parser.rs"));
    }

    /// Trace: L2-DES-TOOL-001
    /// Verifies: path penalties prefer production code over test chunks when base relevance ties.
    #[test]
    fn rerank_penalizes_test_paths() {
        let index = index_with_chunks(vec![
            Chunk {
                content: "fn parse_input() { parser(); }".to_string(),
                file_path: PathBuf::from("tests/parser_test.rs"),
                start_line: 1,
                end_line: 1,
                language: "rust".to_string(),
            },
            Chunk {
                content: "fn parse_input() { parser(); }".to_string(),
                file_path: PathBuf::from("src/parser.rs"),
                start_line: 1,
                end_line: 1,
                language: "rust".to_string(),
            },
        ]);

        let results = rank_search(
            &index,
            "parse_input",
            &[1.0, 0.0],
            1,
            &SearchFilters::empty(),
        );

        assert_eq!(results[0].chunk.file_path, PathBuf::from("src/parser.rs"));
    }
}
