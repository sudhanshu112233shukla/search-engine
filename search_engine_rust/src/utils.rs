use std::collections::HashMap;

const STOPWORDS_EN: &[&str] = &[
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

const STOPWORDS_ES: &[&str] = &["de", "la", "que", "el", "en", "y", "a", "los", "del", "se", "las", "por", "un", "para", "con", "no", "una", "su", "al", "lo"]; 
const STOPWORDS_HI: &[&str] = &["और", "का", "के", "की", "में", "है", "था", "थे", "तो", "से", "पर", "यह", "वह", "एक"]; 

const SYNONYMS_EN: &[(&str, &[&str])] = &[
    ("bm25", &["ranking", "tfidf", "tf-idf"]),
    ("vector", &["embedding", "semantic"]),
    ("search", &["retrieve", "retrieval"]),
];

const SYNONYMS_ES: &[(&str, &[&str])] = &[
    ("buscar", &["busqueda", "recuperar"]),
    ("vector", &["embebido", "semantico"]),
];

const SYNONYMS_HI: &[(&str, &[&str])] = &[
    ("खोज", &["सर्च", "खोजना"]),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    En,
    Es,
    Hi,
    Other,
}

pub fn detect_language(text: &str) -> Lang {
    for c in text.chars() {
        if ('\u{0900}'..='\u{097F}').contains(&c) {
            return Lang::Hi;
        }
        if ('\u{0400}'..='\u{04FF}').contains(&c) {
            return Lang::Other;
        }
    }
    if text.to_lowercase().contains(" el ") || text.to_lowercase().contains(" la ") {
        Lang::Es
    } else {
        Lang::En
    }
}

pub fn normalize_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut prev_space = false;
    for c in text.chars() {
        if c.is_alphanumeric() {
            for lc in c.to_lowercase() {
                out.push(lc);
            }
            prev_space = false;
        } else if !prev_space {
            out.push(' ');
            prev_space = true;
        }
    }
    out.trim().to_string()
}

pub fn is_stopword(token: &str) -> bool {
    STOPWORDS_EN.iter().any(|w| *w == token)
}

fn is_stopword_lang(token: &str, lang: Lang) -> bool {
    match lang {
        Lang::En => STOPWORDS_EN.iter().any(|w| *w == token),
        Lang::Es => STOPWORDS_ES.iter().any(|w| *w == token),
        Lang::Hi => STOPWORDS_HI.iter().any(|w| *w == token),
        Lang::Other => false,
    }
}

pub fn tokenize(text: &str) -> Vec<String> {
    let normalized = normalize_text(text);
    normalized
        .split_whitespace()
        .map(|s| stem_token(s))
        .filter(|t| !is_stopword(t))
        .collect()
}

pub fn tokenize_with_lang(text: &str, lang: Lang) -> Vec<String> {
    let normalized = normalize_text(text);
    normalized
        .split_whitespace()
        .map(|s| stem_token(s))
        .filter(|t| !is_stopword_lang(t, lang))
        .collect()
}

pub fn tokenize_with_positions(text: &str) -> (Vec<String>, HashMap<String, Vec<usize>>) {
    let normalized = normalize_text(text);
    let mut tokens = Vec::new();
    let mut positions: HashMap<String, Vec<usize>> = HashMap::new();

    for (idx, raw) in normalized.split_whitespace().enumerate() {
        let token = stem_token(raw);
        if is_stopword(&token) {
            continue;
        }
        tokens.push(token.clone());
        positions.entry(token).or_default().push(idx);
    }

    (tokens, positions)
}

pub fn expand_tokens(tokens: &[String], lang: Lang) -> Vec<String> {
    let mut expanded = Vec::with_capacity(tokens.len());
    for t in tokens.iter() {
        expanded.push(t.clone());
        let syns = match lang {
            Lang::En => SYNONYMS_EN.iter().find(|(k, _)| *k == t).map(|(_, v)| *v),
            Lang::Es => SYNONYMS_ES.iter().find(|(k, _)| *k == t).map(|(_, v)| *v),
            Lang::Hi => SYNONYMS_HI.iter().find(|(k, _)| *k == t).map(|(_, v)| *v),
            Lang::Other => None,
        };
        if let Some(syns) = syns {
            for s in syns {
                expanded.push(s.to_string());
            }
        }
    }
    expanded
}

pub fn process_query(query: &str) -> Vec<String> {
    let base = tokenize(query);
    let mut expanded = Vec::with_capacity(base.len());
    for t in base.iter() {
        expanded.push(t.clone());
        if let Some(syns) = SYNONYMS_EN.iter().find(|(k, _)| *k == t).map(|(_, v)| *v) {
            for s in syns {
                expanded.push(s.to_string());
            }
        }
    }
    expanded
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

pub fn make_snippet(text: &str, tokens: &[String], max_len: usize) -> String {
    if text.len() <= max_len {
        return highlight_terms(text, tokens);
    }

    let lower = text.to_lowercase();
    let mut start = 0usize;
    for t in tokens {
        if let Some(pos) = lower.find(t) {
            start = pos.saturating_sub(40);
            break;
        }
    }
    let end = (start + max_len).min(text.len());
    let snippet = text[start..end].trim().to_string();
    highlight_terms(&snippet, tokens)
}

fn highlight_terms(text: &str, tokens: &[String]) -> String {
    let mut out = String::new();
    for word in text.split_whitespace() {
        let clean = word.to_lowercase().replace(|c: char| !c.is_ascii_alphanumeric(), "");
        if tokens.iter().any(|t| t == &clean) {
            out.push('[');
            out.push_str(word);
            out.push(']');
        } else {
            out.push_str(word);
        }
        out.push(' ');
    }
    out.trim().to_string()
}

fn stem_token(token: &str) -> String {
    if !token.is_ascii() {
        return token.to_string();
    }
    let mut t = token.to_string();
    for suf in ["ing", "edly", "ed", "ly", "es", "s"] {
        if t.len() > suf.len() + 2 && t.ends_with(suf) {
            t.truncate(t.len() - suf.len());
            break;
        }
    }
    t
}
