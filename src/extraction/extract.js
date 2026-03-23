const { normalizeText, tokenize, removeStopwords } = require('../utils/text');
const { computeConfidence } = require('../confidence/confidence');

function splitSentences(text) {
  if (!text) return [];
  const matches = text.match(/[^.!?]+[.!?]+|[^.!?]+$/g);
  if (!matches) return [];
  return matches.map((s) => s.trim()).filter(Boolean);
}

function sentenceOverlapScore(sentence, queryTokens) {
  if (queryTokens.length === 0) return 0;
  const tokens = new Set(removeStopwords(tokenize(sentence)));
  let overlap = 0;
  for (const t of queryTokens) {
    if (tokens.has(t)) overlap += 1;
  }
  return overlap / queryTokens.length;
}

function extractShortestSpan(sentence, queryTokens) {
  if (queryTokens.length === 0) return sentence.trim();

  const words = sentence.split(/\s+/).filter(Boolean);
  const norms = words.map((w) => normalizeText(w));
  const needed = new Set(queryTokens);
  const missing = new Set();

  for (const token of needed) {
    if (!norms.includes(token)) missing.add(token);
  }
  if (missing.size > 0) {
    return sentence.trim();
  }

  const counts = new Map();
  let have = 0;
  let left = 0;
  let best = { len: Infinity, start: 0, end: 0 };

  for (let right = 0; right < norms.length; right += 1) {
    const word = norms[right];
    if (needed.has(word)) {
      const prev = counts.get(word) || 0;
      counts.set(word, prev + 1);
      if (prev === 0) have += 1;
    }

    while (have === needed.size && left <= right) {
      const windowLen = right - left + 1;
      if (windowLen < best.len) {
        best = { len: windowLen, start: left, end: right };
      }
      const leftWord = norms[left];
      if (needed.has(leftWord)) {
        const prev = counts.get(leftWord) || 0;
        counts.set(leftWord, prev - 1);
        if (prev - 1 === 0) have -= 1;
      }
      left += 1;
    }
  }

  if (!isFinite(best.len)) {
    return sentence.trim();
  }

  return words.slice(best.start, best.end + 1).join(' ').trim();
}

function findBestSentence(passage, queryTokens) {
  const sentences = splitSentences(passage);
  if (sentences.length === 0) return { sentence: '', score: 0, span: '' };

  let best = { sentence: sentences[0], score: 0, span: sentences[0] };
  for (const sentence of sentences) {
    const score = sentenceOverlapScore(sentence, queryTokens);
    const span = extractShortestSpan(sentence, queryTokens);
    const spanLen = span.split(/\s+/).filter(Boolean).length;
    const bestSpanLen = best.span.split(/\s+/).filter(Boolean).length;

    if (score > best.score || (score === best.score && spanLen < bestSpanLen)) {
      best = { sentence, score, span };
    }
  }

  return best;
}

function extractAnswer(query, passages) {
  const queryTokens = removeStopwords(tokenize(query));
  let best = {
    answer: '',
    source: null,
    confidence: 0,
  };

  for (const passage of passages) {
    const passageText = passage.text || '';
    const bestSentence = findBestSentence(passageText, queryTokens);
    const answer = bestSentence.span || bestSentence.sentence || passageText;
    const retrievalScore = passage.score || 0;
    const confidence = computeConfidence(query, answer, retrievalScore);

    if (confidence > best.confidence) {
      best = {
        answer,
        source: {
          id: passage.id,
          sourceId: passage.sourceId,
          chunkIndex: passage.chunkIndex,
          totalChunks: passage.totalChunks,
          meta: passage.meta || {},
        },
        confidence,
      };
    }
  }

  return best;
}

module.exports = {
  splitSentences,
  sentenceOverlapScore,
  extractShortestSpan,
  findBestSentence,
  extractAnswer,
};
