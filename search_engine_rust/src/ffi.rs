use std::ffi::{CStr, CString};
use std::fs;
use std::os::raw::c_char;

use crate::{Config, Document, SearchEngine};
use crate::{init_engine_once, search};

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
    let engine = SearchEngine::new(docs, Config::default());
    if !init_engine_once(engine) {
        eprintln!("[ffi] engine already initialized; skipping");
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
    let engine = SearchEngine::new(docs, Config::default());
    if !init_engine_once(engine) {
        eprintln!("[ffi] engine already initialized; skipping");
    }
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
    use super::{init_engine_from_file, search_query, free_string};
    use jni::objects::{JClass, JString};
    use jni::sys::jstring;
    use jni::JNIEnv;
    use std::ffi::CString;

    #[no_mangle]
    pub extern "system" fn Java_com_app_SearchEngine_init(
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
    pub extern "system" fn Java_com_app_SearchEngine_search(
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
