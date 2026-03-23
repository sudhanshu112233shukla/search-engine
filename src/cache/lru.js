class LruCache {
  constructor(options = {}) {
    this.maxEntries = options.maxEntries || 500;
    this.ttlMs = options.ttlMs ?? 60000;
    this.map = new Map();
  }

  get(key) {
    if (!this.map.has(key)) return null;
    const entry = this.map.get(key);
    if (entry.expiresAt && entry.expiresAt <= Date.now()) {
      this.map.delete(key);
      return null;
    }
    this.map.delete(key);
    this.map.set(key, entry);
    return entry.value;
  }

  set(key, value) {
    if (this.map.has(key)) this.map.delete(key);
    const expiresAt = this.ttlMs > 0 ? Date.now() + this.ttlMs : null;
    this.map.set(key, { value, expiresAt });
    if (this.map.size > this.maxEntries) {
      const oldestKey = this.map.keys().next().value;
      this.map.delete(oldestKey);
    }
  }

  clear() {
    this.map.clear();
  }
}

module.exports = {
  LruCache,
};
