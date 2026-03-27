use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BundleProfile {
    pub name: String,
    pub max_bytes: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShardInfo {
    pub name: String,
    pub path: String,
    pub docs: usize,
    pub bytes: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BundleManifest {
    pub version: u32,
    pub language: String,
    pub profiles: Vec<BundleProfile>,
    pub shards: Vec<ShardInfo>,
}

impl BundleManifest {
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let data = serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string());
        fs::write(path, data)
    }

    pub fn load(path: &Path) -> std::io::Result<Self> {
        let data = fs::read_to_string(path)?;
        let parsed = serde_json::from_str(&data).unwrap_or(BundleManifest {
            version: 1,
            language: "en".to_string(),
            profiles: Vec::new(),
            shards: Vec::new(),
        });
        Ok(parsed)
    }
}

pub fn dir_size(path: &Path) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                total += dir_size(&p);
            } else if let Ok(meta) = entry.metadata() {
                total += meta.len();
            }
        }
    }
    total
}

pub fn shard_dir(out_root: &Path, index: usize) -> PathBuf {
    out_root.join(format!("shard_{:04}", index))
}
