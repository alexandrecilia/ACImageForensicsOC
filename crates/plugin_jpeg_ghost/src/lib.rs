use plugin_api::ImageAnalysisPlugin;
use image::{ImageBuffer, Rgb, RgbImage};
use jpeg_encoder::{Encoder, ColorType};
use ndarray::prelude::*;

/// Detects JPEG ghost artifacts by sweeping a range of quality levels and identifying re-compressed regions.
pub struct JPEGGhostPlugin;

impl ImageAnalysisPlugin for JPEGGhostPlugin {
    fn name(&self) -> &str {
        "JPEG Ghost"
    }

    fn description(&self) -> &str {
        "Detects JPEG ghost artifacts by re-saving the image at multiple quality levels and identifying regions that appear darker at a specific quality setting. The algorithm iterates through a grid of JPEG quality values, re-compresses the image at each level, and computes the difference from the original. When the re-compression quality matches the original save quality of a particular region, that region will exhibit minimal error (a ghost). This technique is highly effective at identifying spliced regions that originate from JPEG images saved at a different quality than the surrounding area. Darker regions in the output correspond to JPEG ghosts that reveal the original compression quality of tampered areas. The multi-quality grid approach ensures that ghosts at any quality level are detected."
    }

    fn process(&self, img: &RgbImage) -> Option<RgbImage> {
        let (width, height) = img.dimensions();
        let rgb_data: Vec<u8> = img.pixels().flat_map(|p| p.0).collect();

        // Define quality sweep range
        let qmin = 50;
        let qmax = 90;
        let qstep = 10;
        let qualities: Vec<u8> = (qmin..=qmax).step_by(qstep).collect();
        let nq = qualities.len();
        let averaging_block = 16;

        let mut ghostmap = Array3::<f64>::zeros((height as usize, width as usize, nq));

        // For each quality level: encode, decode, compute MSE
        for (i, &quality) in qualities.iter().enumerate() {
            let mut buf = Vec::new();
            let encoder = Encoder::new(&mut buf, quality);
            encoder.encode(&rgb_data, width as u16, height as u16, ColorType::Rgb).ok()?;

            let compressed_img = image::load_from_memory_with_format(&buf, image::ImageFormat::Jpeg).ok()?.to_rgb8();

            for y in 0..height {
                for x in 0..width {
                    let p_orig = img.get_pixel(x, y);
                    let p_comp = compressed_img.get_pixel(x, y);
                    let mut sum_sq = 0.0;
                    for c in 0..3 {
                        let diff = p_orig[c] as f64 - p_comp[c] as f64;
                        sum_sq += diff * diff;
                    }
                    ghostmap[[y as usize, x as usize, i]] = sum_sq / 3.0;
                }
            }
        }

        // Average MSE over 16x16 blocks
        let blk_h = height as usize / averaging_block;
        let blk_w = width as usize / averaging_block;
        let mut blk_e = Array3::<f64>::zeros((blk_h, blk_w, nq));

        for c in 0..nq {
            for by in 0..blk_h {
                for bx in 0..blk_w {
                    let y_start = by * averaging_block;
                    let x_start = bx * averaging_block;
                    let block = ghostmap.slice(s![y_start..y_start+averaging_block, x_start..x_start+averaging_block, c]);
                    blk_e[[by, bx, c]] = block.mean().unwrap_or(0.0);
                }
            }
        }

        // Normalize each block across quality levels (min-max per block)
        let minval = blk_e.fold_axis(Axis(2), f64::INFINITY, |a, &b| a.min(b));
        let maxval = blk_e.fold_axis(Axis(2), f64::NEG_INFINITY, |a, &b| a.max(b));

        for c in 0..nq {
            for by in 0..blk_h {
                for bx in 0..blk_w {
                    let m = minval[[by, bx]];
                    let max_v = maxval[[by, bx]];
                    if max_v > m {
                        blk_e[[by, bx, c]] = (blk_e[[by, bx, c]] - m) / (max_v - m);
                    } else {
                        blk_e[[by, bx, c]] = 0.0;
                    }
                }
            }
        }

        // Render grid of quality tiles
        let sub_h = blk_h * averaging_block;
        let sub_w = blk_w * averaging_block;
        let sp = (nq as f64).sqrt().ceil() as usize;
        let grid_h = sp * sub_h;
        let grid_w = sp * sub_w;
        let mut output = ImageBuffer::new(grid_w as u32, grid_h as u32);

        for i in 0..nq {
            let row = i / sp;
            let col = i % sp;
            let y_offset = row * sub_h;
            let x_offset = col * sub_w;
            for by in 0..blk_h {
                for bx in 0..blk_w {
                    let val = blk_e[[by, bx, i]];
                    let gray = (val * 255.0) as u8;
                    for dy in 0..averaging_block {
                        for dx in 0..averaging_block {
                            let x = x_offset + bx * averaging_block + dx;
                            let y = y_offset + by * averaging_block + dy;
                            if x < grid_w && y < grid_h {
                                output.put_pixel(x as u32, y as u32, Rgb([gray, gray, gray]));
                            }
                        }
                    }
                }
            }
        }

        Some(output)
    }
}

plugin_api::declare_plugin!(JPEGGhostPlugin, create_jpeg_ghost_plugin);

fn create_jpeg_ghost_plugin() -> JPEGGhostPlugin {
    JPEGGhostPlugin
}
