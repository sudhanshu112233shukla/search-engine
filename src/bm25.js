const STOPWORDS = new Set([
  'a','about','above','after','again','against','all','am','an','and','any','are','as','at',
  'be','because','been','before','being','below','between','both','but','by','can','could',
  'did','do','does','doing','down','during','each','few','for','from','further','had','has','have','having','he','her','here',
  'hers','herself','him','himself','his','how','i','if','in','into','is','it','its','itself','just','me','more','most','my','myself',
  'no','nor','not','now','of','off','on','once','only','or','other','our','ours','ourselves','out','over','own','same','she','should','so','some','such',
  'than','that','the','their','theirs','them','themselves','then','there','these','they','this','those','through','to','too','under','until','up','very','was','we','were','what','when','where','which','while','who','whom','why','with','you','your','yours','yourself','yourselves'
]);

function normalize(text) {
  return text
    .toLowerCase()
    .replace(/[^a-z0-9\s]/g, ' ')
    .replace(/\s+/g, ' ')
    .trim();
}

function tokenize(text) {
  if (!text) return [];
  return normalize(text)
    .split(' ')
    .filter((t) => t && !STOPWORDS.has(t));
}

class BM25 {
  constructor(docs, options = {}) {
    this.k1 = options.k1 ?? 1.5;
    this.b = options.b ?? 0.75;
    this.docs = docs;
    this.docLengths = [];
    this.avgDocLength = 0;
    this.inverted = new Map();
    this._build();
  }

  _build() {
    let totalLen = 0;
    this.docs.forEach((doc, docId) => {
      const tokens = tokenize(doc.text);
      this.docLengths[docId] = tokens.length;
      totalLen += tokens.length;

      const tf = new Map();
      tokens.forEach((t) => tf.set(t, (tf.get(t) || 0) + 1));

      for (const [term, freq] of tf.entries()) {
        if (!this.inverted.has(term)) {
          this.inverted.set(term, { df: 0, postings: [] });
        }
        const entry = this.inverted.get(term);
        entry.df += 1;
        entry.postings.push([docId, freq]);
      }
    });

    this.avgDocLength = this.docs.length > 0 ? totalLen / this.docs.length : 0;
  }

  search(query, topK = 50) {
    const tokens = tokenize(query);
    if (tokens.length === 0) return [];

    const N = this.docs.length;
    const scores = new Array(N).fill(0);

    for (const term of tokens) {
      const entry = this.inverted.get(term);
      if (!entry) continue;
      const df = entry.df;
      const idf = Math.log(1 + (N - df + 0.5) / (df + 0.5));

      for (const [docId, tf] of entry.postings) {
        const dl = this.docLengths[docId] || 0;
        const numerator = tf * (this.k1 + 1);
        const denominator = tf + this.k1 * (1 - this.b + this.b * (dl / (this.avgDocLength || 1)));
        const score = idf * (numerator / (denominator || 1));
        scores[docId] += score;
      }
    }

    const results = scores
      .map((score, i) => ({ id: this.docs[i].id, text: this.docs[i].text, score }))
      .filter((r) => r.score > 0)
      .sort((a, b) => b.score - a.score)
      .slice(0, topK);

    return results;
  }
}

module.exports = { BM25, tokenize };
