const https = require('https');
const http = require('http');

function fetchUrl(url, options = {}) {
  const timeoutMs = options.timeoutMs || 10000;
  const userAgent = options.userAgent || 'MiniCrawler/1.0';

  return new Promise((resolve, reject) => {
    const lib = url.startsWith('https') ? https : http;
    const req = lib.get(url, { headers: { 'User-Agent': userAgent } }, (res) => {
      let data = '';
      res.on('data', (chunk) => { data += chunk.toString(); });
      res.on('end', () => {
        resolve({ url, status: res.statusCode, html: data });
      });
    });

    req.on('error', reject);
    req.setTimeout(timeoutMs, () => {
      req.destroy(new Error('Request timeout'));
    });
  });
}

function stripTags(html) {
  if (!html) return '';
  let text = html.replace(/<script[\s\S]*?<\/script>/gi, ' ');
  text = text.replace(/<style[\s\S]*?<\/style>/gi, ' ');
  text = text.replace(/<[^>]+>/g, ' ');
  return text;
}

function decodeEntities(text) {
  if (!text) return '';
  return text
    .replace(/&nbsp;/g, ' ')
    .replace(/&amp;/g, '&')
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'")
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>');
}

function cleanText(text) {
  return decodeEntities(text).replace(/\s+/g, ' ').trim();
}

async function fetchAndClean(url, options) {
  const { status, html } = await fetchUrl(url, options);
  if (!html || status >= 400) {
    return { url, status, text: '' };
  }
  const stripped = stripTags(html);
  const text = cleanText(stripped);
  return { url, status, text };
}

module.exports = {
  fetchUrl,
  fetchAndClean,
  cleanText,
};
