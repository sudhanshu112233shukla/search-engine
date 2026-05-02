use crate::ranking::Ranked;
use crate::processing::Chunk;
use crate::utils::{detect_intent, detect_language, normalize_text, split_sentences, tokenize, QueryIntent};

#[derive(Clone, Debug)]
pub struct AnswerCandidate {
    pub text: String,
    pub score: f32,
    pub source: String,
}

fn keyword_overlap(sentence: &str, query_tokens: &[String]) -> f32 {
    if query_tokens.is_empty() {
        return 0.0;
    }
    let sentence_tokens = tokenize(sentence);
    if sentence_tokens.is_empty() {
        return 0.0;
    }
    let mut overlap = 0usize;
    for qt in query_tokens {
        if sentence_tokens.iter().any(|t| t == qt) {
            overlap += 1;
        }
    }
    overlap as f32 / query_tokens.len() as f32
}

fn length_penalty(sentence: &str, intent: QueryIntent) -> f32 {
    let words = sentence.split_whitespace().count().max(1) as f32;
    let scale = match intent {
        QueryIntent::Factual => 40.0,
        QueryIntent::List => 32.0,
        QueryIntent::Comparison => 28.0,
        QueryIntent::Other => 25.0,
    };
    1.0 / (1.0 + (words / scale))
}

fn factual_subject(query_tokens: &[String]) -> Vec<String> {
    query_tokens
        .iter()
        .filter(|t| {
            !matches!(
                t.as_str(),
                "what" | "who" | "when" | "where" | "why" | "how" | "define" | "meaning" | "is" | "are" | "was" | "were" | "the" | "a" | "an" | "of" | "to"
            )
        })
        .cloned()
        .collect()
}

fn definition_boost(sentence: &str, subject: &[String], intent: QueryIntent) -> f32 {
    let clean = normalize_text(sentence);
    if clean.is_empty() {
        return 0.0;
    }

    let mut boost = 0.0f32;
    let cues = [
        " is a ",
        " is an ",
        " is the ",
        " was a ",
        " was an ",
        " refers to ",
        " means ",
        " is a type of ",
        " are a ",
        " are the ",
    ];
    if cues.iter().any(|cue| clean.contains(cue.trim())) {
        boost += 0.35;
    }

    if !subject.is_empty() {
        let starts_with_subject = subject.iter().any(|s| clean.starts_with(s));
        let contains_subject = subject.iter().any(|s| clean.contains(s));
        if starts_with_subject {
            boost += 0.45;
        } else if contains_subject {
            boost += 0.2;
        }
    }

    if matches!(intent, QueryIntent::Factual) && clean.split_whitespace().count() <= 40 {
        boost += 0.15;
    }

    boost
}

fn is_trivial_sentence(sentence: &str) -> bool {
    sentence.split_whitespace().count() < 5
}

fn noise_penalty(sentence: &str) -> f32 {
    let clean = normalize_text(sentence);
    if clean.is_empty() {
        return 0.0;
    }
    let words: Vec<&str> = clean.split_whitespace().collect();
    let numbers = words.iter().filter(|w| w.chars().any(|c| c.is_ascii_digit())).count() as f32;
    let pipes = sentence.matches('|').count() as f32;
    let brackets = sentence.matches('[').count() as f32 + sentence.matches(']').count() as f32;
    let table_like = if sentence.contains("wikitable") || sentence.contains("sortable") { 1.0 } else { 0.0 };
    -((numbers * 0.04) + (pipes * 0.12) + (brackets * 0.03) + table_like * 0.5)
}

fn title_like_prefix(sentence: &str, subject: &[String]) -> f32 {
    if subject.is_empty() {
        return 0.0;
    }
    let clean = normalize_text(sentence);
    if clean.is_empty() {
        return 0.0;
    }
    let head = clean.split_whitespace().take(subject.len().max(3)).collect::<Vec<_>>().join(" ");
    let subject_joined = subject.join(" ");
    if head == subject_joined {
        return 0.8;
    }
    if clean.starts_with(&subject_joined) {
        return 0.6;
    }
    let matches = subject.iter().filter(|s| clean.starts_with(s.as_str()) || clean.contains(s.as_str())).count() as f32;
    matches / subject.len().max(1) as f32 * 0.4
}

pub fn extract_answers<F>(query: &str, ranked: &[Ranked], chunks: &[Chunk], get_text: F) -> Vec<AnswerCandidate>
where
    F: Fn(usize) -> String,
{
    let query_tokens = tokenize(query);
    let clean_query = normalize_text(query);
    let lang = detect_language(query);
    let intent = detect_intent(query, lang);
    let subject = if matches!(intent, QueryIntent::Factual) {
        factual_subject(&query_tokens)
    } else {
        Vec::new()
    };
    let mut candidates: Vec<AnswerCandidate> = Vec::new();

    for r in ranked.iter().take(10) {
        let chunk = &chunks[r.doc_id];
        let text = get_text(r.doc_id);
        let sentences = split_sentences(&text);
        for (idx, sentence) in sentences.into_iter().enumerate() {
            if is_trivial_sentence(&sentence) {
                continue;
            }
            let overlap = keyword_overlap(&sentence, &query_tokens);
            let exact = if !clean_query.is_empty() && normalize_text(&sentence).contains(&clean_query) { 1.0 } else { 0.0 };
            let lead_boost = if idx == 0 { 0.12 } else { 0.0 };
            let def_boost = definition_boost(&sentence, &subject, intent);
            let title_boost = if idx == 0 { title_like_prefix(&sentence, &subject) } else { 0.0 };
            let factual_bonus = if matches!(intent, QueryIntent::Factual) { 0.12 * overlap.max(exact) } else { 0.0 };
            let word_count = sentence.split_whitespace().count() as f32;
            let length_shape = if matches!(intent, QueryIntent::Factual) {
                let ideal = 18.0;
                1.0 / (1.0 + ((word_count - ideal).abs() / ideal))
            } else {
                length_penalty(&sentence, intent)
            };
            let verb_boost = if matches!(intent, QueryIntent::Factual) {
                let clean = normalize_text(&sentence);
                if clean.contains(" is ")
                    || clean.contains(" are ")
                    || clean.contains(" was ")
                    || clean.contains(" were ")
                    || clean.contains(" refers to ")
                    || clean.contains(" means ")
                {
                    0.2
                } else {
                    0.0
                }
            } else {
                0.0
            };
            let base = 0.34 * r.score + 0.3 * overlap + 0.12 * exact + lead_boost + def_boost + title_boost + factual_bonus + verb_boost + noise_penalty(&sentence);
            let score = base * length_shape;
            candidates.push(AnswerCandidate {
                text: sentence,
                score,
                source: chunk.id.clone(),
            });
        }
    }

    candidates.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    candidates
}
