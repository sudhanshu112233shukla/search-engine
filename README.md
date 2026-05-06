# Offline Hybrid Search Engine

Production-grade, **offline-first** search engine with a Rust core and Android app.  
It runs fully on-device with hybrid retrieval (BM25 + semantic), multi-signal ranking, and answer extraction.

## Non-Technical Quick Start (Android)

If you just want to try the app, follow this:

1. Open this release page:  
   `https://github.com/sudhanshu112233shukla/search-engine/releases/tag/SEARCHENGCORE`
2. Download and install:  
   `SEARCHENG-debug-v2.1-classic-both-pages.apk`
3. Open the app -> tap `Settings`
4. Select language `en` and profile `default`
5. Tap `Download pack` (internet needed once)
6. Return to search screen and wait for indexing to finish
7. Search offline (internet can be turned off)

Try these first queries:

- `what is earth`
- `what is google`
- `what is wikipedia`

## Key Capabilities

- Hybrid retrieval: BM25 keyword + semantic similarity
- Multi-signal ranking: exact, phrase, and proximity signals
- Answer extraction with confidence and source
- ANN acceleration: IVF/PQ and HNSW for larger corpora
- Persistent on-disk index with mmap support
- Crash-safe updates via WAL replay
- Downloadable offline packs (language + profile)
- Android app (Compose + MVVM) via JNI/FFI bridge

## Architecture

```text
Android App (Compose)
    -> JNI/FFI
Rust Search Core
    -> BM25 + Vector Retrieval + ANN + Ranking + Answering
```

## Repository Structure

```text
android_app/         Android application
search_engine_rust/  Rust core engine + FFI + pack tooling
src/                 Node demo/search utilities
evaluation/          Evaluation datasets
docs/                Additional documentation
```

## Quick Start

### 1) Run Android Demo Build

```bash
cd android_app
.\gradlew.bat assembleDebug
```

APK output:

```text
android_app/app/build/outputs/apk/debug/app-debug.apk
```

### 2) Public Demo Pack (GitHub Releases)

Current demo release:

- Tag: `SEARCHENGCORE`
- Manifest file used by app: `android_app/app/src/main/assets/packs/manifest.json`
- Download base: `https://github.com/sudhanshu112233shukla/search-engine/releases/download/SEARCHENGCORE`

In app:

1. Open `Settings`
2. Select language `en`
3. Select profile `default` (or `power`)
4. Tap `Download pack`
5. Wait for indexing to complete, then search

Suggested test queries:

- `what is earth`
- `what is google`
- `what is wikipedia`

## Local Validation (Rust)

Validate pack:

```bash
cd search_engine_rust
cargo run --release -- validate-pack --dir data/packs_core/en --smoke-query "what is earth"
```

Single-shard query test:

```bash
cargo run --release -- search-index --dir data/packs_core/en/shard_0000 --query "what is google"
```

Pack info:

```bash
cargo run --release -- pack-info --dir data/packs_core/en
```

## Rebuild and Publish Demo Pack

Export release-ready assets:

```bash
cd search_engine_rust
cargo run --release -- export-packs --in data/packs_core --out dist/packs_core_release --method deflate --download-base https://github.com/sudhanshu112233shukla/search-engine/releases/download/SEARCHENGCORE
```

Upload generated files from `search_engine_rust/dist/packs_core_release/` to release tag `SEARCHENGCORE`.

More details: `docs/PACKS.md`.

## Node Demo (CLI/API)

CLI:

```bash
cd search-engine
npm install
node demo.js "what is bm25"
```

API:

```bash
cd search-engine
npm install
node src/server.js
```

Open:

```text
http://localhost:3001/search?q=what%20is%20bm25
```

## Ingestion Pipeline (Rust)

```bash
cd search_engine_rust
cargo run -- crawl --seed https://example.com --limit 1000
cargo run -- process
cargo run -- index
```

Config: `search_engine_rust/config.json`

Outputs:

- `data/raw/pages.jsonl`
- `data/processed/chunks.jsonl`
- `data/index/dataset.json`

## Importers

Wikipedia:

```bash
cargo run -- import-wiki --dump enwiki-latest-pages-articles-multistream.xml.bz2 --out data/wiki.jsonl --limit 10000
```

OpenStreetMap:

```bash
cargo run -- import-osm --pbf planet-latest.osm.pbf --out data/osm.jsonl --limit 10000
```

Common Crawl:

```bash
cargo run -- import-warc --warc CC-MAIN-2024-10.warc.gz --out data/web.jsonl --limit 10000
```

## Evaluation

```bash
cd search_engine_rust
cargo run -- eval ../evaluation/queries.json
```

Metrics include precision@K, recall@K, MRR, answer accuracy, and latency.

## Roadmap

- Better multilingual ranking and stemming
- More robust pack tiering and diff updates
- Higher corpus scale with improved on-device latency

## License

MIT




