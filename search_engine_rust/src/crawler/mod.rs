use std::collections::{HashMap, HashSet, VecDeque};
use std::time::Duration;

use reqwest::blocking::Client;
use reqwest::Url;
use scraper::{Html, Selector};

use crate::storage::{RawPage, StorageManager};

#[derive(Clone, Debug)]
pub struct CrawlConfig {
    pub crawl_limit: usize,
    pub max_depth: usize,
    pub timeout_ms: u64,
    pub user_agent: String,
}

#[derive(Clone, Debug)]
struct UrlTask {
    url: Url,
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

pub struct Crawler {
    client: Client,
    config: CrawlConfig,
    storage: StorageManager,
    seen: HashSet<String>,
    queue: VecDeque<UrlTask>,
    robots: RobotsCache,
}

impl Crawler {
    pub fn new(config: CrawlConfig, storage: StorageManager) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_millis(config.timeout_ms))
            .user_agent(config.user_agent.clone())
            .build()
            .expect("failed to build http client");
        Self {
            client,
            config,
            storage,
            seen: HashSet::new(),
            queue: VecDeque::new(),
            robots: RobotsCache::default(),
        }
    }

    pub fn add_seed(&mut self, url: &str) {
        if let Ok(parsed) = Url::parse(url) {
            let norm = normalize_url(&parsed);
            if self.seen.insert(norm) {
                self.queue.push_back(UrlTask { url: parsed, depth: 0 });
            }
        }
    }

    pub fn crawl(&mut self) {
        let mut crawled = 0usize;
        while let Some(task) = self.queue.pop_front() {
            if crawled >= self.config.crawl_limit {
                break;
            }
            if task.depth > self.config.max_depth {
                continue;
            }
            if !self.robots.allowed(&self.client, &task.url) {
                continue;
            }
            let resp = match self.client.get(task.url.clone()).send() {
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
            let (title, text, links) = parse_html(&html, &task.url);
            if text.is_empty() {
                continue;
            }
            let page = RawPage::new(&task.url, &title, &text, &links);
            if self.storage.write_raw(&page).is_ok() {
                crawled += 1;
            }

            for link in links {
                if let Ok(parsed) = Url::parse(&link) {
                    let norm = normalize_url(&parsed);
                    if self.seen.insert(norm) {
                        self.queue.push_back(UrlTask { url: parsed, depth: task.depth + 1 });
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
