function normalizeScores(results) {
  if (!results || results.length === 0) return new Map();
  let min = Infinity;
  let max = -Infinity;
  for (const r of results) {
    if (r.score < min) min = r.score;
    if (r.score > max) max = r.score;
  }

  const normalized = new Map();
  const range = max - min;
  for (const r of results) {
    let value = 0;
    if (range === 0) {
      value = r.score > 0 ? 1 : 0;
    } else {
      value = (r.score - min) / range;
    }
    normalized.set(r.id, value);
  }
  return normalized;
}

function combineScores(bm25Results, vectorResults, weights) {
  const bm25Norm = normalizeScores(bm25Results);
  const vectorNorm = normalizeScores(vectorResults);
  const combined = new Map();

  const bm25Weight = weights.bm25Weight ?? 0.5;
  const vectorWeight = weights.vectorWeight ?? 0.5;

  for (const [id, score] of bm25Norm.entries()) {
    combined.set(id, (combined.get(id) || 0) + score * bm25Weight);
  }
  for (const [id, score] of vectorNorm.entries()) {
    combined.set(id, (combined.get(id) || 0) + score * vectorWeight);
  }

  const results = Array.from(combined.entries()).map(([id, score]) => ({ id, score }));
  results.sort((a, b) => b.score - a.score);
  return results;
}

module.exports = {
  normalizeScores,
  combineScores,
};
