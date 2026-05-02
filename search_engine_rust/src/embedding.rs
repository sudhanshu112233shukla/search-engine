use crate::utils::normalize_text;

pub fn fnv1a_hash_bytes(bytes: &[u8]) -> u32 {
    let mut hash: u32 = 2166136261;
    for b in bytes {
        hash ^= *b as u32;
        hash = hash.wrapping_mul(16777619);
    }
    hash
}

pub fn embed_text(text: &str, dims: usize, ngram_min: usize, ngram_max: usize) -> Vec<f32> {
    let normalized = normalize_text(text);
    let padded = format!(" {} ", normalized);
    let mut vec = vec![0.0f32; dims];

    let bytes = padded.as_bytes();
    for n in ngram_min..=ngram_max {
        if bytes.len() < n { continue; }
        for i in 0..=bytes.len() - n {
            let gram = &bytes[i..i + n];
            let idx = (fnv1a_hash_bytes(gram) as usize) % dims;
            vec[idx] += 1.0;
        }
    }

    let mut norm = 0.0f32;
    for v in &vec {
        norm += v * v;
    }
    norm = norm.sqrt();
    if norm > 0.0 {
        for v in &mut vec {
            *v /= norm;
        }
    }
    vec
}
