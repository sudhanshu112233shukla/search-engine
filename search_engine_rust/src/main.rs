use std::env;

use search_engine_rust::evaluation::{evaluate, load_eval_dataset, print_debug_breakdown, print_report};
use search_engine_rust::{Document, SearchEngine, Config};
use search_engine_rust::{PipelineCommand, PipelineConfig};
use search_engine_rust::cli as pipeline;

fn load_docs_from_example() -> Vec<Document> {
    let data = std::fs::read_to_string("example_dataset.json").unwrap_or_else(|_| "[]".to_string());
    serde_json::from_str::<Vec<Document>>(&data).unwrap_or_default()
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_usage();
        return;
    }

    let config_path = value_after(&args, "--config").unwrap_or_else(|| "config.json".to_string());
    let pipeline_config = pipeline::load_config(&config_path);

    match args[1].as_str() {
        "eval" => {
            if args.len() < 3 {
                eprintln!("Usage: cargo run -- eval evaluation/queries.json");
                return;
            }
            let dataset_path = &args[2];
            let eval = match load_eval_dataset(dataset_path) {
                Some(d) => d,
                None => {
                    eprintln!("Failed to load evaluation dataset");
                    return;
                }
            };

            let docs = load_docs_from_example();
            if docs.is_empty() {
                eprintln!("No docs loaded. Provide example_dataset.json in crate root.");
                return;
            }

            let engine = SearchEngine::new(docs, Config::default());
            let report = evaluate(&engine, eval);
            print_report(&report);
            print_debug_breakdown(&report, 5);
        }
        "crawl" => {
            let seed = value_after(&args, "--seed").unwrap_or_else(|| "".to_string());
            if seed.is_empty() {
                eprintln!("crawl requires --seed <url>");
                return;
            }
            let limit = value_after(&args, "--limit").and_then(|v| v.parse::<usize>().ok());
            pipeline::run(PipelineCommand::Crawl { seed, limit }, pipeline_config);
        }
        "process" => {
            pipeline::run(PipelineCommand::Process, pipeline_config);
        }
        "index" => {
            pipeline::run(PipelineCommand::Index, pipeline_config);
        }
        "build-index" => {
            let dataset = value_after(&args, "--dataset").unwrap_or_else(|| "data/index/dataset.json".to_string());
            let out_dir = value_after(&args, "--out").unwrap_or_else(|| "data/index_store".to_string());
            let docs = std::fs::read_to_string(&dataset).unwrap_or_else(|_| "[]".to_string());
            let parsed = serde_json::from_str::<Vec<Document>>(&docs).unwrap_or_default();
            if parsed.is_empty() {
                eprintln!("No documents loaded from dataset");
                return;
            }
            let mut config = Config::default();
            config.vector_quantize = true;
            config.ann_enabled = true;
            config.pq_enabled = true;
            config.text_store_path = Some(format!("{}/textstore.bin", out_dir));
            let engine = SearchEngine::new(parsed, config);
            if let Err(err) = engine.save_index(&out_dir) {
                eprintln!("Failed to save index: {err}");
            }
        }
        "load-index" => {
            let dir = value_after(&args, "--dir").unwrap_or_else(|| "data/index_store".to_string());
            let config = Config::default();
            let engine = SearchEngine::load_index(&dir, config);
            match engine {
                Ok(_) => println!("Index loaded successfully"),
                Err(err) => eprintln!("Load failed: {err}"),
            }
        }
        _ => print_usage(),
    }
}

fn value_after(args: &[String], flag: &str) -> Option<String> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned()
}

fn print_usage() {
    eprintln!("Usage:");
    eprintln!("  cargo run -- crawl --seed https://example.com --limit 1000");
    eprintln!("  cargo run -- process");
    eprintln!("  cargo run -- index");
    eprintln!("  cargo run -- build-index --dataset data/index/dataset.json --out data/index_store");
    eprintln!("  cargo run -- load-index --dir data/index_store");
    eprintln!("  cargo run -- eval evaluation/queries.json");
}
