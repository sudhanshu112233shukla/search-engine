use std::collections::{HashMap, HashSet};

use crate::processing::ChunkingConfig;
use crate::storage::{ProcessedChunk, RawPage, StorageManager};
use crate::utils::{normalize_text, tokenize};

#[derive(Clone, Debug)]
pub struct ProcessConfig {
    pub min_words: usize,
    pub max_words: usize,
}

pub struct Processor {
    config: ProcessConfig,
    chunking: ChunkingConfig,
    storage: StorageManager,
}

impl Processor {
    pub fn new(config: ProcessConfig, storage: StorageManager) -> Self {
        let chunking = ChunkingConfig {
            min_words: config.min_words,
            max_words: config.max_words,
            target_words: ((config.min_words + config.max_words) / 2).max(1),
        };
        Self { config, chunking, storage }
    }

    pub fn process_all(&self) {
        let pages = self.storage.read_raw();
        let mut exact_seen: HashSet<String> = HashSet::new();
        let mut buckets: HashMap<u16, Vec<u64>> = HashMap::new();
        for page in pages {
            let hash = normalize_text(&page.text);
            if !exact_seen.insert(hash) {
                continue;
            }
            if is_near_duplicate(&page.text, &mut buckets) {
                continue;
            }
            let chunks = chunk_text(&page, &self.chunking);
            let _ = self.storage.write_processed(&chunks);
        }
    }
}

fn chunk_text(page: &RawPage, config: &ChunkingConfig) -> Vec<ProcessedChunk> {
    let words: Vec<&str> = page.text.split_whitespace().collect();
    if words.is_empty() {
        return Vec::new();
    }

    let mut chunks = Vec::new();
    let mut i = 0usize;
    while i < words.len() {
        let remaining = words.len() - i;
        let size = if remaining <= config.max_words {
            remaining
        } else {
            config.target_words.min(config.max_words)
        };
        let text = words[i..i + size].join(" ");
        let tokens = tokenize(&text);
        let id = format!("{}::chunk{}", page.id, chunks.len() + 1);
        chunks.push(ProcessedChunk {
            id,
            text,
            tokens,
            source_url: page.url.clone(),
        });
        i += size;
    }

    chunks
}

fn is_near_duplicate(text: &str, buckets: &mut HashMap<u16, Vec<u64>>) -> bool {
    let sig = simhash(text);
    let bucket = (sig >> 48) as u16;
    if let Some(list) = buckets.get(&bucket) {
        for &other in list {
            if hamming(sig, other) <= 3 {
                return true;
            }
        }
    }
    buckets.entry(bucket).or_default().push(sig);
    false
}

fn simhash(text: &str) -> u64 {
    let tokens = tokenize(text);
    if tokens.is_empty() {
        return 0;
    }
    let mut weights = [0i32; 64];
    for tok in tokens {
        let h = murmur64(&tok);
        for i in 0..64 {
            if (h >> i) & 1 == 1 {
                weights[i] += 1;
            } else {
                weights[i] -= 1;
            }
        }
    }
    let mut out = 0u64;
    for i in 0..64 {
        if weights[i] > 0 {
            out |= 1u64 << i;
        }
    }
    out
}

fn murmur64(s: &str) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn hamming(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}
