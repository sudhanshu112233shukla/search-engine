const fs = require('fs');
const path = require('path');
const readline = require('readline');
const { ensureDir, readJson, writeJson, fileExists } = require('../utils/fs');
const { ingestFromPath } = require('../ingestion/ingest');
const { processDocumentsToChunks } = require('../processing/processor');
const { buildIndexes } = require('../indexing/build');
const { addToBm25Index } = require('../indexing/bm25');
const { addToVectorIndex } = require('../indexing/vector');
const { fetchAndClean } = require('../crawler/fetch');

function slugify(input) {
  return input.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '').slice(0, 80);
}

function hashString(input) {
  let hash = 0;
  for (let i = 0; i < input.length; i += 1) {
    hash = (hash * 31 + input.charCodeAt(i)) >>> 0;
  }
  return hash.toString(16);
}

async function parseWikipediaJsonl(filePath) {
  const docs = [];
  const stream = fs.createReadStream(filePath, { encoding: 'utf8' });
  const rl = readline.createInterface({ input: stream, crlfDelay: Infinity });

  for await (const line of rl) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    try {
      const obj = JSON.parse(trimmed);
      const text = obj.text || obj.content || '';
      if (!text) continue;
      const title = obj.title || obj.name || '';
      const id = obj.id || `wiki:${slugify(title) || hashString(text)}`;
      docs.push({
        id,
        text: String(text),
        meta: {
          source: 'wikipedia',
          title,
        },
      });
    } catch {
      continue;
    }
  }

  return docs;
}

async function fetchUrls(urls, options) {
  const docs = [];
  const failed = [];
  for (const url of urls) {
    try {
      const result = await fetchAndClean(url, options);
      if (!result.text) {
        failed.push(url);
        continue;
      }
      const id = `url:${hashString(url)}`;
      docs.push({
        id,
        text: result.text,
        meta: {
          source: url,
          fetchedAt: new Date().toISOString(),
        },
      });
    } catch {
      failed.push(url);
    }
  }
  return { docs, failed };
}

async function persistDocs(docs, rawDir, prefix) {
  if (!docs.length) return;
  await ensureDir(rawDir);
  const fileName = `${prefix}-${Date.now()}.json`;
  const filePath = path.join(rawDir, fileName);
  await writeJson(filePath, docs);
}

async function loadOrInitIndexes(paths, config) {
  const bm25Path = path.join(paths.index, 'bm25.json');
  const vectorPath = path.join(paths.index, 'vector.json');
  if (fileExists(bm25Path) && fileExists(vectorPath)) {
    const bm25Index = await readJson(bm25Path);
    const vectorIndex = await readJson(vectorPath);
    return { bm25Index, vectorIndex };
  }
  return await buildIndexes(paths, config);
}

async function appendChunks(processedDir, chunking, newChunks) {
  await ensureDir(processedDir);
  const chunksPath = path.join(processedDir, 'chunks.json');
  if (fileExists(chunksPath)) {
    const data = await readJson(chunksPath);
    if (data.chunking
      && (data.chunking.minWords !== chunking.minWords
        || data.chunking.maxWords !== chunking.maxWords
        || data.chunking.targetWords !== chunking.targetWords)) {
      throw new Error('Chunking configuration mismatch; rebuild required');
    }
    data.chunks = (data.chunks || []).concat(newChunks);
    data.generatedAt = new Date().toISOString();
    await writeJson(chunksPath, data);
  } else {
    await writeJson(chunksPath, { generatedAt: new Date().toISOString(), chunking, chunks: newChunks });
  }
}

async function loadPendingUrls(pendingPath) {
  if (!pendingPath || !fileExists(pendingPath)) return [];
  const data = await readJson(pendingPath);
  return Array.isArray(data) ? data : [];
}

async function savePendingUrls(pendingPath, urls) {
  if (!pendingPath) return;
  await ensureDir(path.dirname(pendingPath));
  const unique = Array.from(new Set(urls));
  await writeJson(pendingPath, unique);
}

async function enqueuePendingUrls(pendingPath, urls) {
  if (!urls || urls.length === 0) return;
  const existing = await loadPendingUrls(pendingPath);
  await savePendingUrls(pendingPath, existing.concat(urls));
}

async function incrementalUpdate(options) {
  const {
    paths,
    config,
    externalPaths = [],
    wikipediaPath = null,
    urls = [],
    documents = [],
    bm25Index: providedBm25 = null,
    vectorIndex: providedVector = null,
  } = options;

  let docs = [];
  let queuedUrls = 0;

  for (const p of externalPaths) {
    const ingested = await ingestFromPath(p, paths.raw);
    docs.push(...ingested);
  }

  if (wikipediaPath) {
    const wikiDocs = await parseWikipediaJsonl(wikipediaPath);
    docs.push(...wikiDocs);
  }

  if (urls && urls.length > 0) {
    const fetched = await fetchUrls(urls, config.crawler || {});
    docs.push(...fetched.docs);
    queuedUrls = fetched.failed.length;
    await enqueuePendingUrls(config.update.pendingUrlsPath, fetched.failed);
  }

  if (documents && documents.length > 0) {
    docs.push(...documents);
  }

  const seen = new Set();
  const uniqueDocs = [];
  for (const doc of docs) {
    if (!doc || !doc.id || !doc.text) continue;
    if (seen.has(doc.id)) continue;
    seen.add(doc.id);
    const meta = doc.meta || {};
    if (!meta.ingestedAt) meta.ingestedAt = new Date().toISOString();
    uniqueDocs.push({ id: doc.id, text: doc.text, meta });
  }

  if (uniqueDocs.length === 0) {
    return { addedDocs: 0, addedChunks: 0, queuedUrls };
  }

  await persistDocs(uniqueDocs, paths.raw, 'update');
  const newChunks = processDocumentsToChunks(uniqueDocs, config.chunking);
  await appendChunks(paths.processed, config.chunking, newChunks);

  let bm25Index = providedBm25;
  let vectorIndex = providedVector;
  if (!bm25Index || !vectorIndex) {
    const loaded = await loadOrInitIndexes(paths, config);
    bm25Index = loaded.bm25Index;
    vectorIndex = loaded.vectorIndex;
  }

  const addedBm25 = addToBm25Index(bm25Index, newChunks);
  const addedVector = addToVectorIndex(vectorIndex, newChunks);

  const updatedAt = new Date().toISOString();
  bm25Index.chunking = config.chunking;
  vectorIndex.chunking = config.chunking;
  bm25Index.updatedAt = updatedAt;
  vectorIndex.updatedAt = updatedAt;

  await writeJson(path.join(paths.index, 'bm25.json'), bm25Index);
  await writeJson(path.join(paths.index, 'vector.json'), vectorIndex);

  return {
    addedDocs: uniqueDocs.length,
    addedChunks: newChunks.length,
    bm25Added: addedBm25,
    vectorAdded: addedVector,
    queuedUrls,
  };
}

async function syncPendingUrls(options) {
  const { paths, config, bm25Index, vectorIndex } = options;
  const pending = await loadPendingUrls(config.update.pendingUrlsPath);
  if (pending.length === 0) {
    return { synced: 0, remaining: 0 };
  }

  const fetched = await fetchUrls(pending, config.crawler || {});
  if (fetched.docs.length > 0) {
    await incrementalUpdate({
      paths,
      config,
      documents: fetched.docs,
      bm25Index,
      vectorIndex,
    });
  }

  await savePendingUrls(config.update.pendingUrlsPath, fetched.failed);
  return {
    synced: fetched.docs.length,
    remaining: fetched.failed.length,
  };
}

module.exports = {
  incrementalUpdate,
  parseWikipediaJsonl,
  fetchUrls,
  syncPendingUrls,
};
