const { normalizeText } = require('../utils/text');

function fnv1aHash(str) {
  let hash = 2166136261;
  for (let i = 0; i < str.length; i += 1) {
    hash ^= str.charCodeAt(i);
    hash = (hash * 16777619) >>> 0;
  }
  return hash >>> 0;
}

function embedText(text, options) {
  const dims = options.dims || 384;
  const ngramMin = options.ngramMin || 3;
  const ngramMax = options.ngramMax || 5;

  const normalized = normalizeText(text);
  const padded = ` ${normalized} `;
  const vector = new Array(dims).fill(0);

  for (let n = ngramMin; n <= ngramMax; n += 1) {
    for (let i = 0; i <= padded.length - n; i += 1) {
      const gram = padded.slice(i, i + n);
      const idx = fnv1aHash(gram) % dims;
      vector[idx] += 1;
    }
  }

  let norm = 0;
  for (let i = 0; i < vector.length; i += 1) {
    norm += vector[i] * vector[i];
  }
  norm = Math.sqrt(norm) || 1;
  for (let i = 0; i < vector.length; i += 1) {
    vector[i] = vector[i] / norm;
  }

  return vector;
}

module.exports = {
  embedText,
};
