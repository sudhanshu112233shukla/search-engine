# Offline Hybrid Search Engine

A production-grade, offline-first search engine with a Rust core and an Android app. It delivers fast, private, on-device search using hybrid retrieval (BM25 + semantic), multi-signal ranking, and exact answer extraction. Built for mobile scale and designed to run without servers.

---

## Why This Exists

Most search systems assume connectivity, servers, and cloud compute. This project proves the opposite: a full search engine that works entirely offline, optimized for mobile devices and large local datasets. It is designed for teams building private knowledge bases, offline apps, and device-first products.

---

## Highlights

- Hybrid retrieval: BM25 keyword + semantic similarity
- Multi-signal ranking: exact match, phrase match, proximity
- Exact answer extraction with confidence
- ANN acceleration: IVF + PQ and HNSW for large corpora
- Persistent on-disk index (fast startup, low memory)
- Memory-mapped vectors for 100K+ to 1M docs
- Disk-based BM25 postings (mmap)\r\n- Incremental updates with WAL replay (crash-safe)
- Offline dataset packs with language + profile selection
- Android app with Compose + MVVM
- Evaluation framework (precision@K, recall@K, MRR)

---

## Architecture

```
User
  |
Android App (Compose UI)
  |
Rust FFI (JNI)
  |
Search Engine Core
  - BM25
  - Vector Similarity
  - ANN (IVF + PQ / HNSW)
  - Multi-Signal Ranking
  - Answer Extraction
  - Snippet Generation
  |
Results + Answers
```

---

## Repository Layout

```
search-engine/
+-- android_app/        # Android app (Compose + MVVM)
+-- search_engine_rust/ # Rust search core + FFI
+-- evaluation/         # Evaluation datasets
+-- docs/               # Documentation and assets
+-- README.md
```

---

## Quickstart

### Option A: Android App (recommended)

1. Build Rust libraries
```bash
cd search_engine_rust
cargo build --release --target aarch64-linux-android
cargo build --release --target armv7-linux-androideabi
```

2. Copy `.so` files
```
search_engine_rust/target/aarch64-linux-android/release/libsearch_engine_rust.so
search_engine_rust/target/armv7-linux-androideabi/release/libsearch_engine_rust.so
```
To:
```
android_app/app/src/main/jniLibs/arm64-v8a/
android_app/app/src/main/jniLibs/armeabi-v7a/
```

3. Open `android_app/` in Android Studio and run.

### Option B: Rust CLI

```bash
cd search_engine_rust
cargo run -- build-index --dataset data/index/dataset.json --out data/index_store
cargo run -- load-index --dir data/index_store
```

---

## Offline Packs (Language + Profile)

Packs allow the Android app to run fully offline with large datasets.

Build packs:
```bash
cd search_engine_rust
cargo run -- pack --dataset data/index/dataset.json --out data/packs --max-docs 50000 --lang en --download-base https://example.com/packs
```

Pack structure:
```
data/packs/
  manifest.json
  en/
    shard_0000/
    shard_0001/
```

Android uses:
- Downloaded packs in `filesDir/packs_download/<lang>/<profile>`
- Assets fallback if no download exists

---

## Ingestion Pipeline

From `search_engine_rust/`:

```bash
cargo run -- crawl --seed https://example.com --limit 1000
cargo run -- process
cargo run -- index
```

Config file: `search_engine_rust/config.json`

```json
{
  "crawl_limit": 10000,
  "max_depth": 3,
  "timeout_ms": 5000,
  "storage_path": "./data",
  "use_disk_frontier": true,
  "frontier_path": null
}
```

Outputs:
- Raw pages: `data/raw/pages.jsonl`
- Processed chunks: `data/processed/chunks.jsonl`
- Index dataset: `data/index/dataset.json`

---

## Import Real-World Datasets

### Wikipedia (XML dump)
```bash
cargo run -- import-wiki --dump enwiki-latest-pages-articles-multistream.xml.bz2 --out data/wiki.jsonl --limit 10000
```

### OpenStreetMap (PBF)
```bash
cargo run -- import-osm --pbf planet-latest.osm.pbf --out data/osm.jsonl --limit 10000
```

### Common Crawl (WARC)
```bash
cargo run -- import-warc --warc CC-MAIN-2024-10.warc.gz --out data/web.jsonl --limit 10000
```

---

## Persistent Index

Build on-disk index:
```bash
cargo run -- build-index --dataset data/index/dataset.json --out data/index_store
```

Load index:
```bash
cargo run -- load-index --dir data/index_store
```

Merge updates:
```bash
cargo run -- merge-index --dir data/index_store --update data/index/dataset.json
```

Delete by id:
```bash
cargo run -- delete --dir data/index_store doc:<id> doc:<id>
```

Compact index:
```bash
cargo run -- compact --dir data/index_store --out data/index_store_compact
```

---

## Crash-Safe Updates (WAL)

Updates are logged to `wal.jsonl` before applying. On restart, the engine automatically replays and clears the WAL to keep the index consistent.

---

## Evaluation

```bash
cd search_engine_rust
cargo run -- eval ../evaluation/queries.json
```

Metrics:
- Precision@10
- Recall@10
- MRR
- Answer Accuracy
- Latency (avg + max)

---

## Performance Targets

- <200ms search on mobile-sized datasets
- Low-memory mode for 100K+ docs
- Memory-mapped vectors for 1M docs
- ANN acceleration for large vector corpora
- Fully offline operation

---

## Roadmap

- Multi-million document scale beyond 1M
- Personalization and feedback signals
- Better multilingual stemming
- Streaming updates and pack diffing

---

## License

MIT (add a LICENSE file if needed)


