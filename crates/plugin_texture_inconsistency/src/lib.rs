use plugin_api::ImageAnalysisPlugin;
use image::{ImageBuffer, Rgb, RgbImage, DynamicImage};

/// Detects local texture inconsistencies by comparing each pixel against its neighborhood mean.
pub struct TextureInconsistencyPlugin;

impl TextureInconsistencyPlugin {
    /// Computes a deviation map from local mean texture to highlight inpainted or cloned areas.
    fn analyze_texture_inconsistency(&self, img: &RgbImage) -> ImageBuffer<Rgb<u8>, Vec<u8>> {
        let (width, height) = img.dimensions();
        let dyn_img = DynamicImage::ImageRgb8(img.clone());
        let gray = dyn_img.to_luma8();

        let mut output = ImageBuffer::new(width, height);
        let ws: i32 = 3;

        // Compute local cross-correlation as a measure of texture consistency
        for y in ws..(height as i32 - ws) {
            for x in ws..(width as i32 - ws) {
                // Compute local mean in 7x7 window
                let mut local_mean = 0.0;
                let mut count = 0.0;

                for dy in -ws..=ws {
                    for dx in -ws..=ws {
                        local_mean += gray.get_pixel((x + dx) as u32, (y + dy) as u32)[0] as f32;
                        count += 1.0;
                    }
                }
                local_mean /= count;

                let center = gray.get_pixel(x as u32, y as u32)[0] as f32;
                // Measure pixel deviation from local mean
                let diff = (center - local_mean).abs();
                // Map deviation to grayscale
                let val = (diff * 2.0).min(255.0) as u8;

                output.put_pixel(x as u32, y as u32, Rgb([val, val, val]));
            }
        }

        output
    }
}

impl ImageAnalysisPlugin for TextureInconsistencyPlugin {
    fn name(&self) -> &str {
        "Texture Inconsistency"
    }

    fn description(&self) -> &str {
        "Detects local texture inconsistencies by analyzing cross-correlation within sliding windows. The algorithm computes the absolute difference between each pixel and the local mean of its surrounding neighborhood, measuring how well each pixel's texture matches its local context. Regions where the texture deviates from the expected local pattern are highlighted as potential forgeries. This technique is effective at revealing inpainting, cloning, or splicing operations that disrupt natural texture coherence."
    }

    fn process(&self, img: &RgbImage) -> Option<RgbImage> {
        Some(self.analyze_texture_inconsistency(img))
    }
}

plugin_api::declare_plugin!(TextureInconsistencyPlugin, create_texture_inconsistency_plugin);

fn create_texture_inconsistency_plugin() -> TextureInconsistencyPlugin {
    TextureInconsistencyPlugin
}