use image::{ImageBuffer, Rgb, RgbImage};
use plugin_api::{declare_plugin, ImageAnalysisPlugin};
use rayon::prelude::*;

const BLOCK: usize = 5;
const MACRO: usize = 256;
const SAMPLES_PER_BIN: usize = 20000;
const N_RATIO: f64 = 0.05;
const M_RATIO: f64 = 0.3;
const DEV_THRESH: f64 = 0.15;

fn dct_basis(size: usize) -> Vec<Vec<f64>> {
    let n = size * size;
    let mut basis = Vec::with_capacity(n);
    for u in 0..size {
        for v in 0..size {
            let mut coeffs = vec![0.0f64; n];
            let cu = if u == 0 { 1.0 / 2.0f64.sqrt() } else { 1.0 };
            let cv = if v == 0 { 1.0 / 2.0f64.sqrt() } else { 1.0 };
            for y in 0..size {
                for x in 0..size {
                    let cos_x = ((2 * x + 1) as f64 * u as f64 * std::f64::consts::PI
                        / (2.0 * size as f64)).cos();
                    let cos_y = ((2 * y + 1) as f64 * v as f64 * std::f64::consts::PI
                        / (2.0 * size as f64)).cos();
                    coeffs[y * size + x] = 2.0 / size as f64 * cu * cv * cos_x * cos_y;
                }
            }
            basis.push(coeffs);
        }
    }
    basis
}

fn dct_2d(block: &[f64], basis: &[Vec<f64>]) -> Vec<f64> {
    let n = block.len();
    let mut coeffs = vec![0.0f64; n];
    for k in 0..n {
        let b = &basis[k];
        coeffs[k] = b.iter().zip(block).map(|(b_i, p_i)| b_i * p_i).sum();
    }
    coeffs
}

fn low_freq_mask(size: usize) -> Vec<bool> {
    let mut mask = vec![false; size * size];
    for u in 0..size {
        for v in 0..size {
            if u + v > 0 && u + v < size { mask[u * size + v] = true; }
        }
    }
    mask
}

struct BlockMeta {
    cx: usize, cy: usize,
    mean: f64, dct_var: f64, std_val: f64,
    saturated: bool,
}

fn process_channel(channel: &[f64], width: usize, height: usize, sat_mask: &[bool]) -> (Vec<u32>, Vec<u32>) {
    let basis = dct_basis(BLOCK);
    let lf_mask = low_freq_mask(BLOCK);
    let num_bx = width - BLOCK + 1;
    let num_by = height - BLOCK + 1;
    let macro_w = width / MACRO + 1;
    let macro_h = height / MACRO + 1;
    let total_mb = macro_w * macro_h;

    let meta: Vec<BlockMeta> = (0..num_by).into_par_iter().flat_map(|by| {
        let basis_ref = &basis;
        let lf_mask_ref = &lf_mask;
        let sat_ref = sat_mask;
        let ch_ref = channel;
        (0..num_bx).into_par_iter().map(move |bx| {
            let mut saturated = false;
            let mut block = [0.0f64; BLOCK * BLOCK];
            for dy in 0..BLOCK {
                for dx in 0..BLOCK {
                    let idx = (by + dy) * width + (bx + dx);
                    block[dy * BLOCK + dx] = ch_ref[idx];
                    if !sat_ref[idx] { saturated = true; }
                }
            }
            if saturated {
                return BlockMeta { cx: bx, cy: by, mean: 0.0, dct_var: 0.0, std_val: 0.0, saturated: true };
            }
            let mean = block.iter().sum::<f64>() / (BLOCK * BLOCK) as f64;
            let coeffs = dct_2d(&block, basis_ref);
            let mut sum_sq = 0.0; let mut cnt = 0;
            for (k, &use_it) in lf_mask_ref.iter().enumerate() {
                if use_it { sum_sq += coeffs[k] * coeffs[k]; cnt += 1; }
            }
            let dct_var = if cnt > 0 { sum_sq / cnt as f64 } else { 0.0 };
            let mut sq_sum = 0.0;
            for &v in &block { let d = v - mean; sq_sum += d * d; }
            let std_val = (sq_sum / (BLOCK * BLOCK) as f64).sqrt();
            BlockMeta { cx: bx, cy: by, mean, dct_var, std_val, saturated: false }
        })
    }).collect();

    let valid: Vec<&BlockMeta> = meta.iter().filter(|m| !m.saturated).collect();
    let mut all_blocks = vec![0u32; total_mb];
    let mut red_blocks = vec![0u32; total_mb];
    if valid.is_empty() { return (all_blocks, red_blocks); }

    let mut by_mean: Vec<&BlockMeta> = valid.iter().copied().collect();
    by_mean.sort_by(|a, b| a.mean.partial_cmp(&b.mean).unwrap());
    let total = by_mean.len();
    let num_bins = std::cmp::max(1, (total + SAMPLES_PER_BIN - 1) / SAMPLES_PER_BIN);

    for bin in 0..num_bins {
        let start = bin * total / num_bins;
        let end = if bin == num_bins - 1 { total } else { (bin + 1) * total / num_bins };
        if start >= end { continue; }
        let slice = &by_mean[start..end];
        let mut by_dct: Vec<&BlockMeta> = slice.to_vec();
        by_dct.sort_by(|a, b| a.dct_var.partial_cmp(&b.dct_var).unwrap());
        let n = std::cmp::max(1, (by_dct.len() as f64 * N_RATIO) as usize);
        let candidates = &by_dct[..n];
        let mut by_std: Vec<&BlockMeta> = candidates.to_vec();
        by_std.sort_by(|a, b| a.std_val.partial_cmp(&b.std_val).unwrap());
        let m_val = std::cmp::max(1, (n as f64 * M_RATIO) as usize);
        for (k, bi) in by_std.iter().enumerate() {
            let idx = (bi.cy / MACRO) * macro_w + (bi.cx / MACRO);
            all_blocks[idx] += 1;
            if k < m_val { red_blocks[idx] += 1; }
        }
    }
    (all_blocks, red_blocks)
}

/// NoiseSniffer binary forgery mask.
pub struct NoisesnifferMaskPlugin;

impl NoisesnifferMaskPlugin {
    fn analyze(&self, img: &RgbImage) -> RgbImage {
        let (w, h) = img.dimensions();
        if w < MACRO as u32 || h < MACRO as u32 {
            return RgbImage::from_pixel(w, h, Rgb([0, 0, 0]));
        }
        let bw = w as usize;
        let bh = h as usize;
        let total_px = bw * bh;

        let mut ch = [vec![0.0f64; total_px], vec![0.0f64; total_px], vec![0.0f64; total_px]];
        let mut sat_mask = vec![true; total_px];

        for y in 0..bh {
            for x in 0..bw {
                let p = img.get_pixel(x as u32, y as u32);
                let idx = y * bw + x;
                ch[0][idx] = p[0] as f64;
                ch[1][idx] = p[1] as f64;
                ch[2][idx] = p[2] as f64;
            }
        }
        for c in 0..3 {
            let mx = ch[c].iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let mn = ch[c].iter().cloned().fold(f64::INFINITY, f64::min);
            for i in 0..total_px {
                if ch[c][i] == mx || ch[c][i] == mn { sat_mask[i] = false; }
            }
        }

        let (a0, r0) = process_channel(&ch[0], bw, bh, &sat_mask);
        let (a1, r1) = process_channel(&ch[1], bw, bh, &sat_mask);
        let (a2, r2) = process_channel(&ch[2], bw, bh, &sat_mask);

        let macro_w = bw / MACRO + 1;
        let macro_h = bh / MACRO + 1;
        let num_mb = macro_w * macro_h;

        let mut mask_mb = vec![false; num_mb];
        for i in 0..num_mb {
            let total = (a0[i] + a1[i] + a2[i]) as f64;
            let red = (r0[i] + r1[i] + r2[i]) as f64;
            if total > 0.0 {
                let ratio = red / total;
                if (ratio - M_RATIO).abs() > DEV_THRESH {
                    mask_mb[i] = true;
                }
            }
        }

        let mut output = vec![0u8; (bw * bh * 3) as usize];
        output.par_chunks_exact_mut(bw * 3).enumerate().for_each(|(y, row)| {
            for x in 0..bw {
                let val = if mask_mb[(y / MACRO) * macro_w + (x / MACRO)] { 255u8 } else { 0u8 };
                let idx = x * 3;
                row[idx] = val;
                row[idx + 1] = val;
                row[idx + 2] = val;
            }
        });

        ImageBuffer::from_raw(w, h, output).unwrap()
    }
}

impl ImageAnalysisPlugin for NoisesnifferMaskPlugin {
    fn name(&self) -> &str { "Noisesniffer Mask" }
    fn description(&self) -> &str {
        "Binary forgery mask based on Noisesniffer (github.com/marinagardella/Noisesniffer) by Marina Gardella et al., IWBF 2021. Uses the same DCT-based noise analysis as Noisesniffer Distribution but renders a binary mask: white pixels indicate 256x256 macro-blocks where the noise consistency deviates significantly from the expected model, signaling potential tampering. Black regions are consistent with the expected noise distribution."
    }
    fn process(&self, img: &RgbImage) -> Option<RgbImage> { Some(self.analyze(img)) }
    fn reference(&self) -> &str { "https://github.com/marinagardella/Noisesniffer" }
}

declare_plugin!(NoisesnifferMaskPlugin, create_noisesniffer_mask_plugin);
fn create_noisesniffer_mask_plugin() -> NoisesnifferMaskPlugin { NoisesnifferMaskPlugin }
