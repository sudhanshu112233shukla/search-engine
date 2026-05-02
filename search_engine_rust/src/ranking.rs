use crate::processing::Chunk;
use crate::utils::{normalize_text, split_sentences, tokenize, tokenize_with_positions, QueryIntent};

#[derive(Clone, Debug)]
pub struct Ranked {
    pub doc_id: usize,
    pub score: f32,
    pub breakdown: ScoreBreakdown,
}

#[derive(Clone, Debug)]
pub struct ScoreBreakdown {
    pub bm25: f32,
    pub semantic: f32,
    pub exact: f32,
    pub phrase: f32,
    pub proximity: f32,
}

#[derive(Clone, Debug)]
pub struct RankingWeights {
    pub bm25: f32,
    pub semantic: f32,
    pub exact: f32,
    pub phrase: f32,
    pub proximity: f32,
}

impl Default for RankingWeights {
    fn default() -> Self {
        Self {
            bm25: 0.5,
            semantic: 0.5,
            exact: 0.8,
            phrase: 0.6,
            proximity: 0.4,
        }
    }
}

pub fn adjust_weights_for_intent(base: &RankingWeights, intent: QueryIntent) -> RankingWeights {
    let mut w = base.clone();
    match intent {
        QueryIntent::Factual => {
            w.bm25 *= 1.15;
            w.exact *= 1.2;
            w.phrase *= 1.1;
            w.semantic *= 0.9;
        }
        QueryIntent::List => {
            w.bm25 *= 1.1;
            w.semantic *= 1.1;
            w.proximity *= 0.9;
        }
        QueryIntent::Comparison => {
            w.phrase *= 1.2;
            w.proximity *= 1.2;
            w.semantic *= 1.05;
        }
        QueryIntent::Other => {}
    }
    w
}

fn min_max_norm(scores: &std::collections::HashMap<usize, f32>) -> std::collections::HashMap<usize, f32> {
    if scores.is_empty() {
        return std::collections::HashMap::new();
    }
    let mut min = f32::MAX;
    let mut max = f32::MIN;
    for s in scores.values() {
        if *s < min { min = *s; }
        if *s > max { max = *s; }
    }
    let range = (max - min).max(1e-6);
    let mut out = std::collections::HashMap::new();
    for (k, v) in scores.iter() {
        let norm = if range == 0.0 { 0.0 } else { (*v - min) / range };
        out.insert(*k, norm);
    }
    out
}

fn exact_match(clean_text: &str, clean_query: &str) -> f32 {
    if clean_query.is_empty() { return 0.0; }
    if clean_text.contains(clean_query) { 1.0 } else { 0.0 }
}

fn phrase_match(tokens: &[String], query_tokens: &[String]) -> f32 {
    if query_tokens.is_empty() || tokens.len() < query_tokens.len() {
        return 0.0;
    }
    for i in 0..=tokens.len() - query_tokens.len() {
        let mut ok = true;
        for j in 0..query_tokens.len() {
            if tokens[i + j] != query_tokens[j] {
                ok = false;
                break;
            }
        }
        if ok { return 1.0; }
    }
    0.0
}

fn proximity_score(chunk: &Chunk, query_tokens: &[String]) -> f32 {
    if query_tokens.len() < 2 {
        return 0.0;
    }
    let mut points: Vec<(usize, usize)> = Vec::new();
    if chunk.positions.is_empty() {
        let (_, positions) = tokenize_with_positions(&chunk.clean);
        for (ti, term) in query_tokens.iter().enumerate() {
            if let Some(pos) = positions.get(term) {
                for p in pos {
                    points.push((*p, ti));
                }
            } else {
                return 0.0;
            }
        }
    } else {
        for (ti, term) in query_tokens.iter().enumerate() {
            if let Some(pos) = chunk.positions.get(term) {
                for p in pos {
                    points.push((*p, ti));
                }
            } else {
                return 0.0;
            }
        }
    }
    points.sort_by_key(|p| p.0);

    let mut counts = vec![0usize; query_tokens.len()];
    let mut covered = 0usize;
    let mut left = 0usize;
    let mut best = usize::MAX;

    for right in 0..points.len() {
        let (_, ti) = points[right];
        counts[ti] += 1;
        if counts[ti] == 1 { covered += 1; }

        while covered == query_tokens.len() && left <= right {
            let window = points[right].0 - points[left].0 + 1;
            if window < best { best = window; }
            let (_, lti) = points[left];
            counts[lti] -= 1;
            if counts[lti] == 0 { covered -= 1; }
            left += 1;
        }
    }

    if best == usize::MAX { 0.0 } else { 1.0 / (1.0 + best as f32) }
}

fn lead_boost(chunk: &Chunk, query_tokens: &[String], intent: QueryIntent) -> f32 {
    if query_tokens.is_empty() {
        return 0.0;
    }
    let source = if !chunk.clean.is_empty() {
        chunk.clean.clone()
    } else {
        chunk.text.clone()
    };
    if source.is_empty() {
        return 0.0;
    }

    let first_sentence = split_sentences(&source).into_iter().next().unwrap_or_default();
    if first_sentence.is_empty() {
        return 0.0;
    }

    let clean_first = normalize_text(&first_sentence);
    let mut boost = 0.0f32;
    let overlap = query_tokens
        .iter()
        .filter(|t| clean_first.contains(t.as_str()))
        .count() as f32
        / query_tokens.len().max(1) as f32;

    if matches!(intent, QueryIntent::Factual) {
        boost += overlap * 0.45;
        if clean_first.contains(" is ")
            || clean_first.contains(" are ")
            || clean_first.contains(" was ")
            || clean_first.contains(" were ")
            || clean_first.contains(" refers to ")
            || clean_first.contains(" means ")
        {
            boost += 0.25;
        }
    } else {
        boost += overlap * 0.2;
    }
    boost
}

fn title_boost(chunk: &Chunk, query_tokens: &[String], intent: QueryIntent) -> f32 {
    if query_tokens.is_empty() {
        return 0.0;
    }
    let source = if !chunk.clean.is_empty() { chunk.clean.as_str() } else { chunk.text.as_str() };
    if source.is_empty() {
        return 0.0;
    }
    let title_line = source.lines().next().unwrap_or_default();
    if title_line.is_empty() {
        return 0.0;
    }
    let clean_title = normalize_text(title_line);
    if clean_title.is_empty() {
        return 0.0;
    }

    let exact = if clean_title == query_tokens.join(" ") { 1.0 } else { 0.0 };
    let starts = if query_tokens.iter().all(|t| clean_title.starts_with(t)) { 1.0 } else { 0.0 };
    let overlap = query_tokens
        .iter()
        .filter(|t| clean_title.contains(t.as_str()))
        .count() as f32
        / query_tokens.len().max(1) as f32;

    let mut boost = overlap * 0.25 + starts * 0.35 + exact * 0.75;
    if matches!(intent, QueryIntent::Factual) {
        boost *= 1.4;
    }
    boost
}

pub fn rank_candidates(
    bm25_results: &[(usize, f32)],
    vector_results: &[(usize, f32)],
    chunks: &[Chunk],
    query_tokens: &[String],
    clean_query: &str,
    weights: &RankingWeights,
    intent: QueryIntent,
) -> Vec<Ranked> {
    let mut bm25_map: std::collections::HashMap<usize, f32> = std::collections::HashMap::new();
    let mut vec_map: std::collections::HashMap<usize, f32> = std::collections::HashMap::new();

    for (id, score) in bm25_results {
        bm25_map.insert(*id, *score);
    }
    for (id, score) in vector_results {
        vec_map.insert(*id, *score);
    }

    let bm25_norm = min_max_norm(&bm25_map);
    let vec_norm = min_max_norm(&vec_map);

    let mut candidates: Vec<usize> = bm25_map.keys().chain(vec_map.keys()).cloned().collect();
    candidates.sort();
    candidates.dedup();

    let mut ranked = Vec::new();
    for doc_id in candidates {
        if doc_id >= chunks.len() { continue; }
        let chunk = &chunks[doc_id];
        let exact = exact_match(&chunk.clean, clean_query);
        let token_view = if chunk.tokens.is_empty() {
            tokenize(&chunk.clean)
        } else {
            chunk.tokens.clone()
        };
        let phrase = phrase_match(&token_view, query_tokens);
        let proximity = proximity_score(chunk, query_tokens);
        let lead = lead_boost(chunk, query_tokens, intent);
        let title = title_boost(chunk, query_tokens, intent);

        let bm25_c = weights.bm25 * bm25_norm.get(&doc_id).copied().unwrap_or(0.0);
        let sem_c = weights.semantic * vec_norm.get(&doc_id).copied().unwrap_or(0.0);
        let exact_c = weights.exact * exact;
        let phrase_c = weights.phrase * phrase;
        let prox_c = weights.proximity * proximity;
        let lead_c = if matches!(intent, QueryIntent::Factual) { lead } else { lead * 0.5 };
        let title_c = title;

        let score = bm25_c + sem_c + exact_c + phrase_c + prox_c + lead_c + title_c;

        ranked.push(Ranked {
            doc_id,
            score,
            breakdown: ScoreBreakdown {
                bm25: bm25_c,
                semantic: sem_c,
                exact: exact_c,
                phrase: phrase_c,
                proximity: prox_c + lead_c + title_c,
            },
        });
    }

    ranked.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

    if let Some(top) = ranked.first() {
        if top.score < 0.15 && !bm25_norm.is_empty() {
            let mut fallback: Vec<Ranked> = bm25_norm
                .iter()
                .map(|(id, s)| Ranked {
                    doc_id: *id,
                    score: *s,
                    breakdown: ScoreBreakdown { bm25: *s, semantic: 0.0, exact: 0.0, phrase: 0.0, proximity: 0.0 },
                })
                .collect();
            fallback.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
            return fallback;
        }
    }

    ranked
}
