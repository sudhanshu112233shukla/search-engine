const fsp = require('fs/promises');
const path = require('path');
const { ensureDir, writeJson } = require('../utils/fs');

async function listRawFiles(rawDir) {
  const entries = await fsp.readdir(rawDir, { withFileTypes: true });
  return entries.filter((e) => e.isFile()).map((e) => path.join(rawDir, e.name));
}

function extractDocsFromJson(jsonData, sourceName) {
  const docs = [];
  if (Array.isArray(jsonData)) {
    for (let i = 0; i < jsonData.length; i += 1) {
      const item = jsonData[i];
      if (!item) continue;
      const text = item.text || item.content || item.body || '';
      if (!text) continue;
      docs.push({
        id: item.id || `${sourceName}#${i}`,
        text: String(text),
        meta: item.meta || item.metadata || {},
      });
    }
  } else if (typeof jsonData === 'object') {
    if (jsonData.text || jsonData.content || jsonData.body) {
      docs.push({
        id: jsonData.id || sourceName,
        text: String(jsonData.text || jsonData.content || jsonData.body),
        meta: jsonData.meta || jsonData.metadata || {},
      });
    } else {
      for (const [key, value] of Object.entries(jsonData)) {
        if (!value) continue;
        if (typeof value === 'string') {
          docs.push({ id: key, text: value, meta: { source: sourceName } });
        } else if (typeof value === 'object') {
          const text = value.text || value.content || value.body;
          if (text) {
            docs.push({
              id: value.id || key,
              text: String(text),
              meta: value.meta || value.metadata || { source: sourceName },
            });
          }
        }
      }
    }
  }
  return docs;
}

async function loadRawDocuments(rawDir) {
  const files = await listRawFiles(rawDir);
  const docs = [];

  for (const filePath of files) {
    const ext = path.extname(filePath).toLowerCase();
    const baseName = path.basename(filePath);
    if (ext === '.txt') {
      const text = await fsp.readFile(filePath, 'utf8');
      docs.push({ id: baseName, text, meta: { source: baseName } });
    } else if (ext === '.json') {
      const data = JSON.parse(await fsp.readFile(filePath, 'utf8'));
      docs.push(...extractDocsFromJson(data, baseName));
    }
  }

  return docs;
}

function chunkWords(words, chunking) {
  const minWords = chunking.minWords || 100;
  const maxWords = chunking.maxWords || 200;
  const targetWords = chunking.targetWords || 150;

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

function chunkDocument(doc, chunking) {
  const words = doc.text.split(/\s+/).filter(Boolean);
  if (words.length === 0) return [];

  const wordChunks = chunkWords(words, chunking);
  return wordChunks.map((chunkWordsArr, idx) => {
    const text = chunkWordsArr.join(' ');
    return {
      id: `${doc.id}::chunk${idx + 1}`,
      sourceId: doc.id,
      chunkIndex: idx + 1,
      totalChunks: wordChunks.length,
      text,
      meta: doc.meta || {},
    };
  });
}

function processDocumentsToChunks(docs, chunking) {
  const chunks = [];
  for (const doc of docs) {
    const docChunks = chunkDocument(doc, chunking);
    chunks.push(...docChunks);
  }
  return chunks;
}

async function processRawToChunks(rawDir, processedDir, chunking) {
  await ensureDir(processedDir);
  const docs = await loadRawDocuments(rawDir);
  const chunks = processDocumentsToChunks(docs, chunking);
  const outputPath = path.join(processedDir, 'chunks.json');
  await writeJson(outputPath, { generatedAt: new Date().toISOString(), chunking, chunks });
  return chunks;
}

module.exports = {
  loadRawDocuments,
  processRawToChunks,
  processDocumentsToChunks,
  chunkDocument,
};
