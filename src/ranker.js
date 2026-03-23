function minMaxNormalize(results) {
  if (results.length === 0) return new Map();
  let min = Infinity;
  let max = -Infinity;
  for (const r of results) {
    if (r.score < min) min = r.score;
    if (r.score > max) max = r.score;
  }
  const range = max - min;
  const out = new Map();
  for (const r of results) {
    const norm = range === 0 ? (r.score > 0 ? 1 : 0) : (r.score - min) / range;
    out.set(r.id, norm);
  }
  return out;
}

function hybridRank(bm25Results, vectorResults, weights = { bm25: 0.7, vector: 0.3 }) {
  const bm25Norm = minMaxNormalize(bm25Results);
  const vectorNorm = minMaxNormalize(vectorResults);

  const combined = new Map();
  for (const [id, score] of bm25Norm.entries()) {
    combined.set(id, (combined.get(id) || 0) + score * weights.bm25);
  }
  for (const [id, score] of vectorNorm.entries()) {
    combined.set(id, (combined.get(id) || 0) + score * weights.vector);
  }

  const resultList = Array.from(combined.entries()).map(([id, score]) => ({ id, score }));
  resultList.sort((a, b) => b.score - a.score);
  return resultList;
}

module.exports = { hybridRank };
