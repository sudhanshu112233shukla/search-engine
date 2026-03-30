const fs = require('fs');
const path = require('path');
const { BM25 } = require('./bm25');
const { VectorIndex } = require('./vector');
const { hybridRank } = require('./ranker');
const { extractBestSentence } = require('./extractor');

const DATA_PATH = path.join(__dirname, '..', 'data', 'docs.json');

let cached = null;

function loadDocs() {
  const raw = fs.readFileSync(DATA_PATH, 'utf8');
  return JSON.parse(raw);
}

function getEngine() {
  if (cached) return cached;
  const docs = loadDocs();
  const bm25 = new BM25(docs);
  const vector = new VectorIndex(docs);
  cached = { docs, bm25, vector };
  return cached;
}

function search(query) {
  const { docs, bm25, vector } = getEngine();
  const bm25Results = bm25.search(query, 50);
  const vectorResults = vector.search(query, 50);
  const ranked = hybridRank(bm25Results, vectorResults, { bm25: 0.7, vector: 0.3 });

  const results = ranked.slice(0, 3).map((r) => {
    const doc = docs.find((d) => d.id === r.id);
    return { id: r.id, text: doc ? doc.text : '', score: r.score };
  });

  const answerPick = extractBestSentence(query, results);\n  const confidence = Math.min(1, answerPick.score || 0);\n\n  return {\n    query,\n    results,\n    answer: answerPick.sentence || '',\n    answerSource: answerPick.sourceId || null,\n    answerScore: answerPick.score || 0,\n    confidence,\n  };
}

module.exports = { search };

