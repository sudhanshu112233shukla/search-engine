const { tokenize, removeStopwords } = require('../utils/text');

function buildBm25Index(chunks, options) {
  const k1 = options.k1 || 1.5;
  const b = options.b || 0.75;

  const docLengths = {};
  const docs = {};
  const inverted = {};

  for (const chunk of chunks) {
    const tokens = removeStopwords(tokenize(chunk.text));
    const docId = chunk.id;
    docs[docId] = {
      id: chunk.id,
      text: chunk.text,
      sourceId: chunk.sourceId,
      chunkIndex: chunk.chunkIndex,
      totalChunks: chunk.totalChunks,
      meta: chunk.meta || {},
    };
    docLengths[docId] = tokens.length;

    const tf = new Map();
    for (const token of tokens) {
      tf.set(token, (tf.get(token) || 0) + 1);
    }

    for (const [term, freq] of tf.entries()) {
      if (!inverted[term]) {
        inverted[term] = { df: 0, postings: [] };
      }
      inverted[term].df += 1;
      inverted[term].postings.push([docId, freq]);
    }
  }

  const docCount = Object.keys(docs).length;
  const totalLen = Object.values(docLengths).reduce((sum, len) => sum + len, 0);
  const avgDocLen = docCount > 0 ? totalLen / docCount : 0;

  return {
    type: 'bm25',
    k1,
    b,
    docCount,
    totalLen,
    avgDocLen,
    docLengths,
    docs,
    inverted,
  };
}

function addToBm25Index(index, chunks) {
  if (!index.totalLen) {
    index.totalLen = Object.values(index.docLengths || {}).reduce((sum, len) => sum + len, 0);
  }
  if (!index.docCount) {
    index.docCount = Object.keys(index.docs || {}).length;
  }

  let added = 0;
  for (const chunk of chunks) {
    const docId = chunk.id;
    if (index.docs[docId]) continue;

    const tokens = removeStopwords(tokenize(chunk.text));
    index.docs[docId] = {
      id: chunk.id,
      text: chunk.text,
      sourceId: chunk.sourceId,
      chunkIndex: chunk.chunkIndex,
      totalChunks: chunk.totalChunks,
      meta: chunk.meta || {},
    };
    index.docLengths[docId] = tokens.length;
    index.totalLen += tokens.length;
    index.docCount += 1;
    added += 1;

    const tf = new Map();
    for (const token of tokens) {
      tf.set(token, (tf.get(token) || 0) + 1);
    }

    for (const [term, freq] of tf.entries()) {
      if (!index.inverted[term]) {
        index.inverted[term] = { df: 0, postings: [] };
      }
      index.inverted[term].df += 1;
      index.inverted[term].postings.push([docId, freq]);
    }
  }

  index.avgDocLen = index.docCount > 0 ? index.totalLen / index.docCount : 0;
  return added;
}

function bm25Search(index, query, topK) {
  const tokens = removeStopwords(tokenize(query));
  if (tokens.length === 0) return [];

  const scores = new Map();
  const N = index.docCount || 0;
  const k1 = index.k1;
  const b = index.b;

  for (const term of tokens) {
    const entry = index.inverted[term];
    if (!entry) continue;
    const df = entry.df;
    const idf = Math.log(1 + (N - df + 0.5) / (df + 0.5));

    for (const [docId, tf] of entry.postings) {
      const docLen = index.docLengths[docId] || 0;
      const numerator = tf * (k1 + 1);
      const denominator = tf + k1 * (1 - b + b * (docLen / (index.avgDocLen || 1)));
      const score = idf * (numerator / (denominator || 1));
      scores.set(docId, (scores.get(docId) || 0) + score);
    }
  }

  const results = Array.from(scores.entries()).map(([id, score]) => ({ id, score }));
  results.sort((a, b) => b.score - a.score);
  return results.slice(0, topK || 50);
}

module.exports = {
  buildBm25Index,
  addToBm25Index,
  bm25Search,
};
