use crate::ranking::Ranked;
use crate::processing::Chunk;
use crate::utils::{split_sentences, tokenize};

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

pub fn extract_answer(query: &str, ranked: &[Ranked], chunks: &[Chunk]) -> Option<AnswerCandidate> {
    let query_tokens = tokenize(query);
    let mut best: Option<AnswerCandidate> = None;

    for r in ranked.iter().take(10) {
        let chunk = &chunks[r.doc_id];
        let sentences = split_sentences(&chunk.text);
        for sentence in sentences {
            let overlap = keyword_overlap(&sentence, &query_tokens);
            let score = 0.6 * r.score + 0.4 * overlap;
            match &best {
                None => {
                    best = Some(AnswerCandidate {
                        text: sentence,
                        score,
                        source: chunk.id.clone(),
                    });
                }
                Some(b) => {
                    if score > b.score {
                        best = Some(AnswerCandidate {
                            text: sentence,
                            score,
                            source: chunk.id.clone(),
                        });
                    }
                }
            }
        }
    }

    best
}
