const fsp = require('fs/promises');
const path = require('path');
const readline = require('readline');
const fs = require('fs');
const { ensureDir } = require('../utils/fs');

async function listFiles(inputPath) {
  const stat = await fsp.stat(inputPath);
  if (stat.isFile()) return [inputPath];
  const entries = await fsp.readdir(inputPath, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const fullPath = path.join(inputPath, entry.name);
    if (entry.isFile()) files.push(fullPath);
  }
  return files;
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

async function extractDocsFromJsonl(filePath, sourceName) {
  const docs = [];
  const stream = fs.createReadStream(filePath, { encoding: 'utf8' });
  const rl = readline.createInterface({ input: stream, crlfDelay: Infinity });
  let i = 0;
  for await (const line of rl) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    try {
      const obj = JSON.parse(trimmed);
      const text = obj.text || obj.content || obj.body || '';
      if (!text) continue;
      docs.push({
        id: obj.id || `${sourceName}#${i}`,
        text: String(text),
        meta: obj.meta || obj.metadata || {},
      });
      i += 1;
    } catch {
      continue;
    }
  }
  return docs;
}

async function ingestFromPath(inputPath, rawDir) {
  await ensureDir(rawDir);
  const files = await listFiles(inputPath);
  const ingestedDocs = [];

  for (const filePath of files) {
    const ext = path.extname(filePath).toLowerCase();
    if (ext !== '.txt' && ext !== '.json' && ext !== '.jsonl') continue;

    const baseName = path.basename(filePath);
    const destPath = path.join(rawDir, baseName);
    await fsp.copyFile(filePath, destPath);

    if (ext === '.txt') {
      const text = await fsp.readFile(filePath, 'utf8');
      ingestedDocs.push({ id: baseName, text, meta: { source: baseName } });
    }

    if (ext === '.json') {
      const data = JSON.parse(await fsp.readFile(filePath, 'utf8'));
      const docs = extractDocsFromJson(data, baseName);
      ingestedDocs.push(...docs);
    }

    if (ext === '.jsonl') {
      const docs = await extractDocsFromJsonl(filePath, baseName);
      ingestedDocs.push(...docs);
    }
  }

  return ingestedDocs;
}

module.exports = {
  ingestFromPath,
};
