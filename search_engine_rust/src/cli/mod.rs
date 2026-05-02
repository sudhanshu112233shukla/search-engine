use std::fs;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

use crate::bundle::{BundleManifest, BundleProfile, LanguagePack, ShardInfo, dir_size, shard_dir};
use crate::crawler::{CrawlConfig, Crawler};
use crate::datasets::{import_osm_pbf, import_warc, import_wikipedia};
use crate::processor::{ProcessConfig, Processor};
use crate::storage::{PipelineConfig, StorageManager};
use crate::{Config, Document, SearchEngine};

#[derive(Debug)]
pub enum Command {
    Crawl { seed: String, limit: Option<usize> },
    Process,
    Index,
    BuildIndex { dataset: String, out: String },
    LoadIndex { dir: String },
    MergeIndex { dir: String, update: String },
    Delete { dir: String, ids: Vec<String> },
    Compact { dir: String, out: String },
    Pack { dataset: String, out: String, max_docs: usize, lang: String, download_base: Option<String> },
    ImportWiki { dump: String, out: String, limit: Option<usize> },
    ImportOsm { pbf: String, out: String, limit: Option<usize> },
    ImportWarc { warc: String, out: String, limit: Option<usize> },
}

pub fn load_config(path: &str) -> PipelineConfig {
    let data = fs::read_to_string(path).unwrap_or_else(|_| "".to_string());
    serde_json::from_str(&data).unwrap_or_default()
}

pub fn run(cmd: Command, config: PipelineConfig) {
    let storage = StorageManager::new(&config.storage_path);

    match cmd {
        Command::Crawl { seed, limit } => {
            let limit = limit.unwrap_or(config.crawl_limit);
            let crawl_config = CrawlConfig {
                crawl_limit: limit,
                max_depth: config.max_depth,
                timeout_ms: config.timeout_ms,
                user_agent: "OfflineSearchCrawler/1.0".to_string(),
                use_disk_frontier: config.use_disk_frontier,
                frontier_path: config.frontier_path.clone(),
            };
            let mut crawler = Crawler::new(crawl_config, storage);
            crawler.add_seed(&seed);
            crawler.crawl();
        }
        Command::Process => {
            let processor = Processor::new(
                ProcessConfig { min_words: 100, max_words: 300 },
                storage,
            );
            processor.process_all();
        }
        Command::Index => {
            let chunks = storage.read_processed();
            if let Err(err) = storage.write_dataset(&chunks) {
                eprintln!("index write failed: {err}");
            }
        }
        Command::BuildIndex { dataset, out } => {
            build_index_from_dataset(&dataset, &out);
        }
        Command::LoadIndex { dir } => {
            let cfg = Config::default();
            let engine = SearchEngine::load_index(&dir, cfg);
            match engine {
                Ok(_) => println!("Index loaded successfully"),
                Err(err) => eprintln!("Load failed: {err}"),
            }
        }
        Command::MergeIndex { dir, update } => {
            let cfg = Config::default();
            let mut engine = match SearchEngine::load_index(&dir, cfg) {
                Ok(e) => e,
                Err(err) => {
                    eprintln!("Load failed: {err}");
                    return;
                }
            };
            let docs = std::fs::read_to_string(&update).unwrap_or_else(|_| "[]".to_string());
            let parsed = serde_json::from_str::<Vec<Document>>(&docs).unwrap_or_default();
            let added = engine.update_documents(parsed);
            println!("Added {added} chunks");
            if let Err(err) = engine.save_index(&dir) {
                eprintln!("Save failed: {err}");
            }
        }
        Command::Delete { dir, ids } => {
            let cfg = Config::default();
            let mut engine = match SearchEngine::load_index(&dir, cfg) {
                Ok(e) => e,
                Err(err) => {
                    eprintln!("Load failed: {err}");
                    return;
                }
            };
            let removed = engine.delete_documents(&ids);
            println!("Marked {removed} docs as deleted");
            if let Err(err) = engine.save_index(&dir) {
                eprintln!("Save failed: {err}");
            }
        }
        Command::Compact { dir, out } => {
            let cfg = Config::default();
            let engine = match SearchEngine::load_index(&dir, cfg) {
                Ok(e) => e,
                Err(err) => {
                    eprintln!("Load failed: {err}");
                    return;
                }
            };
            let docs = engine.live_documents();
            let mut cfg = Config::default();
            cfg.vector_quantize = true;
            cfg.ann_enabled = true;
            cfg.pq_enabled = true;
            cfg.text_store_path = Some(format!("{}/textstore.bin", out));
            cfg.low_memory = true;
            let new_engine = SearchEngine::new(docs, cfg);
            if let Err(err) = new_engine.save_index(&out) {
                eprintln!("Compact save failed: {err}");
            }
        }
        Command::Pack { dataset, out, max_docs, lang, download_base } => {
            pack_dataset(&dataset, &out, max_docs, &lang, download_base);
        }
        Command::ImportWiki { dump, out, limit } => {
            if let Err(err) = import_wikipedia(&dump, &out, limit) {
                eprintln!("Wiki import failed: {err}");
            }
        }
        Command::ImportOsm { pbf, out, limit } => {
            if let Err(err) = import_osm_pbf(&pbf, &out, limit) {
                eprintln!("OSM import failed: {err}");
            }
        }
        Command::ImportWarc { warc, out, limit } => {
            if let Err(err) = import_warc(&warc, &out, limit) {
                eprintln!("WARC import failed: {err}");
            }
        }
    }
}

fn build_index_from_dataset(dataset: &str, out: &str) {
    let docs = std::fs::read_to_string(dataset).unwrap_or_else(|_| "[]".to_string());
    let parsed = serde_json::from_str::<Vec<Document>>(&docs).unwrap_or_default();
    if parsed.is_empty() {
        eprintln!("No documents loaded from dataset");
        return;
    }
    let mut cfg = Config::default();
    cfg.vector_quantize = true;
    cfg.ann_enabled = true;
    cfg.pq_enabled = true;
    cfg.text_store_path = Some(format!("{}/textstore.bin", out));
    cfg.low_memory = true;
    let engine = SearchEngine::new(parsed, cfg);
    if let Err(err) = engine.save_index(out) {
        eprintln!("Failed to save index: {err}");
    }
}

fn pack_dataset(dataset: &str, out: &str, max_docs: usize, lang: &str, download_base: Option<String>) {
    fn stream_json_array<R: std::io::Read, F: FnMut(Document)>(reader: R, mut on_doc: F) -> Result<(), serde_json::Error> {
        struct DocVisitor<F>(F);
        impl<'de, F: FnMut(Document)> serde::de::Visitor<'de> for DocVisitor<F> {
            type Value = ();
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "array of documents")
            }
            fn visit_seq<A>(mut self, mut seq: A) -> Result<(), A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                while let Some(doc) = seq.next_element::<Document>()? {
                    (self.0)(doc);
                }
                Ok(())
            }
        }
        let mut de = serde_json::Deserializer::from_reader(reader);
        serde::de::Deserializer::deserialize_any(&mut de, DocVisitor(on_doc))
    }

    let out_root = Path::new(out).join(lang);
    let mut shards = Vec::new();
    let mut index = 0usize;
    let mut buffer: Vec<Document> = Vec::with_capacity(max_docs);

    let file = match fs::File::open(dataset) {
        Ok(f) => f,
        Err(err) => {
            eprintln!("Failed to open dataset: {err}");
            return;
        }
    };
    let mut reader = BufReader::new(file);

    let mut first_byte = None;
    {
        let mut buf = Vec::new();
        if reader.read_until(b'\n', &mut buf).is_ok() {
            for b in &buf {
                if !b.is_ascii_whitespace() {
                    first_byte = Some(*b);
                    break;
                }
            }
        }
    }

    let file = match fs::File::open(dataset) {
        Ok(f) => f,
        Err(err) => {
            eprintln!("Failed to reopen dataset: {err}");
            return;
        }
    };
    reader = BufReader::new(file);

    let is_json_array = matches!(first_byte, Some(b'['));

    if is_json_array {
        let result = stream_json_array(reader, |doc| {
            buffer.push(doc);
            if buffer.len() >= max_docs {
                if !flush_shard(&out_root, index, &buffer, &mut shards) {
                    buffer.clear();
                    return;
                }
                buffer.clear();
                index += 1;
            }
        });
        if result.is_err() {
            eprintln!("Failed to parse JSON array dataset");
            return;
        }
    } else {
        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => continue,
            };
            if line.trim().is_empty() {
                continue;
            }
            let doc = match serde_json::from_str::<Document>(&line) {
                Ok(d) => d,
                Err(_) => continue,
            };
            buffer.push(doc);
            if buffer.len() >= max_docs {
                if !flush_shard(&out_root, index, &buffer, &mut shards) {
                    return;
                }
                buffer.clear();
                index += 1;
            }
        }
    }

    if !buffer.is_empty() {
        if !flush_shard(&out_root, index, &buffer, &mut shards) {
            return;
        }
    }

    if shards.is_empty() {
        eprintln!("No documents loaded from dataset");
        return;
    }

    let manifest_path = Path::new(out).join("manifest.json");
    let mut manifest = if manifest_path.exists() {
        BundleManifest::load(&manifest_path).unwrap_or(BundleManifest { version: 1, download_base: None, languages: Vec::new() })
    } else {
        BundleManifest { version: 1, download_base: None, languages: Vec::new() }
    };

    if download_base.is_some() {
        manifest.download_base = download_base;
    }

    let profiles = vec![
        BundleProfile { name: "default".to_string(), max_bytes: 1_000_000_000 },
        BundleProfile { name: "power".to_string(), max_bytes: 5_000_000_000 },
    ];

    let pack = LanguagePack {
        code: lang.to_string(),
        profiles,
        shards,
    };

    if let Some(existing) = manifest.languages.iter_mut().find(|l| l.code == lang) {
        *existing = pack;
    } else {
        manifest.languages.push(pack);
    }

    let _ = manifest.save(&manifest_path);
}



fn flush_shard(out_root: &Path, index: usize, docs: &[Document], shards: &mut Vec<ShardInfo>) -> bool {
    if docs.is_empty() {
        return true;
    }
    let shard_path = shard_dir(out_root, index);
    let shard_str = shard_path.to_string_lossy().to_string();
    let mut cfg = Config::default();
    cfg.vector_quantize = true;
    cfg.ann_enabled = true;
    cfg.pq_enabled = true;
    cfg.text_store_path = Some(format!("{}/textstore.bin", shard_str));
    cfg.low_memory = true;
    let engine = SearchEngine::new(docs.to_vec(), cfg);
    if let Err(err) = engine.save_index(&shard_str) {
        eprintln!("Failed to save shard: {err}");
        return false;
    }
    let bytes = dir_size(&shard_path);
    shards.push(ShardInfo {
        name: format!("shard_{:04}", index),
        path: shard_path.to_string_lossy().to_string(),
        docs: docs.len(),
        bytes,
    });
    true
}
