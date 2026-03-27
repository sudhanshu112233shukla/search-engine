use std::fs;

use crate::crawler::{CrawlConfig, Crawler};
use crate::processor::{ProcessConfig, Processor};
use crate::storage::{PipelineConfig, StorageManager};

#[derive(Debug)]
pub enum Command {
    Crawl { seed: String, limit: Option<usize> },
    Process,
    Index,
}

pub fn load_config(path: &str) -> PipelineConfig {
    let data = fs::read_to_string(path).unwrap_or_else(|_| "".to_string());
    serde_json::from_str(&data).unwrap_or_default()
}

pub fn run(cmd: Command, config: PipelineConfig) {
    let storage = StorageManager::new(&config.storage_path);

    match cmd {
        Command::Crawl { seed, limit } => {
            let limit = limit.unwrap_or(config.crawl_limit);
            let crawl_config = CrawlConfig {
                crawl_limit: limit,
                max_depth: config.max_depth,
                timeout_ms: config.timeout_ms,
                user_agent: "OfflineSearchCrawler/1.0".to_string(),
                use_disk_frontier: config.use_disk_frontier,
                frontier_path: config.frontier_path.clone(),
            };
            let mut crawler = Crawler::new(crawl_config, storage);
            crawler.add_seed(&seed);
            crawler.crawl();
        }
        Command::Process => {
            let processor = Processor::new(
                ProcessConfig { min_words: 100, max_words: 300 },
                storage,
            );
            processor.process_all();
        }
        Command::Index => {
            let chunks = storage.read_processed();
            if let Err(err) = storage.write_dataset(&chunks) {
                eprintln!("index write failed: {err}");
            }
        }
    }
}
