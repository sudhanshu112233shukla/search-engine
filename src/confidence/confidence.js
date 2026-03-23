const { tokenize, removeStopwords } = require('../utils/text');

function keywordOverlap(query, text) {
  const queryTokens = removeStopwords(tokenize(query));
  if (queryTokens.length === 0) return 0;
  const textTokens = new Set(removeStopwords(tokenize(text)));
  let overlap = 0;
  for (const t of queryTokens) {
    if (textTokens.has(t)) overlap += 1;
  }
  return overlap / queryTokens.length;
}

function computeConfidence(query, answerText, retrievalScore) {
  const overlap = keywordOverlap(query, answerText);
  return retrievalScore + overlap;
}

module.exports = {
  keywordOverlap,
  computeConfidence,
};
