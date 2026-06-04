//! Row-major embedding matrix.
//!
//! The rest of code-search treats chunk id, embedding row id, and BM25 document
//! id as the same number. This small type centralizes the row-major vector
//! invariant so cache loading, incremental refresh, exact scans, and HNSW
//! construction all reject dimension drift in the same way.

use crate::types::CodeSearchError;

/// Flat row-major f32 matrix used for dense embeddings.
#[derive(Debug, Clone, PartialEq)]
pub struct EmbeddingMatrix {
    dimensions: usize,
    rows: Vec<f32>,
}

impl EmbeddingMatrix {
    /// Creates an empty matrix with no known dimension.
    pub fn empty() -> Self {
        Self {
            dimensions: 0,
            rows: Vec::new(),
        }
    }

    /// Creates a matrix from already-flat row-major values.
    ///
    /// Empty matrices must have zero dimensions; non-empty matrices must have a
    /// value count divisible by the dimension so row slicing stays safe.
    pub fn new(dimensions: usize, rows: Vec<f32>) -> Result<Self, CodeSearchError> {
        if dimensions == 0 {
            if rows.is_empty() {
                return Ok(Self::empty());
            }
            return Err(CodeSearchError::Index(
                "embedding dimensions must be non-zero when rows are present".to_string(),
            ));
        }
        if !rows.len().is_multiple_of(dimensions) {
            return Err(CodeSearchError::Index(format!(
                "embedding matrix has {} values, not divisible by {dimensions} dimensions",
                rows.len()
            )));
        }
        Ok(Self { dimensions, rows })
    }

    /// Flattens provider vectors into the matrix representation.
    ///
    /// Providers are required to keep dimensions stable. This check catches
    /// mismatched rows before the cache can assign chunk ids to invalid vectors.
    pub fn from_vectors(vectors: Vec<Vec<f32>>) -> Result<Self, CodeSearchError> {
        let Some(first) = vectors.first() else {
            return Ok(Self::empty());
        };
        let dimensions = first.len();
        if dimensions == 0 {
            return Err(CodeSearchError::Index(
                "embedding provider returned zero-dimensional vectors".to_string(),
            ));
        }
        let mut rows = Vec::with_capacity(vectors.len() * dimensions);
        for vector in vectors {
            if vector.len() != dimensions {
                return Err(CodeSearchError::Index(
                    "embedding provider returned vectors with inconsistent dimensions".to_string(),
                ));
            }
            rows.extend(vector);
        }
        Self::new(dimensions, rows)
    }

    /// Returns the embedding dimension for every row.
    pub fn dimensions(&self) -> usize {
        self.dimensions
    }

    /// Returns the number of complete rows.
    pub fn row_count(&self) -> usize {
        if self.dimensions == 0 {
            0
        } else {
            self.rows.len() / self.dimensions
        }
    }

    /// Returns a row slice without allocating.
    pub fn row(&self, idx: usize) -> Option<&[f32]> {
        if idx >= self.row_count() {
            return None;
        }
        let start = idx * self.dimensions;
        let end = start + self.dimensions;
        Some(&self.rows[start..end])
    }

    /// Returns all row-major values for binary cache serialization.
    pub fn rows(&self) -> &[f32] {
        &self.rows
    }

    /// Appends one row while preserving matrix dimensionality.
    pub fn append_row_slice(&mut self, row: &[f32]) -> Result<(), CodeSearchError> {
        if row.is_empty() {
            return Err(CodeSearchError::Index(
                "cannot append zero-dimensional embedding row".to_string(),
            ));
        }
        if self.dimensions == 0 {
            self.dimensions = row.len();
        } else if self.dimensions != row.len() {
            return Err(CodeSearchError::Index(format!(
                "embedding dimensions changed from {} to {}",
                self.dimensions,
                row.len()
            )));
        }
        self.rows.extend_from_slice(row);
        Ok(())
    }

    /// Copies a range of rows from another matrix.
    ///
    /// Incremental refresh uses this for reused cache records so the new matrix
    /// can drop deleted files while keeping chunk id == row id in the runtime
    /// index.
    pub fn extend_rows_from(
        &mut self,
        other: &Self,
        start: usize,
        count: usize,
    ) -> Result<(), CodeSearchError> {
        for row_idx in start..start + count {
            let row = other.row(row_idx).ok_or_else(|| {
                CodeSearchError::Index(format!(
                    "cached embedding row {row_idx} is outside the matrix"
                ))
            })?;
            self.append_row_slice(row)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    /// Trace: L2-DES-TOOL-001
    /// Verifies: embedding matrices flatten vectors and expose stable row slices.
    #[test]
    fn matrix_flattens_vectors_to_row_major_storage() {
        let matrix =
            EmbeddingMatrix::from_vectors(vec![vec![1.0, 2.0], vec![3.0, 4.0]]).expect("matrix");

        assert_eq!(matrix.dimensions(), 2);
        assert_eq!(matrix.row_count(), 2);
        assert_eq!(matrix.row(1), Some([3.0, 4.0].as_slice()));
    }

    /// Trace: L2-DES-TOOL-001
    /// Verifies: inconsistent embedding dimensions are rejected before indexing.
    #[test]
    fn matrix_rejects_inconsistent_dimensions() {
        let matrix = EmbeddingMatrix::from_vectors(vec![vec![1.0], vec![2.0, 3.0]]);

        assert_eq!(matrix.is_err(), true);
    }
}
