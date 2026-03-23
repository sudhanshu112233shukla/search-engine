const path = require('path');

const rootDir = __dirname;

module.exports = {
  paths: {
    root: rootDir,
    raw: path.join(rootDir, 'data', 'raw'),
    processed: path.join(rootDir, 'data', 'processed'),
    index: path.join(rootDir, 'data', 'index'),
  },
  chunking: {
    minWords: 100,
    maxWords: 200,
    targetWords: 150,
  },
  bm25: {
    k1: 1.5,
    b: 0.75,
  },
  vector: {
    dims: 384,
    ngramMin: 3,
    ngramMax: 5,
    quantize: true,
    quantizeScale: 127,\n    storeText: false,
  },
  retrieval: {
    bm25TopK: 50,
    vectorTopK: 50,
    finalTopK: 10,
  },
  ranking: {
    weights: {
      bm25: 1.0,
      semantic: 1.0,
      exact: 0.8,
      phrase: 0.6,
      freshness: 0.4,
      feedback: 0.5,
    },
    profiles: {
      factual: {
        exact: 1.0,
        phrase: 0.7,
      },
      list: {
        semantic: 1.2,
        bm25: 0.9,
        phrase: 0.5,
      },
      comparison: {
        phrase: 0.9,
        bm25: 1.1,
      },
    },
  },
  feedback: {
    path: path.join(rootDir, 'data', 'index', 'feedback.json'),
  },
  crawler: {
    userAgent: 'MiniCrawler/1.0',
    timeoutMs: 10000,
  },
  update: {
    pendingUrlsPath: path.join(rootDir, 'data', 'index', 'pending_urls.json'),
  },
  cache: {
    enabled: true,
    maxEntries: 500,
    ttlMs: 60000,
  },
  server: {
    port: 3000,
  },
};
