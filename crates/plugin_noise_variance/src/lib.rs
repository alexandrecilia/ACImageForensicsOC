use plugin_api::{ImageAnalysisPlugin, declare_plugin};
use image::{ImageBuffer, Luma, RgbImage, Rgb};
use imageproc::filter::gaussian_blur_f32;

/// Detects noise inconsistency across the image by analyzing local variance of the noise residual.
pub struct NoiseVariancePlugin;

impl ImageAnalysisPlugin for NoiseVariancePlugin {
    fn name(&self) -> &str {
        "Noise Variance"
    }

    fn description(&self) -> &str {
        "Detects noise inconsistency across the image by analyzing local variance of the noise residual. The algorithm subtracts a Gaussian-blurred version of the image to isolate high-frequency noise, then computes the variance in sliding windows. A consistent noise variance across the image suggests an authentic capture, while regions with significantly different variance may indicate compositing or local editing. Brighter areas in the output correspond to higher local noise variance, revealing potential tampered regions."
    }

    fn process(&self, img: &RgbImage) -> Option<RgbImage> {
        let (width, height) = img.dimensions();
        
        // 1. Convert to grayscale
        let gray = ImageBuffer::from_fn(width, height, |x, y| {
            let pixel = img.get_pixel(x, y);
            let luma = (0.299 * pixel[0] as f32 + 0.587 * pixel[1] as f32 + 0.114 * pixel[2] as f32) as u8;
            Luma([luma])
        });
        
        // 2. Create smoothed version (low-pass filter)
        let blurred = gaussian_blur_f32(&gray, 1.0);

        let mut noise_map: ImageBuffer<Luma<u8>, Vec<u8>> = ImageBuffer::new(width, height);

        // 3. Sliding window to compute local noise variance
        let window_size: i32 = 4;
        
        for y in window_size..(height as i32 - window_size) {
            for x in window_size..(width as i32 - window_size) {
                let mut variances = Vec::new();

                for dy in -window_size..window_size {
                    for dx in -window_size..window_size {
                        let px = (x + dx) as u32;
                        let py = (y + dy) as u32;
                        
                        // Isolate noise: Original - Blurred
                        let original_val = gray.get_pixel(px, py)[0] as f32;
                        let blurred_val = blurred.get_pixel(px, py)[0] as f32;
                        let noise = original_val - blurred_val;
                        variances.push(noise);
                    }
                }

                // Compute statistical noise variance within the window
                let mean = variances.iter().sum::<f32>() / variances.len() as f32;
                let variance = variances.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / variances.len() as f32;

                // Amplification pour le rendu visuel
                let val = (variance * 50.0).min(255.0) as u8;
                noise_map.put_pixel(x as u32, y as u32, Luma([val]));
            }
        }

        // 4. Fill borders with black
        for y in 0..height {
            for x in 0..width {
                if x < window_size as u32 || x >= width - window_size as u32 ||
                   y < window_size as u32 || y >= height - window_size as u32 {
                    noise_map.put_pixel(x, y, Luma([0]));
                }
            }
        }

        // Render noise map as grayscale RGB image for display
        // 5. Convert to RGB for display
        let result = ImageBuffer::from_fn(width, height, |x, y| {
            let gray_val = noise_map.get_pixel(x, y)[0];
            Rgb([gray_val, gray_val, gray_val])
        });

        Some(result)
    }
}

declare_plugin!(NoiseVariancePlugin, create_noise_variance_plugin);

fn create_noise_variance_plugin() -> NoiseVariancePlugin {
    NoiseVariancePlugin
}
