const { embedText } = require('./embedding');

function quantizeVector(vec, scale) {
  const out = new Array(vec.length);
  for (let i = 0; i < vec.length; i += 1) {
    const v = Math.max(-1, Math.min(1, vec[i]));
    out[i] = Math.max(-scale, Math.min(scale, Math.round(v * scale)));
  }
  return out;
}

function toInt8Array(arr) {
  return Int8Array.from(arr);
}

function buildVectorIndex(chunks, options) {
  const vectors = {};
  const docs = {};
  const dims = options.dims || 384;
  const quantized = options.quantize !== false;
  const scale = options.quantizeScale || 127;
  const storeText = options.storeText === true;

  for (const chunk of chunks) {
    const vector = embedText(chunk.text, options);
    vectors[chunk.id] = quantized ? quantizeVector(vector, scale) : vector;
    docs[chunk.id] = {
      id: chunk.id,
      text: storeText ? chunk.text : '',
      sourceId: chunk.sourceId,
      chunkIndex: chunk.chunkIndex,
      totalChunks: chunk.totalChunks,
      meta: chunk.meta || {},
    };
  }

  return {
    type: 'vector',
    dims,
    vectors,
    docs,
    quantized,
    quantizeScale: scale,
    embedOptions: {
      dims: options.dims || 384,
      ngramMin: options.ngramMin || 3,
      ngramMax: options.ngramMax || 5,
      storeText,
    },
  };
}

function addToVectorIndex(index, chunks) {
  const options = index.embedOptions || { dims: index.dims, ngramMin: 3, ngramMax: 5 };
  if (index.dims !== options.dims) {
    throw new Error('Vector index dimension mismatch');
  }
  const quantized = index.quantized === true;
  const scale = index.quantizeScale || 127;
  const storeText = options.storeText === true;

  let added = 0;
  for (const chunk of chunks) {
    const docId = chunk.id;
    if (index.vectors[docId]) continue;
    const vector = embedText(chunk.text, options);
    const stored = quantized ? quantizeVector(vector, scale) : vector;
    index.vectors[docId] = stored;
    index.docs[docId] = {
      id: chunk.id,
      text: storeText ? chunk.text : '',
      sourceId: chunk.sourceId,
      chunkIndex: chunk.chunkIndex,
      totalChunks: chunk.totalChunks,
      meta: chunk.meta || {},
    };
    if (index.vectorIds && index.vectorList) {
      index.vectorIds.push(docId);
      index.vectorList.push(quantized ? toInt8Array(stored) : Float32Array.from(stored));
    }
    added += 1;
  }

  return added;
}

function hydrateVectorIndex(index) {
  if (!index || !index.vectors) return index;
  const ids = Object.keys(index.vectors);
  index.vectorIds = ids;
  index.vectorList = new Array(ids.length);
  const quantized = index.quantized === true;
  for (let i = 0; i < ids.length; i += 1) {
    const id = ids[i];
    const vec = index.vectors[id];
    index.vectorList[i] = quantized ? toInt8Array(vec) : Float32Array.from(vec);
  }
  return index;
}

function cosineSimilarity(vecA, vecB) {
  let dot = 0;
  for (let i = 0; i < vecA.length; i += 1) {
    dot += vecA[i] * vecB[i];
  }
  return dot;
}

function dotQuantized(vecA, vecB, scale) {
  let dot = 0;
  for (let i = 0; i < vecA.length; i += 1) {
    dot += vecA[i] * vecB[i];
  }
  return dot / (scale * scale);
}

function vectorSearch(index, query, topK) {
  const queryVec = embedText(query, index.embedOptions);
  const quantized = index.quantized === true;
  const scale = index.quantizeScale || 127;
  const ids = index.vectorIds || Object.keys(index.vectors);
  const vectors = index.vectorList || ids.map((id) => index.vectors[id]);

  const queryStored = quantized ? toInt8Array(quantizeVector(queryVec, scale)) : Float32Array.from(queryVec);
  const results = new Array(ids.length);

  for (let i = 0; i < ids.length; i += 1) {
    const vec = vectors[i];
    const score = quantized ? dotQuantized(queryStored, vec, scale) : cosineSimilarity(queryStored, vec);
    results[i] = { id: ids[i], score };
  }

  results.sort((a, b) => b.score - a.score);
  return results.slice(0, topK || 50);
}

module.exports = {
  buildVectorIndex,
  addToVectorIndex,
  hydrateVectorIndex,
  vectorSearch,
};
