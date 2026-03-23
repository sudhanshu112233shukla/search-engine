const http = require('http');
const path = require('path');
const { readJson, fileExists } = require('../utils/fs');
const { buildIndexes, chunkingMatches } = require('../indexing/build');
const { hybridSearch } = require('../retrieval/search');
const { classifyQuery } = require('../query/understand');
const { FeedbackStore } = require('../feedback/store');
const { incrementalUpdate, syncPendingUrls } = require('../update/pipeline');
const { optimizeIndexes } = require('../performance/optimize');
const { LruCache } = require('../cache/lru');

async function loadIndexes(paths, config) {
  const bm25Path = path.join(paths.index, 'bm25.json');
  const vectorPath = path.join(paths.index, 'vector.json');

  if (fileExists(bm25Path) && fileExists(vectorPath)) {
    const bm25Index = await readJson(bm25Path);
    const vectorIndex = await readJson(vectorPath);
    const bm25Ok = chunkingMatches(bm25Index.chunking, config.chunking);
    const vectorOk = chunkingMatches(vectorIndex.chunking, config.chunking);
    if (bm25Ok && vectorOk) {
      return { bm25Index, vectorIndex };
    }
  }

  return await buildIndexes(paths, config);
}

function parseJsonBody(req) {
  return new Promise((resolve, reject) => {
    let body = '';
    req.on('data', (chunk) => {
      body += chunk.toString();
      if (body.length > 5 * 1024 * 1024) {
        reject(new Error('Payload too large'));
        req.destroy();
      }
    });
    req.on('end', () => {
      if (!body) return resolve({});
      try {
        resolve(JSON.parse(body));
      } catch (err) {
        reject(err);
      }
    });
  });
}

async function createServer(config) {
  let { bm25Index, vectorIndex } = await loadIndexes(config.paths, config);
  ({ bm25Index, vectorIndex } = optimizeIndexes(bm25Index, vectorIndex));
  const feedbackStore = new FeedbackStore(config.feedback.path);
  await feedbackStore.load();

  const cache = config.cache && config.cache.enabled ? new LruCache(config.cache) : null;
  let indexVersion = 1;

  const server = http.createServer(async (req, res) => {
    if (req.method === 'POST' && req.url === '/search') {
      try {
        const body = await parseJsonBody(req);
        const query = (body.query || '').toString().trim();
        if (!query) {
          res.writeHead(400, { 'Content-Type': 'application/json' });
          res.end(JSON.stringify({ error: 'Missing query' }));
          return;
        }

        const cacheKey = query.toLowerCase();
        if (cache) {
          const cached = cache.get(cacheKey);
          if (cached && cached.version === indexVersion) {
            res.writeHead(200, { 'Content-Type': 'application/json' });
            res.end(JSON.stringify(cached.result));
            return;
          }
        }

        const queryType = classifyQuery(query);
        const result = hybridSearch(bm25Index, vectorIndex, query, queryType, feedbackStore, config);

        if (cache) {
          cache.set(cacheKey, { version: indexVersion, result });
        }

        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify(result));
      } catch (err) {
        res.writeHead(400, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ error: err.message }));
      }
      return;
    }

    if (req.method === 'POST' && req.url === '/feedback') {
      try {
        const body = await parseJsonBody(req);
        const query = (body.query || '').toString();
        const docId = (body.docId || body.id || body.resultId || '').toString();
        if (!docId) {
          res.writeHead(400, { 'Content-Type': 'application/json' });
          res.end(JSON.stringify({ error: 'Missing docId' }));
          return;
        }
        await feedbackStore.recordClick(query, docId);
        if (cache) cache.clear();
        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ status: 'ok' }));
      } catch (err) {
        res.writeHead(400, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ error: err.message }));
      }
      return;
    }

    if (req.method === 'POST' && req.url === '/update') {
      try {
        const body = await parseJsonBody(req);
        const summary = await incrementalUpdate({
          paths: config.paths,
          config,
          externalPaths: body.paths || [],
          wikipediaPath: body.wikipediaPath || null,
          urls: body.urls || [],
          documents: body.documents || [],
          bm25Index,
          vectorIndex,
        });
        indexVersion += 1;
        if (cache) cache.clear();
        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ status: 'ok', summary }));
      } catch (err) {
        res.writeHead(400, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ error: err.message }));
      }
      return;
    }

    if (req.method === 'POST' && req.url === '/sync') {
      try {
        const summary = await syncPendingUrls({
          paths: config.paths,
          config,
          bm25Index,
          vectorIndex,
        });
        if (summary.synced > 0) {
          indexVersion += 1;
          if (cache) cache.clear();
        }
        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ status: 'ok', summary }));
      } catch (err) {
        res.writeHead(400, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ error: err.message }));
      }
      return;
    }

    res.writeHead(404, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({ error: 'Not found' }));
  });

  return server;
}

module.exports = {
  createServer,
};
