mod ingestion;
mod processing;
mod bm25;
mod embedding;
mod vector;
mod retrieval;
mod ranking;
mod extraction;
mod confidence;
mod utils;
mod text_store;
mod index_store;
mod ffi;
mod evaluation;
mod crawler;
mod processor;
mod storage;
mod cli;

use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use serde::Serialize;

pub use ingestion::{Document, load_text_dir, load_text_file};
pub use processing::ChunkingConfig;
pub use evaluation::{EvalDataset, EvalQuery, EvalReport};
pub use crawler::{Crawler, CrawlConfig};
pub use processor::{Processor, ProcessConfig};
pub use storage::{StorageManager, PipelineConfig, RawPage, ProcessedChunk};
pub use cli::{Command as PipelineCommand};

use processing::{process_documents, Chunk};
use bm25::BM25Index;
use vector::{AnnConfig, PQConfig, VectorIndex};
use retrieval::retrieve;
use ranking::{rank_candidates, RankingWeights, Ranked};
use extraction::extract_answers;
use confidence::compute_confidence;
use utils::{make_snippet, normalize_text, process_query, tokenize};
use text_store::TextStore;

#[derive(Clone, Debug, Serialize)]
pub struct SearchResponse {
    pub answer: Option<Answer>,
    pub answers: Vec<Answer>,
    pub results: Vec<ResultItem>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Answer {
    pub text: String,
    pub confidence: f32,
    pub source: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ResultItem {
    pub id: String,
    pub text: String,
    pub score: f32,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub chunking: ChunkingConfig,
    pub bm25_k1: f32,
    pub bm25_b: f32,
    pub vector_dims: usize,
    pub vector_ngram_min: usize,
    pub vector_ngram_max: usize,
    pub vector_quantize: bool,
    pub ann_enabled: bool,
    pub ann_nlist: usize,
    pub ann_nprobe: usize,
    pub ann_max_kmeans_iters: usize,
    pub ann_sample_size: usize,
    pub pq_enabled: bool,
    pub pq_m: usize,
    pub pq_k: usize,
    pub pq_max_kmeans_iters: usize,
    pub pq_sample_size: usize,
    pub low_memory: bool,
    pub retrieval_top_k: usize,
    pub results_top_k: usize,
    pub ranking_weights: RankingWeights,
    pub cache_size: usize,
    pub text_store_path: Option<String>,
    pub text_store_mmap: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            chunking: ChunkingConfig::default(),
            bm25_k1: 1.5,
            bm25_b: 0.75,
            vector_dims: 384,
            vector_ngram_min: 3,
            vector_ngram_max: 5,
            vector_quantize: false,
            ann_enabled: false,
            ann_nlist: 64,
            ann_nprobe: 4,
            ann_max_kmeans_iters: 8,
            ann_sample_size: 5000,
            pq_enabled: false,
            pq_m: 8,
            pq_k: 256,
            pq_max_kmeans_iters: 8,
            pq_sample_size: 5000,
            low_memory: false,
            retrieval_top_k: 50,
            results_top_k: 10,
            ranking_weights: RankingWeights::default(),
            cache_size: 100,
            text_store_path: None,
            text_store_mmap: true,
        }
    }
}

struct QueryCache {
    map: HashMap<String, SearchResponse>,
    order: VecDeque<String>,
    cap: usize,
}

impl QueryCache {
    fn new(cap: usize) -> Self {
        Self { map: HashMap::new(), order: VecDeque::new(), cap }
    }

    fn get(&mut self, key: &str) -> Option<SearchResponse> {
        if let Some(v) = self.map.get(key) {
            self.order.retain(|k| k != key);
            self.order.push_back(key.to_string());
            return Some(v.clone());
        }
        None
    }

    fn put(&mut self, key: String, value: SearchResponse) {
        if self.map.contains_key(&key) {
            self.order.retain(|k| k != &key);
        }
        self.map.insert(key.clone(), value);
        self.order.push_back(key);
        while self.order.len() > self.cap {
            if let Some(old) = self.order.pop_front() {
                self.map.remove(&old);
            }
        }
    }
}

pub struct SearchEngine {
    chunks: Vec<Chunk>,
    bm25: BM25Index,
    vector: VectorIndex,
    config: Config,
    cache: Mutex<QueryCache>,
    text_store: Option<TextStore>,
}

impl SearchEngine {
    pub fn new(docs: Vec<Document>, config: Config) -> Self {
        let mut chunks = process_documents(&docs, &config.chunking);
        let bm25 = BM25Index::build(&chunks, config.bm25_k1, config.bm25_b);
        let ann = AnnConfig {
            enabled: config.ann_enabled,
            nlist: config.ann_nlist,
            nprobe: config.ann_nprobe,
            max_iters: config.ann_max_kmeans_iters,
            sample_size: config.ann_sample_size,
        };
        let pq = PQConfig {
            enabled: config.pq_enabled,
            m: config.pq_m,
            k: config.pq_k,
            max_iters: config.pq_max_kmeans_iters,
            sample_size: config.pq_sample_size,
        };
        let vector = VectorIndex::build(
            &chunks,
            config.vector_dims,
            config.vector_ngram_min,
            config.vector_ngram_max,
            config.vector_quantize,
            &ann,
            &pq,
        );
        let cache = Mutex::new(QueryCache::new(config.cache_size));
        let text_store = if let Some(path) = config.text_store_path.as_ref() {
            let path = Path::new(path);
            match TextStore::build(path, &mut chunks, config.text_store_mmap) {
                Ok(store) => Some(store),
                Err(err) => {
                    eprintln!("[text_store] failed to build: {err}");
                    None
                }
            }
        } else {
            None
        };

        if config.low_memory {
            for chunk in &mut chunks {
                chunk.tokens.clear();
                chunk.positions.clear();
            }
        }
        Self { chunks, bm25, vector, config, cache, text_store }
    }

    pub fn search(&self, query: &str) -> SearchResponse {
        if query.trim().is_empty() {
            return SearchResponse { answer: None, answers: Vec::new(), results: Vec::new() };
        }

        let cache_key = normalize_text(query);
        if let Ok(mut cache) = self.cache.lock() {
            if let Some(cached) = cache.get(&cache_key) {
                return cached;
            }
        }

        let response = self.search_inner(query);
        if let Ok(mut cache) = self.cache.lock() {
            cache.put(cache_key, response.clone());
        }
        response
    }

    fn search_inner(&self, query: &str) -> SearchResponse {
        let base_tokens = tokenize(query);
        let expanded_tokens = process_query(query);
        let clean_query = normalize_text(query);

        let (bm25_results, vector_results) = retrieve(
            &self.bm25,
            &self.vector,
            &expanded_tokens,
            query,
            self.config.retrieval_top_k,
        );

        let ranked = rank_candidates(
            &bm25_results,
            &vector_results,
            &self.chunks,
            &base_tokens,
            &clean_query,
            &self.config.ranking_weights,
        );

        let mut results = Vec::new();
        for r in ranked.iter().take(self.config.results_top_k) {
            let chunk = &self.chunks[r.doc_id];
            let text = self.chunk_text(r.doc_id);
            let snippet = make_snippet(&text, &base_tokens, 180);
            results.push(ResultItem {
                id: chunk.id.clone(),
                text: snippet,
                score: r.score,
            });
        }

        let candidates = extract_answers(query, &ranked, &self.chunks, |idx| self.chunk_text(idx));
        let mut answers: Vec<Answer> = candidates
            .iter()
            .take(3)
            .map(|a| {
                let (confidence, _level) = compute_confidence(a.score);
                Answer {
                    text: a.text.clone(),
                    confidence,
                    source: a.source.clone(),
                }
            })
            .collect();

        let answer = answers.first().cloned();
        if let Some(top) = answer.as_ref() {
            if top.confidence > 0.5 {
                answers.truncate(1);
            }
        }

        SearchResponse { answer, answers, results }
    }

    pub fn rank_debug(&self, query: &str) -> (Vec<Ranked>, Vec<(String, f32, crate::ranking::ScoreBreakdown)>) {
        let base_tokens = tokenize(query);
        let expanded_tokens = process_query(query);
        let clean_query = normalize_text(query);

        let (bm25_results, vector_results) = retrieve(
            &self.bm25,
            &self.vector,
            &expanded_tokens,
            query,
            self.config.retrieval_top_k,
        );

        let ranked = rank_candidates(
            &bm25_results,
            &vector_results,
            &self.chunks,
            &base_tokens,
            &clean_query,
            &self.config.ranking_weights,
        );

        let breakdowns = ranked.iter().map(|r| {
            let id = self.chunks[r.doc_id].id.clone();
            (id, r.score, r.breakdown.clone())
        }).collect();

        (ranked, breakdowns)
    }

    pub fn doc_id(&self, idx: usize) -> Option<String> {
        self.chunks.get(idx).map(|c| c.id.clone())
    }

    pub fn approx_memory_bytes(&self) -> usize {
        let mut total = 0usize;
        for c in &self.chunks {
            total += c.text.len();
            total += c.clean.len();
            for t in &c.tokens {
                total += t.len();
            }
            for positions in c.positions.values() {
                total += positions.len() * std::mem::size_of::<usize>();
            }
        }
        total += self.vector.approx_bytes();
        if let Some(store) = &self.text_store {
            total += store.byte_len() as usize;
        }
        total
    }

    pub fn update_documents(&mut self, docs: Vec<Document>) -> usize {
        if docs.is_empty() {
            return 0;
        }
        let mut new_chunks = process_documents(&docs, &self.config.chunking);
        if let Some(store) = &mut self.text_store {
            if let Err(err) = store.append(&mut new_chunks) {
                eprintln!("[text_store] append failed: {err}");
            }
        }
        let added = new_chunks.len();
        self.bm25.add_chunks(&new_chunks);
        self.vector.add_chunks(&new_chunks);
        self.chunks.extend(new_chunks);
        added
    }

    fn chunk_text(&self, idx: usize) -> String {
        if let Some(chunk) = self.chunks.get(idx) {
            if !chunk.text.is_empty() {
                return chunk.text.clone();
            }
            if let Some(store) = &self.text_store {
                if let Some(text) = store.get_text(chunk.text_offset, chunk.text_len) {
                    return text;
                }
            }
        }
        String::new()
    }

    pub fn save_index(&self, dir: &str) -> std::io::Result<()> {
        let store = IndexStore::new(dir);
        let text_store_file = if let Some(ts) = &self.text_store {
            let path = store.copy_text_store(ts.path())?;
            Some(path.to_string_lossy().to_string())
        } else {
            None
        };
        store.save_meta(&IndexMeta { text_store_file })?;
        store.save_chunks(&self.chunks)?;
        store.save_bm25(&self.bm25)?;
        store.save_vector(&self.vector)?;
        Ok(())
    }

    pub fn load_index(dir: &str, config: Config) -> std::io::Result<Self> {
        let store = IndexStore::new(dir);
        let meta = store.load_meta()?;
        let chunk_meta = store.load_chunks()?;
        let mut chunks: Vec<Chunk> = chunk_meta
            .into_iter()
            .map(|c| Chunk {
                id: c.id,
                text: String::new(),
                clean: c.clean,
                tokens: Vec::new(),
                positions: std::collections::HashMap::new(),
                text_offset: c.text_offset,
                text_len: c.text_len,
            })
            .collect();
        let bm25 = store.load_bm25()?;
        let vector = store.load_vector()?;
        let cache = Mutex::new(QueryCache::new(config.cache_size));

        let text_store = match meta.text_store_file {
            Some(path) => TextStore::open(Path::new(&path), config.text_store_mmap).ok(),
            None => None,
        };

        if config.low_memory {
            for chunk in &mut chunks {
                chunk.tokens.clear();
                chunk.positions.clear();
            }
        }
        Ok(Self { chunks, bm25, vector, config, cache, text_store })
    }
}

static ENGINE: OnceLock<Mutex<SearchEngine>> = OnceLock::new();

pub fn init_engine(engine: SearchEngine) {
    let _ = ENGINE.set(Mutex::new(engine));
}

pub fn init_engine_once(engine: SearchEngine) -> bool {
    ENGINE.set(Mutex::new(engine)).is_ok()
}

pub fn search(query: &str) -> SearchResponse {
    if let Some(engine) = ENGINE.get() {
        if let Ok(engine) = engine.lock() {
            return engine.search(query);
        }
    }
    SearchResponse { answer: None, answers: Vec::new(), results: Vec::new() }
}

pub fn update_documents(docs: Vec<Document>) -> usize {
    if let Some(engine) = ENGINE.get() {
        if let Ok(mut engine) = engine.lock() {
            return engine.update_documents(docs);
        }
    }
    0
}
use index_store::{IndexMeta, IndexStore};
