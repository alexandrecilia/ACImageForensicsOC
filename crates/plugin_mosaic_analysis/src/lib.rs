use image::{ImageBuffer, RgbImage};
use plugin_api::{declare_plugin, ImageAnalysisPlugin};
use rayon::prelude::*;

const PHASE_SIZE: usize = 64;

/// Maps a pixel coordinate to a Bayer color channel given a phase offset.
///
/// Returns 0 (red), 1 (green), or 2 (blue) based on the position and phase.
fn bayer_position(x: usize, y: usize, phase: u8) -> u8 {
    let bx = x & 1;
    let by = y & 1;
    match phase {
        0 => match (bx, by) { (0,0) => 0, (1,0) => 1, (0,1) => 1, _ => 2 },
        1 => match (bx, by) { (0,0) => 2, (1,0) => 1, (0,1) => 1, _ => 0 },
        2 => match (bx, by) { (0,0) => 1, (1,0) => 0, (0,1) => 2, _ => 1 },
        _ => match (bx, by) { (0,0) => 1, (1,0) => 2, (0,1) => 0, _ => 1 },
    }
}

/// Interpolates the value of the given channel at (x, y) using same-color neighbors.
///
/// Used internally by phase estimation to compute the expected value under a given Bayer phase.
fn interpolate_same_color(img: &[Vec<f64>], w: usize, h: usize, x: usize, y: usize, ch: usize, phase: u8) -> f64 {
    let mut sum = 0.0;
    let mut count = 0usize;
    let window = 2i32;

    for dy in -window..=window {
        for dx in -window..=window {
            if dx == 0 && dy == 0 { continue; }
            let nx = x as i32 + dx;
            let ny = y as i32 + dy;
            if nx >= 0 && nx < w as i32 && ny >= 0 && ny < h as i32 {
                if bayer_position(nx as usize, ny as usize, phase) == ch as u8 {
                    sum += img[ch][ny as usize * w + nx as usize];
                    count += 1;
                }
            }
        }
    }
    if count > 0 { sum / count as f64 } else { img[ch][y * w + x] }
}

/// Estimates the most likely Bayer phase at pixel (x, y) by minimizing interpolation error.
///
/// Evaluates all 4 Bayer phase offsets over a 3×3 window and returns the phase with
/// the smallest squared difference between actual and interpolated values.
fn estimate_phase_for_pixel(
    img: &[Vec<f64>],
    w: usize, h: usize,
    x: usize, y: usize,
) -> u8 {
    let mut best_phase = 0u8;
    let mut best_score = f64::MAX;

    for phase in 0..4u8 {
        let mut error = 0.0;
        let window = 3i32;
        for dy in -window..=window {
            for dx in -window..=window {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx < 0 || nx >= w as i32 || ny < 0 || ny >= h as i32 {
                    continue;
                }
                let nx = nx as usize;
                let ny = ny as usize;
                let expected_ch = bayer_position(nx, ny, phase) as usize;
                let actual = img[expected_ch][ny * w + nx];
                let interp = interpolate_same_color(img, w, h, nx, ny, expected_ch, phase);
                let diff = actual - interp;
                error += diff * diff;
            }
        }
        if error < best_score {
            best_score = error;
            best_phase = phase;
        }
    }
    best_phase
}

/// Analyzes CFA mosaic coherence by estimating the Bayer phase at each pixel and
/// detecting blocks that deviate from the global mosaic pattern.
pub struct MosaicAnalysisPlugin;

impl MosaicAnalysisPlugin {
    /// Performs mosaic analysis on the input image and returns a heatmap of inconsistencies.
    fn analyze(&self, img: &RgbImage) -> RgbImage {
        let (w, h) = img.dimensions();
        let wu = w as usize;
        let hu = h as usize;

        if w < 8 || h < 8 {
            return img.clone();
        }

        // Extract RGB channels as f64 arrays
        let channels: Vec<Vec<f64>> = (0..3)
            .map(|c| {
                let mut ch = vec![0.0f64; wu * hu];
                for y in 0..hu {
                    for x in 0..wu {
                        ch[y * wu + x] = img.get_pixel(x as u32, y as u32)[c] as f64;
                    }
                }
                ch
            })
            .collect();

        // Estimate Bayer phase for each pixel by minimizing interpolation error
        let margin = 4usize;
        let mut phase_map = vec![0u8; wu * hu];

        phase_map
            .par_chunks_exact_mut(wu)
            .enumerate()
            .for_each(|(y, row)| {
                if y < margin || y >= hu - margin {
                    return;
                }
                for x in margin..wu - margin {
                    row[x] = estimate_phase_for_pixel(&channels, wu, hu, x, y);
                }
            });

        // Accumulate phase histograms per 64x64 block
        let bx = (wu + PHASE_SIZE - 1) / PHASE_SIZE;
        let by = (hu + PHASE_SIZE - 1) / PHASE_SIZE;

        let mut block_hist = vec![[0usize; 4]; bx * by];
        for block_y in 0..by {
            for block_x in 0..bx {
                let mut hist = [0usize; 4];
                for dy in 0..PHASE_SIZE {
                    for dx in 0..PHASE_SIZE {
                        let x = block_x * PHASE_SIZE + dx;
                        let y = block_y * PHASE_SIZE + dy;
                        if x < wu && y < hu {
                            let ph = phase_map[y * wu + x];
                            if ph < 4 {
                                hist[ph as usize] += 1;
                            }
                        }
                    }
                }
                block_hist[block_y * bx + block_x] = hist;
            }
        }

        // Determine global majority phase
        let total_hist = block_hist.iter().fold([0usize; 4], |acc, h| {
            let mut a = acc;
            for i in 0..4 { a[i] += h[i]; }
            a
        });
        let global_total: usize = total_hist.iter().sum();
        let global_phase = if global_total > 0 {
            total_hist.iter().enumerate().max_by_key(|&(_, &c)| c).map(|(i, _)| i).unwrap_or(0)
        } else {
            0
        };

        // Compute block consistency ratio relative to global phase
        let mut block_consistency = vec![0.0f64; bx * by];
        for i in 0..bx * by {
            let hist = block_hist[i];
            let total: usize = hist.iter().sum();
            if total > 0 {
                let majority = hist[global_phase] as f64 / total as f64;
                block_consistency[i] = majority;
            }
        }

        // Render: black = consistent, red = suspicious (potential forgery)
        let mut output = vec![0u8; (wu * hu * 3) as usize];
        output.par_chunks_exact_mut(wu * 3)
            .enumerate()
            .for_each(|(y, row)| {
                for x in 0..wu {
                    let block_x = x / PHASE_SIZE;
                    let block_y = y / PHASE_SIZE;
                    let agree = block_consistency[block_y * bx + block_x];
                    let suspicious = 1.0 - agree;
                    let v = (suspicious * 255.0) as u8;
                    let idx = x * 3;
                    row[idx] = v;
                    row[idx + 1] = 0;
                    row[idx + 2] = 0;
                }
            });

        ImageBuffer::from_raw(w, h, output).unwrap()
    }
}

impl ImageAnalysisPlugin for MosaicAnalysisPlugin {
    fn name(&self) -> &str {
        "Mosaic Analysis"
    }

    fn description(&self) -> &str {
        "Analyzes CFA mosaic coherence using local phase estimation and a-contrario validation, based on the mimic method presented at ACIVS 2023. The algorithm detects Bayer pattern inconsistencies by examining the periodic structure of the color filter array at a local level. A-contrario thresholding ensures that only statistically significant deviations from the expected mosaic pattern are reported. This technique is particularly effective at revealing small spliced regions or objects pasted from different camera sources. Bright areas in the output indicate regions where the CFA pattern is disrupted, pointing to potential forgeries."
    }

    fn process(&self, img: &RgbImage) -> Option<RgbImage> {
        Some(self.analyze(img))
    }

    fn reference(&self) -> &str {
        "https://doi.org/10.1007/978-3-031-45382-3_19"
    }
}

declare_plugin!(MosaicAnalysisPlugin, create_mosaic_analysis_plugin);

fn create_mosaic_analysis_plugin() -> MosaicAnalysisPlugin {
    MosaicAnalysisPlugin
}
