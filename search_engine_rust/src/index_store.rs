use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::bm25::BM25Index;
use crate::processing::Chunk;
use crate::vector::VectorIndex;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChunkPersist {
    pub id: String,
    pub clean: String,
    pub text_offset: u64,
    pub text_len: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IndexMeta {
    pub version: u32,
    pub text_store_file: Option<String>,
    pub doc_count: usize,
    pub deleted_count: usize,
    pub updated_at: u64,
}

pub struct IndexStore {
    root: PathBuf,
}

impl IndexStore {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self { root: root.as_ref().to_path_buf() }
    }

    pub fn ensure_dirs(&self) -> io::Result<()> {
        fs::create_dir_all(&self.root)
    }

    pub fn save_meta(&self, meta: &IndexMeta) -> io::Result<()> {
        self.ensure_dirs()?;
        write_bin(self.root.join("meta.bin"), meta)
    }

    pub fn load_meta(&self) -> io::Result<IndexMeta> {
        read_bin(self.root.join("meta.bin"))
    }

    pub fn save_chunks(&self, chunks: &[Chunk]) -> io::Result<()> {
        self.ensure_dirs()?;
        let persist: Vec<ChunkPersist> = chunks
            .iter()
            .map(|c| ChunkPersist {
                id: c.id.clone(),
                clean: c.clean.clone(),
                text_offset: c.text_offset,
                text_len: c.text_len,
            })
            .collect();
        write_bin(self.root.join("chunks.bin"), &persist)
    }

    pub fn load_chunks(&self) -> io::Result<Vec<ChunkPersist>> {
        read_bin(self.root.join("chunks.bin"))
    }

    pub fn save_bm25(&self, bm25: &BM25Index) -> io::Result<()> {
        self.ensure_dirs()?;
        write_bin(self.root.join("bm25.bin"), bm25)
    }

    pub fn load_bm25(&self) -> io::Result<BM25Index> {
        read_bin(self.root.join("bm25.bin"))
    }

    pub fn save_vector(&self, vector: &VectorIndex) -> io::Result<()> {
        self.ensure_dirs()?;
        let persist = vector.to_persist();
        write_bin(self.root.join("vector.bin"), &persist)
    }

    pub fn load_vector(&self) -> io::Result<VectorIndex> {
        let persist = read_bin(self.root.join("vector.bin"))?;
        Ok(VectorIndex::from_persist(persist))
    }

    pub fn save_deleted(&self, deleted: &[String]) -> io::Result<()> {
        self.ensure_dirs()?;
        write_bin(self.root.join("deleted.bin"), deleted)
    }

    pub fn load_deleted(&self) -> io::Result<Vec<String>> {
        read_bin(self.root.join("deleted.bin"))
    }

    pub fn copy_text_store(&self, src: &Path) -> io::Result<PathBuf> {
        self.ensure_dirs()?;
        let dst = self.root.join("textstore.bin");
        if src != dst {
            fs::copy(src, &dst)?;
        }
        Ok(dst)
    }

    pub fn now_ts() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
}

fn write_bin<P: AsRef<Path>, T: Serialize>(path: P, value: &T) -> io::Result<()> {
    let data = bincode::serialize(value).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    let mut file = fs::File::create(path)?;
    file.write_all(&data)?;
    Ok(())
}

fn read_bin<P: AsRef<Path>, T: for<'de> Deserialize<'de>>(path: P) -> io::Result<T> {
    let mut file = fs::File::open(path)?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)?;
    let data = bincode::deserialize(&buf).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    Ok(data)
}
