const fs = require('fs');
const fsp = require('fs/promises');
const path = require('path');
const { ensureDir } = require('../utils/fs');

class FeedbackStore {
  constructor(filePath) {
    this.filePath = filePath;
    this.data = {
      updatedAt: new Date().toISOString(),
      global: {},
      queries: {},
    };
  }

  async load() {
    if (fs.existsSync(this.filePath)) {
      const raw = await fsp.readFile(this.filePath, 'utf8');
      this.data = JSON.parse(raw);
    } else {
      await ensureDir(path.dirname(this.filePath));
      await this.save();
    }
  }

  async save() {
    this.data.updatedAt = new Date().toISOString();
    const payload = JSON.stringify(this.data, null, 2);
    await fsp.writeFile(this.filePath, payload, 'utf8');
  }

  async recordClick(query, docId) {
    if (!docId) return;
    const q = (query || '').toLowerCase().trim();
    this.data.global[docId] = (this.data.global[docId] || 0) + 1;
    if (q) {
      if (!this.data.queries[q]) this.data.queries[q] = {};
      this.data.queries[q][docId] = (this.data.queries[q][docId] || 0) + 1;
    }
    await this.save();
  }

  getBoost(docId, query) {
    const q = (query || '').toLowerCase().trim();
    const globalClicks = this.data.global[docId] || 0;
    const queryClicks = q && this.data.queries[q] ? (this.data.queries[q][docId] || 0) : 0;

    const weighted = globalClicks + 2 * queryClicks;
    if (weighted <= 0) return 0;
    return Math.log1p(weighted) / 5;
  }
}

module.exports = {
  FeedbackStore,
};
