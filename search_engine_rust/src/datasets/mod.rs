use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Read, Write};

use bzip2::read::BzDecoder;
use flate2::read::MultiGzDecoder;
use quick_xml::events::Event;
use quick_xml::Reader;
use scraper::{Html, Selector};

use crate::Document;
use crate::utils::split_sentences;

pub fn import_wikipedia(dump_path: &str, out_path: &str, limit: Option<usize>) -> io::Result<usize> {
    let reader: Box<dyn Read> = if dump_path.ends_with(".bz2") {
        Box::new(BzDecoder::new(File::open(dump_path)?))
    } else {
        Box::new(File::open(dump_path)?)
    };

    let buf_reader = BufReader::new(reader);
    let mut xml = Reader::from_reader(buf_reader);
    xml.trim_text(true);
    let mut buf = Vec::new();

    let mut in_page = false;
    let mut in_title = false;
    let mut in_id = false;
    let mut in_text = false;
    let mut title = String::new();
    let mut page_id = String::new();
    let mut text = String::new();
    let mut count = 0usize;

    let mut out = OpenOptions::new().create(true).write(true).truncate(true).open(out_path)?;

    loop {
        match xml.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                match e.name().as_ref() {
                    b"page" => {
                        in_page = true;
                        title.clear();
                        page_id.clear();
                        text.clear();
                    }
                    b"title" => in_title = true,
                    b"id" => {
                        if in_page && page_id.is_empty() {
                            in_id = true;
                        }
                    }
                    b"text" => in_text = true,
                    _ => {}
                }
            }
            Ok(Event::End(e)) => {
                match e.name().as_ref() {
                    b"page" => {
                        in_page = false;
                        in_title = false;
                        in_id = false;
                        in_text = false;
                        let cleaned = clean_wiki_text(&text);
                        if !title.is_empty() && !cleaned.is_empty() {
                            let doc = Document {
                                id: format!("wiki:{}", page_id),
                                text: format!("{}\n{}", title, cleaned),
                            };
                            writeln!(out, "{}", serde_json::to_string(&doc).unwrap_or_else(|_| "{}".to_string()))?;
                            count += 1;
                        }
                        if let Some(limit) = limit {
                            if count >= limit {
                                break;
                            }
                        }
                    }
                    b"title" => in_title = false,
                    b"id" => in_id = false,
                    b"text" => in_text = false,
                    _ => {}
                }
            }
            Ok(Event::Text(e)) => {
                if in_title {
                    if let Ok(t) = e.unescape() {
                        title.push_str(&t);
                    }
                } else if in_id {
                    if let Ok(t) = e.unescape() {
                        page_id.push_str(&t);
                    }
                } else if in_text {
                    if let Ok(t) = e.unescape() {
                        text.push_str(&t);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(count)
}

pub fn import_wikipedia_core(
    dump_path: &str,
    out_path: &str,
    limit: Option<usize>,
    sentences: usize,
    max_chars: usize,
) -> io::Result<usize> {
    let reader: Box<dyn Read> = if dump_path.ends_with(".bz2") {
        Box::new(BzDecoder::new(File::open(dump_path)?))
    } else {
        Box::new(File::open(dump_path)?)
    };

    let buf_reader = BufReader::new(reader);
    let mut xml = Reader::from_reader(buf_reader);
    xml.trim_text(true);
    let mut buf = Vec::new();

    let mut in_page = false;
    let mut in_title = false;
    let mut in_id = false;
    let mut in_text = false;
    let mut title = String::new();
    let mut page_id = String::new();
    let mut text = String::new();
    let mut count = 0usize;

    let mut out = OpenOptions::new().create(true).write(true).truncate(true).open(out_path)?;

    fn is_namespace_title(title: &str) -> bool {
        let t = title.to_lowercase();
        let prefixes = [
            "wikipedia:",
            "category:",
            "file:",
            "template:",
            "help:",
            "portal:",
            "draft:",
            "user:",
            "talk:",
            "special:",
        ];
        prefixes.iter().any(|p| t.starts_with(p))
    }

    loop {
        match xml.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => match e.name().as_ref() {
                b"page" => {
                    in_page = true;
                    title.clear();
                    page_id.clear();
                    text.clear();
                }
                b"title" => in_title = true,
                b"id" => {
                    if in_page && page_id.is_empty() {
                        in_id = true;
                    }
                }
                b"text" => in_text = true,
                _ => {}
            },
            Ok(Event::End(e)) => match e.name().as_ref() {
                b"page" => {
                    in_page = false;
                    in_title = false;
                    in_id = false;
                    in_text = false;

                    if title.is_empty() || page_id.is_empty() || is_namespace_title(&title) {
                        // Skip non-article namespaces for core pack.
                        buf.clear();
                        continue;
                    }

                    let cleaned = clean_wiki_text(&text);
                    if cleaned.is_empty() {
                        buf.clear();
                        continue;
                    }
                    let cleaned_norm = cleaned.trim_start().to_lowercase();
                    if cleaned_norm.starts_with("#redirect") || cleaned_norm.starts_with("redirect") {
                        // Skip redirects for the core pack; they're low value and waste space.
                        buf.clear();
                        continue;
                    }

                    let mut lead = String::new();
                    for s in split_sentences(&cleaned).into_iter().take(sentences.max(1)) {
                        if s.trim().is_empty() {
                            continue;
                        }
                        if !lead.is_empty() {
                            lead.push(' ');
                        }
                        lead.push_str(s.trim());
                        if lead.len() >= max_chars {
                            lead.truncate(max_chars);
                            break;
                        }
                    }

                    let lead = lead
                        .replace("'''", "")
                        .replace("''", "")
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ");
                    if !lead.is_empty() {
                        let doc = Document {
                            id: format!("wiki:{}", page_id),
                            text: format!("{}\n{}", title, lead),
                        };
                        writeln!(out, "{}", serde_json::to_string(&doc).unwrap_or_else(|_| "{}".to_string()))?;
                        count += 1;
                    }

                    if let Some(limit) = limit {
                        if count >= limit {
                            break;
                        }
                    }
                }
                b"title" => in_title = false,
                b"id" => in_id = false,
                b"text" => in_text = false,
                _ => {}
            },
            Ok(Event::Text(e)) => {
                if in_title {
                    if let Ok(t) = e.unescape() {
                        title.push_str(&t);
                    }
                } else if in_id {
                    if let Ok(t) = e.unescape() {
                        page_id.push_str(&t);
                    }
                } else if in_text {
                    if let Ok(t) = e.unescape() {
                        text.push_str(&t);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(count)
}

pub fn import_osm_pbf(pbf_path: &str, out_path: &str, limit: Option<usize>) -> io::Result<usize> {
    use osmpbf::{ElementReader, Element};

    let mut out = OpenOptions::new().create(true).write(true).truncate(true).open(out_path)?;
    let reader = ElementReader::from_path(pbf_path).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    let mut count = 0usize;

    reader.for_each(|element| {
        if let Some(limit) = limit {
            if count >= limit {
                return;
            }
        }
        match element {
            Element::Node(node) => {
                let mut name: Option<String> = None;
                let mut parts = Vec::new();
                for (k, v) in node.tags() {
                    if k == "name" {
                        name = Some(v.to_string());
                    }
                    if k.starts_with("addr:") || k == "amenity" || k == "place" || k == "shop" {
                        parts.push(format!("{}={}", k, v));
                    }
                }
                if let Some(n) = name {
                    if parts.is_empty() {
                        parts.push(n.clone());
                    } else {
                        parts.insert(0, n.clone());
                    }
                    let doc = Document { id: format!("osm:node:{}", node.id()), text: parts.join(" ") };
                    let _ = writeln!(out, "{}", serde_json::to_string(&doc).unwrap_or_else(|_| "{}".to_string()));
                    count += 1;
                }
            }
            Element::Way(way) => {
                let mut name: Option<String> = None;
                let mut parts = Vec::new();
                for (k, v) in way.tags() {
                    if k == "name" {
                        name = Some(v.to_string());
                    }
                    if k.starts_with("addr:") || k == "amenity" || k == "place" || k == "shop" {
                        parts.push(format!("{}={}", k, v));
                    }
                }
                if let Some(n) = name {
                    if parts.is_empty() {
                        parts.push(n.clone());
                    } else {
                        parts.insert(0, n.clone());
                    }
                    let doc = Document { id: format!("osm:way:{}", way.id()), text: parts.join(" ") };
                    let _ = writeln!(out, "{}", serde_json::to_string(&doc).unwrap_or_else(|_| "{}".to_string()));
                    count += 1;
                }
            }
            _ => {}
        }
    }).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    Ok(count)
}

pub fn import_warc(warc_path: &str, out_path: &str, limit: Option<usize>) -> io::Result<usize> {
    let reader: Box<dyn Read> = if warc_path.ends_with(".gz") {
        Box::new(MultiGzDecoder::new(File::open(warc_path)?))
    } else {
        Box::new(File::open(warc_path)?)
    };
    let mut buf_reader = BufReader::new(reader);
    let mut out = OpenOptions::new().create(true).write(true).truncate(true).open(out_path)?;

    let title_sel = Selector::parse("title").unwrap();
    let body_sel = Selector::parse("body").unwrap();

    let mut count = 0usize;
    loop {
        let record = match read_warc_record(&mut buf_reader) {
            Ok(Some(r)) => r,
            Ok(None) => break,
            Err(_) => break,
        };
        if record.warc_type != "response" {
            continue;
        }
        if let Some(limit) = limit {
            if count >= limit {
                break;
            }
        }
        let (url, html) = match split_http_body(&record.body, &record.target_uri) {
            Some(v) => v,
            None => continue,
        };
        let doc = html_to_doc(&url, &html, &title_sel, &body_sel);
        if let Some(doc) = doc {
            writeln!(out, "{}", serde_json::to_string(&doc).unwrap_or_else(|_| "{}".to_string()))?;
            count += 1;
        }
    }

    Ok(count)
}

fn clean_wiki_text(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '{' && chars.peek() == Some(&'{') {
            skip_nested(&mut chars, '{', '}');
            continue;
        }
        if c == '<' {
            skip_tag(&mut chars);
            continue;
        }
        if c == '[' && chars.peek() == Some(&'[') {
            chars.next();
            let content = read_until(&mut chars, "]]");
            if let Some(content) = content {
                if content.starts_with("File:") || content.starts_with("Image:") || content.starts_with("Category:") {
                    continue;
                }
                let parts: Vec<&str> = content.split('|').collect();
                out.push_str(parts.last().unwrap_or(&""));
                out.push(' ');
            }
            continue;
        }
        if c == '\n' { out.push(' '); } else { out.push(c); }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn skip_nested<I: Iterator<Item = char>>(chars: &mut std::iter::Peekable<I>, open: char, close: char) {
    let mut depth = 1;
    while let Some(c) = chars.next() {
        if c == open && chars.peek() == Some(&open) {
            chars.next();
            depth += 1;
        } else if c == close && chars.peek() == Some(&close) {
            chars.next();
            depth -= 1;
            if depth == 0 { break; }
        }
    }
}

fn skip_tag<I: Iterator<Item = char>>(chars: &mut std::iter::Peekable<I>) {
    while let Some(c) = chars.next() {
        if c == '>' { break; }
    }
}

fn read_until<I: Iterator<Item = char>>(chars: &mut std::iter::Peekable<I>, end: &str) -> Option<String> {
    let mut buf = String::new();
    let mut tail = String::new();
    while let Some(c) = chars.next() {
        buf.push(c);
        tail.push(c);
        if tail.len() > end.len() {
            tail.remove(0);
        }
        if tail == end {
            let len = buf.len() - end.len();
            buf.truncate(len);
            return Some(buf);
        }
    }
    None
}

struct WarcRecord {
    warc_type: String,
    target_uri: String,
    content_length: usize,
    body: Vec<u8>,
}

fn read_warc_record<R: BufRead>(reader: &mut R) -> io::Result<Option<WarcRecord>> {
    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line)?;
        if n == 0 {
            return Ok(None);
        }
        if line.starts_with("WARC/") {
            break;
        }
    }

    let mut warc_type = String::new();
    let mut target_uri = String::new();
    let mut content_length: usize = 0;

    loop {
        line.clear();
        let n = reader.read_line(&mut line)?;
        if n == 0 || line.trim().is_empty() {
            break;
        }
        let parts: Vec<&str> = line.splitn(2, ':').collect();
        if parts.len() != 2 { continue; }
        let key = parts[0].trim();
        let val = parts[1].trim();
        match key.to_lowercase().as_str() {
            "warc-type" => warc_type = val.to_lowercase(),
            "warc-target-uri" => target_uri = val.to_string(),
            "content-length" => content_length = val.parse().unwrap_or(0),
            _ => {}
        }
    }

    let mut body = Vec::new();
    if content_length > 0 {
        body.resize(content_length, 0u8);
        reader.read_exact(&mut body)?;
    }

    Ok(Some(WarcRecord { warc_type, target_uri, content_length, body }))
}

fn split_http_body(body: &[u8], target_uri: &str) -> Option<(String, String)> {
    let text = String::from_utf8_lossy(body);
    let mut parts: Vec<&str> = text.splitn(2, "\r\n\r\n").collect();
    if parts.len() < 2 {
        parts = text.splitn(2, "\n\n").collect();
    }
    let url = if target_uri.is_empty() { "unknown".to_string() } else { target_uri.to_string() };
    if parts.len() < 2 {
        return Some((url, text.to_string()));
    }
    let html = parts[1];
    Some((url, html.to_string()))
}

fn html_to_doc(url: &str, html: &str, title_sel: &Selector, body_sel: &Selector) -> Option<Document> {
    let doc = Html::parse_document(html);
    let title = doc
        .select(title_sel)
        .next()
        .map(|t| t.text().collect::<Vec<_>>().join(" "))
        .unwrap_or_default();
    let mut text = String::new();
    if let Some(body) = doc.select(body_sel).next() {
        for t in body.text() {
            text.push_str(t);
            text.push(' ');
        }
    } else {
        for t in doc.root_element().text() {
            text.push_str(t);
            text.push(' ');
        }
    }
    let cleaned = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if cleaned.is_empty() {
        return None;
    }
    let id = format!("web:{:x}", md5::compute(url.as_bytes()));
    Some(Document { id, text: format!("{}\n{}", title, cleaned) })
}
