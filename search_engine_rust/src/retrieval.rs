use std::collections::HashMap;

use crate::bm25::BM25Index;
use crate::vector::VectorIndex;

#[derive(Clone, Debug)]
pub struct Candidate {
    pub doc_id: usize,
    pub bm25_score: f32,
    pub semantic_score: f32,
}

pub fn retrieve(bm25: &BM25Index, vector: &VectorIndex, query: &str, top_k: usize) -> Vec<Candidate> {
    let bm25_results = bm25.search(query, top_k);
    let vector_results = vector.search(query, top_k);

    let mut map: HashMap<usize, (f32, f32)> = HashMap::new();
    for (doc_id, score) in bm25_results {
        map.insert(doc_id, (score, 0.0));
    }
    for (doc_id, score) in vector_results {
        map.entry(doc_id)
            .and_modify(|e| e.1 = score)
            .or_insert((0.0, score));
    }

    map.into_iter()
        .map(|(doc_id, (bm25_score, semantic_score))| Candidate { doc_id, bm25_score, semantic_score })
        .collect()
}
