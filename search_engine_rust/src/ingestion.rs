use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Document {
    pub id: String,
    pub text: String,
}

pub fn load_text_file<P: AsRef<Path>>(path: P) -> io::Result<Document> {
    let path_ref = path.as_ref();
    let text = fs::read_to_string(path_ref)?;
    let id = path_ref
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "doc".to_string());
    Ok(Document { id, text })
}

pub fn load_text_dir<P: AsRef<Path>>(dir: P) -> io::Result<Vec<Document>> {
    let mut docs = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension() {
                if ext == "txt" {
                    if let Ok(doc) = load_text_file(&path) {
                        docs.push(doc);
                    }
                }
            }
        }
    }
    Ok(docs)
}
