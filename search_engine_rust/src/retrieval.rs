use crate::bm25::BM25Index;
use crate::vector::VectorIndex;

pub fn retrieve(
    bm25: &BM25Index,
    vector: &VectorIndex,
    query_tokens: &[String],
    query_raw: &str,
    top_k: usize,
) -> (Vec<(usize, f32)>, Vec<(usize, f32)>) {
    let bm25_results = bm25.search(query_tokens, top_k);
    let vector_results = vector.search(query_raw, top_k);
    (bm25_results, vector_results)
}
