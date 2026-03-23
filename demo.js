const { search } = require('./src/search');

const query = process.argv.slice(2).join(' ').trim();
if (!query) {
  console.log('Usage: node demo.js "your query"');
  process.exit(1);
}

const result = search(query);

console.log(`Query: ${result.query}`);
console.log('Top 3 results:');
result.results.forEach((r, i) => {
  console.log(`${i + 1}. ${r.id} | score=${r.score.toFixed(4)}`);
});
console.log(`Answer: ${result.answer}`);
console.log(`Confidence: ${result.confidence.toFixed(2)}`);
