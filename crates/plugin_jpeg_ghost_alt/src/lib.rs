use plugin_api::ImageAnalysisPlugin;
use image::{ImageBuffer, Rgb, RgbImage};
use jpeg_encoder::Encoder;
use jpeg_encoder::ColorType;

/// Detects JPEG ghost artifacts using an alternative color gradient visualization.
pub struct JPEGGhostAltPlugin;

/// Internal helper methods for the alternative JPEG ghost analysis.
impl JPEGGhostAltPlugin {
    /// Builds a sliding window index list centered at position `pos` within an image of size `width x height`.
    /// The window of size `norm x norm` slides smoothly from the top-left corner to the bottom-right corner
    /// as `pos` moves across the image, matching the GIMP-Forensics normalization approach.
    fn get_norm_window(&self, width: usize, height: usize, pos: usize, norm: usize) -> Vec<usize> {
        let x_pos = pos % width;
        let y_pos = pos / width;

        let (w_xb, l_x) = if norm >= width {
            (0, width)
        } else {
            (((width - norm) * x_pos) / (width.saturating_sub(1)), norm)
        };

        let (w_yb, l_y) = if norm >= height {
            (0, height)
        } else {
            (((height - norm) * y_pos) / (height.saturating_sub(1)), norm)
        };

        let mut index = Vec::with_capacity(norm * norm);
        for y in 0..l_y {
            for x in 0..l_x {
                index.push((w_xb + x) + (w_yb + y) * width);
            }
        }

        index
    }
}

impl ImageAnalysisPlugin for JPEGGhostAltPlugin {
    fn name(&self) -> &str {
        "JPEG Ghost Alt"
    }

    fn description(&self) -> &str {
        "Detects JPEG ghost artifacts using an alternative color gradient visualization approach. The algorithm re-saves the image at a fixed JPEG quality and encodes the error levels into a red-to-green-to-blue color gradient for enhanced visual interpretation. Each color channel maps to a different error intensity range, making it easier to distinguish subtle compression inconsistencies. This method provides a complementary view to the standard JPEG ghost detection, particularly useful when analyzing images with complex compression histories. The color gradient output allows rapid visual identification of regions with differing compression origins."
    }

    fn process(&self, img: &RgbImage) -> Option<RgbImage> {
        let (width, height) = img.dimensions();
        let quality = 90u8;
        let norm = 16usize;

        let rgb_data: Vec<u8> = img.pixels().flat_map(|p| p.0).collect();
        // Recompress image at fixed quality
        let mut buf = Vec::new();
        let encoder = Encoder::new(&mut buf, quality);
        encoder.encode(&rgb_data, width as u16, height as u16, ColorType::Rgb).ok()?;

        let compressed_img = image::load_from_memory_with_format(&buf, image::ImageFormat::Jpeg).ok()?.to_rgb8();

        let pixel_count = (width as usize) * (height as usize);
        let mut square_error = vec![0.0f64; pixel_count];

        // Compute per-pixel squared error across RGB channels
        for y in 0..height {
            for x in 0..width {
                let idx = (y as usize) * (width as usize) + (x as usize);
                let p_orig = img.get_pixel(x, y);
                let p_comp = compressed_img.get_pixel(x, y);
                let mut sum_sq = 0.0;
                for c in 0..3 {
                    let diff = p_orig[c] as f64 - p_comp[c] as f64;
                    sum_sq += diff * diff;
                }
                square_error[idx] = sum_sq;
            }
        }

        let delta = 3.0 * (norm as f64).powi(2);
        let mut gamma = vec![0.0f64; pixel_count];

        // Apply sliding normalization window (GIMP-Forensics style)
        for idx in 0..pixel_count {
            let window = self.get_norm_window(width as usize, height as usize, idx, norm);
            let mut sum = 0.0;
            for &i in &window {
                sum += square_error[i];
            }
            gamma[idx] = sum / delta;
        }

        // Global min-max normalization to [0,1]
        let mut min_gamma = f64::INFINITY;
        let mut max_gamma = f64::NEG_INFINITY;
        for &value in &gamma {
            if value < min_gamma {
                min_gamma = value;
            }
            if value > max_gamma {
                max_gamma = value;
            }
        }

        if max_gamma <= min_gamma {
            return None;
        }

        let mut output = ImageBuffer::new(width, height);
        // Render RGB color gradient: red→green→blue
        for y in 0..height {
            for x in 0..width {
                let idx = (y as usize) * (width as usize) + (x as usize);
                let correct = (gamma[idx] - min_gamma) / (max_gamma - min_gamma);
                let (r, g, b) = if correct > 0.5 {
                    let ratio = (correct - 0.5) / 0.5;
                    (
                        (255.0 * ratio).min(255.0) as u8,
                        (255.0 * (1.0 - ratio)).max(0.0) as u8,
                        0,
                    )
                } else {
                    let ratio = correct / 0.5;
                    (
                        0,
                        (255.0 * ratio).min(255.0) as u8,
                        (255.0 * (1.0 - ratio)).max(0.0) as u8,
                    )
                };
                output.put_pixel(x, y, Rgb([r, g, b]));
            }
        }

        Some(output)
    }
}

plugin_api::declare_plugin!(JPEGGhostAltPlugin, create_jpeg_ghost_alt_plugin);

fn create_jpeg_ghost_alt_plugin() -> JPEGGhostAltPlugin {
    JPEGGhostAltPlugin
}
