use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use url::Url;

use crate::Document;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RawPage {
    pub id: String,
    pub url: String,
    pub title: String,
    pub text: String,
    pub links: Vec<String>,
}

impl RawPage {
    pub fn new(url: &Url, title: &str, text: &str, links: &[String]) -> Self {
        let id = format!("{:x}", md5::compute(url.as_str()));
        Self {
            id,
            url: url.as_str().to_string(),
            title: title.to_string(),
            text: text.to_string(),
            links: links.to_vec(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProcessedChunk {
    pub id: String,
    pub text: String,
    pub tokens: Vec<String>,
    pub source_url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PipelineConfig {
    pub crawl_limit: usize,
    pub max_depth: usize,
    pub timeout_ms: u64,
    pub storage_path: String,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            crawl_limit: 10000,
            max_depth: 3,
            timeout_ms: 5000,
            storage_path: "./data".to_string(),
        }
    }
}

pub struct StorageManager {
    root: PathBuf,
}

impl StorageManager {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self { root: root.as_ref().to_path_buf() }
    }

    pub fn raw_path(&self) -> PathBuf {
        self.root.join("raw").join("pages.jsonl")
    }

    pub fn processed_path(&self) -> PathBuf {
        self.root.join("processed").join("chunks.jsonl")
    }

    pub fn index_path(&self) -> PathBuf {
        self.root.join("index").join("dataset.json")
    }

    pub fn ensure_dirs(&self) -> std::io::Result<()> {
        fs::create_dir_all(self.root.join("raw"))?;
        fs::create_dir_all(self.root.join("processed"))?;
        fs::create_dir_all(self.root.join("index"))?;
        Ok(())
    }

    pub fn write_raw(&self, page: &RawPage) -> std::io::Result<()> {
        self.ensure_dirs()?;
        append_jsonl(&self.raw_path(), page)
    }

    pub fn read_raw(&self) -> Vec<RawPage> {
        read_jsonl(&self.raw_path())
    }

    pub fn write_processed(&self, chunks: &[ProcessedChunk]) -> std::io::Result<()> {
        self.ensure_dirs()?;
        for chunk in chunks {
            append_jsonl(&self.processed_path(), chunk)?;
        }
        Ok(())
    }

    pub fn read_processed(&self) -> Vec<ProcessedChunk> {
        read_jsonl(&self.processed_path())
    }

    pub fn write_dataset(&self, chunks: &[ProcessedChunk]) -> std::io::Result<()> {
        self.ensure_dirs()?;
        let docs: Vec<Document> = chunks
            .iter()
            .map(|c| Document { id: c.id.clone(), text: c.text.clone() })
            .collect();
        let data = serde_json::to_string_pretty(&docs).unwrap_or_else(|_| "[]".to_string());
        fs::write(self.index_path(), data)
    }
}

fn append_jsonl<T: Serialize>(path: &Path, item: &T) -> std::io::Result<()> {
    let file = OpenOptions::new().create(true).append(true).open(path)?;
    let mut writer = std::io::BufWriter::new(file);
    let line = serde_json::to_string(item).unwrap_or_else(|_| "{}".to_string());
    writer.write_all(line.as_bytes())?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}

fn read_jsonl<T: for<'de> Deserialize<'de>>(path: &Path) -> Vec<T> {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };
    let reader = BufReader::new(file);
    reader
        .lines()
        .filter_map(|l| l.ok())
        .filter_map(|line| serde_json::from_str::<T>(&line).ok())
        .collect()
}
