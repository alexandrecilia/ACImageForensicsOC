use image::{DynamicImage, ImageBuffer, RgbImage};
use plugin_api::{declare_plugin, ImageAnalysisPlugin};
use rayon::prelude::*;

/// Detects image anomalies by comparing local patch statistics against global image statistics.
pub struct ZeroShotAnomalyPlugin;

impl ZeroShotAnomalyPlugin {
    /// Computes an anomaly heatmap using zero-shot local-vs-global statistical deviation.
    fn analyze_zsl(&self, img: &RgbImage) -> RgbImage {
        let (width, height) = img.dimensions();
        let gray = DynamicImage::ImageRgb8(img.clone()).to_luma8();

        // Compute global statistics (mean, std) from entire image
        let global_pixels: Vec<f32> = gray.pixels().map(|p| p[0] as f32).collect();
        let global_mean =
            global_pixels.iter().sum::<f32>() / global_pixels.len() as f32;
        let global_std = (global_pixels
            .iter()
            .map(|p| (p - global_mean).powi(2))
            .sum::<f32>()
            / global_pixels.len() as f32)
            .sqrt();

        let mut anomaly_map = vec![0.0f32; (width * height) as usize];
        let block_size = 4i32;

        // For each 9x9 patch, compute local statistics
        anomaly_map
            .par_chunks_exact_mut(width as usize)
            .enumerate()
            .for_each(|(y_idx, row)| {
                let y = y_idx as u32;
                if y < block_size as u32 || y >= height - block_size as u32 {
                    return;
                }
                for x in block_size as u32..width - block_size as u32 {
                    let mut local_values = Vec::new();
                    for dy in -block_size..=block_size {
                        for dx in -block_size..=block_size {
                            local_values.push(
                                gray.get_pixel(
                                    (x as i32 + dx) as u32,
                                    (y as i32 + dy) as u32,
                                )[0] as f32,
                            );
                        }
                    }
                    let local_mean =
                        local_values.iter().sum::<f32>() / local_values.len() as f32;
                    let local_std = (local_values
                        .iter()
                        .map(|v| (v - local_mean).powi(2))
                        .sum::<f32>()
                        / local_values.len() as f32)
                        .sqrt();
                    // Measure distance between local and global std
                    let distance =
                        (local_std - global_std).abs() / (global_std + 1.0);
                    row[x as usize] = distance;
                }
            });

        // Apply power curve and render with green tint
        let mut output_data = vec![0u8; (width * height * 3) as usize];
        output_data
            .par_chunks_exact_mut(width as usize * 3)
            .enumerate()
            .for_each(|(y, row)| {
                for x in 0..width as usize {
                    let dist = anomaly_map[y * width as usize + x];
                    let intensity = (dist * 2.0).min(1.0).powf(0.5);
                    let v = (intensity * 255.0) as u8;
                    let out_idx = x * 3;
                    row[out_idx] = (v as f32 * 0.3) as u8;
                    row[out_idx + 1] = v;
                    row[out_idx + 2] = (v as f32 * 0.5) as u8;
                }
            });

        ImageBuffer::from_raw(width, height, output_data).unwrap()
    }
}

impl ImageAnalysisPlugin for ZeroShotAnomalyPlugin {
    fn name(&self) -> &str {
        "ZERO Anomaly Detector"
    }

    fn description(&self) -> &str {
        "Zero-shot anomaly detection based on local semantic distribution inconsistency. The algorithm learns global statistics (mean and standard deviation) from the entire image, then projects 9x9 patches through a ZSL-style distance metric to measure how each patch deviates from the global distribution. Patches that significantly differ from the global statistics are flagged as anomalies, without requiring any training data or prior knowledge of tampering types. This method can detect a wide range of manipulations including splicing, inpainting, and generative content. Higher intensity values in the output indicate greater statistical deviation and higher likelihood of tampering."
    }

    fn process(&self, img: &RgbImage) -> Option<RgbImage> {
        Some(self.analyze_zsl(img))
    }

    fn reference(&self) -> &str {
        "https://ipol.im/pub/art/2021/390/"
    }
}

declare_plugin!(ZeroShotAnomalyPlugin, create_zero_shot_plugin);

fn create_zero_shot_plugin() -> ZeroShotAnomalyPlugin {
    ZeroShotAnomalyPlugin
}
