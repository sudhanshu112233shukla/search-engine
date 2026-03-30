use std::collections::HashMap;

use crate::processing::Chunk;
use memmap2::Mmap;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug)]
pub struct TermEntry {
    pub df: u32,
    pub postings: Option<Vec<(u32, u32)>>,
    pub offset: u64,
    pub len: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TermMeta {
    pub df: u32,
    pub offset: u64,
    pub len: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BM25Persist {
    pub terms: HashMap<String, TermMeta>,
    pub doc_lens: Vec<usize>,
    pub avg_doc_len: f32,
    pub k1: f32,
    pub b: f32,
    pub total_len: usize,
}

#[derive(Clone, Debug)]
pub struct BM25Index {
    pub terms: HashMap<String, TermEntry>,
    pub delta_terms: HashMap<String, Vec<(u32, u32)>>,
    pub doc_lens: Vec<usize>,
    pub avg_doc_len: f32,
    pub k1: f32,
    pub b: f32,
    pub total_len: usize,
    postings_mmap: Option<Mmap>,
}

impl BM25Index {
    pub fn build(chunks: &[Chunk], k1: f32, b: f32) -> Self {
        let mut terms: HashMap<String, TermEntry> = HashMap::new();
        let mut doc_lens = Vec::with_capacity(chunks.len());

        for (doc_id, chunk) in chunks.iter().enumerate() {
            let tokens = &chunk.tokens;
            doc_lens.push(tokens.len());
            let mut tf: HashMap<String, u32> = HashMap::new();
            for token in tokens {
                *tf.entry(token.clone()).or_insert(0) += 1;
            }
            for (term, freq) in tf {
                let entry = terms.entry(term).or_insert(TermEntry {
                    df: 0,
                    postings: Some(Vec::new()),
                    offset: 0,
                    len: 0,
                });
                entry.df += 1;
                if let Some(postings) = &mut entry.postings {
                    postings.push((doc_id as u32, freq));
                    entry.len = postings.len() as u32;
                }
            }
        }

        let total_len: usize = doc_lens.iter().sum();
        let avg_doc_len = if doc_lens.is_empty() { 0.0 } else { total_len as f32 / doc_lens.len() as f32 };

        Self {
            terms,
            delta_terms: HashMap::new(),
            doc_lens,
            avg_doc_len,
            k1,
            b,
            total_len,
            postings_mmap: None,
        }
    }

    pub fn search(&self, query_tokens: &[String], top_k: usize) -> Vec<(usize, f32)> {
        if query_tokens.is_empty() || self.doc_lens.is_empty() {
            return Vec::new();
        }

        let n_docs = self.doc_lens.len() as f32;
        let mut scores = vec![0.0f32; self.doc_lens.len()];

        for term in query_tokens {
            if let Some(entry) = self.terms.get(term) {
                let delta_df = self.delta_terms.get(term).map(|v| v.len() as u32).unwrap_or(0);
                let df = (entry.df + delta_df) as f32;
                let idf = (1.0 + (n_docs - df + 0.5) / (df + 0.5)).ln();

                if let Some(postings) = &entry.postings {
                    for (doc_id, tf) in postings {
                        let tf = *tf as f32;
                        let doc_id = *doc_id as usize;
                        let doc_len = self.doc_lens[doc_id] as f32;
                        let denom = tf + self.k1 * (1.0 - self.b + self.b * (doc_len / self.avg_doc_len.max(1.0)));
                        let score = idf * (tf * (self.k1 + 1.0) / denom);
                        scores[doc_id] += score;
                    }
                } else if let Some(mmap) = &self.postings_mmap {
                    for (doc_id, tf) in postings_from_mmap(mmap, entry.offset, entry.len) {
                        let tf = tf as f32;
                        let doc_id = doc_id as usize;
                        let doc_len = self.doc_lens[doc_id] as f32;
                        let denom = tf + self.k1 * (1.0 - self.b + self.b * (doc_len / self.avg_doc_len.max(1.0)));
                        let score = idf * (tf * (self.k1 + 1.0) / denom);
                        scores[doc_id] += score;
                    }
                }

                if let Some(delta) = self.delta_terms.get(term) {
                    for (doc_id, tf) in delta {
                        let tf = *tf as f32;
                        let doc_id = *doc_id as usize;
                        let doc_len = self.doc_lens[doc_id] as f32;
                        let denom = tf + self.k1 * (1.0 - self.b + self.b * (doc_len / self.avg_doc_len.max(1.0)));
                        let score = idf * (tf * (self.k1 + 1.0) / denom);
                        scores[doc_id] += score;
                    }
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

    pub fn add_chunks(&mut self, chunks: &[Chunk]) {
        if chunks.is_empty() {
            return;
        }
        let mut next_id = self.doc_lens.len();
        for chunk in chunks {
            let tokens = &chunk.tokens;
            self.doc_lens.push(tokens.len());
            self.total_len += tokens.len();

            let mut tf: HashMap<String, u32> = HashMap::new();
            for token in tokens {
                *tf.entry(token.clone()).or_insert(0) += 1;
            }
            for (term, freq) in tf {
                let entry = self.terms.entry(term.clone()).or_insert(TermEntry {
                    df: 0,
                    postings: Some(Vec::new()),
                    offset: 0,
                    len: 0,
                });
                if entry.postings.is_some() {
                    entry.df += 1;
                    if let Some(postings) = &mut entry.postings {
                        postings.push((next_id as u32, freq));
                        entry.len = postings.len() as u32;
                    }
                } else {
                    self.delta_terms.entry(term).or_default().push((next_id as u32, freq));
                }
            }
            next_id += 1;
        }
        self.avg_doc_len = if self.doc_lens.is_empty() {
            0.0
        } else {
            self.total_len as f32 / self.doc_lens.len() as f32
        };
    }

    pub fn has_term(&self, term: &str) -> bool {
        self.terms.contains_key(term)
    }

    pub fn terms_iter(&self) -> impl Iterator<Item = &String> {
        self.terms.keys()
    }

    pub fn set_postings_mmap(&mut self, mmap: Option<Mmap>) {
        self.postings_mmap = mmap;
    }

    pub fn to_persist(&self) -> BM25Persist {
        let mut terms = HashMap::new();
        for (term, entry) in &self.terms {
            terms.insert(
                term.clone(),
                TermMeta {
                    df: entry.df,
                    offset: entry.offset,
                    len: entry.len,
                },
            );
        }
        BM25Persist {
            terms,
            doc_lens: self.doc_lens.clone(),
            avg_doc_len: self.avg_doc_len,
            k1: self.k1,
            b: self.b,
            total_len: self.total_len,
        }
    }

    pub fn from_persist(persist: BM25Persist) -> Self {
        let mut terms = HashMap::new();
        for (term, meta) in persist.terms {
            terms.insert(
                term,
                TermEntry {
                    df: meta.df,
                    postings: None,
                    offset: meta.offset,
                    len: meta.len,
                },
            );
        }
        Self {
            terms,
            delta_terms: HashMap::new(),
            doc_lens: persist.doc_lens,
            avg_doc_len: persist.avg_doc_len,
            k1: persist.k1,
            b: persist.b,
            total_len: persist.total_len,
            postings_mmap: None,
        }
    }

    pub fn collect_postings(&self, term: &str) -> Vec<(u32, u32)> {
        let mut out = Vec::new();
        if let Some(entry) = self.terms.get(term) {
            if let Some(postings) = &entry.postings {
                out.extend_from_slice(postings);
            } else if let Some(mmap) = &self.postings_mmap {
                out.extend_from_slice(&postings_from_mmap(mmap, entry.offset, entry.len));
            }
        }
        if let Some(delta) = self.delta_terms.get(term) {
            out.extend_from_slice(delta);
        }
        out
    }
}

fn postings_from_mmap(mmap: &Mmap, offset: u64, len: u32) -> Vec<(u32, u32)> {
    let start = offset as usize;
    let byte_len = len as usize * 8;
    if start + byte_len > mmap.len() {
        return Vec::new();
    }
    let slice = &mmap[start..start + byte_len];
    let mut out = Vec::with_capacity(len as usize);
    for i in 0..len as usize {
        let base = i * 8;
        let doc_id = u32::from_le_bytes([slice[base], slice[base + 1], slice[base + 2], slice[base + 3]]);
        let tf = u32::from_le_bytes([slice[base + 4], slice[base + 5], slice[base + 6], slice[base + 7]]);
        out.push((doc_id, tf));
    }
    out
}

