const fs = require('fs');
const fsp = require('fs/promises');
const path = require('path');

async function ensureDir(dirPath) {
  await fsp.mkdir(dirPath, { recursive: true });
}

async function readJson(filePath) {
  const data = await fsp.readFile(filePath, 'utf8');
  return JSON.parse(data);
}

async function writeJson(filePath, obj) {
  const dir = path.dirname(filePath);
  await ensureDir(dir);
  const data = JSON.stringify(obj, null, 2);
  await fsp.writeFile(filePath, data, 'utf8');
}

function fileExists(filePath) {
  try {
    fs.accessSync(filePath, fs.constants.F_OK);
    return true;
  } catch {
    return false;
  }
}

module.exports = {
  ensureDir,
  readJson,
  writeJson,
  fileExists,
};
