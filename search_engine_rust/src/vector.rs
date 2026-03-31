use crate::embedding::embed_text;
use crate::processing::Chunk;
use hnsw_rs::prelude::*;
use memmap2::Mmap;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnnConfig {
    pub enabled: bool,
    pub nlist: usize,
    pub nprobe: usize,
    pub max_iters: usize,
    pub sample_size: usize,
    pub hnsw_enabled: bool,
    pub hnsw_m: usize,
    pub hnsw_ef_construction: usize,
    pub hnsw_ef_search: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PQConfig {
    pub enabled: bool,
    pub m: usize,
    pub k: usize,
    pub max_iters: usize,
    pub sample_size: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct IVFIndex {
    centroids: Vec<Vec<f32>>,
    lists: Vec<Vec<usize>>,
}

struct HnswIndex {
    hnsw: Hnsw<'static, f32, DistCosine>,
    ef_search: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VectorPersist {
    pub dims: usize,
    pub ngram_min: usize,
    pub ngram_max: usize,
    pub quantized: bool,
    pub vectors_f32: Option<Vec<Vec<f32>>>,
    pub vectors_i8: Option<Vec<i8>>,
    pub scales: Option<Vec<f32>>,
    #[serde(default)]
    pub vectors_i8_file: Option<String>,
    #[serde(default)]
    pub scales_file: Option<String>,
    pub ivf: Option<IVFIndex>,
    pub ann_nprobe: usize,
    pub pq: Option<PQIndex>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PQIndex {
    m: usize,
    k: usize,
    sub_dim: usize,
    codebooks: Vec<Vec<Vec<f32>>>,
    codes: Vec<u8>,
}

pub struct VectorIndex {
    pub dims: usize,
    pub ngram_min: usize,
    pub ngram_max: usize,
    pub quantized: bool,
    pub vectors_f32: Option<Vec<Vec<f32>>>,
    pub vectors_i8: Option<VectorI8Store>,
    pub scales: Option<Vec<f32>>,
    ivf: Option<IVFIndex>,
    ann_nprobe: usize,
    pq: Option<PQIndex>,
    hnsw: Option<HnswIndex>,
}

#[derive(Debug)]
pub enum VectorI8Store {
    Mem(Vec<i8>),
    Mmap { mmap: Mmap, len: usize },
}

impl VectorI8Store {
    pub fn as_slice(&self) -> &[i8] {
        match self {
            VectorI8Store::Mem(v) => v.as_slice(),
            VectorI8Store::Mmap { mmap, len } => unsafe {
                std::slice::from_raw_parts(mmap.as_ptr() as *const i8, *len)
            },
        }
    }

    pub fn len(&self) -> usize {
        match self {
            VectorI8Store::Mem(v) => v.len(),
            VectorI8Store::Mmap { len, .. } => *len,
        }
    }
}

impl VectorIndex {
    pub fn build(
        chunks: &[Chunk],
        dims: usize,
        ngram_min: usize,
        ngram_max: usize,
        quantize: bool,
        ann: &AnnConfig,
        pq: &PQConfig,
    ) -> Self {
        let use_hnsw = ann.hnsw_enabled;
        let use_ann = ann.enabled && !use_hnsw && chunks.len() >= ann.nlist.max(1);
        let use_pq = pq.enabled && !use_hnsw && dims % pq.m.max(1) == 0;
        let mut centroids: Vec<Vec<f32>> = Vec::new();

        if use_ann {
            let sample = sample_vectors(chunks, dims, ngram_min, ngram_max, ann.sample_size);
            centroids = kmeans(&sample, ann.nlist, ann.max_iters);
        }

        let mut lists: Vec<Vec<usize>> = if use_ann {
            vec![Vec::new(); centroids.len()]
        } else {
            Vec::new()
        };

        if use_hnsw {
            let mut vectors = Vec::with_capacity(chunks.len());
            for chunk in chunks {
                let v = embed_text(&chunk.clean, dims, ngram_min, ngram_max);
                vectors.push(v);
            }
            let hnsw = build_hnsw_index(&vectors, ann.hnsw_m, ann.hnsw_ef_construction, ann.hnsw_ef_search);
            return Self {
                dims,
                ngram_min,
                ngram_max,
                quantized: false,
                vectors_f32: Some(vectors),
                vectors_i8: None,
                scales: None,
                ivf: None,
                ann_nprobe: ann.nprobe.max(1),
                pq: None,
                hnsw: Some(hnsw),
            };
        }

        if use_pq {
            let sample = sample_vectors(chunks, dims, ngram_min, ngram_max, pq.sample_size);
            let pq_index = build_pq(&sample, pq.m, pq.k, pq.max_iters, dims);
            let mut codes = Vec::with_capacity(chunks.len() * pq_index.m);
            for (i, chunk) in chunks.iter().enumerate() {
                let v = embed_text(&chunk.clean, dims, ngram_min, ngram_max);
                if use_ann {
                    let cid = nearest_centroid(&v, &centroids);
                    lists[cid].push(i);
                }
                encode_pq(&v, &pq_index, &mut codes);
            }
            let ivf = if use_ann { Some(IVFIndex { centroids, lists }) } else { None };
            let pq_index = PQIndex {
                m: pq_index.m,
                k: pq_index.k,
                sub_dim: pq_index.sub_dim,
                codebooks: pq_index.codebooks,
                codes,
            };
            return Self {
                dims,
                ngram_min,
                ngram_max,
                quantized: false,
                vectors_f32: None,
                vectors_i8: None,
                scales: None,
                ivf,
                ann_nprobe: ann.nprobe.max(1),
                pq: Some(pq_index),
                hnsw: None,
            };
        }

        if quantize {
            let mut vectors_i8 = Vec::with_capacity(chunks.len() * dims);
            let mut scales = Vec::with_capacity(chunks.len());
            for (i, chunk) in chunks.iter().enumerate() {
                let v = embed_text(&chunk.clean, dims, ngram_min, ngram_max);
                if use_ann {
                    let cid = nearest_centroid(&v, &centroids);
                    lists[cid].push(i);
                }
                let (q, scale) = quantize_vec(&v);
                vectors_i8.extend_from_slice(&q);
                scales.push(scale);
            }
            let ivf = if use_ann { Some(IVFIndex { centroids, lists }) } else { None };
            Self {
                dims,
                ngram_min,
                ngram_max,
                quantized: true,
                vectors_f32: None,
                vectors_i8: Some(VectorI8Store::Mem(vectors_i8)),
                scales: Some(scales),
                ivf,
                ann_nprobe: ann.nprobe.max(1),
                pq: None,
                hnsw: None,
            }
        } else {
            let mut vectors = Vec::with_capacity(chunks.len());
            for (i, chunk) in chunks.iter().enumerate() {
                let v = embed_text(&chunk.clean, dims, ngram_min, ngram_max);
                if use_ann {
                    let cid = nearest_centroid(&v, &centroids);
                    lists[cid].push(i);
                }
                vectors.push(v);
            }
            let ivf = if use_ann { Some(IVFIndex { centroids, lists }) } else { None };
            Self {
                dims,
                ngram_min,
                ngram_max,
                quantized: false,
                vectors_f32: Some(vectors),
                vectors_i8: None,
                scales: None,
                ivf,
                ann_nprobe: ann.nprobe.max(1),
                pq: None,
                hnsw: None,
            }
        }
    }

    pub fn search(&self, query: &str, top_k: usize) -> Vec<(usize, f32)> {
        if self.hnsw.is_some() {
            return self.search_hnsw(query, top_k);
        }
        if self.pq.is_some() {
            return self.search_pq(query, top_k);
        }
        if self.ivf.is_some() {
            return self.search_ann(query, top_k);
        }
        if self.quantized {
            return self.search_quantized(query, top_k);
        }
        self.search_full(query, top_k)
    }

    pub fn add_chunks(&mut self, chunks: &[Chunk]) {
        if chunks.is_empty() {
            return;
        }
        let base_id = self.len();
        for (offset, chunk) in chunks.iter().enumerate() {
            let doc_id = base_id + offset;
            let v = embed_text(&chunk.clean, self.dims, self.ngram_min, self.ngram_max);
            if let Some(hnsw) = &mut self.hnsw {
                if let Some(store) = &mut self.vectors_f32 {
                    store.push(v.clone());
                }
                let _ = hnsw.hnsw.insert((&v, doc_id));
                continue;
            }
            if let Some(ivf) = &mut self.ivf {
                let cid = nearest_centroid(&v, &ivf.centroids);
                ivf.lists[cid].push(doc_id);
            }
            if let Some(pq) = &mut self.pq {
                let mut tmp = Vec::with_capacity(pq.m);
                encode_pq(&v, pq, &mut tmp);
                pq.codes.extend_from_slice(&tmp);
                continue;
            }
            if self.quantized {
                let (q, scale) = quantize_vec(&v);
                if let Some(store) = &mut self.vectors_i8 {
                    match store {
                        VectorI8Store::Mem(buf) => buf.extend_from_slice(&q),
                        VectorI8Store::Mmap { .. } => {
                            let mut buf = store.as_slice().to_vec();
                            buf.extend_from_slice(&q);
                            *store = VectorI8Store::Mem(buf);
                        }
                    }
                }
                if let Some(scales) = &mut self.scales {
                    scales.push(scale);
                }
            } else if let Some(store) = &mut self.vectors_f32 {
                store.push(v);
            }
        }
    }

    pub fn approx_bytes(&self) -> usize {
        if let Some(pq) = &self.pq {
            let codebook_bytes: usize = pq
                .codebooks
                .iter()
                .map(|cb| cb.len() * cb[0].len() * std::mem::size_of::<f32>())
                .sum();
            let codes_bytes = pq.codes.len() * std::mem::size_of::<u8>();
            return codebook_bytes + codes_bytes;
        }
        if self.quantized {
            let v = self.vectors_i8.as_ref().map(|v| v.len()).unwrap_or(0);
            let s = self.scales.as_ref().map(|s| s.len()).unwrap_or(0);
            v * std::mem::size_of::<i8>() + s * std::mem::size_of::<f32>()
        } else {
            let mut total = 0usize;
            if let Some(vs) = &self.vectors_f32 {
                for v in vs {
                    total += v.len() * std::mem::size_of::<f32>();
                }
            }
            total
        }
    }

    pub fn len(&self) -> usize {
        if let Some(pq) = &self.pq {
            return pq.codes.len() / pq.m.max(1);
        }
        if self.quantized {
            self.scales.as_ref().map(|s| s.len()).unwrap_or(0)
        } else {
            self.vectors_f32.as_ref().map(|v| v.len()).unwrap_or(0)
        }
    }

    pub fn to_persist(&self) -> VectorPersist {
        VectorPersist {
            dims: self.dims,
            ngram_min: self.ngram_min,
            ngram_max: self.ngram_max,
            quantized: self.quantized,
            vectors_f32: self.vectors_f32.clone(),
            vectors_i8: self
                .vectors_i8
                .as_ref()
                .and_then(|v| match v {
                    VectorI8Store::Mem(buf) => Some(buf.clone()),
                    VectorI8Store::Mmap { .. } => None,
                }),
            scales: self.scales.clone(),
            vectors_i8_file: None,
            scales_file: None,
            ivf: self.ivf.clone(),
            ann_nprobe: self.ann_nprobe,
            pq: self.pq.clone(),
        }
    }

    pub fn from_persist(
        persist: VectorPersist,
        vectors_i8: Option<VectorI8Store>,
        scales: Option<Vec<f32>>,
    ) -> Self {
        let vectors_i8 = vectors_i8.or_else(|| persist.vectors_i8.map(VectorI8Store::Mem));
        let scales = scales.or(persist.scales);
        Self {
            dims: persist.dims,
            ngram_min: persist.ngram_min,
            ngram_max: persist.ngram_max,
            quantized: persist.quantized,
            vectors_f32: persist.vectors_f32,
            vectors_i8,
            scales,
            ivf: persist.ivf,
            ann_nprobe: persist.ann_nprobe.max(1),
            pq: persist.pq,
            hnsw: None,
        }
    }

    pub fn rebuild_hnsw(&mut self, ann: &AnnConfig) {
        if !ann.hnsw_enabled {
            self.hnsw = None;
            return;
        }
        let vectors = match &self.vectors_f32 {
            Some(v) if !v.is_empty() => v,
            _ => {
                self.hnsw = None;
                return;
            }
        };
        let hnsw = build_hnsw_index(vectors, ann.hnsw_m, ann.hnsw_ef_construction, ann.hnsw_ef_search);
        self.hnsw = Some(hnsw);
    }

    fn search_full(&self, query: &str, top_k: usize) -> Vec<(usize, f32)> {
        let vectors = match &self.vectors_f32 {
            Some(v) if !v.is_empty() => v,
            _ => return Vec::new(),
        };
        let q = embed_text(query, self.dims, self.ngram_min, self.ngram_max);
        let mut results = Vec::with_capacity(vectors.len());
        for (i, v) in vectors.iter().enumerate() {
            let mut dotv = 0.0f32;
            for j in 0..self.dims {
                dotv += q[j] * v[j];
            }
            results.push((i, dotv));
        }
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(top_k);
        results
    }

    fn search_quantized(&self, query: &str, top_k: usize) -> Vec<(usize, f32)> {
        let vectors_i8 = match &self.vectors_i8 {
            Some(v) if v.len() > 0 => v,
            _ => return Vec::new(),
        };
        let scales = match &self.scales {
            Some(s) => s,
            None => return Vec::new(),
        };

        let q = embed_text(query, self.dims, self.ngram_min, self.ngram_max);
        let mut results = Vec::with_capacity(scales.len());
        for (i, scale) in scales.iter().enumerate() {
            let base = i * self.dims;
            let mut dotv = 0.0f32;
            for j in 0..self.dims {
                let v = vectors_i8.as_slice()[base + j] as f32 * *scale;
                dotv += q[j] * v;
            }
            results.push((i, dotv));
        }
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(top_k);
        results
    }

    fn search_ann(&self, query: &str, top_k: usize) -> Vec<(usize, f32)> {
        let ivf = match &self.ivf {
            Some(ivf) => ivf,
            None => {
                return if self.quantized {
                    self.search_quantized(query, top_k)
                } else {
                    self.search_full(query, top_k)
                };
            }
        };

        let q = embed_text(query, self.dims, self.ngram_min, self.ngram_max);
        let mut centroid_scores: Vec<(usize, f32)> = ivf
            .centroids
            .iter()
            .enumerate()
            .map(|(i, c)| (i, dot(&q, c)))
            .collect();
        centroid_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        centroid_scores.truncate(self.ann_nprobe.min(centroid_scores.len()));

        let mut candidates: Vec<usize> = Vec::new();
        for (cid, _) in centroid_scores {
            candidates.extend_from_slice(&ivf.lists[cid]);
        }
        if candidates.is_empty() {
            return if self.quantized {
                self.search_quantized(query, top_k)
            } else {
                self.search_full(query, top_k)
            };
        }

        let mut results = Vec::with_capacity(candidates.len());
        if self.quantized {
            let vectors_i8 = match &self.vectors_i8 {
                Some(v) => v,
                None => return Vec::new(),
            };
            let scales = match &self.scales {
                Some(s) => s,
                None => return Vec::new(),
            };
            for &i in &candidates {
                let base = i * self.dims;
                let mut dotv = 0.0f32;
                let scale = scales[i];
                for j in 0..self.dims {
                    let v = vectors_i8.as_slice()[base + j] as f32 * scale;
                    dotv += q[j] * v;
                }
                results.push((i, dotv));
            }
        } else if let Some(vectors) = &self.vectors_f32 {
            for &i in &candidates {
                let v = &vectors[i];
                results.push((i, dot(&q, v)));
            }
        }

        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(top_k);
        results
    }

    fn search_pq(&self, query: &str, top_k: usize) -> Vec<(usize, f32)> {
        let pq = match &self.pq {
            Some(pq) => pq,
            None => return Vec::new(),
        };
        let q = embed_text(query, self.dims, self.ngram_min, self.ngram_max);
        let q_tables = pq_query_tables(&q, pq);

        let mut candidates: Vec<usize> = if let Some(ivf) = &self.ivf {
            let mut centroid_scores: Vec<(usize, f32)> = ivf
                .centroids
                .iter()
                .enumerate()
                .map(|(i, c)| (i, dot(&q, c)))
                .collect();
            centroid_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            centroid_scores.truncate(self.ann_nprobe.min(centroid_scores.len()));
            let mut ids = Vec::new();
            for (cid, _) in centroid_scores {
                ids.extend_from_slice(&ivf.lists[cid]);
            }
            ids
        } else {
            (0..self.len()).collect()
        };

        if candidates.is_empty() {
            candidates = (0..self.len()).collect();
        }

        let mut results = Vec::with_capacity(candidates.len());
        for &doc_id in &candidates {
            let mut score = 0.0f32;
            let base = doc_id * pq.m;
            for m in 0..pq.m {
                let code = pq.codes[base + m] as usize;
                score += q_tables[m][code];
            }
            results.push((doc_id, score));
        }

        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(top_k);
        results
    }

    fn search_hnsw(&self, query: &str, top_k: usize) -> Vec<(usize, f32)> {
        let hnsw = match &self.hnsw {
            Some(h) => h,
            None => return Vec::new(),
        };
        let vectors = match &self.vectors_f32 {
            Some(v) => v,
            None => return Vec::new(),
        };
        let q = embed_text(query, self.dims, self.ngram_min, self.ngram_max);
        let ef = hnsw.ef_search.max(top_k);
        let neighbors = hnsw.hnsw.search(&q, top_k, ef);
        let mut results = Vec::with_capacity(neighbors.len());
        for n in neighbors {
            let id = n.d_id;
            if id < vectors.len() {
                let score = dot(&q, &vectors[id]);
                results.push((id, score));
            }
        }
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(top_k);
        results
    }
}

fn sample_vectors(
    chunks: &[Chunk],
    dims: usize,
    ngram_min: usize,
    ngram_max: usize,
    sample_size: usize,
) -> Vec<Vec<f32>> {
    if chunks.is_empty() {
        return Vec::new();
    }
    let target = sample_size.max(1).min(chunks.len());
    if target == chunks.len() {
        return chunks
            .iter()
            .map(|c| embed_text(&c.clean, dims, ngram_min, ngram_max))
            .collect();
    }

    let step = (chunks.len() / target).max(1);
    let mut samples = Vec::with_capacity(target);
    let mut idx = 0usize;
    while samples.len() < target && idx < chunks.len() {
        let c = &chunks[idx];
        samples.push(embed_text(&c.clean, dims, ngram_min, ngram_max));
        idx += step;
    }
    samples
}

fn kmeans(samples: &[Vec<f32>], k: usize, iters: usize) -> Vec<Vec<f32>> {
    if samples.is_empty() || k == 0 {
        return Vec::new();
    }
    let dims = samples[0].len();
    let k = k.min(samples.len());
    let step = (samples.len() / k).max(1);
    let mut centroids: Vec<Vec<f32>> = (0..k)
        .map(|i| samples[(i * step).min(samples.len() - 1)].clone())
        .collect();

    for _ in 0..iters.max(1) {
        let mut sums = vec![vec![0.0f32; dims]; k];
        let mut counts = vec![0usize; k];
        for v in samples {
            let cid = nearest_centroid(v, &centroids);
            counts[cid] += 1;
            for d in 0..dims {
                sums[cid][d] += v[d];
            }
        }
        for i in 0..k {
            if counts[i] == 0 {
                continue;
            }
            for d in 0..dims {
                centroids[i][d] = sums[i][d] / counts[i] as f32;
            }
        }
    }
    centroids
}

fn build_pq(samples: &[Vec<f32>], m: usize, k: usize, iters: usize, dims: usize) -> PQIndex {
    let sub_dim = dims / m.max(1);
    let mut codebooks = Vec::with_capacity(m);
    for i in 0..m {
        let mut sub_samples = Vec::with_capacity(samples.len());
        let start = i * sub_dim;
        let end = start + sub_dim;
        for v in samples {
            sub_samples.push(v[start..end].to_vec());
        }
        let centroids = kmeans(&sub_samples, k, iters);
        codebooks.push(centroids);
    }
    PQIndex {
        m,
        k,
        sub_dim,
        codebooks,
        codes: Vec::new(),
    }
}

fn encode_pq(v: &[f32], pq: &PQIndex, out: &mut Vec<u8>) {
    for m in 0..pq.m {
        let start = m * pq.sub_dim;
        let end = start + pq.sub_dim;
        let sub = &v[start..end];
        let mut best = 0usize;
        let mut best_dist = f32::MAX;
        for (i, c) in pq.codebooks[m].iter().enumerate() {
            let d = l2(sub, c);
            if d < best_dist {
                best_dist = d;
                best = i;
            }
        }
        out.push(best as u8);
    }
}

fn pq_query_tables(query: &[f32], pq: &PQIndex) -> Vec<Vec<f32>> {
    let mut tables = Vec::with_capacity(pq.m);
    for m in 0..pq.m {
        let start = m * pq.sub_dim;
        let end = start + pq.sub_dim;
        let qsub = &query[start..end];
        let mut table = Vec::with_capacity(pq.k);
        for c in &pq.codebooks[m] {
            table.push(dot(qsub, c));
        }
        tables.push(table);
    }
    tables
}

fn l2(a: &[f32], b: &[f32]) -> f32 {
    let mut total = 0.0f32;
    for i in 0..a.len() {
        let diff = a[i] - b[i];
        total += diff * diff;
    }
    total
}

fn nearest_centroid(v: &[f32], centroids: &[Vec<f32>]) -> usize {
    let mut best = 0usize;
    let mut best_score = f32::MIN;
    for (i, c) in centroids.iter().enumerate() {
        let score = dot(v, c);
        if score > best_score {
            best_score = score;
            best = i;
        }
    }
    best
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    let mut total = 0.0f32;
    for i in 0..a.len() {
        total += a[i] * b[i];
    }
    total
}

fn build_hnsw_index(
    vectors: &[Vec<f32>],
    m: usize,
    ef_construction: usize,
    ef_search: usize,
) -> HnswIndex {
    let nb_elem = vectors.len().max(1);
    let nb_layer = ((nb_elem as f32).ln().max(1.0) as usize) + 1;
    let mut hnsw: Hnsw<'static, f32, DistCosine> = Hnsw::new(
        m.max(4),
        nb_elem,
        nb_layer,
        ef_construction.max(10),
        DistCosine,
    );
    for (i, v) in vectors.iter().enumerate() {
        let _ = hnsw.insert((v.as_slice(), i));
    }
    HnswIndex {
        hnsw,
        ef_search: ef_search.max(10),
    }
}

fn quantize_vec(v: &[f32]) -> (Vec<i8>, f32) {
    let mut max_abs = 0.0f32;
    for val in v {
        let abs = val.abs();
        if abs > max_abs {
            max_abs = abs;
        }
    }
    let scale = if max_abs < 1e-6 { 1.0 } else { max_abs / 127.0 };
    let mut out = Vec::with_capacity(v.len());
    for val in v {
        let q = (val / scale).round().clamp(-127.0, 127.0) as i8;
        out.push(q);
    }
    (out, scale)
}

