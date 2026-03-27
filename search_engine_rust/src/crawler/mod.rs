use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use reqwest::blocking::Client;
use reqwest::Url;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};

use crate::storage::{RawPage, StorageManager};

#[derive(Clone, Debug)]
pub struct CrawlConfig {
    pub crawl_limit: usize,
    pub max_depth: usize,
    pub timeout_ms: u64,
    pub user_agent: String,
    pub use_disk_frontier: bool,
    pub frontier_path: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct UrlTask {
    url: String,
    depth: usize,
}

#[derive(Default)]
struct RobotsCache {
    rules: HashMap<String, Vec<String>>,
}

impl RobotsCache {
    fn allowed(&mut self, client: &Client, url: &Url) -> bool {
        let host = match url.host_str() {
            Some(h) => h.to_string(),
            None => return false,
        };
        if !self.rules.contains_key(&host) {
            let robots_url = format!("{}://{}/robots.txt", url.scheme(), host);
            let mut disallow = Vec::new();
            if let Ok(resp) = client.get(&robots_url).send() {
                if let Ok(text) = resp.text() {
                    for line in text.lines() {
                        let line = line.trim();
                        if line.to_lowercase().starts_with("disallow:") {
                            let path = line.splitn(2, ':').nth(1).unwrap_or("").trim();
                            if !path.is_empty() {
                                disallow.push(path.to_string());
                            }
                        }
                    }
                }
            }
            self.rules.insert(host.clone(), disallow);
        }

        if let Some(disallow) = self.rules.get(&host) {
            let path = url.path();
            for rule in disallow {
                if rule == "/" || (!rule.is_empty() && path.starts_with(rule)) {
                    return false;
                }
            }
        }
        true
    }
}

struct DiskFrontier {
    queue_path: PathBuf,
    seen_path: PathBuf,
    cursor_path: PathBuf,
    cursor: u64,
    queue: VecDeque<UrlTask>,
    seen: HashSet<String>,
}

impl DiskFrontier {
    fn new(root: &Path) -> Self {
        let queue_path = root.join("frontier.queue");
        let seen_path = root.join("frontier.seen");
        let cursor_path = root.join("frontier.cursor");
        let cursor = fs::read_to_string(&cursor_path)
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);
        let seen = load_seen(&seen_path);
        Self {
            queue_path,
            seen_path,
            cursor_path,
            cursor,
            queue: VecDeque::new(),
            seen,
        }
    }

    fn enqueue(&mut self, task: UrlTask) {
        if self.seen.insert(task.url.clone()) {
            let _ = append_jsonl(&self.queue_path, &task);
            let _ = append_line(&self.seen_path, &task.url);
        }
    }

    fn dequeue(&mut self) -> Option<UrlTask> {
        if let Some(task) = self.queue.pop_front() {
            return Some(task);
        }
        self.fill_queue();
        self.queue.pop_front()
    }

    fn fill_queue(&mut self) {
        let file = match File::open(&self.queue_path) {
            Ok(f) => f,
            Err(_) => return,
        };
        let mut reader = BufReader::new(file);
        if reader.seek(SeekFrom::Start(self.cursor)).is_err() {
            return;
        }
        let mut buf = String::new();
        let mut read_bytes = 0u64;
        while self.queue.len() < 1000 {
            buf.clear();
            let bytes = match reader.read_line(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(_) => break,
            };
            read_bytes += bytes as u64;
            if let Ok(task) = serde_json::from_str::<UrlTask>(buf.trim()) {
                self.queue.push_back(task);
            }
        }
        self.cursor += read_bytes;
        let _ = fs::write(&self.cursor_path, self.cursor.to_string());
    }
}

fn append_jsonl<T: Serialize>(path: &Path, item: &T) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = OpenOptions::new().create(true).append(true).open(path)?;
    let mut writer = std::io::BufWriter::new(file);
    let line = serde_json::to_string(item).unwrap_or_else(|_| "{}".to_string());
    writer.write_all(line.as_bytes())?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}

fn append_line(path: &Path, line: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = OpenOptions::new().create(true).append(true).open(path)?;
    let mut writer = std::io::BufWriter::new(file);
    writer.write_all(line.as_bytes())?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}

fn load_seen(path: &Path) -> HashSet<String> {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return HashSet::new(),
    };
    let reader = BufReader::new(file);
    reader.lines().filter_map(|l| l.ok()).collect()
}

pub struct Crawler {
    client: Client,
    config: CrawlConfig,
    storage: StorageManager,
    seen: HashSet<String>,
    queue: VecDeque<UrlTask>,
    robots: RobotsCache,
    disk_frontier: Option<DiskFrontier>,
}

impl Crawler {
    pub fn new(config: CrawlConfig, storage: StorageManager) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_millis(config.timeout_ms))
            .user_agent(config.user_agent.clone())
            .build()
            .expect("failed to build http client");
        let disk_frontier = if config.use_disk_frontier {
            let root = config
                .frontier_path
                .clone()
                .map(PathBuf::from)
                .unwrap_or_else(|| storage.root_path().join("frontier"));
            Some(DiskFrontier::new(&root))
        } else {
            None
        };

        Self {
            client,
            config,
            storage,
            seen: HashSet::new(),
            queue: VecDeque::new(),
            robots: RobotsCache::default(),
            disk_frontier,
        }
    }

    pub fn add_seed(&mut self, url: &str) {
        if let Ok(parsed) = Url::parse(url) {
            let norm = normalize_url(&parsed);
            if let Some(frontier) = &mut self.disk_frontier {
                frontier.enqueue(UrlTask { url: norm, depth: 0 });
                return;
            }
            if self.seen.insert(norm) {
                self.queue.push_back(UrlTask { url: parsed.to_string(), depth: 0 });
            }
        }
    }

    pub fn crawl(&mut self) {
        let mut crawled = 0usize;
        loop {
            if crawled >= self.config.crawl_limit {
                break;
            }
            let task = if let Some(frontier) = &mut self.disk_frontier {
                frontier.dequeue()
            } else {
                self.queue.pop_front()
            };
            let task = match task {
                Some(t) => t,
                None => break,
            };

            if task.depth > self.config.max_depth {
                continue;
            }
            let parsed = match Url::parse(&task.url) {
                Ok(u) => u,
                Err(_) => continue,
            };
            if !self.robots.allowed(&self.client, &parsed) {
                continue;
            }
            let resp = match self.client.get(parsed.clone()).send() {
                Ok(r) => r,
                Err(_) => continue,
            };
            if !resp.status().is_success() {
                continue;
            }
            let html = match resp.text() {
                Ok(t) => t,
                Err(_) => continue,
            };
            let (title, text, links) = parse_html(&html, &parsed);
            if text.is_empty() {
                continue;
            }
            let page = RawPage::new(&parsed, &title, &text, &links);
            if self.storage.write_raw(&page).is_ok() {
                crawled += 1;
            }

            for link in links {
                if let Ok(parsed) = Url::parse(&link) {
                    let norm = normalize_url(&parsed);
                    if let Some(frontier) = &mut self.disk_frontier {
                        frontier.enqueue(UrlTask { url: norm, depth: task.depth + 1 });
                    } else if self.seen.insert(norm) {
                        self.queue.push_back(UrlTask { url: parsed.to_string(), depth: task.depth + 1 });
                    }
                }
            }
        }
    }
}

fn parse_html(html: &str, base: &Url) -> (String, String, Vec<String>) {
    let doc = Html::parse_document(html);
    let title_sel = Selector::parse("title").unwrap();
    let body_sel = Selector::parse("body").unwrap();
    let a_sel = Selector::parse("a").unwrap();

    let title = doc
        .select(&title_sel)
        .next()
        .map(|t| t.text().collect::<Vec<_>>().join(" "))
        .unwrap_or_default();

    let mut text = String::new();
    if let Some(body) = doc.select(&body_sel).next() {
        for t in body.text() {
            text.push_str(t);
            text.push(' ');
        }
    }
    text = clean_text(&text);

    let mut links = Vec::new();
    for node in doc.select(&a_sel) {
        if let Some(href) = node.value().attr("href") {
            if let Ok(resolved) = base.join(href) {
                links.push(resolved.to_string());
            }
        }
    }

    (title, text, links)
}

fn clean_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut prev_space = false;
    for c in text.chars() {
        if c.is_whitespace() {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(c);
            prev_space = false;
        }
    }
    out.trim().to_string()
}

fn normalize_url(url: &Url) -> String {
    let mut normalized = url.clone();
    normalized.set_fragment(None);
    normalized.to_string()
}
