const LIST_PATTERNS = [
  'list', 'top', 'best', 'types', 'examples', 'steps', 'ways', 'benefits', 'pros', 'cons',
];

const COMPARISON_PATTERNS = [
  'compare', 'vs', 'versus', 'difference', 'differences', 'better than', 'compared to',
];

function classifyQuery(query) {
  const q = query.toLowerCase();
  for (const phrase of COMPARISON_PATTERNS) {
    if (q.includes(phrase)) return 'comparison';
  }
  for (const phrase of LIST_PATTERNS) {
    if (q.includes(phrase)) return 'list';
  }
  return 'factual';
}

module.exports = {
  classifyQuery,
};
