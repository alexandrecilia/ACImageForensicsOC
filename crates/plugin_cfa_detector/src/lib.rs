use image::{ImageBuffer, RgbImage};
use plugin_api::{declare_plugin, ImageAnalysisPlugin};
use rayon::prelude::*;

/// Detects Bayer demosaicing interpolation inconsistencies by comparing each pixel
/// against a bilinear interpolation of its neighbors.
pub struct CFADetectorPlugin;

impl CFADetectorPlugin {
    /// Computes a cross-channel error map highlighting regions where CFA interpolation is disrupted.
    fn analyze_cfa(&self, img: &RgbImage) -> RgbImage {
        let (width, height) = img.dimensions();
        let mut error_map = vec![0.0f32; (width * height) as usize];

        // Compute bilinear interpolation error at each pixel
        error_map
            .par_chunks_exact_mut(width as usize)
            .enumerate()
            .for_each(|(y_idx, row)| {
                let y = y_idx as u32;
                if y < 2 || y >= height - 2 {
                    return;
                }
                for x in 2..width - 2 {
                    let p = img.get_pixel(x, y);
                    let v_left = img.get_pixel(x - 1, y);
                    let v_right = img.get_pixel(x + 1, y);
                    let v_up = img.get_pixel(x, y - 1);
                    let v_down = img.get_pixel(x, y + 1);

                    let mut total_err = 0.0;
                    for c in 0..3 {
                        // Interpolated value = average of 4 neighbors
                        let interp = (v_left[c] as f32 + v_right[c] as f32
                            + v_up[c] as f32
                            + v_down[c] as f32)
                            / 4.0;
                        total_err += (p[c] as f32 - interp).abs();
                    }
                    row[x as usize] = total_err;
                }
            });

        let mut output_data = vec![0u8; (width * height * 3) as usize];

        output_data
            .par_chunks_exact_mut(width as usize * 3)
            .enumerate()
            .for_each(|(y_idx, row)| {
                let y = y_idx as u32;
                for x in 0..width {
                    let err = error_map[(y * width + x) as usize];
                    // Scale error with gamma curve for better visibility
                    let intensity = (err / 20.0).min(1.0).powf(0.3);
                    let v = (intensity * 255.0) as u8;
                    let out_idx = x as usize * 3;
                    // Render with subtle red tint
                    row[out_idx] = (v as f32 * 0.2) as u8;
                    row[out_idx + 1] = v;
                    row[out_idx + 2] = v;
                }
            });

        ImageBuffer::from_raw(width, height, output_data).unwrap()
    }
}

impl ImageAnalysisPlugin for CFADetectorPlugin {
    fn name(&self) -> &str {
        "CFA Artifact Detector"
    }

    fn description(&self) -> &str {
        "Detects Bayer matrix (demosaicing) interpolation inconsistencies that reveal tampered regions. The algorithm simulates re-demosaicing by bilinear interpolation and compares the result against the original, highlighting areas where the CFA interpolation pattern is disrupted. Most digital cameras use a fixed Bayer pattern for demosaicing, so spliced regions from different sources will exhibit different interpolation artifacts. The output image uses a cross-channel error map where brighter pixels indicate stronger CFA interpolation inconsistencies, suggesting potential image manipulation."
    }

    fn process(&self, img: &RgbImage) -> Option<RgbImage> {
        Some(self.analyze_cfa(img))
    }

    fn reference(&self) -> &str {
        "https://doi.org/10.1109/TSP.2005.855393"
    }
}

declare_plugin!(CFADetectorPlugin, create_cfa_plugin);

fn create_cfa_plugin() -> CFADetectorPlugin {
    CFADetectorPlugin
}
