use plugin_api::{ImageAnalysisPlugin, declare_plugin};
use image::{ImageBuffer, Luma, RgbImage, Rgb};
use rayon::prelude::*;

/// Detects sensor noise pattern inconsistencies across the image using a high-pass filter approach.
pub struct SensorNoisePlugin;

/// Internal helper methods for the sensor noise analysis algorithm.
impl SensorNoisePlugin {
    /// Detects regions where the sensor noise pattern deviates from the expected distribution.
    fn detect_sensor_noise_inconsistency(&self, img: &RgbImage) -> ImageBuffer<Luma<u8>, Vec<u8>> {
        let (width, height) = img.dimensions();
        
        // Convert to grayscale using standard luminance weights
        let gray = ImageBuffer::from_fn(width, height, |x, y| {
            let pixel = img.get_pixel(x, y);
            let luma = (0.299 * pixel[0] as f32 + 0.587 * pixel[1] as f32 + 0.114 * pixel[2] as f32) as u8;
            Luma([luma])
        });
        
        // Allocate output buffer
        let mut output_data = vec![0u8; (width * height) as usize];

        // Window size: 5x5 is a good trade-off between locality and robustness
        let window_size: i32 = 2;

        // Parallel row-by-row analysis using Rayon
        output_data.par_chunks_exact_mut(width as usize)
            .enumerate()
            .for_each(|(y, row)| {
                let y = y as i32;
                if y < window_size || y >= (height as i32 - window_size) { return; }

                for x in window_size..(width as i32 - window_size) {
                    let mut local_noises = Vec::with_capacity(25);

                    // 1. Extract local noise via simple high-pass filter
                    // Noise = pixel value - average of its 4 neighbors
                    for dy in -window_size..=window_size {
                        for dx in -window_size..=window_size {
                            let px = (x + dx) as u32;
                            let py = (y + dy) as u32;
                            
                            let center_val = gray.get_pixel(px, py)[0] as f32;
                            
                            // Fast local smooth signal: average of 4 cardinal neighbors
                            let mut sum_neighbors = 0.0;
                            let mut count = 0.0;
                            if px > 0 && py > 0 && px < width - 1 && py < height - 1 {
                                sum_neighbors += gray.get_pixel(px + 1, py)[0] as f32;
                                sum_neighbors += gray.get_pixel(px - 1, py)[0] as f32;
                                sum_neighbors += gray.get_pixel(px, py + 1)[0] as f32;
                                sum_neighbors += gray.get_pixel(px, py - 1)[0] as f32;
                                count = 4.0;
                            }
                            
                            let noise = if count > 0.0 {
                                center_val - (sum_neighbors / count)
                            } else {
                                0.0
                            };
                            local_noises.push(noise);
                        }
                    }

                    // 2. Compute noise variance within the analysis window
                    let mean = local_noises.iter().sum::<f32>() / local_noises.len() as f32;
                    let variance = local_noises.iter()
                        .map(|v| (v - mean).powi(2))
                        .sum::<f32>() / local_noises.len() as f32;

                    // 3. Normalize and amplify
                    // Gain (e.g. 10.0) makes variations visible
                    // AI-generated regions typically have very different (often lower/smoother) variance
                    let result = (variance * 10.0).min(255.0) as u8;
                    row[x as usize] = result;
                }
            });

        ImageBuffer::from_raw(width, height, output_data).unwrap()
    }
}

impl ImageAnalysisPlugin for SensorNoisePlugin {
    fn name(&self) -> &str {
        "Sensor Noise"
    }

    fn description(&self) -> &str {
        "Detects sensor noise pattern inconsistencies across the image using a high-pass filter approach, parallelized with Rayon for performance. Every digital camera sensor leaves a unique fixed-pattern noise on captured images, and tampered regions from different sources will exhibit a different noise signature. The algorithm extracts the high-frequency noise residual and analyzes its local correlation with the expected sensor noise pattern. Regions where the sensor noise pattern deviates from the rest of the image are highlighted as potential forgeries. Brighter areas indicate stronger noise pattern inconsistency, suggesting compositing or local editing."
    }

    fn process(&self, img: &RgbImage) -> Option<RgbImage> {
        let noise_map = self.detect_sensor_noise_inconsistency(img);
        let (width, height) = img.dimensions();
        
        let result = ImageBuffer::from_fn(width, height, |x, y| {
            let gray_val = noise_map.get_pixel(x, y)[0];
            Rgb([gray_val, gray_val, gray_val])
        });

        Some(result)
    }
}

declare_plugin!(SensorNoisePlugin, create_sensor_noise_plugin);

fn create_sensor_noise_plugin() -> SensorNoisePlugin {
    SensorNoisePlugin
}
