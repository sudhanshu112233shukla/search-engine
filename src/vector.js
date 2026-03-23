const { tokenize } = require('./bm25');

function termFreq(tokens) {
  const tf = new Map();
  for (const t of tokens) {
    tf.set(t, (tf.get(t) || 0) + 1);
  }
  return tf;
}

function cosineSimilarity(a, b) {
  let dot = 0;
  let normA = 0;
  let normB = 0;
  for (const [term, valA] of a.entries()) {
    const valB = b.get(term) || 0;
    dot += valA * valB;
    normA += valA * valA;
  }
  for (const valB of b.values()) {
    normB += valB * valB;
  }
  if (normA === 0 || normB === 0) return 0;
  return dot / (Math.sqrt(normA) * Math.sqrt(normB));
}

class VectorIndex {
  constructor(docs) {
    this.docs = docs;
    this.docVectors = docs.map((d) => termFreq(tokenize(d.text)));
  }

  search(query, topK = 50) {
    const qVec = termFreq(tokenize(query));
    const results = this.docVectors.map((vec, i) => {
      const score = cosineSimilarity(qVec, vec);
      return { id: this.docs[i].id, text: this.docs[i].text, score };
    });

    return results
      .filter((r) => r.score > 0)
      .sort((a, b) => b.score - a.score)
      .slice(0, topK);
  }
}

module.exports = { VectorIndex };
