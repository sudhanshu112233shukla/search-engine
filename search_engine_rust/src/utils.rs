const STOPWORDS: &[&str] = &[
    "a", "about", "above", "after", "again", "against", "all", "am", "an", "and", "any", "are", "as", "at",
    "be", "because", "been", "before", "being", "below", "between", "both", "but", "by",
    "can", "could",
    "did", "do", "does", "doing", "down", "during",
    "each",
    "few", "for", "from", "further",
    "had", "has", "have", "having", "he", "her", "here", "hers", "herself", "him", "himself", "his", "how",
    "i", "if", "in", "into", "is", "it", "its", "itself",
    "just",
    "me", "more", "most", "my", "myself",
    "no", "nor", "not", "now",
    "of", "off", "on", "once", "only", "or", "other", "our", "ours", "ourselves", "out", "over", "own",
    "same", "she", "should", "so", "some", "such",
    "than", "that", "the", "their", "theirs", "them", "themselves", "then", "there", "these", "they", "this",
    "those", "through", "to", "too",
    "under", "until", "up",
    "very",
    "was", "we", "were", "what", "when", "where", "which", "while", "who", "whom", "why", "with",
    "you", "your", "yours", "yourself", "yourselves",
];

pub fn normalize_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut prev_space = false;
    for c in text.chars() {
        let lc = c.to_ascii_lowercase();
        if lc.is_ascii_alphanumeric() {
            out.push(lc);
            prev_space = false;
        } else if !prev_space {
            out.push(' ');
            prev_space = true;
        }
    }
    out.trim().to_string()
}

pub fn tokenize(text: &str) -> Vec<String> {
    let normalized = normalize_text(text);
    normalized
        .split_whitespace()
        .filter(|t| !is_stopword(t))
        .map(|s| s.to_string())
        .collect()
}

pub fn is_stopword(token: &str) -> bool {
    STOPWORDS.iter().any(|w| *w == token)
}

pub fn split_sentences(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut current = String::new();
    for c in text.chars() {
        current.push(c);
        if c == '.' || c == '!' || c == '?' {
            let trimmed = current.trim();
            if !trimmed.is_empty() {
                sentences.push(trimmed.to_string());
            }
            current.clear();
        }
    }
    let trimmed = current.trim();
    if !trimmed.is_empty() {
        sentences.push(trimmed.to_string());
    }
    sentences
}
