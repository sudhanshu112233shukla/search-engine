use std::ffi::{CStr, CString};
use std::fs;
use std::path::PathBuf;
use std::os::raw::c_char;

use crate::{Config, Document, SearchEngine};
use crate::{init_engine_once, search, update_documents};

const EMPTY_JSON: &str = "{\"answer\":null,\"answers\":[],\"results\":[]}";

fn search_json(query: &str) -> String {
    match serde_json::to_string(&search(query)) {
        Ok(s) => s,
        Err(_) => EMPTY_JSON.to_string(),
    }
}

fn parse_docs(json: &str) -> Option<Vec<Document>> {
    match serde_json::from_str::<Vec<Document>>(json) {
        Ok(docs) => Some(docs),
        Err(err) => {
            eprintln!("[ffi] JSON parse error: {err}");
            None
        }
    }
}

#[no_mangle]
pub extern "C" fn init_engine_from_file(path: *const c_char) {
    if path.is_null() {
        eprintln!("[ffi] init_engine_from_file: null path");
        return;
    }
    let cstr = unsafe { CStr::from_ptr(path) };
    let path_str = cstr.to_string_lossy();
    let data = match fs::read_to_string(path_str.as_ref()) {
        Ok(s) => s,
        Err(err) => {
            eprintln!("[ffi] file read error: {err}");
            return;
        }
    };
    let docs = match parse_docs(&data) {
        Some(d) => d,
        None => return,
    };
    let mut config = Config::default();
    let mut store_path = PathBuf::from(path_str.as_ref());
    store_path.set_extension("textstore");
    config.text_store_path = Some(store_path.to_string_lossy().to_string());
    config.text_store_mmap = true;
    config.vector_quantize = false;
    config.ann_enabled = false;
    config.hnsw_enabled = true;
    config.hnsw_m = 16;
    config.hnsw_ef_construction = 128;
    config.hnsw_ef_search = 64;
    config.pq_enabled = false;
    config.low_memory = true;

    let (head, tail) = split_docs(docs);
    let engine = SearchEngine::new(head, config);
    if !init_engine_once(engine) {
        eprintln!("[ffi] engine already initialized; skipping");
        return;
    }
    if !tail.is_empty() {
        std::thread::spawn(move || {
            for batch in tail.chunks(500) {
                let _ = update_documents(batch.to_vec());
            }
        });
    }
}

#[no_mangle]
pub extern "C" fn init_engine_from_json(json: *const c_char) {
    if json.is_null() {
        eprintln!("[ffi] init_engine_from_json: null input");
        return;
    }
    let cstr = unsafe { CStr::from_ptr(json) };
    let json_str = cstr.to_string_lossy();
    let docs = match parse_docs(json_str.as_ref()) {
        Some(d) => d,
        None => return,
    };
    let mut config = Config::default();
    config.vector_quantize = false;
    config.ann_enabled = false;
    config.hnsw_enabled = true;
    config.hnsw_m = 16;
    config.hnsw_ef_construction = 128;
    config.hnsw_ef_search = 64;
    config.pq_enabled = false;
    config.low_memory = true;

    let (head, tail) = split_docs(docs);
    let engine = SearchEngine::new(head, config);
    if !init_engine_once(engine) {
        eprintln!("[ffi] engine already initialized; skipping");
        return;
    }
    if !tail.is_empty() {
        std::thread::spawn(move || {
            for batch in tail.chunks(500) {
                let _ = update_documents(batch.to_vec());
            }
        });
    }
}

#[no_mangle]
pub extern "C" fn init_engine_from_index(dir: *const c_char) {
    if dir.is_null() {
        eprintln!("[ffi] init_engine_from_index: null dir");
        return;
    }
    let cstr = unsafe { CStr::from_ptr(dir) };
    let dir_str = cstr.to_string_lossy();
    let mut config = Config::default();
    config.low_memory = true;
    config.text_store_mmap = true;
    config.hnsw_enabled = true;
    config.hnsw_m = 16;
    config.hnsw_ef_construction = 128;
    config.hnsw_ef_search = 64;
    match SearchEngine::load_index(dir_str.as_ref(), config) {
        Ok(engine) => {
            if !init_engine_once(engine) {
                eprintln!("[ffi] engine already initialized; skipping");
            }
        }
        Err(err) => {
            eprintln!("[ffi] load index failed: {err}");
        }
    }
}

fn split_docs(mut docs: Vec<Document>) -> (Vec<Document>, Vec<Document>) {
    if docs.len() <= 1000 {
        return (docs, Vec::new());
    }
    let split = (docs.len() / 10).max(1000).min(docs.len());
    let tail = docs.split_off(split);
    (docs, tail)
}

#[no_mangle]
pub extern "C" fn search_query(query: *const c_char) -> *mut c_char {
    if query.is_null() {
        return CString::new(EMPTY_JSON).unwrap().into_raw();
    }
    let cstr = unsafe { CStr::from_ptr(query) };
    let query_str = cstr.to_string_lossy();
    let json = search_json(query_str.as_ref());
    match CString::new(json) {
        Ok(s) => s.into_raw(),
        Err(_) => CString::new(EMPTY_JSON).unwrap().into_raw(),
    }
}

#[no_mangle]
pub extern "C" fn update_engine_from_file(path: *const c_char) {
    if path.is_null() {
        eprintln!("[ffi] update_engine_from_file: null path");
        return;
    }
    let cstr = unsafe { CStr::from_ptr(path) };
    let path_str = cstr.to_string_lossy();
    let data = match fs::read_to_string(path_str.as_ref()) {
        Ok(s) => s,
        Err(err) => {
            eprintln!("[ffi] update file read error: {err}");
            return;
        }
    };
    let docs = match parse_docs(&data) {
        Some(d) => d,
        None => return,
    };
    let added = update_documents(docs);
    if added == 0 {
        eprintln!("[ffi] update skipped (engine not initialized or no docs)");
    }
}

#[no_mangle]
pub extern "C" fn update_engine_from_json(json: *const c_char) {
    if json.is_null() {
        eprintln!("[ffi] update_engine_from_json: null input");
        return;
    }
    let cstr = unsafe { CStr::from_ptr(json) };
    let json_str = cstr.to_string_lossy();
    let docs = match parse_docs(json_str.as_ref()) {
        Some(d) => d,
        None => return,
    };
    let added = update_documents(docs);
    if added == 0 {
        eprintln!("[ffi] update skipped (engine not initialized or no docs)");
    }
}

#[no_mangle]
pub extern "C" fn free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        let _ = CString::from_raw(ptr);
    }
}

#[cfg(target_os = "android")]
mod jni_bridge {
    use super::{init_engine_from_file, init_engine_from_index, search_query, free_string};
    use jni::objects::{JClass, JString};
    use jni::sys::jstring;
    use jni::JNIEnv;
    use std::ffi::CString;

    #[no_mangle]
    pub extern "system" fn Java_com_app_search_NativeSearchEngine_init(
        env: JNIEnv,
        _class: JClass,
        path: JString,
    ) {
        let path_str = match env.get_string(&path) {
            Ok(s) => s.into(),
            Err(_) => "".to_string(),
        };
        if let Ok(c_path) = CString::new(path_str) {
            init_engine_from_file(c_path.as_ptr());
        }
    }

    #[no_mangle]
    pub extern "system" fn Java_com_app_search_NativeSearchEngine_initIndex(
        env: JNIEnv,
        _class: JClass,
        path: JString,
    ) {
        let path_str = match env.get_string(&path) {
            Ok(s) => s.into(),
            Err(_) => "".to_string(),
        };
        if let Ok(c_path) = CString::new(path_str) {
            init_engine_from_index(c_path.as_ptr());
        }
    }

    #[no_mangle]
    pub extern "system" fn Java_com_app_search_NativeSearchEngine_update(
        env: JNIEnv,
        _class: JClass,
        path: JString,
    ) {
        let path_str = match env.get_string(&path) {
            Ok(s) => s.into(),
            Err(_) => "".to_string(),
        };
        if let Ok(c_path) = CString::new(path_str) {
            super::update_engine_from_file(c_path.as_ptr());
        }
    }

    #[no_mangle]
    pub extern "system" fn Java_com_app_search_NativeSearchEngine_search(
        env: JNIEnv,
        _class: JClass,
        query: JString,
    ) -> jstring {
        let query_str = match env.get_string(&query) {
            Ok(s) => s.into(),
            Err(_) => "".to_string(),
        };
        let c_query = CString::new(query_str).unwrap_or_else(|_| CString::new("").unwrap());
        let raw = search_query(c_query.as_ptr());
        if raw.is_null() {
            return env.new_string("{}").unwrap().into_raw();
        }
        let cstr = unsafe { std::ffi::CStr::from_ptr(raw) };
        let json = cstr.to_string_lossy();
        let out = env.new_string(json.as_ref()).unwrap();
        free_string(raw);
        out.into_raw()
    }
}
