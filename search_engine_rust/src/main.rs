use std::env;

use search_engine_rust::evaluation::{evaluate, load_eval_dataset, print_debug_breakdown, print_report};
use search_engine_rust::{Document, SearchEngine, Config};

fn load_docs_from_example() -> Vec<Document> {
    let data = std::fs::read_to_string("example_dataset.json").unwrap_or_else(|_| "[]".to_string());
    serde_json::from_str::<Vec<Document>>(&data).unwrap_or_default()
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 || args[1] != "eval" {
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
