use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use memmap2::Mmap;
use serde::{Deserialize, Serialize};

use crate::bm25::BM25Index;
use crate::processing::Chunk;
use crate::vector::{VectorI8Store, VectorIndex, VectorPersist};

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
        let mut persist = vector.to_persist();
        if vector.quantized {
            if let (Some(vectors), Some(scales)) = (vector.vectors_i8.as_ref(), vector.scales.as_ref()) {
                write_raw(self.root.join("vectors_i8.bin"), vectors.as_slice())?;
                write_bin(self.root.join("scales.bin"), scales)?;
                persist.vectors_i8 = None;
                persist.scales = None;
                persist.vectors_i8_file = Some("vectors_i8.bin".to_string());
                persist.scales_file = Some("scales.bin".to_string());
            }
        }
        write_bin(self.root.join("vector.bin"), &persist)
    }

    pub fn load_vector(&self, use_mmap: bool) -> io::Result<VectorIndex> {
        let persist: VectorPersist = read_bin(self.root.join("vector.bin"))?;
        let mut vectors_i8: Option<VectorI8Store> = None;
        let mut scales: Option<Vec<f32>> = persist.scales.clone();

        if let Some(v) = persist.vectors_i8.clone() {
            vectors_i8 = Some(VectorI8Store::Mem(v));
        } else if let Some(file) = &persist.vectors_i8_file {
            let vec_path = self.root.join(file);
            let data_len = fs::metadata(&vec_path)?.len() as usize;
            if use_mmap {
                let file = fs::File::open(&vec_path)?;
                let mmap = unsafe { Mmap::map(&file)? };
                vectors_i8 = Some(VectorI8Store::Mmap { mmap, len: data_len });
            } else {
                let raw = read_raw(&vec_path)?;
                let vec_i8: Vec<i8> = raw.into_iter().map(|b| b as i8).collect();
                vectors_i8 = Some(VectorI8Store::Mem(vec_i8));
            }
            if let Some(scale_file) = &persist.scales_file {
                scales = Some(read_bin(self.root.join(scale_file))?);
            }
        }

        Ok(VectorIndex::from_persist(persist, vectors_i8, scales))
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

fn write_raw<P: AsRef<Path>>(path: P, data: &[i8]) -> io::Result<()> {
    let mut file = fs::File::create(path)?;
    let bytes: &[u8] = unsafe { std::slice::from_raw_parts(data.as_ptr() as *const u8, data.len()) };
    file.write_all(bytes)?;
    Ok(())
}

fn read_raw<P: AsRef<Path>>(path: P) -> io::Result<Vec<u8>> {
    let mut file = fs::File::open(path)?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)?;
    Ok(buf)
}
