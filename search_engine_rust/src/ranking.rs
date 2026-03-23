use crate::retrieval::Candidate;

#[derive(Clone, Debug)]
pub struct Ranked {
    pub doc_id: usize,
    pub score: f32,
}

pub fn hybrid_rank(mut candidates: Vec<Candidate>) -> Vec<Ranked> {
    let mut ranked: Vec<Ranked> = candidates
        .drain(..)
        .map(|c| Ranked {
            doc_id: c.doc_id,
            score: 0.5 * c.bm25_score + 0.5 * c.semantic_score,
        })
        .collect();

    ranked.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    ranked
}
