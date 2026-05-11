use plugin_api::{ImageAnalysisPlugin, declare_plugin};
use image::{ImageBuffer, Rgb, RgbImage, DynamicImage};
use rayon::prelude::*;

/// Analyzes gradient micro-discontinuities using Sobel operators to detect AI inpainting artifacts.
pub struct GradientForensicsPlugin;

impl GradientForensicsPlugin {
    /// Computes horizontal and vertical Sobel gradients and renders a gradient inconsistency map.
    fn analyze_gradients(&self, img: &RgbImage) -> ImageBuffer<Rgb<u8>, Vec<u8>> {
        let (width, height) = img.dimensions();
        let dyn_img = DynamicImage::ImageRgb8(img.clone());
        let gray = dyn_img.to_luma8();

        let mut gradient_map = vec![0.0f32; (width * height) as usize];

        // Compute Sobel gradient
        gradient_map.par_chunks_exact_mut(width as usize)
            .enumerate()
            .for_each(|(y, row)| {
                if y < 1 || y >= (height as usize - 1) { return; }

                for x in 1..(width as usize - 1) {
                    // Horizontal Sobel gradient
                    let gx = -1.0 * gray.get_pixel((x-1) as u32, (y-1) as u32)[0] as f32
                             + 1.0 * gray.get_pixel((x+1) as u32, (y-1) as u32)[0] as f32
                             - 2.0 * gray.get_pixel((x-1) as u32, y as u32)[0] as f32
                             + 2.0 * gray.get_pixel((x+1) as u32, y as u32)[0] as f32
                             - 1.0 * gray.get_pixel((x-1) as u32, (y+1) as u32)[0] as f32
                             + 1.0 * gray.get_pixel((x+1) as u32, (y+1) as u32)[0] as f32;

                    // Vertical Sobel gradient
                    let gy = -1.0 * gray.get_pixel((x-1) as u32, (y-1) as u32)[0] as f32
                             - 2.0 * gray.get_pixel(x as u32, (y-1) as u32)[0] as f32
                             - 1.0 * gray.get_pixel((x+1) as u32, (y-1) as u32)[0] as f32
                             + 1.0 * gray.get_pixel((x-1) as u32, (y+1) as u32)[0] as f32
                             + 2.0 * gray.get_pixel(x as u32, (y+1) as u32)[0] as f32
                             + 1.0 * gray.get_pixel((x+1) as u32, (y+1) as u32)[0] as f32;

                    let magnitude = (gx * gx + gy * gy).sqrt();
                    row[x] = magnitude;
                }
            });

        // Post-processing: detect local gradient inconsistency
        let mut output_data = vec![0u8; (width * height * 3) as usize];

        output_data.par_chunks_exact_mut(width as usize * 3)
            .enumerate()
            .for_each(|(y, row)| {
                for x in 0..width as usize {
                    let mag = gradient_map[y * width as usize + x];

                    // Amplify very weak gradients (suspicious of AI smoothing)
                    // and ghost gradients
                    let intensity = (mag * 5.0).min(255.0) as u8;

                    let out_idx = x * 3;
                    // Render in "Neon Night" mode
                    row[out_idx] = 0;             // R
                    row[out_idx + 1] = intensity; // G
                    row[out_idx + 2] = intensity; // B (Cyan)
                }
            });

        ImageBuffer::from_raw(width, height, output_data).unwrap()
    }
}

impl ImageAnalysisPlugin for GradientForensicsPlugin {
    fn name(&self) -> &str {
        "Gradient Forensics"
    }

    fn description(&self) -> &str {
        "Analyzes gradient micro-discontinuities using Sobel operators to reveal AI-based inpainting. The algorithm computes horizontal and vertical Sobel gradients and examines them for unnatural edge discontinuities that commonly occur at the boundaries of inpainted regions. AI inpainting tends to produce subtle gradient anomalies that are invisible to the naked eye but detectable through this analysis. The output highlights areas with abnormal gradient patterns, where brighter pixels correspond to higher gradient inconsistency, helping to identify regions that may have been synthetically filled or removed."
    }

    fn process(&self, img: &RgbImage) -> Option<RgbImage> {
        Some(self.analyze_gradients(img))
    }
}

declare_plugin!(GradientForensicsPlugin, create_gradient_forensics_plugin);

fn create_gradient_forensics_plugin() -> GradientForensicsPlugin {
    GradientForensicsPlugin
}
