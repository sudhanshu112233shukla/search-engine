#[derive(Clone, Debug)]
pub enum ConfidenceLevel {
    High,
    Medium,
    Low,
}

pub fn compute_confidence(score: f32) -> (f32, ConfidenceLevel) {
    let confidence = score.min(1.0).max(0.0);
    let level = if confidence > 0.75 {
        ConfidenceLevel::High
    } else if confidence > 0.5 {
        ConfidenceLevel::Medium
    } else {
        ConfidenceLevel::Low
    };
    (confidence, level)
}
