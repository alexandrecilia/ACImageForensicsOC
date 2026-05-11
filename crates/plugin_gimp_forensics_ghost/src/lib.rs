use plugin_api::{ImageAnalysisPlugin, ParameterDef};
use image::{Rgb, RgbImage};
use jpeg_encoder::{Encoder, ColorType};
use std::sync::atomic::{AtomicI32, AtomicU32, Ordering};

/// JPEG Ghost analysis plugin based on GIMP-Forensics by BernardobL,
/// detecting compression ghosts via sliding-window normalization and RGB color gradient rendering.
pub struct GimpForensicsGhostPlugin {
    quality: AtomicI32,
    block: AtomicU32,
}

impl ImageAnalysisPlugin for GimpForensicsGhostPlugin {
    fn name(&self) -> &str {
        "GIMP Forensics JPEG Ghost"
    }

    fn description(&self) -> &str {
        "JPEG Ghost analysis implementation based on GIMP-Forensics (sourceforge.net/projects/gimp-forensics/) by Bernardo Bulgarelli Labronici. The algorithm re-saves the image at a lower JPEG quality and computes per-pixel squared error summed across RGB channels. A sliding normalization window (size controlled by the Block parameter) averages the squared error over a local neighborhood whose position smoothly varies across the image — pixels at top-left use windows anchored at the top-left corner, while pixels at bottom-right use windows anchored at the bottom-right corner. The result is globally normalized to [0,1] and rendered as an RGB color gradient: high error (1.0–0.5) maps from red to green, low error (0.5–0.0) maps from green to blue. Uniformly colored regions indicate consistent compression, while color transitions reveal areas with a different JPEG compression history."
    }

    fn reference(&self) -> &str {
        "https://sourceforge.net/projects/gimp-forensics/"
    }

    fn parameters(&self) -> Vec<ParameterDef> {
        vec![
            ParameterDef { name: "Quality", min: 1.0, max: 99.0, default: 90.0, is_boolean: false },
            ParameterDef { name: "Block", min: 2.0, max: 64.0, default: 16.0, is_boolean: false },
        ]
    }

    fn set_parameter(&self, name: &str, value: f64) {
        match name {
            "Quality" => self.quality.store(value.round() as i32, Ordering::Relaxed),
            "Block" => self.block.store((value.round() as u32).max(2), Ordering::Relaxed),
            _ => {}
        }
    }

    fn get_parameter(&self, name: &str) -> Option<f64> {
        match name {
            "Quality" => Some(self.quality.load(Ordering::Relaxed) as f64),
            "Block" => Some(self.block.load(Ordering::Relaxed) as f64),
            _ => None,
        }
    }

    fn process(&self, img: &RgbImage) -> Option<RgbImage> {
        let (width, height) = img.dimensions();
        let quality = self.quality.load(Ordering::Relaxed).clamp(1, 99) as u8;
        let norm = self.block.load(Ordering::Relaxed).max(2) as usize;

        let rgb_data: Vec<u8> = img.pixels().flat_map(|p| p.0).collect();
        let mut buf = Vec::new();
        let encoder = Encoder::new(&mut buf, quality);
        encoder.encode(&rgb_data, width as u16, height as u16, ColorType::Rgb).ok()?;
        let compressed = image::load_from_memory_with_format(&buf, image::ImageFormat::Jpeg).ok()?.to_rgb8();

        let total_pixels = (width * height) as usize;

        // Step 1: compute per-pixel squared error summed across channels
        let mut sq_error = vec![0.0f64; total_pixels];
        for y in 0..height {
            for x in 0..width {
                let p_orig = img.get_pixel(x, y);
                let p_comp = compressed.get_pixel(x, y);
                let idx = (y * width + x) as usize;
                let mut sum_sq = 0.0;
                for c in 0..3 {
                    let d = p_orig[c] as f64 - p_comp[c] as f64;
                    sum_sq += d * d;
                }
                sq_error[idx] = sum_sq;
            }
        }

        // Step 2: slide normalization window per pixel (same algorithm as GIMP-Forensics)
        let w = width as usize;
        let h = height as usize;
        let delta = (3.0 * (norm * norm) as f64).max(1.0);
        let mut gamma = vec![0.0f64; total_pixels];

        if norm >= w && norm >= h {
            // window covers the whole image — just sum everything
            let total_sum: f64 = sq_error.iter().sum();
            let val = total_sum / delta;
            for g in gamma.iter_mut() {
                *g = val;
            }
        } else {
            let norm_f = norm as f64;
            let w_f = w as f64;
            let h_f = h as f64;

            for pos in 0..total_pixels {
                let x_pos = pos % w;
                let y_pos = pos / w;

                // sliding window top-left corner (matches GIMP-Forensics algorithm)
                let w_xb = if norm >= w { 0 } else {
                    ((w_f - norm_f) * x_pos as f64 / (w_f - 1.0)).round() as usize
                };
                let w_yb = if norm >= h { 0 } else {
                    ((h_f - norm_f) * y_pos as f64 / (h_f - 1.0)).round() as usize
                };

                let lx = norm.min(w - w_xb);
                let ly = norm.min(h - w_yb);

                let mut sum = 0.0;
                for dy in 0..ly {
                    let row = (w_yb + dy) * w;
                    for dx in 0..lx {
                        sum += sq_error[row + (w_xb + dx)];
                    }
                }
                gamma[pos] = sum / delta;
            }
        }

        // Step 3: global min/max normalization to [0, 1]
        let mut min_g = f64::MAX;
        let mut max_g = f64::MIN;
        for &g in gamma.iter() {
            if g < min_g { min_g = g; }
            if g > max_g { max_g = g; }
        }

        // Step 4: render RGB color gradient (same as GIMP-Forensics)
        let mut output = RgbImage::new(width, height);
        let range = max_g - min_g;

        for pos in 0..total_pixels {
            let correct = if range > 0.0 {
                (gamma[pos] - min_g) / range
            } else {
                0.0
            };

            let (r, g, b) = if correct > 0.5 {
                let t = (correct - 0.5) / 0.5;
                let r = (255.0 * t).round().min(255.0) as u8;
                let g = (255.0 * (1.0 - t)).round().min(255.0) as u8;
                (r, g, 0)
            } else {
                let t = correct / 0.5;
                let g = (255.0 * t).round().min(255.0) as u8;
                let b = (255.0 * (1.0 - t)).round().min(255.0) as u8;
                (0, g, b)
            };

            let x = pos % w;
            let y = pos / w;
            output.put_pixel(x as u32, y as u32, Rgb([r, g, b]));
        }

        Some(output)
    }
}

plugin_api::declare_plugin!(GimpForensicsGhostPlugin, create_gimp_forensics_ghost_plugin);

fn create_gimp_forensics_ghost_plugin() -> GimpForensicsGhostPlugin {
    GimpForensicsGhostPlugin {
        quality: AtomicI32::new(90),
        block: AtomicU32::new(16),
    }
}
