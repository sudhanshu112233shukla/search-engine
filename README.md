# Offline Hybrid Search Engine (Rust Core + Android App)

A production-grade, **offline-first search engine** with a Rust core and a modern Android app. It combines **BM25 keyword search**, **semantic similarity**, multi-signal ranking, exact answer extraction, and evaluation tooling. Everything runs **on-device** without a server.

---

## 1. Project Overview

What it does:
- On-device hybrid search (BM25 + semantic)
- Multi-signal ranking (exact, phrase, proximity)
- Exact answer extraction with confidence
- Offline Android app (Jetpack Compose)
- Evaluation tooling (precision, recall, MRR)

Why it matters:
- Works without internet
- Fast and private
- Ideal for offline knowledge bases and local docs

---

## 2. Features

- Hybrid retrieval (BM25 + semantic)
- Multi-signal ranking (bm25, semantic, exact, phrase, proximity)
- Answer extraction and confidence scoring
- Snippet generation + highlighting
- Query cache
- IVF ANN vector search + int8 quantized vectors
- PQ (Product Quantization) for large corpora
- Persistent on-disk index (save/load)
- Disk-based URL frontier (crawler scale)
- Low-memory mode (text on disk; tokens on demand)
- Incremental index maintenance (merge, delete, compact)
- Deduplication (exact + near-duplicate simhash)
- Unicode-aware tokenization + light stemming
- Offline dataset loading
- Android app (Compose + MVVM)
- Evaluation CLI (precision@10, recall@10, MRR)
- Full ingestion pipeline (crawler -> processor -> index)
- Language packs + offline pack downloader

---

## 3. Architecture

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
  - ANN (IVF + PQ)
  - Multi-Signal Ranking
  - Answer Extraction
  - Snippet Generation
  |
Results + Answers
```

---

## 4. Repo Structure

```
search-engine/
+-- android_app/        # Android app (Compose + MVVM)
+-- search_engine_rust/ # Rust search core + FFI
+-- evaluation/         # Evaluation queries
+-- docs/               # Documentation + assets
+-- README.md
```

---

## 5. Quickstart (Demo)

### Option A: Android App (recommended)

1) Build Rust libs:
```bash
cd search_engine_rust
cargo build --release --target aarch64-linux-android
cargo build --release --target armv7-linux-androideabi
```

2) Copy .so files:
```
search_engine_rust/target/aarch64-linux-android/release/libsearch_engine_rust.so
search_engine_rust/target/armv7-linux-androideabi/release/libsearch_engine_rust.so
```
To:
```
android_app/app/src/main/jniLibs/arm64-v8a/
android_app/app/src/main/jniLibs/armeabi-v7a/
```

3) Open `android_app/` in Android Studio and run.

### Option B: Rust CLI (index + search)
```bash
cd search_engine_rust
cargo run -- build-index --dataset data/index/dataset.json --out data/index_store
cargo run -- load-index --dir data/index_store
```

---

## 6. Dataset Packs (Offline Bundles)

We ship and load **index packs** so Android can run fully offline.

### Build packs
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

### Android usage
- If a downloaded pack exists in `filesDir/packs_download/<lang>/<profile>`, it is used.
- Otherwise, the app loads from assets.

---

## 7. Ingestion Pipeline (Crawler -> Processor -> Index)

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
- Dataset for engine: `data/index/dataset.json`

---

## 8. Import Real-World Datasets

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

## 9. Persistent Index (Save/Load)

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

## 10. Evaluation (Quality Metrics)

```bash
cd search_engine_rust
cargo run -- eval ../evaluation/queries.json
```

Metrics include:
- Precision@10
- Recall@10
- MRR
- Answer Accuracy
- Latency (avg + max)

---

## 11. Demo Queries

See `docs/demo_queries.md` for sample queries and expected behavior.

---

## 12. Performance Notes

- Designed for **<200ms** search on mobile-sized datasets
- Fully offline; no network calls
- Memory scales with dataset size
- Query cache reduces repeated computation
- Low-memory mode for **100K+ docs**
- Quantized vectors reduce memory ~4x
- IVF + PQ reduces vector search latency on large corpora
- Partial indexing allows search before full build completes

---

## 13. Future Work

- Larger datasets (1M+ docs)
- Personalization + feedback signals
- Better multilingual stemming
- HNSW for higher ANN recall

---

## License

MIT (add your license file if needed)
