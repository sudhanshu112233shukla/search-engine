const path = require('path');
const { readJson, writeJson, fileExists } = require('../utils/fs');
const { processRawToChunks } = require('../processing/processor');
const { buildBm25Index } = require('./bm25');
const { buildVectorIndex } = require('./vector');

function chunkingMatches(stored, current) {
  if (!stored || !current) return false;
  return stored.minWords === current.minWords
    && stored.maxWords === current.maxWords
    && stored.targetWords === current.targetWords;
}

async function loadOrBuildChunks(rawDir, processedDir, chunking) {
  const chunksPath = path.join(processedDir, 'chunks.json');
  if (fileExists(chunksPath)) {
    const data = await readJson(chunksPath);
    if (chunkingMatches(data.chunking, chunking)) {
      return data.chunks || [];
    }
  }
  return await processRawToChunks(rawDir, processedDir, chunking);
}

async function buildIndexes(paths, config) {
  const chunks = await loadOrBuildChunks(paths.raw, paths.processed, config.chunking);
  const bm25Index = buildBm25Index(chunks, config.bm25);
  const vectorIndex = buildVectorIndex(chunks, config.vector);
  const updatedAt = new Date().toISOString();

  bm25Index.chunking = config.chunking;
  vectorIndex.chunking = config.chunking;
  bm25Index.updatedAt = updatedAt;
  vectorIndex.updatedAt = updatedAt;

  await writeJson(path.join(paths.index, 'bm25.json'), bm25Index);
  await writeJson(path.join(paths.index, 'vector.json'), vectorIndex);

  return { bm25Index, vectorIndex };
}

module.exports = {
  buildIndexes,
  loadOrBuildChunks,
  chunkingMatches,
};
