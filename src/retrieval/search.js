const { bm25Search } = require('../indexing/bm25');
const { vectorSearch } = require('../indexing/vector');
const { extractAnswer } = require('../extraction/extract');
const { rankCandidates } = require('../ranking/advanced');

function hybridSearch(bm25Index, vectorIndex, query, queryType, feedbackStore, config) {
  const bm25Results = bm25Search(bm25Index, query, config.retrieval.bm25TopK);
  const vectorResults = vectorSearch(vectorIndex, query, config.retrieval.vectorTopK);

  const ranked = rankCandidates({
    query,
    queryType,
    bm25Results,
    vectorResults,
    bm25Index,
    vectorIndex,
    feedbackStore,
    config,
  });

  const topK = config.retrieval.finalTopK || 10;
  const passages = ranked.slice(0, topK);
  const answer = extractAnswer(query, passages);

  return {
    query,
    queryType,
    answer: answer.answer,
    source: answer.source,
    confidence: answer.confidence,
  };
}

module.exports = {
  hybridSearch,
};
