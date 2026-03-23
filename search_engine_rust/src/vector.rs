use crate::embedding::embed_text;
use crate::processing::Chunk;

#[derive(Clone, Debug)]
pub struct VectorIndex {
    pub vectors: Vec<Vec<f32>>,
    pub dims: usize,
    pub ngram_min: usize,
    pub ngram_max: usize,
}

impl VectorIndex {
    pub fn build(chunks: &[Chunk], dims: usize, ngram_min: usize, ngram_max: usize) -> Self {
        let mut vectors = Vec::with_capacity(chunks.len());
        for chunk in chunks {
            let v = embed_text(&chunk.clean, dims, ngram_min, ngram_max);
            vectors.push(v);
        }
        Self { vectors, dims, ngram_min, ngram_max }
    }

    pub fn search(&self, query: &str, top_k: usize) -> Vec<(usize, f32)> {
        if self.vectors.is_empty() {
            return Vec::new();
        }
        let q = embed_text(query, self.dims, self.ngram_min, self.ngram_max);
        let mut results = Vec::with_capacity(self.vectors.len());
        for (i, v) in self.vectors.iter().enumerate() {
            let mut dot = 0.0f32;
            for j in 0..self.dims {
                dot += q[j] * v[j];
            }
            results.push((i, dot));
        }
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(top_k);
        results
    }
}
