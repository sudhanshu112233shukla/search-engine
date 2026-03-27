use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use memmap2::Mmap;

use crate::processing::Chunk;

#[derive(Debug)]
pub struct TextStore {
    path: PathBuf,
    mmap: Option<Mmap>,
    len: u64,
}

impl TextStore {
    pub fn build(path: &Path, chunks: &mut [Chunk], use_mmap: bool) -> io::Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut file = File::create(path)?;
        let mut cursor: u64 = 0;
        for chunk in chunks.iter_mut() {
            let bytes = chunk.text.as_bytes();
            let len = bytes.len() as u32;
            file.write_all(bytes)?;
            file.write_all(b"\n")?;
            chunk.text_offset = cursor;
            chunk.text_len = len;
            cursor += len as u64 + 1;
            chunk.text.clear();
        }
        file.flush()?;

        let mmap = if use_mmap {
            let file = File::open(path)?;
            Some(unsafe { Mmap::map(&file)? })
        } else {
            None
        };

        Ok(Self { path: path.to_path_buf(), mmap, len: cursor })
    }

    pub fn open(path: &Path, use_mmap: bool) -> io::Result<Self> {
        let len = fs::metadata(path)?.len();
        let mmap = if use_mmap {
            let file = File::open(path)?;
            Some(unsafe { Mmap::map(&file)? })
        } else {
            None
        };
        Ok(Self { path: path.to_path_buf(), mmap, len })
    }

    pub fn get_text(&self, offset: u64, len: u32) -> Option<String> {
        if len == 0 {
            return None;
        }
        let start = offset as usize;
        let end = start.saturating_add(len as usize);

        if let Some(mmap) = &self.mmap {
            if end <= mmap.len() {
                return Some(String::from_utf8_lossy(&mmap[start..end]).to_string());
            }
            return None;
        }

        let mut file = File::open(&self.path).ok()?;
        file.seek(SeekFrom::Start(offset)).ok()?;
        let mut buf = vec![0u8; len as usize];
        file.read_exact(&mut buf).ok()?;
        Some(String::from_utf8_lossy(&buf).to_string())
    }

    pub fn append(&mut self, chunks: &mut [Chunk]) -> io::Result<()> {
        if chunks.is_empty() {
            return Ok(());
        }
        let mut file = OpenOptions::new().append(true).open(&self.path)?;
        let mut cursor = self.len;
        for chunk in chunks.iter_mut() {
            let bytes = chunk.text.as_bytes();
            let len = bytes.len() as u32;
            file.write_all(bytes)?;
            file.write_all(b"\n")?;
            chunk.text_offset = cursor;
            chunk.text_len = len;
            cursor += len as u64 + 1;
            chunk.text.clear();
        }
        file.flush()?;
        self.len = cursor;

        if self.mmap.is_some() {
            let file = File::open(&self.path)?;
            self.mmap = Some(unsafe { Mmap::map(&file)? });
        }

        Ok(())
    }

    pub fn byte_len(&self) -> u64 {
        self.len
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}
