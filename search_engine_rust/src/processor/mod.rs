use std::collections::HashSet;

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
        let mut seen: HashSet<String> = HashSet::new();
        for page in pages {
            let hash = normalize_text(&page.text);
            if !seen.insert(hash) {
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
