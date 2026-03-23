const { hydrateVectorIndex } = require('../indexing/vector');

function optimizeIndexes(bm25Index, vectorIndex) {
  hydrateVectorIndex(vectorIndex);
  return { bm25Index, vectorIndex };
}

module.exports = {
  optimizeIndexes,
};
