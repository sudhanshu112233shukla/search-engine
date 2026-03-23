const DEFAULT_STOPWORDS = new Set([
  'a','about','above','after','again','against','all','am','an','and','any','are','as','at',
  'be','because','been','before','being','below','between','both','but','by',
  'could','did','do','does','doing','down','during','each','few','for','from','further',
  'had','has','have','having','he','her','here','hers','herself','him','himself','his',
  'how','i','if','in','into','is','it','its','itself',
  'just','me','more','most','my','myself','no','nor','not','now','of','off','on','once','only','or','other','our','ours','ourselves',
  'out','over','own','same','she','should','so','some','such','than','that','the','their','theirs','them','themselves','then','there','these','they','this','those','through','to','too','under','until','up','very','was','we','were','what','when','where','which','while','who','whom','why','with','you','your','yours','yourself','yourselves'
]);

function normalizeText(text) {
  if (!text) return '';
  const lowered = text.toLowerCase();
  const cleaned = lowered.replace(/[^a-z0-9\s]/g, ' ');
  return cleaned.replace(/\s+/g, ' ').trim();
}

function tokenize(text) {
  const cleaned = normalizeText(text);
  if (!cleaned) return [];
  return cleaned.split(' ').filter(Boolean);
}

function removeStopwords(tokens, stopwords = DEFAULT_STOPWORDS) {
  return tokens.filter((t) => !stopwords.has(t));
}

function chunkWords(words, options) {
  const minWords = options.minWords || 200;
  const maxWords = options.maxWords || 500;
  const targetWords = options.targetWords || 300;

  const chunks = [];
  let i = 0;
  while (i < words.length) {
    const remaining = words.length - i;
    if (remaining <= maxWords) {
      if (remaining < minWords && chunks.length > 0) {
        chunks[chunks.length - 1] = chunks[chunks.length - 1].concat(words.slice(i));
      } else {
        chunks.push(words.slice(i));
      }
      break;
    }

    const size = Math.min(targetWords, maxWords);
    chunks.push(words.slice(i, i + size));
    i += size;
  }
  return chunks;
}

module.exports = {
  DEFAULT_STOPWORDS,
  normalizeText,
  tokenize,
  removeStopwords,
  chunkWords,
};
