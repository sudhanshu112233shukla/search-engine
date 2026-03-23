const express = require('express');
const { search } = require('./search');

const app = express();
const PORT = process.env.PORT || 3001;

app.get('/search', (req, res) => {
  const query = (req.query.q || '').toString().trim();
  if (!query) {
    res.status(400).json({ error: 'Missing q parameter' });
    return;
  }
  const result = search(query);
  res.json(result);
});

app.listen(PORT, () => {
  console.log(`Demo search API running on http://localhost:${PORT}`);
});
