use plugin_api::{ImageAnalysisPlugin, ParameterDef};
use image::{Rgb, RgbImage};
use jpeg_encoder::{Encoder, ColorType};
use std::sync::atomic::{AtomicI32, AtomicBool, Ordering};
use std::sync::Arc;

/// Applies a circular shift to the image by `shift_x` and `shift_y` pixels.
/// Pixels that wrap past the right/bottom edge reappear on the left/top edge.
fn roll_image(img: &RgbImage, shift_x: u32, shift_y: u32) -> RgbImage {
    let (w, h) = img.dimensions();
    let sx = shift_x % w;
    let sy = shift_y % h;
    let mut rolled = RgbImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let src_x = (x + w - sx) % w;
            let src_y = (y + h - sy) % h;
            rolled.put_pixel(x, y, *img.get_pixel(src_x, src_y));
        }
    }
    rolled
}

/// JPEG Ghost detection plugin based on Sherloq by GuidoBartoli, implementing
/// the multi-quality ghost analysis from Farid's 'Exposing Digital Forgeries from JPEG Ghosts'.
pub struct SherloqGhostPlugin {
    qmin: AtomicI32,
    qmax: AtomicI32,
    qstep: AtomicI32,
    x_off: AtomicI32,
    y_off: AtomicI32,
    grayscale: Arc<AtomicBool>,
}

impl ImageAnalysisPlugin for SherloqGhostPlugin {
    fn name(&self) -> &str {
        "Sherloq JPEG Ghost"
    }

    fn description(&self) -> &str {
        "JPEG Ghost detection based on Sherloq (github.com/GuidoBartoli/sherloq) by GuidoBartoli, implementing the algorithm from 'Exposing Digital Forgeries from JPEG Ghosts' by Hany Farid. The image is re-compressed at multiple quality levels (Qmin to Qmax by Qstep) and the per-pixel mean squared error across RGB channels is computed at each quality. A 16x16 block averaging removes JPEG lattice noise, and each block position is normalized across quality levels to reveal the quality that produces minimal error. X/Y Offset parameters (0-7) shift the JPEG grid alignment to compensate for 8x8 block lattice misalignment. The output is a grid of block-averaged error maps; darker blocks at a given quality indicate the original compression quality of that region."
    }

    fn reference(&self) -> &str {
        "https://github.com/GuidoBartoli/sherloq"
    }

    fn parameters(&self) -> Vec<ParameterDef> {
        vec![
            ParameterDef { name: "Qmin", min: 0.0, max: 100.0, default: 50.0, is_boolean: false },
            ParameterDef { name: "Qmax", min: 0.0, max: 100.0, default: 90.0, is_boolean: false },
            ParameterDef { name: "Qstep", min: 1.0, max: 20.0, default: 5.0, is_boolean: false },
            ParameterDef { name: "X Offset", min: 0.0, max: 7.0, default: 0.0, is_boolean: false },
            ParameterDef { name: "Y Offset", min: 0.0, max: 7.0, default: 0.0, is_boolean: false },
            ParameterDef { name: "Grayscale", min: 0.0, max: 1.0, default: 1.0, is_boolean: true },
        ]
    }

    fn set_parameter(&self, name: &str, value: f64) {
        match name {
            "Qmin" => self.qmin.store(value.round() as i32, Ordering::Relaxed),
            "Qmax" => self.qmax.store(value.round() as i32, Ordering::Relaxed),
            "Qstep" => self.qstep.store((value.round() as i32).max(1), Ordering::Relaxed),
            "X Offset" => self.x_off.store(value.round() as i32, Ordering::Relaxed),
            "Y Offset" => self.y_off.store(value.round() as i32, Ordering::Relaxed),
            "Grayscale" => self.grayscale.store(value > 0.5, Ordering::Relaxed),
            _ => {}
        }
    }

    fn get_parameter(&self, name: &str) -> Option<f64> {
        match name {
            "Qmin" => Some(self.qmin.load(Ordering::Relaxed) as f64),
            "Qmax" => Some(self.qmax.load(Ordering::Relaxed) as f64),
            "Qstep" => Some(self.qstep.load(Ordering::Relaxed) as f64),
            "X Offset" => Some(self.x_off.load(Ordering::Relaxed) as f64),
            "Y Offset" => Some(self.y_off.load(Ordering::Relaxed) as f64),
            "Grayscale" => Some(if self.grayscale.load(Ordering::Relaxed) { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    fn process(&self, img: &RgbImage) -> Option<RgbImage> {
        let (width, height) = img.dimensions();
        let qmin = self.qmin.load(Ordering::Relaxed).clamp(0, 100);
        let qmax = self.qmax.load(Ordering::Relaxed).clamp(0, 100).max(qmin);
        let qstep = self.qstep.load(Ordering::Relaxed).max(1);
        let x_off = self.x_off.load(Ordering::Relaxed).clamp(0, 7) as u32;
        let y_off = self.y_off.load(Ordering::Relaxed).clamp(0, 7) as u32;
        let grayscale = self.grayscale.load(Ordering::Relaxed);

        let avg_block: u32 = 16;
        let blk_h = (height / avg_block) as usize;
        let blk_w = (width / avg_block) as usize;

        if blk_h == 0 || blk_w == 0 {
            return None;
        }

        // Build quality list
        let qualities: Vec<u8> = (qmin..=qmax).step_by(qstep as usize).map(|q| q as u8).collect();
        let nq = qualities.len();
        if nq == 0 {
            return None;
        }

        // For each quality, apply shift and compute block-averaged MSE
        let mut blk_e = vec![vec![vec![0.0f64; nq]; blk_w]; blk_h];
        // Roll (shift) image to align JPEG grid
        let rolled = roll_image(img, x_off, y_off);

        // For each quality level...
        for (qi, &quality) in qualities.iter().enumerate() {
            let rgb_data: Vec<u8> = rolled.pixels().flat_map(|p| p.0).collect();
            let mut buf = Vec::new();
            let encoder = Encoder::new(&mut buf, quality);
            encoder.encode(&rgb_data, width as u16, height as u16, ColorType::Rgb).ok()?;
            let compressed = image::load_from_memory_with_format(&buf, image::ImageFormat::Jpeg).ok()?.to_rgb8();

            // Compute per-pixel MSE
            // Compute per-pixel MSE across RGB
            let mut mse = vec![vec![0.0f64; width as usize]; height as usize];
            for y in 0..height as usize {
                for x in 0..width as usize {
                    let p_orig = rolled.get_pixel(x as u32, y as u32);
                    let p_comp = compressed.get_pixel(x as u32, y as u32);
                    let mut sum_sq = 0.0f64;
                    for c in 0..3 {
                        let d = p_orig[c] as f64 - p_comp[c] as f64;
                        sum_sq += d * d;
                    }
                    mse[y][x] = sum_sq / 3.0;
                }
            }

            // Average over 16x16 blocks
            for by in 0..blk_h {
                for bx in 0..blk_w {
                    let y_start = by * avg_block as usize;
                    let x_start = bx * avg_block as usize;
                    let mut sum = 0.0f64;
                    for dy in 0..avg_block as usize {
                        for dx in 0..avg_block as usize {
                            sum += mse[y_start + dy][x_start + dx];
                        }
                    }
                    blk_e[by][bx][qi] = sum / (avg_block as f64 * avg_block as f64);
                }
            }
        }

        // Normalize each block position across quality levels: (v - min) / (max - min)
        for by in 0..blk_h {
            for bx in 0..blk_w {
                let mut min_val = f64::INFINITY;
                let mut max_val = f64::NEG_INFINITY;
                for qi in 0..nq {
                    let v = blk_e[by][bx][qi];
                    if v < min_val { min_val = v; }
                    if v > max_val { max_val = v; }
                }
                let range = max_val - min_val;
                if range > 1e-10 {
                    for qi in 0..nq {
                        blk_e[by][bx][qi] = (blk_e[by][bx][qi] - min_val) / range;
                    }
                } else {
                    for qi in 0..nq {
                        blk_e[by][bx][qi] = 0.0;
                    }
                }
            }
        }

        // Render grid of quality tiles
        let sub_h = blk_h * avg_block as usize;
        let sub_w = blk_w * avg_block as usize;
        let sp = (nq as f64).sqrt().ceil() as usize;
        let grid_h = sp * sub_h;
        let grid_w = sp * sub_w;

        let mut output = RgbImage::new(grid_w as u32, grid_h as u32);
        for qi in 0..nq {
            let row = qi / sp;
            let col = qi % sp;
            let y_off_grid = row * sub_h;
            let x_off_grid = col * sub_w;
            for by in 0..blk_h {
                for bx in 0..blk_w {
                    let val = blk_e[by][bx][qi];
                    let gray = (val * 255.0) as u8;
                    for dy in 0..avg_block as usize {
                        for dx in 0..avg_block as usize {
                            let px = x_off_grid + bx * avg_block as usize + dx;
                            let py = y_off_grid + by * avg_block as usize + dy;
                            if px < grid_w && py < grid_h {
                                if grayscale || qi >= (nq - 1) {
                                    // Last quality or grayscale: render as-is
                                    output.put_pixel(px as u32, py as u32, Rgb([gray, gray, gray]));
                                } else {
                                    // Color gradient: earlier qualities red-tinted, later ones blue-tinted
                                    let ratio = qi as f32 / (nq - 1).max(1) as f32;
                                    let r = (gray as f32 * (1.0 - ratio)) as u8;
                                    let b = (gray as f32 * ratio) as u8;
                                    output.put_pixel(px as u32, py as u32, Rgb([r, gray, b]));
                                }
                            }
                        }
                    }
                }
            }
        }

        Some(output)
    }
}

plugin_api::declare_plugin!(SherloqGhostPlugin, create_sherloq_ghost_plugin);

fn create_sherloq_ghost_plugin() -> SherloqGhostPlugin {
    SherloqGhostPlugin {
        qmin: AtomicI32::new(50),
        qmax: AtomicI32::new(90),
        qstep: AtomicI32::new(5),
        x_off: AtomicI32::new(0),
        y_off: AtomicI32::new(0),
        grayscale: Arc::new(AtomicBool::new(true)),
    }
}
