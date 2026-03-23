use std::collections::HashMap;

use crate::processing::Chunk;
use crate::utils::tokenize;

#[derive(Clone, Debug)]
pub struct TermEntry {
    pub df: u32,
    pub postings: Vec<(usize, u32)>,
}

#[derive(Clone, Debug)]
pub struct BM25Index {
    pub terms: HashMap<String, TermEntry>,
    pub doc_lens: Vec<usize>,
    pub avg_doc_len: f32,
    pub k1: f32,
    pub b: f32,
}

impl BM25Index {
    pub fn build(chunks: &[Chunk], k1: f32, b: f32) -> Self {
        let mut terms: HashMap<String, TermEntry> = HashMap::new();
        let mut doc_lens = Vec::with_capacity(chunks.len());

        for (doc_id, chunk) in chunks.iter().enumerate() {
            let tokens = tokenize(&chunk.clean);
            doc_lens.push(tokens.len());
            let mut tf: HashMap<String, u32> = HashMap::new();
            for token in tokens {
                *tf.entry(token).or_insert(0) += 1;
            }
            for (term, freq) in tf {
                let entry = terms.entry(term).or_insert(TermEntry { df: 0, postings: Vec::new() });
                entry.df += 1;
                entry.postings.push((doc_id, freq));
            }
        }

        let total_len: usize = doc_lens.iter().sum();
        let avg_doc_len = if doc_lens.is_empty() { 0.0 } else { total_len as f32 / doc_lens.len() as f32 };

        Self { terms, doc_lens, avg_doc_len, k1, b }
    }

    pub fn search(&self, query: &str, top_k: usize) -> Vec<(usize, f32)> {
        let tokens = tokenize(query);
        if tokens.is_empty() || self.doc_lens.is_empty() {
            return Vec::new();
        }

        let n_docs = self.doc_lens.len() as f32;
        let mut scores = vec![0.0f32; self.doc_lens.len()];

        for term in tokens {
            if let Some(entry) = self.terms.get(&term) {
                let df = entry.df as f32;
                let idf = (1.0 + (n_docs - df + 0.5) / (df + 0.5)).ln();
                for (doc_id, tf) in &entry.postings {
                    let tf = *tf as f32;
                    let doc_len = self.doc_lens[*doc_id] as f32;
                    let denom = tf + self.k1 * (1.0 - self.b + self.b * (doc_len / self.avg_doc_len.max(1.0)));
                    let score = idf * (tf * (self.k1 + 1.0) / denom);
                    scores[*doc_id] += score;
                }
            }
        }

        let mut results: Vec<(usize, f32)> = scores
            .into_iter()
            .enumerate()
            .filter(|(_, s)| *s > 0.0)
            .collect();

        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(top_k);
        results
    }
}
