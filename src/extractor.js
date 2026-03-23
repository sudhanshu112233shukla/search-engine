const { tokenize } = require('./bm25');

function splitSentences(text) {
  const matches = text.match(/[^.!?]+[.!?]+|[^.!?]+$/g);
  return matches ? matches.map((s) => s.trim()).filter(Boolean) : [];
}

function overlapScore(sentence, queryTokens) {
  if (queryTokens.length === 0) return 0;
  const sentTokens = new Set(tokenize(sentence));
  let overlap = 0;
  for (const t of queryTokens) {
    if (sentTokens.has(t)) overlap += 1;
  }
  return overlap / queryTokens.length;
}

function extractBestSentence(query, rankedResults) {
  const queryTokens = tokenize(query);
  let best = { sentence: '', score: 0, sourceId: null };

  for (const result of rankedResults) {
    const sentences = splitSentences(result.text || '');
    for (const s of sentences) {
      const overlap = overlapScore(s, queryTokens);
      const score = 0.6 * result.score + 0.4 * overlap;
      if (score > best.score) {
        best = { sentence: s, score, sourceId: result.id };
      }
    }
  }

  return best;
}

module.exports = { extractBestSentence, splitSentences, overlapScore };
