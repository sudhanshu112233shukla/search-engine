use std::env;
use std::path::Path;
use std::io::{self};

use search_engine_rust::evaluation::{evaluate, load_eval_dataset, print_debug_breakdown, print_report};
use search_engine_rust::{Document, SearchEngine, Config, load_engine_from_dir};
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
            let out = value_after(&args, "--out").unwrap_or_else(|| "data/index_store".to_string());
            pipeline::run(PipelineCommand::BuildIndex { dataset, out }, pipeline_config);
        }
        "load-index" => {
            let dir = value_after(&args, "--dir").unwrap_or_else(|| "data/index_store".to_string());
            pipeline::run(PipelineCommand::LoadIndex { dir }, pipeline_config);
        }
        "search-index" => {
            let dir = value_after(&args, "--dir").unwrap_or_else(|| "data/index_store".to_string());
            let query = value_after(&args, "--query").unwrap_or_else(|| "".to_string());
            if query.trim().is_empty() {
                eprintln!("search-index requires --query <text>");
                return;
            }
            let config = Config::default();
            let engine = match load_engine_from_dir(&dir, config) {
                Ok(engine) => engine,
                Err(err) => {
                    eprintln!("Load failed: {err}");
                    return;
                }
            };
            let response = engine.search(&query);
            if let Some(answer) = response.answer {
                println!("Answer: {}", answer.text);
                println!("Confidence: {:.2}", answer.confidence);
                println!("Source: {}", answer.source);
            } else {
                println!("Answer: <none>");
            }
            println!("Results:");
            for result in response.results.iter().take(5) {
                println!("- {} ({:.3})", result.id, result.score);
                println!("  {}", result.text);
            }
        }
        "pack-info" => {
            let dir = value_after(&args, "--dir").unwrap_or_else(|| "data/packs/en".to_string());
            let manifest_path = Path::new(&dir).join("..").join("manifest.json");
            if manifest_path.exists() {
                println!("Manifest: {}", manifest_path.to_string_lossy());
            } else {
                println!("Manifest: <not found> (expected {})", manifest_path.to_string_lossy());
            }
            match pack_info(&dir) {
                Ok((shards, bytes)) => {
                    println!("Shards: {shards}");
                    println!("Bytes:  {bytes}");
                }
                Err(err) => eprintln!("pack-info failed: {err}"),
            }
        }
        "validate-pack" => {
            let dir = value_after(&args, "--dir").unwrap_or_else(|| "data/packs/en".to_string());
            let smoke_query = value_after(&args, "--smoke-query");
            match validate_pack(&dir, smoke_query.as_deref()) {
                Ok(_) => println!("Pack OK"),
                Err(err) => {
                    eprintln!("Pack invalid: {err}");
                    std::process::exit(2);
                }
            }
        }
        "export-packs" => {
            let in_dir = value_after(&args, "--in").unwrap_or_else(|| "data/packs".to_string());
            let out_dir = value_after(&args, "--out").unwrap_or_else(|| "dist/packs".to_string());
            let download_base = value_after(&args, "--download-base");
            let method = value_after(&args, "--method").unwrap_or_else(|| "stored".to_string());
            match export_packs(&in_dir, &out_dir, download_base.as_deref(), &method) {
                Ok(_) => println!("Export complete: {}", out_dir),
                Err(err) => {
                    eprintln!("export-packs failed: {err}");
                    std::process::exit(2);
                }
            }
        }
        "merge-index" => {
            let dir = value_after(&args, "--dir").unwrap_or_else(|| "data/index_store".to_string());
            let update = value_after(&args, "--update").unwrap_or_else(|| "data/index/dataset.json".to_string());
            pipeline::run(PipelineCommand::MergeIndex { dir, update }, pipeline_config);
        }
        "delete" => {
            let dir = value_after(&args, "--dir").unwrap_or_else(|| "data/index_store".to_string());
            let ids: Vec<String> = args.iter().filter(|a| a.starts_with("doc:")).map(|s| s[4..].to_string()).collect();
            if ids.is_empty() {
                eprintln!("delete requires doc:<id> arguments");
                return;
            }
            pipeline::run(PipelineCommand::Delete { dir, ids }, pipeline_config);
        }
        "compact" => {
            let dir = value_after(&args, "--dir").unwrap_or_else(|| "data/index_store".to_string());
            let out = value_after(&args, "--out").unwrap_or_else(|| "data/index_store_compact".to_string());
            pipeline::run(PipelineCommand::Compact { dir, out }, pipeline_config);
        }
        "pack" => {
            let dataset = value_after(&args, "--dataset").unwrap_or_else(|| "data/index/dataset.json".to_string());
            let out = value_after(&args, "--out").unwrap_or_else(|| "data/packs".to_string());
            let max_docs = value_after(&args, "--max-docs")
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(50000);
            let lang = value_after(&args, "--lang").unwrap_or_else(|| "en".to_string());
            let download_base = value_after(&args, "--download-base");
            pipeline::run(PipelineCommand::Pack { dataset, out, max_docs, lang, download_base }, pipeline_config);
        }
        "import-wiki" => {
            let dump = value_after(&args, "--dump").unwrap_or_else(|| "".to_string());
            let out = value_after(&args, "--out").unwrap_or_else(|| "data/wiki.jsonl".to_string());
            let limit = value_after(&args, "--limit").and_then(|v| v.parse::<usize>().ok());
            if dump.is_empty() {
                eprintln!("import-wiki requires --dump <path>");
                return;
            }
            pipeline::run(PipelineCommand::ImportWiki { dump, out, limit }, pipeline_config);
        }
        "import-osm" => {
            let pbf = value_after(&args, "--pbf").unwrap_or_else(|| "".to_string());
            let out = value_after(&args, "--out").unwrap_or_else(|| "data/osm.jsonl".to_string());
            let limit = value_after(&args, "--limit").and_then(|v| v.parse::<usize>().ok());
            if pbf.is_empty() {
                eprintln!("import-osm requires --pbf <path>");
                return;
            }
            pipeline::run(PipelineCommand::ImportOsm { pbf, out, limit }, pipeline_config);
        }
        "import-warc" => {
            let warc = value_after(&args, "--warc").unwrap_or_else(|| "".to_string());
            let out = value_after(&args, "--out").unwrap_or_else(|| "data/web.jsonl".to_string());
            let limit = value_after(&args, "--limit").and_then(|v| v.parse::<usize>().ok());
            if warc.is_empty() {
                eprintln!("import-warc requires --warc <path>");
                return;
            }
            pipeline::run(PipelineCommand::ImportWarc { warc, out, limit }, pipeline_config);
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
    eprintln!("  cargo run -- search-index --dir data/packs/en --query \"what is bm25\"");
    eprintln!("  cargo run -- pack-info --dir data/packs/en");
    eprintln!("  cargo run -- validate-pack --dir data/packs/en --smoke-query \"what is google\"");
    eprintln!("  cargo run -- export-packs --in data/packs --out dist/packs --download-base https://example.com/packs");
    eprintln!("  cargo run -- merge-index --dir data/index_store --update data/index/dataset.json");
    eprintln!("  cargo run -- delete --dir data/index_store doc:<id> doc:<id>");
    eprintln!("  cargo run -- compact --dir data/index_store --out data/index_store_compact");
    eprintln!("  cargo run -- pack --dataset data/index/dataset.json --out data/packs --max-docs 50000 --lang en --download-base https://example.com/packs");
    eprintln!("  cargo run -- import-wiki --dump enwiki-latest-pages-articles-multistream.xml.bz2 --out data/wiki.jsonl --limit 10000");
    eprintln!("  cargo run -- import-osm --pbf planet-latest.osm.pbf --out data/osm.jsonl --limit 10000");
    eprintln!("  cargo run -- import-warc --warc CC-MAIN-2024-10.warc.gz --out data/web.jsonl --limit 10000");
    eprintln!("  cargo run -- eval evaluation/queries.json");
}

fn pack_info(root: &str) -> std::io::Result<(usize, u64)> {
    let mut shards = 0usize;
    let mut bytes = 0u64;
    let root = Path::new(root);
    if !root.exists() {
        return Ok((0, 0));
    }
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if !name.starts_with("shard_") {
                continue;
            }
        }
        shards += 1;
        for file in std::fs::read_dir(&path)? {
            let file = file?;
            let meta = file.metadata()?;
            if meta.is_file() {
                bytes += meta.len();
            }
        }
    }
    Ok((shards, bytes))
}

fn validate_pack(root: &str, smoke_query: Option<&str>) -> std::io::Result<()> {
    let root = Path::new(root);
    if !root.exists() {
        return Err(std::io::Error::new(std::io::ErrorKind::NotFound, "pack dir not found"));
    }

    let required = [
        "bm25_terms.bin",
        "bm25_postings.bin",
        "chunks.bin",
        "deleted.bin",
        "meta.bin",
        "textstore.bin",
        "vector.bin",
    ];

    let mut shard_dirs: Vec<std::path::PathBuf> = Vec::new();
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with("shard_") {
                    shard_dirs.push(path);
                }
            }
        }
    }
    shard_dirs.sort();
    if shard_dirs.is_empty() {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "no shard_* dirs found"));
    }

    for dir in &shard_dirs {
        for file in required {
            let p = dir.join(file);
            if !p.exists() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("missing {} in {}", file, dir.to_string_lossy()),
                ));
            }
        }
        let meta_len = std::fs::metadata(dir.join("meta.bin"))?.len();
        if meta_len == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("meta.bin empty in {}", dir.to_string_lossy()),
            ));
        }
    }

    if let Some(q) = smoke_query {
        let first = shard_dirs[0].to_string_lossy().to_string();
        let engine = load_engine_from_dir(&first, Config::default())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        let resp = engine.search(q);
        if resp.results.is_empty() {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "smoke query returned no results"));
        }
    }

    Ok(())
}

fn export_packs(in_dir: &str, out_dir: &str, download_base: Option<&str>, method: &str) -> std::io::Result<()> {
    // Layout produced:
    // - <out_dir>/manifest.json
    // - <out_dir>/en/shard_0000.zip (zip contains files at root, no extra dir prefix)
    //
    // This matches Android's downloader, which downloads `${download_base}/en/shard_0000.zip`
    // and unzips into `.../packs_download/en/<profile>/shard_0000/`.
    let in_root = Path::new(in_dir);
    let out_root = Path::new(out_dir);
    std::fs::create_dir_all(out_root.join("en"))?;

    let manifest_src = in_root.join("manifest.json");
    if !manifest_src.exists() {
        return Err(std::io::Error::new(std::io::ErrorKind::NotFound, "manifest.json not found in input"));
    }
    let mut manifest_json = std::fs::read_to_string(&manifest_src)?;

    // Compute total bytes so "power" can mean "all shards" for demo packs.
    let (shard_count, total_bytes) = pack_info(&in_root.join("en").to_string_lossy())?;
    if shard_count > 0 {
        if let Ok(mut v) = serde_json::from_str::<serde_json::Value>(&manifest_json) {
            if let Some(langs) = v.get_mut("languages").and_then(|x| x.as_array_mut()) {
                if let Some(lang0) = langs.first_mut() {
                    if let Some(profiles) = lang0.get_mut("profiles").and_then(|p| p.as_array_mut()) {
                        for p in profiles.iter_mut() {
                            if p.get("name").and_then(|n| n.as_str()) == Some("power") {
                                p["max_bytes"] = serde_json::Value::Number(serde_json::Number::from(total_bytes));
                            }
                        }
                    }
                }
            }
            manifest_json = serde_json::to_string_pretty(&v).unwrap_or(manifest_json);
        }
    }

    // Optionally stamp download_base for hosted demos.
    if let Some(base) = download_base {
        if let Ok(mut v) = serde_json::from_str::<serde_json::Value>(&manifest_json) {
            v["download_base"] = serde_json::Value::String(base.to_string());
            manifest_json = serde_json::to_string_pretty(&v).unwrap_or(manifest_json);
        }
    }
    std::fs::write(out_root.join("manifest.json"), manifest_json.as_bytes())?;

    // Read shards from manifest so exported set matches what Android sees.
    let manifest: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&manifest_src)?)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let languages = manifest.get("languages").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let en = languages.iter().find(|l| l.get("code").and_then(|c| c.as_str()) == Some("en"))
        .or_else(|| languages.first());
    let shards = en
        .and_then(|l| l.get("shards"))
        .and_then(|s| s.as_array())
        .cloned()
        .unwrap_or_default();

    if shards.is_empty() {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "no shards in manifest"));
    }

    for shard in shards {
        let name = shard.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if name.is_empty() {
            continue;
        }
        // Input uses `data/packs\en\shard_0000` style paths. Normalize by just joining by name.
        let shard_dir = in_root.join("en").join(name);
        if !shard_dir.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("missing shard dir: {}", shard_dir.to_string_lossy()),
            ));
        }

        let zip_path = out_root.join("en").join(format!("{name}.zip"));
        let zip_file = std::fs::File::create(&zip_path)?;
        let mut zip = zip::ZipWriter::new(zip_file);
        let compression = match method.to_lowercase().as_str() {
            "deflate" => zip::CompressionMethod::Deflated,
            _ => zip::CompressionMethod::Stored,
        };
        let opts = zip::write::FileOptions::<()>::default().compression_method(compression);

        for entry in std::fs::read_dir(&shard_dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let file_name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n,
                None => continue,
            };
            zip.start_file(file_name, opts)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            let mut f = std::fs::File::open(&path)?;
            io::copy(&mut f, &mut zip)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        }

        zip.finish()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        println!("Wrote {}", zip_path.to_string_lossy());
    }

    Ok(())
}
