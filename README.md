# Offline Hybrid Search Engine (Rust Core + Android App)

A production-grade, **offline-first search engine** with a Rust core and a modern Android app. It combines **BM25 keyword search**, **semantic similarity**, multi-signal ranking, exact answer extraction, and evaluation tooling—all built to run **entirely on device** without a server.

---

## 1. Project Overview

This project delivers a real mobile search experience:

- **On-device hybrid search** (BM25 + semantic similarity)
- **Multi-signal ranking** (exact match, phrase match, proximity)
- **Answer extraction** with confidence scoring
- **Offline-first Android app** using Jetpack Compose
- **Evaluation framework** (precision, recall, MRR)

Why it matters:
- Works without internet
- Fast, private, and reliable
- Ideal for mobile knowledge bases and offline docs

---

## 2. Features

- Hybrid retrieval (BM25 + semantic)
- Multi-signal ranking (bm25, semantic, exact, phrase, proximity)
- Answer extraction (top answers)
- Snippet generation + highlighting
- Query cache
- IVF ANN vector search + int8 quantized vectors
- Low-memory mode (store text on disk; recompute tokens on demand)
- Offline dataset loading
- Jetpack Compose UI + MVVM
- Evaluation CLI (precision@10, recall@10, MRR)
- Full ingestion pipeline (crawler → processor → index)

---

## 3. Architecture

```
User
  ?
Android App (Compose UI)
  ?
Rust FFI (JNI)
  ?
Search Engine Core
  +- BM25
  +- Vector Similarity
  +- Multi-Signal Ranking
  +- Answer Extraction
  +- Snippet Generation
  ?
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

## 5. Demo Screenshots

Add screenshots here:

```
docs/screenshots/home.png
docs/screenshots/results.png
docs/screenshots/detail.png
```

(Placeholders included; add real images for portfolio.)

---

## 6. How To Run

### 6.1 Build Rust Core

From `search_engine_rust/`:

```bash
cargo build --release
```

### 6.2 Build Android Native Libraries

```bash
rustup target add aarch64-linux-android armv7-linux-androideabi

cargo build --release --target aarch64-linux-android
cargo build --release --target armv7-linux-androideabi
```

Copy:
```
search_engine_rust/target/aarch64-linux-android/release/libsearch_engine_rust.so
search_engine_rust/target/armv7-linux-androideabi/release/libsearch_engine_rust.so
```

To:
```
android_app/app/src/main/jniLibs/arm64-v8a/
android_app/app/src/main/jniLibs/armeabi-v7a/
```

### 6.3 Run Android App

Open `android_app/` in Android Studio and run.

---

## 7. Ingestion Pipeline (Crawler → Processor → Index)

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
  "storage_path": "./data"
}
```

Outputs:
- Raw pages: `data/raw/pages.jsonl`
- Processed chunks: `data/processed/chunks.jsonl`
- Dataset for engine: `data/index/dataset.json`

---

## 8. Evaluation (Quality Metrics)

Run evaluation CLI:

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

## 9. Demo Queries

See `docs/demo_queries.md` for sample queries + expected behavior.

---

## 10. Performance Notes

- Designed for **<200ms** search on mobile-sized datasets
- Fully offline; no network calls
- Memory scales with dataset size
- Query cache reduces repeated computation
- Low-memory mode for **100K+ docs** (text on disk + on-demand tokens)
- Quantized vectors reduce memory ~4x
- IVF ANN reduces vector search latency on large corpora

---

## 11. Future Work

- Larger datasets (1M+ docs)
- Personalization + feedback signals
- Better embeddings (small on-device models)
- HNSW or PQ for higher ANN recall

---

## License

MIT (add your license file if needed)
