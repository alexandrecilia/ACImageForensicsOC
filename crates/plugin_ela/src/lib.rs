use plugin_api::ImageAnalysisPlugin;
use image::{ImageBuffer, Rgb, RgbImage};
use std::env;

/// Basic Error Level Analysis (ELA) plugin that detects regions with inconsistent JPEG compression.
pub struct ELAPlugin;

/// Internal helper methods for the ELA algorithm.
impl ELAPlugin {
    /// Computes the per-pixel absolute difference between two images.
    fn compute_difference(&self, img1: &RgbImage, img2: &RgbImage) -> RgbImage {
        let (width, height) = img1.dimensions();
        let mut diff = ImageBuffer::new(width, height);

        for y in 0..height {
            for x in 0..width {
                let p1 = img1.get_pixel(x, y);
                let p2 = img2.get_pixel(x, y);
                let r = ((p1[0] as i32) - (p2[0] as i32)).abs() as u8;
                let g = ((p1[1] as i32) - (p2[1] as i32)).abs() as u8;
                let b = ((p1[2] as i32) - (p2[2] as i32)).abs() as u8;
                diff.put_pixel(x, y, Rgb([r, g, b]));
            }
        }
        diff
    }

    /// Amplifies pixel values by the given factor, clamping to [0, 255].
    fn amplify_image(&self, img: &RgbImage, factor: f32) -> RgbImage {
        let (width, height) = img.dimensions();
        let mut amplified = ImageBuffer::new(width, height);

        for y in 0..height {
            for x in 0..width {
                let p = img.get_pixel(x, y);
                let r = (p[0] as f32 * factor).min(255.0) as u8;
                let g = (p[1] as f32 * factor).min(255.0) as u8;
                let b = (p[2] as f32 * factor).min(255.0) as u8;
                amplified.put_pixel(x, y, Rgb([r, g, b]));
            }
        }
        amplified
    }
}

impl ImageAnalysisPlugin for ELAPlugin {
    fn name(&self) -> &str {
        "ELA"
    }

    fn description(&self) -> &str {
        "Error Level Analysis (ELA) detects regions with inconsistent JPEG compression levels. The algorithm re-saves the image at a known JPEG quality setting and computes the per-pixel absolute difference between the original and the re-compressed version. Areas that were originally saved at a different JPEG quality will exhibit a different error pattern than the rest of the image. This technique is effective at revealing spliced regions, especially when the source image has a different compression history than the composite. Brighter pixels in the output indicate higher error levels and potential tampered regions, while uniformly dark areas suggest consistent compression throughout."
    }

    fn process(&self, img: &RgbImage) -> Option<RgbImage> {
        let temp_dir = env::temp_dir();
        let temp_path = temp_dir.join("ela_temp.jpg");

        let dynamic_img = image::DynamicImage::ImageRgb8(img.clone());
        // Save image to temporary JPEG file
        if dynamic_img.save_with_format(&temp_path, image::ImageFormat::Jpeg).is_ok() {
            if let Ok(compressed) = image::open(&temp_path) {
                // Reload compressed version
                let compressed_rgb = compressed.to_rgb8();
                // Compute per-pixel absolute difference
                let diff = self.compute_difference(img, &compressed_rgb);
                // Amplify error for visibility
                let amplified = self.amplify_image(&diff, 8.0);
                // Clean up temp file
                let _ = std::fs::remove_file(&temp_path);
                return Some(amplified);
            }
        }
        let _ = std::fs::remove_file(&temp_path);
        None
    }
}

plugin_api::declare_plugin!(ELAPlugin, create_ela_plugin);

fn create_ela_plugin() -> ELAPlugin {
    ELAPlugin
}
