const { normalizeText, tokenize, removeStopwords } = require('../utils/text');

function minMaxNormalize(scores) {
  const values = Array.from(scores.values());
  if (values.length === 0) return new Map();
  let min = Infinity;
  let max = -Infinity;
  for (const v of values) {
    if (v < min) min = v;
    if (v > max) max = v;
  }
  const range = max - min;
  const normalized = new Map();
  for (const [id, v] of scores.entries()) {
    if (range === 0) {
      normalized.set(id, v > 0 ? 1 : 0);
    } else {
      normalized.set(id, (v - min) / range);
    }
  }
  return normalized;
}

function exactMatchScore(text, query) {
  const normalizedText = normalizeText(text);
  const normalizedQuery = normalizeText(query);
  if (!normalizedQuery) return 0;
  return normalizedText.includes(normalizedQuery) ? 1 : 0;
}

function phraseMatchScore(text, query) {
  const queryTokens = removeStopwords(tokenize(query));
  if (queryTokens.length === 0) return 0;
  const textTokens = removeStopwords(tokenize(text));
  if (textTokens.length < queryTokens.length) return 0;

  for (let i = 0; i <= textTokens.length - queryTokens.length; i += 1) {
    let match = true;
    for (let j = 0; j < queryTokens.length; j += 1) {
      if (textTokens[i + j] !== queryTokens[j]) {
        match = false;
        break;
      }
    }
    if (match) return 1;
  }
  return 0;
}

function freshnessScore(meta) {
  if (!meta) return 0;
  const candidates = [
    meta.date,
    meta.updatedAt,
    meta.createdAt,
    meta.publishedAt,
    meta.timestamp,
    meta.ingestedAt,
    meta.fetchedAt,
  ];
  let parsed = null;
  for (const c of candidates) {
    if (!c) continue;
    const dt = new Date(c);
    if (!Number.isNaN(dt.valueOf())) {
      parsed = dt;
      break;
    }
  }
  if (!parsed) return 0;
  const ageMs = Date.now() - parsed.getTime();
  if (ageMs < 0) return 1;
  const ageDays = ageMs / (1000 * 60 * 60 * 24);
  return 1 / (1 + ageDays / 30);
}

function buildCandidateMap(bm25Results, vectorResults) {
  const candidates = new Map();
  for (const r of bm25Results) {
    if (!candidates.has(r.id)) candidates.set(r.id, { bm25: 0, semantic: 0 });
    candidates.get(r.id).bm25 = r.score;
  }
  for (const r of vectorResults) {
    if (!candidates.has(r.id)) candidates.set(r.id, { bm25: 0, semantic: 0 });
    candidates.get(r.id).semantic = r.score;
  }
  return candidates;
}

function resolveWeights(config, queryType) {
  const base = config.ranking.weights || {};
  const profile = (config.ranking.profiles && config.ranking.profiles[queryType]) || {};
  return {
    bm25: profile.bm25 ?? base.bm25 ?? 1,
    semantic: profile.semantic ?? base.semantic ?? 1,
    exact: profile.exact ?? base.exact ?? 1,
    phrase: profile.phrase ?? base.phrase ?? 1,
    freshness: profile.freshness ?? base.freshness ?? 1,
    feedback: profile.feedback ?? base.feedback ?? 1,
  };
}

function rankCandidates({
  query,
  queryType,
  bm25Results,
  vectorResults,
  bm25Index,
  vectorIndex,
  feedbackStore,
  config,
}) {
  const candidateScores = buildCandidateMap(bm25Results, vectorResults);
  const bm25Map = new Map();
  const semanticMap = new Map();
  for (const [id, scores] of candidateScores.entries()) {
    bm25Map.set(id, scores.bm25 || 0);
    semanticMap.set(id, scores.semantic || 0);
  }

  const bm25Norm = minMaxNormalize(bm25Map);
  const semanticNorm = minMaxNormalize(semanticMap);
  const weights = resolveWeights(config, queryType);

  const ranked = [];
  for (const [id] of candidateScores.entries()) {
    const doc = bm25Index.docs[id] || vectorIndex.docs[id] || { id };
    const exact = exactMatchScore(doc.text || '', query);
    const phrase = phraseMatchScore(doc.text || '', query);
    const fresh = freshnessScore(doc.meta || {});
    const feedback = feedbackStore ? feedbackStore.getBoost(id, query) : 0;

    const score =
      (bm25Norm.get(id) || 0) * weights.bm25 +
      (semanticNorm.get(id) || 0) * weights.semantic +
      exact * weights.exact +
      phrase * weights.phrase +
      fresh * weights.freshness +
      feedback * weights.feedback;

    ranked.push({
      id,
      score,
      text: doc.text || '',
      sourceId: doc.sourceId,
      chunkIndex: doc.chunkIndex,
      totalChunks: doc.totalChunks,
      meta: doc.meta || {},
      signals: {
        bm25: bm25Norm.get(id) || 0,
        semantic: semanticNorm.get(id) || 0,
        exact,
        phrase,
        freshness: fresh,
        feedback,
      },
    });
  }

  ranked.sort((a, b) => b.score - a.score);
  return ranked;
}

module.exports = {
  rankCandidates,
  exactMatchScore,
  phraseMatchScore,
  freshnessScore,
};
