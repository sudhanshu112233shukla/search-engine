use crate::ingestion::Document;
use crate::utils::normalize_text;

#[derive(Clone, Debug)]
pub struct Chunk {
    pub id: String,
    pub text: String,
    pub clean: String,
}

#[derive(Clone, Debug)]
pub struct ChunkingConfig {
    pub min_words: usize,
    pub max_words: usize,
    pub target_words: usize,
}

impl Default for ChunkingConfig {
    fn default() -> Self {
        Self {
            min_words: 100,
            max_words: 200,
            target_words: 150,
        }
    }
}

pub fn chunk_document(doc: &Document, config: &ChunkingConfig) -> Vec<Chunk> {
    let words: Vec<&str> = doc.text.split_whitespace().collect();
    if words.is_empty() {
        return Vec::new();
    }

    let mut chunks: Vec<Chunk> = Vec::new();
    let mut i = 0usize;
    while i < words.len() {
        let remaining = words.len() - i;
        if remaining <= config.max_words {
            if remaining < config.min_words && !chunks.is_empty() {
                let last = chunks.last_mut().unwrap();
                let mut text = last.text.clone();
                text.push(' ');
                text.push_str(&words[i..].join(" "));
                last.text = text;
                last.clean = normalize_text(&last.text);
            } else {
                let text = words[i..].join(" ");
                let clean = normalize_text(&text);
                let id = format!("{}::chunk{}", doc.id, chunks.len() + 1);
                chunks.push(Chunk { id, text, clean });
            }
            break;
        }

        let size = config.target_words.min(config.max_words);
        let text = words[i..i + size].join(" ");
        let clean = normalize_text(&text);
        let id = format!("{}::chunk{}", doc.id, chunks.len() + 1);
        chunks.push(Chunk { id, text, clean });
        i += size;
    }

    chunks
}

pub fn process_documents(docs: &[Document], config: &ChunkingConfig) -> Vec<Chunk> {
    let mut all = Vec::new();
    for doc in docs {
        let mut chunks = chunk_document(doc, config);
        all.append(&mut chunks);
    }
    all
}
