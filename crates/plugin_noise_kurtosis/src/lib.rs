use plugin_api::ImageAnalysisPlugin;
use image::{DynamicImage, ImageBuffer, Rgb, RgbImage};
use imageproc::filter::gaussian_blur_f32;

/// Detects image regions where the noise distribution deviates from a Gaussian model.
pub struct NoiseKurtosisPlugin;

impl ImageAnalysisPlugin for NoiseKurtosisPlugin {
    fn name(&self) -> &str {
        "Noise Kurtosis"
    }

    fn description(&self) -> &str {
        "Detects image regions where the noise distribution deviates from a Gaussian model. The algorithm computes the kurtosis of noise residuals in local windows after applying a Gaussian blur subtraction. Natural images captured by digital cameras typically exhibit Gaussian noise, so regions with abnormal kurtosis values may indicate tampering. Bright areas in the output highlight regions where the noise is non-Gaussian, suggesting potential splicing or generative editing."
    }

    fn process(&self, img: &RgbImage) -> Option<RgbImage> {
        let (width, height) = img.dimensions();
        let dyn_img = DynamicImage::ImageRgb8(img.clone());
        // Convert to grayscale
        let gray = dyn_img.to_luma8();
        // Apply Gaussian blur to isolate noise
        let blurred = gaussian_blur_f32(&gray, 1.2);

        let mut output = ImageBuffer::new(width, height);
        let ws: i32 = 4;

        // Extract noise residuals in sliding window
        for y in ws..(height as i32 - ws) {
            for x in ws..(width as i32 - ws) {
                let mut noise_values = Vec::with_capacity(((2 * ws + 1) * (2 * ws + 1)) as usize);

                for dy in -ws..=ws {
                    for dx in -ws..=ws {
                        let px = (x + dx) as u32;
                        let py = (y + dy) as u32;
                        let noise = gray.get_pixel(px, py)[0] as f32 - blurred.get_pixel(px, py)[0] as f32;
                        noise_values.push(noise);
                    }
                }

                // Compute kurtosis of noise distribution
                let n = noise_values.len() as f32;
                let mean = noise_values.iter().sum::<f32>() / n;

                let mut m2 = 0.0f32;
                let mut m4 = 0.0f32;

                for &v in &noise_values {
                    let diff = v - mean;
                    m2 += diff.powi(2);
                    m4 += diff.powi(4);
                }

                m2 /= n;
                m4 /= n;

                // Excess kurtosis = m4/m2^2 - 3 (Gaussian = 0)
                let kurtosis = if m2 > 0.001 {
                    (m4 / m2.powi(2)) - 3.0
                } else {
                    0.0
                };

                // Map kurtosis anomaly to grayscale
                let val = (kurtosis.abs() * 60.0).min(255.0) as u8;
                output.put_pixel(x as u32, y as u32, Rgb([val, val, val]));
            }
        }

        Some(output)
    }
}

plugin_api::declare_plugin!(NoiseKurtosisPlugin, create_noise_kurtosis_plugin);

fn create_noise_kurtosis_plugin() -> NoiseKurtosisPlugin {
    NoiseKurtosisPlugin
}
