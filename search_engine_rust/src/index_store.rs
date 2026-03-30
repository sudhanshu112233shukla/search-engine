use std::collections::HashMap;
use std::fs;
use std::io::{self, Read, Write, BufRead};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use memmap2::Mmap;
use serde::{Deserialize, Serialize};

use crate::bm25::{BM25Index, BM25Persist, TermEntry};
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

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op", content = "data")]
pub enum WalOp {
    Add(Vec<crate::Document>),
    Delete(Vec<String>),
}

#[derive(Serialize, Deserialize)]
struct LegacyTermEntry {
    df: u32,
    postings: Vec<(usize, u32)>,
}

#[derive(Serialize, Deserialize)]
struct LegacyBM25Index {
    terms: HashMap<String, LegacyTermEntry>,
    doc_lens: Vec<usize>,
    avg_doc_len: f32,
    k1: f32,
    b: f32,
    total_len: usize,
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
        let mut keys: Vec<String> = bm25.terms.keys().cloned().collect();
        keys.sort();

        let mut postings_file = fs::File::create(self.root.join("bm25_postings.bin"))?;
        let mut offset: u64 = 0;
        let mut terms_meta: HashMap<String, crate::bm25::TermMeta> = HashMap::new();

        for term in keys {
            let postings = bm25.collect_postings(&term);
            let len = postings.len() as u32;
            for (doc_id, tf) in &postings {
                postings_file.write_all(&doc_id.to_le_bytes())?;
                postings_file.write_all(&tf.to_le_bytes())?;
            }
            terms_meta.insert(
                term,
                crate::bm25::TermMeta {
                    df: len,
                    offset,
                    len,
                },
            );
            offset += len as u64 * 8;
        }

        let persist = BM25Persist {
            terms: terms_meta,
            doc_lens: bm25.doc_lens.clone(),
            avg_doc_len: bm25.avg_doc_len,
            k1: bm25.k1,
            b: bm25.b,
            total_len: bm25.total_len,
        };

        write_bin(self.root.join("bm25_terms.bin"), &persist)
    }

    pub fn load_bm25(&self, use_mmap: bool) -> io::Result<BM25Index> {
        let terms_path = self.root.join("bm25_terms.bin");
        let postings_path = self.root.join("bm25_postings.bin");

        if terms_path.exists() && postings_path.exists() {
            let persist: BM25Persist = read_bin(&terms_path)?;
            let mut bm25 = BM25Index::from_persist(persist);
            if use_mmap {
                let file = fs::File::open(&postings_path)?;
                let mmap = unsafe { Mmap::map(&file)? };
                bm25.set_postings_mmap(Some(mmap));
            } else {
                let mut buf = Vec::new();
                fs::File::open(&postings_path)?.read_to_end(&mut buf)?;
                for entry in bm25.terms.values_mut() {
                    let postings = postings_from_raw(&buf, entry.offset, entry.len);
                    entry.postings = Some(postings);
                    entry.offset = 0;
                    entry.len = entry.postings.as_ref().map(|p| p.len()).unwrap_or(0) as u32;
                }
            }
            return Ok(bm25);
        }

        let legacy_path = self.root.join("bm25.bin");
        if legacy_path.exists() {
            let legacy: LegacyBM25Index = read_bin(&legacy_path)?;
            let mut terms = HashMap::new();
            for (term, entry) in legacy.terms {
                let postings: Vec<(u32, u32)> = entry
                    .postings
                    .into_iter()
                    .map(|(doc, tf)| (doc as u32, tf))
                    .collect();
                terms.insert(
                    term,
                    TermEntry {
                        df: entry.df,
                        postings: Some(postings.clone()),
                        offset: 0,
                        len: postings.len() as u32,
                    },
                );
            }
            return Ok(BM25Index {
                terms,
                delta_terms: HashMap::new(),
                doc_lens: legacy.doc_lens,
                avg_doc_len: legacy.avg_doc_len,
                k1: legacy.k1,
                b: legacy.b,
                total_len: legacy.total_len,
                postings_mmap: None,
            });
        }

        Err(io::Error::new(io::ErrorKind::NotFound, "bm25 index not found"))
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

    pub fn append_wal(&self, op: &WalOp) -> io::Result<()> {
        self.ensure_dirs()?;
        let line = serde_json::to_string(op).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.root.join("wal.jsonl"))?;
        writeln!(file, "{}", line)?;
        Ok(())
    }

    pub fn load_wal(&self) -> io::Result<Vec<WalOp>> {
        let path = self.root.join("wal.jsonl");
        if !path.exists() {
            return Ok(Vec::new());
        }
        let file = fs::File::open(path)?;
        let reader = io::BufReader::new(file);
        let mut ops = Vec::new();
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(op) = serde_json::from_str::<WalOp>(&line) {
                ops.push(op);
            }
        }
        Ok(ops)
    }

    pub fn clear_wal(&self) -> io::Result<()> {
        let path = self.root.join("wal.jsonl");
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    pub fn now_ts() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
}

fn postings_from_raw(buf: &[u8], offset: u64, len: u32) -> Vec<(u32, u32)> {
    let start = offset as usize;
    let byte_len = len as usize * 8;
    if start + byte_len > buf.len() {
        return Vec::new();
    }
    let slice = &buf[start..start + byte_len];
    let mut out = Vec::with_capacity(len as usize);
    for i in 0..len as usize {
        let base = i * 8;
        let doc_id = u32::from_le_bytes([slice[base], slice[base + 1], slice[base + 2], slice[base + 3]]);
        let tf = u32::from_le_bytes([slice[base + 4], slice[base + 5], slice[base + 6], slice[base + 7]]);
        out.push((doc_id, tf));
    }
    out
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
