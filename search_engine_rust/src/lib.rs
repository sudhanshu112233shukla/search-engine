use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use serde::Serialize;

pub use ingestion::{Document, load_text_dir, load_text_file};
pub use processing::ChunkingConfig;
pub use evaluation::{EvalDataset, EvalQuery, EvalReport};
pub use crawler::{Crawler, CrawlConfig};
pub use processor::{Processor, ProcessConfig};
pub use storage::{StorageManager, PipelineConfig, RawPage, ProcessedChunk};
pub use cli::{Command as PipelineCommand};
pub use bundle::{BundleManifest, BundleProfile, ShardInfo};
pub use datasets::{import_osm_pbf, import_warc, import_wikipedia};

use processing::{process_documents, Chunk};
use bm25::BM25Index;
use vector::{AnnConfig, PQConfig, VectorIndex};
use retrieval::retrieve;
use ranking::{adjust_weights_for_intent, rank_candidates, RankingWeights, Ranked};
use extraction::extract_answers;
use confidence::compute_confidence;
use utils::{detect_intent, detect_language, expand_tokens, make_snippet, normalize_text, tokenize_with_lang, Lang, QueryIntent};
use text_store::TextStore;
use index_store::{IndexMeta, IndexStore, WalOp};

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
mod bundle;
mod datasets;
mod ffi;
pub mod evaluation;
mod crawler;
mod processor;
mod storage;
pub mod cli;

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

#[derive(Clone, Debug, Serialize)]
pub struct IndexHealth {
    pub doc_count: usize,
    pub deleted_count: usize,
    pub index_version: u32,
    pub index_updated_at: u64,
    pub text_store_bytes: u64,
    pub vector_bytes: usize,
    pub ok: bool,
    pub message: String,
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
    pub hnsw_enabled: bool,
    pub hnsw_m: usize,
    pub hnsw_ef_construction: usize,
    pub hnsw_ef_search: usize,
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
    pub vector_mmap: bool,
    pub bm25_mmap: bool,
    pub vector_enabled: bool,
    pub wal_enabled: bool,
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
            hnsw_enabled: false,
            hnsw_m: 16,
            hnsw_ef_construction: 128,
            hnsw_ef_search: 64,
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
            vector_mmap: true,
            bm25_mmap: true,
            vector_enabled: true,
            wal_enabled: true,
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
    deleted: HashSet<String>,
    term_buckets: HashMap<(String, usize), Vec<String>>,
    meta: IndexMeta,
    wal_dir: Option<String>,
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
            hnsw_enabled: config.hnsw_enabled,
            hnsw_m: config.hnsw_m,
            hnsw_ef_construction: config.hnsw_ef_construction,
            hnsw_ef_search: config.hnsw_ef_search,
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

        let term_buckets = build_term_buckets(&bm25);
        let meta = IndexMeta {
            version: 2,
            text_store_file: config.text_store_path.clone(),
            doc_count: chunks.len(),
            deleted_count: 0,
            updated_at: IndexStore::now_ts(),
        };

        if config.low_memory {
            for chunk in &mut chunks {
                chunk.tokens.clear();
                chunk.positions.clear();
            }
        }
        Self {
            chunks,
            bm25,
            vector,
            config,
            cache,
            text_store,
            deleted: HashSet::new(),
            term_buckets,
            meta,
            wal_dir: None,
        }
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
        let lang = detect_language(query);
        let mut base_tokens = tokenize_with_lang(query, lang);
        base_tokens = self.correct_tokens(lang, base_tokens);
        let expanded_tokens = expand_tokens(&base_tokens, lang);
        let clean_query = normalize_text(query);

        let intent = detect_intent(query, lang);
        let tuned_weights = adjust_weights_for_intent(&self.config.ranking_weights, intent);

        let retrieval_top_k = match intent {
            QueryIntent::Factual => self.config.retrieval_top_k.max(150),
            QueryIntent::List => self.config.retrieval_top_k.max(100),
            QueryIntent::Comparison => self.config.retrieval_top_k.max(100),
            QueryIntent::Other => self.config.retrieval_top_k,
        };

        let (bm25_results, vector_results) = retrieve(
            &self.bm25,
            &self.vector,
            &expanded_tokens,
            query,
            retrieval_top_k,
        );

        let ranked = rank_candidates(
            &bm25_results,
            &vector_results,
            &self.chunks,
            &base_tokens,
            &clean_query,
            &tuned_weights,
            intent,
        );

        let ranked: Vec<Ranked> = ranked
            .into_iter()
            .filter(|r| {
                if let Some(chunk) = self.chunks.get(r.doc_id) {
                    !self.deleted.contains(&chunk.id)
                } else {
                    false
                }
            })
            .collect();

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

        let answer = answers.first().cloned().or_else(|| {
            results.first().map(|item| Answer {
                text: item.text.clone(),
                confidence: 0.35,
                source: item.id.clone(),
            })
        });
        if answer.is_some() {
            answers.truncate(1);
        }

        SearchResponse { answer, answers, results }
    }

    fn correct_tokens(&self, _lang: Lang, tokens: Vec<String>) -> Vec<String> {
        let mut out = Vec::with_capacity(tokens.len());
        for t in tokens {
            if self.bm25.has_term(&t) {
                out.push(t);
                continue;
            }
            if let Some(best) = suggest_correction(&t, &self.term_buckets) {
                out.push(best);
            } else {
                out.push(t);
            }
        }
        out
    }

    pub fn rank_debug(&self, query: &str) -> (Vec<Ranked>, Vec<(String, f32, crate::ranking::ScoreBreakdown)>) {
        let lang = detect_language(query);
        let base_tokens = tokenize_with_lang(query, lang);
        let expanded_tokens = expand_tokens(&base_tokens, lang);
        let clean_query = normalize_text(query);
        let intent = detect_intent(query, lang);
        let tuned_weights = adjust_weights_for_intent(&self.config.ranking_weights, intent);

        let retrieval_top_k = match intent {
            QueryIntent::Factual => self.config.retrieval_top_k.max(150),
            QueryIntent::List => self.config.retrieval_top_k.max(100),
            QueryIntent::Comparison => self.config.retrieval_top_k.max(100),
            QueryIntent::Other => self.config.retrieval_top_k,
        };

        let (bm25_results, vector_results) = retrieve(
            &self.bm25,
            &self.vector,
            &expanded_tokens,
            query,
            retrieval_top_k,
        );

        let ranked = rank_candidates(
            &bm25_results,
            &vector_results,
            &self.chunks,
            &base_tokens,
            &clean_query,
            &tuned_weights,
            intent,
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

    pub fn index_health(&self) -> IndexHealth {
        let text_bytes = self.text_store.as_ref().map(|t| t.byte_len()).unwrap_or(0);
        let ok = self.meta.doc_count >= self.deleted.len();
        let message = if ok { "ok" } else { "metadata mismatch" };
        IndexHealth {
            doc_count: self.meta.doc_count,
            deleted_count: self.deleted.len(),
            index_version: self.meta.version,
            index_updated_at: self.meta.updated_at,
            text_store_bytes: text_bytes,
            vector_bytes: self.vector.approx_bytes(),
            ok,
            message: message.to_string(),
        }
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
        update_term_buckets(&mut self.term_buckets, &new_chunks);
        self.chunks.extend(new_chunks);
        if let Some(dir) = &self.wal_dir {
            let store = IndexStore::new(dir);
            let _ = store.append_wal(&WalOp::Add(docs));
        }
        self.meta.doc_count = self.chunks.len();
        self.meta.updated_at = IndexStore::now_ts();
        added
    }

    pub fn delete_documents(&mut self, ids: &[String]) -> usize {
        let mut count = 0usize;
        for id in ids {
            if self.deleted.insert(id.clone()) {
                count += 1;
            }
        }
        if count > 0 {
            if let Some(dir) = &self.wal_dir {
                let store = IndexStore::new(dir);
                let _ = store.append_wal(&WalOp::Delete(ids.to_vec()));
            }
        }
        self.meta.deleted_count = self.deleted.len();
        self.meta.updated_at = IndexStore::now_ts();
        count
    }

    pub fn live_documents(&self) -> Vec<Document> {
        let mut docs = Vec::new();
        for (idx, chunk) in self.chunks.iter().enumerate() {
            if self.deleted.contains(&chunk.id) {
                continue;
            }
            let text = self.chunk_text(idx);
            if text.is_empty() {
                continue;
            }
            docs.push(Document { id: chunk.id.clone(), text });
        }
        docs
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

    pub fn chunk_text_by_id(&self, id: &str) -> String {
        for (idx, chunk) in self.chunks.iter().enumerate() {
            if chunk.id == id {
                return self.chunk_text(idx);
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
        store.save_meta(&IndexMeta {
            version: 2,
            text_store_file,
            doc_count: self.chunks.len(),
            deleted_count: self.deleted.len(),
            updated_at: IndexStore::now_ts(),
        })?;
        store.save_chunks(&self.chunks)?;
        store.save_bm25(&self.bm25)?;
        store.save_vector(&self.vector)?;
        store.save_deleted(&self.deleted.iter().cloned().collect::<Vec<_>>())?;
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
        let bm25 = store.load_bm25(config.bm25_mmap)?;
        let mut vector = if config.vector_enabled {
            store.load_vector(config.vector_mmap)?
        } else {
            VectorIndex::empty(
                config.vector_dims,
                config.vector_ngram_min,
                config.vector_ngram_max,
            )
        };
        let cache = Mutex::new(QueryCache::new(config.cache_size));
        let deleted = store.load_deleted().unwrap_or_default().into_iter().collect();

        let text_store = match meta.text_store_file {
            Some(ref path) => TextStore::open(Path::new(path), config.text_store_mmap).ok(),
            None => None,
        };

        let term_buckets = build_term_buckets(&bm25);

        let ann = AnnConfig {
            enabled: config.ann_enabled,
            nlist: config.ann_nlist,
            nprobe: config.ann_nprobe,
            max_iters: config.ann_max_kmeans_iters,
            sample_size: config.ann_sample_size,
            hnsw_enabled: config.hnsw_enabled,
            hnsw_m: config.hnsw_m,
            hnsw_ef_construction: config.hnsw_ef_construction,
            hnsw_ef_search: config.hnsw_ef_search,
        };
        vector.rebuild_hnsw(&ann);

        if config.low_memory {
            for chunk in &mut chunks {
                chunk.tokens.clear();
                chunk.positions.clear();
            }
        }
        let mut engine = Self {
            chunks,
            bm25,
            vector,
            config,
            cache,
            text_store,
            deleted,
            term_buckets,
            meta,
            wal_dir: Some(dir.to_string()),
        };

        if engine.config.wal_enabled {
            let store = IndexStore::new(dir);
            if let Ok(ops) = store.load_wal() {
                if !ops.is_empty() {
                    for op in ops {
                        match op {
                            WalOp::Add(docs) => {
                                let _ = engine.update_documents(docs);
                            }
                            WalOp::Delete(ids) => {
                                let _ = engine.delete_documents(&ids);
                            }
                        }
                    }
                    let _ = store.clear_wal();
                }
            }
        }

        Ok(engine)
    }
}

pub struct ShardedEngine {
    shard_dirs: Vec<std::path::PathBuf>,
    config: Config,
    results_top_k: usize,
    shard_cache: Mutex<HashMap<std::path::PathBuf, Arc<SearchEngine>>>,
}

impl ShardedEngine {
    pub fn load_shards(dir: &str, config: Config) -> std::io::Result<Self> {
        let results_top_k = config.results_top_k;
        let shard_dirs = find_shard_dirs(Path::new(dir))?;
        if shard_dirs.is_empty() {
            return Err(std::io::Error::new(std::io::ErrorKind::NotFound, "no shard directories found"));
        }
        Ok(Self {
            shard_dirs,
            config,
            results_top_k,
            shard_cache: Mutex::new(HashMap::new()),
        })
    }

    fn load_or_get_shard(&self, shard: &std::path::PathBuf) -> Option<Arc<SearchEngine>> {
        if let Ok(cache) = self.shard_cache.lock() {
            if let Some(engine) = cache.get(shard) {
                return Some(engine.clone());
            }
        }

        let engine = match SearchEngine::load_index(shard.to_string_lossy().as_ref(), self.config.clone()) {
            Ok(engine) => Arc::new(engine),
            Err(err) => {
                eprintln!("[sharded] failed to load shard {}: {err}", shard.display());
                return None;
            }
        };

        if let Ok(mut cache) = self.shard_cache.lock() {
            cache.insert(shard.clone(), engine.clone());
        }

        Some(engine)
    }

    fn query_shard_budget(&self, query: &str) -> usize {
        let shard_count = self.shard_dirs.len();
        if shard_count <= 1 {
            return shard_count;
        }
        let intent = detect_intent(query, detect_language(query));
        let budget = match intent {
            QueryIntent::Factual => 3,
            QueryIntent::List => 3,
            QueryIntent::Comparison => 3,
            QueryIntent::Other => 2,
        };
        budget.min(shard_count).max(1)
    }

    pub fn search(&self, query: &str) -> SearchResponse {
        let mut handles = Vec::new();
        let shard_count = self.shard_dirs.len();
        let budget = self.query_shard_budget(query);
        let seed = normalize_text(query)
            .bytes()
            .fold(0usize, |acc, byte| acc.wrapping_mul(31).wrapping_add(byte as usize));
        let start = if shard_count == 0 { 0 } else { seed % shard_count };
        for offset in 0..budget {
            let shard = &self.shard_dirs[(start + offset) % shard_count];
            if let Some(engine) = self.load_or_get_shard(shard) {
                let query = query.to_string();
                handles.push(std::thread::spawn(move || engine.search(&query)));
            }
        }

        let mut results: Vec<ResultItem> = Vec::new();
        let mut answers: Vec<Answer> = Vec::new();
        let mut best_answer: Option<Answer> = None;
        for handle in handles {
            let resp = match handle.join() {
                Ok(resp) => resp,
                Err(_) => continue,
            };
            if let Some(ans) = resp.answer {
                let better = best_answer
                    .as_ref()
                    .map(|b| ans.confidence > b.confidence)
                    .unwrap_or(true);
                if better {
                    best_answer = Some(ans.clone());
                }
                answers.push(ans);
            }
            answers.extend(resp.answers);
            results.extend(resp.results);
        }
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.dedup_by(|a, b| a.id == b.id);
        results.truncate(self.results_top_k);

        answers.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));
        answers.truncate(3);

        if best_answer.is_none() {
            if let Some(first) = results.first() {
                best_answer = Some(Answer {
                    text: first.text.clone(),
                    confidence: 0.35,
                    source: first.id.clone(),
                });
            }
        }

        SearchResponse {
            answer: best_answer,
            answers,
            results,
        }
    }

    pub fn index_health(&self) -> IndexHealth {
        let mut doc_count = 0usize;
        let mut deleted_count = 0usize;
        let mut vector_bytes = 0usize;
        let mut text_store_bytes = 0u64;
        let mut version = 0u32;
        let mut updated_at = 0u64;
        for shard in &self.shard_dirs {
            let store = IndexStore::new(shard);
            if let Ok(meta) = store.load_meta() {
                doc_count += meta.doc_count;
                deleted_count += meta.deleted_count;
                version = version.max(meta.version);
                updated_at = updated_at.max(meta.updated_at);
                text_store_bytes += meta
                    .text_store_file
                    .as_ref()
                    .and_then(|p| std::fs::metadata(shard.join(p)).ok())
                    .map(|m| m.len())
                    .unwrap_or(0);
                vector_bytes += std::fs::metadata(shard.join("vector.bin")).map(|m| m.len() as usize).unwrap_or(0);
            }
        }
        IndexHealth {
            doc_count,
            deleted_count,
            index_version: version,
            index_updated_at: updated_at,
            text_store_bytes,
            vector_bytes,
            ok: true,
            message: "sharded index loaded".to_string(),
        }
    }
}

pub enum EngineInstance {
    Single(SearchEngine),
    Sharded(ShardedEngine),
}

impl EngineInstance {
    pub fn search(&self, query: &str) -> SearchResponse {
        match self {
            EngineInstance::Single(engine) => engine.search(query),
            EngineInstance::Sharded(engine) => engine.search(query),
        }
    }

    pub fn update_documents(&mut self, docs: Vec<Document>) -> usize {
        match self {
            EngineInstance::Single(engine) => engine.update_documents(docs),
            EngineInstance::Sharded(_) => 0,
        }
    }

    pub fn delete_documents(&mut self, ids: &[String]) -> usize {
        match self {
            EngineInstance::Single(engine) => engine.delete_documents(ids),
            EngineInstance::Sharded(_) => 0,
        }
    }

    pub fn index_health(&self) -> IndexHealth {
        match self {
            EngineInstance::Single(engine) => engine.index_health(),
            EngineInstance::Sharded(engine) => engine.index_health(),
        }
    }
}

fn find_shard_dirs(root: &Path) -> std::io::Result<Vec<std::path::PathBuf>> {
    let mut dirs = Vec::new();
    if !root.is_dir() {
        return Ok(dirs);
    }
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with("shard_") {
                    dirs.push(path);
                }
            }
        }
    }
    dirs.sort();
    Ok(dirs)
}

pub fn load_engine_from_dir(dir: &str, config: Config) -> std::io::Result<EngineInstance> {
    let shard_dirs = find_shard_dirs(Path::new(dir))?;
    if !shard_dirs.is_empty() {
        let sharded = ShardedEngine::load_shards(dir, config)?;
        Ok(EngineInstance::Sharded(sharded))
    } else {
        let engine = SearchEngine::load_index(dir, config)?;
        Ok(EngineInstance::Single(engine))
    }
}

fn build_term_buckets(bm25: &BM25Index) -> HashMap<(String, usize), Vec<String>> {
    let mut buckets: HashMap<(String, usize), Vec<String>> = HashMap::new();
    for term in bm25.terms_iter() {
        let key: String = term.chars().next().map(|c| c.to_string()).unwrap_or_default();
        let len = term.len();
        buckets.entry((key, len)).or_default().push(term.clone());
    }
    buckets
}

fn update_term_buckets(buckets: &mut HashMap<(String, usize), Vec<String>>, chunks: &[Chunk]) {
    for chunk in chunks {
        for token in &chunk.tokens {
            let key = token.chars().next().map(|c| c.to_string()).unwrap_or_default();
            let len = token.len();
            let entry = buckets.entry((key, len)).or_default();
            if !entry.contains(token) {
                entry.push(token.clone());
            }
        }
    }
}

fn suggest_correction(term: &str, buckets: &HashMap<(String, usize), Vec<String>>) -> Option<String> {
    if term.len() < 3 {
        return None;
    }
    let key = term.chars().next().map(|c| c.to_string()).unwrap_or_default();
    let mut best: Option<(String, usize)> = None;
    for len in term.len().saturating_sub(1)..=term.len() + 1 {
        if let Some(cands) = buckets.get(&(key.clone(), len)) {
            for cand in cands {
                let dist = levenshtein(term, cand);
                if dist <= 2 {
                    if let Some((_, best_dist)) = &best {
                        if dist < *best_dist {
                            best = Some((cand.clone(), dist));
                        }
                    } else {
                        best = Some((cand.clone(), dist));
                    }
                }
            }
        }
    }
    best.map(|b| b.0)
}

fn levenshtein(a: &str, b: &str) -> usize {
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0usize; b.len() + 1];
    for (i, ca) in a.chars().enumerate() {
        curr[0] = i + 1;
        for (j, cb) in b.chars().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            curr[j + 1] = (prev[j + 1] + 1)
                .min(curr[j] + 1)
                .min(prev[j] + cost);
        }
        prev.clone_from_slice(&curr);
    }
    prev[b.len()]
}

static ENGINE: OnceLock<Mutex<EngineInstance>> = OnceLock::new();

pub fn init_engine(engine: SearchEngine) {
    if let Some(existing) = ENGINE.get() {
        if let Ok(mut guard) = existing.lock() {
            *guard = EngineInstance::Single(engine);
            return;
        }
    }
    let _ = ENGINE.set(Mutex::new(EngineInstance::Single(engine)));
}

pub fn init_engine_once(engine: SearchEngine) -> bool {
    if let Some(existing) = ENGINE.get() {
        if let Ok(mut guard) = existing.lock() {
            *guard = EngineInstance::Single(engine);
            return true;
        }
    }
    ENGINE.set(Mutex::new(EngineInstance::Single(engine))).is_ok()
}

pub fn init_engine_instance_once(engine: EngineInstance) -> bool {
    if let Some(existing) = ENGINE.get() {
        if let Ok(mut guard) = existing.lock() {
            *guard = engine;
            return true;
        }
    }
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

pub fn delete_documents(ids: Vec<String>) -> usize {
    if let Some(engine) = ENGINE.get() {
        if let Ok(mut engine) = engine.lock() {
            return engine.delete_documents(&ids);
        }
    }
    0
}

pub fn index_health() -> Option<IndexHealth> {
    if let Some(engine) = ENGINE.get() {
        if let Ok(engine) = engine.lock() {
            return Some(engine.index_health());
        }
    }
    None
}


