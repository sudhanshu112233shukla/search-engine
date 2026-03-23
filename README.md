# Search Engine Core (Hybrid Retrieval + Extraction)

A local, offline-first hybrid search engine core that supports keyword and semantic retrieval, advanced ranking, exact answer extraction, incremental updates, and performance optimizations. This is **not** a chatbot; it returns **extracted answers** with sources and confidence.

## What This Builds

- **Hybrid retrieval**: BM25 + vector embeddings
- **Advanced ranking**: BM25, semantic, exact match, phrase match, freshness, feedback
- **Exact answer extraction**: sentence selection + shortest span
- **Incremental indexing**: append new docs without full rebuild
- **Mini crawler**: fetch + clean web pages
- **Offline-first updates**: queue failed URLs, sync later
- **Performance**: query cache + vector quantization + optimized load

## Architecture (Pipeline)

```
Query ? Cache ? Retrieval ? Multi-Signal Ranking ? Answer Extraction ? Output
```

## Project Structure

```
search-engine
+-- data/
¦   +-- raw/
¦   +-- processed/
¦   +-- index/
+-- src/
¦   +-- api/
¦   +-- cache/
¦   +-- confidence/
¦   +-- crawler/
¦   +-- extraction/
¦   +-- feedback/
¦   +-- indexing/
¦   +-- ingestion/
¦   +-- performance/
¦   +-- processing/
¦   +-- query/
¦   +-- ranking/
¦   +-- retrieval/
¦   +-- update/
¦   +-- utils/
+-- config.js
+-- server.js
```

## Quick Start

1. Put `.txt`, `.json`, or `.jsonl` docs in `data/raw/`
2. Start the server:

```bash
node server.js
```

3. Query:

```bash
curl -X POST http://localhost:3000/search \
  -H "Content-Type: application/json" \
  -d "{\"query\":\"your search\"}"
```

## API

### `POST /search`
Returns a **precise extracted answer** with source and confidence.

Request:
```json
{ "query": "what is bm25" }
```

Response:
```json
{
  "query": "what is bm25",
  "queryType": "factual",
  "answer": "...exact extracted span...",
  "source": {
    "id": "doc::chunk2",
    "sourceId": "doc.txt",
    "chunkIndex": 2,
    "totalChunks": 5,
    "meta": {}
  },
  "confidence": 2.14
}
```

### `POST /feedback`
Tracks clicks and boosts future results.

```json
{ "query": "bm25", "docId": "doc::chunk2" }
```

### `POST /update`
Incremental update without full reindex. Supports:
- `paths`: external folders/files (txt/json/jsonl)
- `wikipediaPath`: Wikipedia JSONL file
- `urls`: list of web pages to crawl
- `documents`: ad-hoc docs with `{id, text, meta}`

```json
{
  "paths": ["C:\\data\\docs"],
  "wikipediaPath": "C:\\data\\wiki.jsonl",
  "urls": ["https://example.com/page"],
  "documents": [{ "id": "doc-1", "text": "content", "meta": {} }]
}
```

Response:
```json
{
  "status": "ok",
  "summary": {
    "addedDocs": 10,
    "addedChunks": 47,
    "bm25Added": 47,
    "vectorAdded": 47,
    "queuedUrls": 2
  }
}
```

### `POST /sync`
Retries previously failed URLs (offline-first sync).

```json
{}
```

## Core Components

### Ingestion
- Supports `.txt`, `.json`, `.jsonl`
- JSON supports objects/arrays with fields: `text`, `content`, `body`

### Processing
- Cleans and chunks documents
- Default chunk size: 100–200 words

### Indexing
- **BM25** inverted index
- **Vector** index with quantized embeddings (int8)

### Retrieval
- Top-K from BM25 + vector
- Multi-signal ranking

### Answer Extraction
- Sentence selection + shortest span extraction
- No generation, extraction-only

### Feedback
- Click tracking boosts results
- Query-aware + global signals

### Crawler
- Fetches page HTML and strips scripts/styles/tags
- Stores cleaned text

### Performance
- Query caching (LRU + TTL)
- Vector quantization for memory + speed
- Hydrated vector arrays for fast dot products

## Ranking Signals
Final score is weighted sum of:
- BM25
- Semantic similarity
- Exact match
- Phrase match
- Freshness
- Feedback

Weights are configurable in `config.js`, with profiles for:
- `factual`
- `list`
- `comparison`

## Config Highlights (`config.js`)
- `chunking`: chunk size
- `bm25`: BM25 params
- `vector`: embedding + quantization options
- `ranking`: signal weights + profiles
- `cache`: TTL + size
- `update`: pending URL queue path

## Data Formats

### Wikipedia JSONL
One JSON object per line. Fields supported:
- `title`, `text`, `id`

Example line:
```json
{"id":"123","title":"BM25","text":"BM25 is a ranking function..."}
```

## Offline-First Behavior
- Failed URL fetches are queued in `data/index/pending_urls.json`
- `POST /sync` retries and updates indexes

## Rebuild Notes
To force a full rebuild, delete:
- `data/index/`
- `data/processed/`

Then restart the server.

## Notes
- Runs fully offline (crawler optional)
- No external dependencies required
- Designed to be extended (storage engines, ANN indexes, etc.)


