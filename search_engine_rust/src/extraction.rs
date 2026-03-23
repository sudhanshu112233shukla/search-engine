use crate::ranking::Ranked;
use crate::processing::Chunk;
use crate::utils::{normalize_text, split_sentences, tokenize};

#[derive(Clone, Debug)]
pub struct AnswerCandidate {
    pub text: String,
    pub score: f32,
    pub source: String,
}

fn keyword_overlap(sentence: &str, query_tokens: &[String]) -> f32 {
    if query_tokens.is_empty() {
        return 0.0;
    }
    let sentence_tokens = tokenize(sentence);
    if sentence_tokens.is_empty() {
        return 0.0;
    }
    let mut overlap = 0usize;
    for qt in query_tokens {
        if sentence_tokens.iter().any(|t| t == qt) {
            overlap += 1;
        }
    }
    overlap as f32 / query_tokens.len() as f32
}

fn length_penalty(sentence: &str) -> f32 {
    let words = sentence.split_whitespace().count().max(1) as f32;
    1.0 / (1.0 + (words / 25.0))
}

pub fn extract_answers(query: &str, ranked: &[Ranked], chunks: &[Chunk]) -> Vec<AnswerCandidate> {
    let query_tokens = tokenize(query);
    let clean_query = normalize_text(query);
    let mut candidates: Vec<AnswerCandidate> = Vec::new();

    for r in ranked.iter().take(10) {
        let chunk = &chunks[r.doc_id];
        let sentences = split_sentences(&chunk.text);
        for sentence in sentences {
            let overlap = keyword_overlap(&sentence, &query_tokens);
            let exact = if !clean_query.is_empty() && normalize_text(&sentence).contains(&clean_query) { 1.0 } else { 0.0 };
            let base = 0.6 * r.score + 0.3 * overlap + 0.1 * exact;
            let score = base * length_penalty(&sentence);
            candidates.push(AnswerCandidate {
                text: sentence,
                score,
                source: chunk.id.clone(),
            });
        }
    }

    candidates.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    candidates
}
