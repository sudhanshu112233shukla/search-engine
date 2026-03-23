use serde::Deserialize;
use std::collections::HashSet;
use std::fs;
use std::time::Instant;

use crate::ranking::Ranked;
use crate::utils::{normalize_text, tokenize};
use crate::SearchEngine;

#[derive(Clone, Debug, Deserialize)]
pub struct EvalQuery {
    pub query: String,
    pub relevant_docs: Vec<String>,
    pub expected_answer: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct EvalDataset {
    pub queries: Vec<EvalQuery>,
}

#[derive(Clone, Debug)]
pub struct EvalResult {
    pub query: String,
    pub precision_at_10: f32,
    pub recall_at_10: f32,
    pub rr: f32,
    pub answer_accuracy: f32,
    pub latency_ms: f32,
    pub top_ids: Vec<String>,
    pub breakdowns: Vec<(String, f32, crate::ranking::ScoreBreakdown)>,
}

#[derive(Clone, Debug)]
pub struct EvalReport {
    pub precision_at_10: f32,
    pub recall_at_10: f32,
    pub mrr: f32,
    pub answer_accuracy: f32,
    pub avg_latency_ms: f32,
    pub max_latency_ms: f32,
    pub memory_bytes: usize,
    pub per_query: Vec<EvalResult>,
}

fn precision_recall_at_k(relevant: &HashSet<String>, ranked: &[String], k: usize) -> (f32, f32) {
    if k == 0 { return (0.0, 0.0); }
    let mut hit = 0usize;
    for id in ranked.iter().take(k) {
        if relevant.contains(id) { hit += 1; }
    }
    let precision = hit as f32 / k as f32;
    let recall = if relevant.is_empty() { 0.0 } else { hit as f32 / relevant.len() as f32 };
    (precision, recall)
}

fn reciprocal_rank(relevant: &HashSet<String>, ranked: &[String]) -> f32 {
    for (i, id) in ranked.iter().enumerate() {
        if relevant.contains(id) {
            return 1.0 / (i as f32 + 1.0);
        }
    }
    0.0
}

fn answer_accuracy(expected: &str, actual: &str) -> f32 {
    let exp_tokens = tokenize(expected);
    if exp_tokens.is_empty() { return 0.0; }
    let act_tokens = tokenize(actual);
    if act_tokens.is_empty() { return 0.0; }

    let mut hit = 0usize;
    for t in exp_tokens.iter() {
        if act_tokens.iter().any(|a| a == t) { hit += 1; }
    }
    hit as f32 / exp_tokens.len() as f32
}

pub fn evaluate(engine: &SearchEngine, dataset: EvalDataset) -> EvalReport {
    let mut sum_p = 0.0;
    let mut sum_r = 0.0;
    let mut sum_rr = 0.0;
    let mut sum_ans = 0.0;
    let mut total_latency = 0.0;
    let mut max_latency = 0.0;

    let mut per_query = Vec::new();

    for q in dataset.queries.iter() {
        let relevant: HashSet<String> = q.relevant_docs.iter().cloned().collect();
        let start = Instant::now();
        let (ranked, breakdowns) = engine.rank_debug(&q.query);
        let response = engine.search(&q.query);
        let elapsed = start.elapsed().as_secs_f32() * 1000.0;

        let ids: Vec<String> = ranked.iter().map(|r| engine.doc_id(r.doc_id).unwrap_or("".into())).collect();
        let (p10, r10) = precision_recall_at_k(&relevant, &ids, 10);
        let rr = reciprocal_rank(&relevant, &ids);
        let answer_text = response.answer.as_ref().map(|a| a.text.as_str()).unwrap_or("");
        let ans_acc = answer_accuracy(&q.expected_answer, answer_text);

        total_latency += elapsed;
        if elapsed > max_latency { max_latency = elapsed; }

        sum_p += p10;
        sum_r += r10;
        sum_rr += rr;
        sum_ans += ans_acc;

        let result = EvalResult {
            query: q.query.clone(),
            precision_at_10: p10,
            recall_at_10: r10,
            rr,
            answer_accuracy: ans_acc,
            latency_ms: elapsed,
            top_ids: ids.iter().take(10).cloned().collect(),
            breakdowns,
        };
        per_query.push(result);
    }

    let n = dataset.queries.len().max(1) as f32;
    EvalReport {
        precision_at_10: sum_p / n,
        recall_at_10: sum_r / n,
        mrr: sum_rr / n,
        answer_accuracy: sum_ans / n,
        avg_latency_ms: total_latency / n,
        max_latency_ms: max_latency,
        memory_bytes: engine.approx_memory_bytes(),
        per_query,
    }
}

pub fn load_eval_dataset(path: &str) -> Option<EvalDataset> {
    let data = fs::read_to_string(path).ok()?;
    serde_json::from_str::<EvalDataset>(&data).ok()
}

pub fn print_report(report: &EvalReport) {
    println!("Overall:");
    println!("  precision@10: {:.3}", report.precision_at_10);
    println!("  recall@10:    {:.3}", report.recall_at_10);
    println!("  mrr:          {:.3}", report.mrr);
    println!("  answer_acc:   {:.3}", report.answer_accuracy);
    println!("  avg_latency:  {:.2}ms", report.avg_latency_ms);
    println!("  max_latency:  {:.2}ms", report.max_latency_ms);
    println!("  memory:       {} bytes", report.memory_bytes);

    println!("\nPer-query:");
    for q in &report.per_query {
        println!("- {}", q.query);
        println!("  p@10={:.3} r@10={:.3} rr={:.3} ans={:.3} latency={:.2}ms", q.precision_at_10, q.recall_at_10, q.rr, q.answer_accuracy, q.latency_ms);
        println!("  top: {:?}", q.top_ids);
    }

    let mut worst = report.per_query.clone();
    worst.sort_by(|a, b| a.answer_accuracy.partial_cmp(&b.answer_accuracy).unwrap());
    println!("\nWorst queries by answer accuracy:");
    for q in worst.iter().take(3) {
        println!("  {} -> {:.3}", q.query, q.answer_accuracy);
    }
}

pub fn print_debug_breakdown(report: &EvalReport, top_k: usize) {
    println!("\nDebug breakdown (top {} per query):", top_k);
    for q in &report.per_query {
        println!("Query: {}", q.query);
        for (id, score, b) in q.breakdowns.iter().take(top_k) {
            println!("  {} score={:.3} (bm25={:.3} sem={:.3} exact={:.3} phrase={:.3} prox={:.3})",
                id, score, b.bm25, b.semantic, b.exact, b.phrase, b.proximity);
        }
    }
}
