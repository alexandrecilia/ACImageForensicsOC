use plugin_api::{ImageAnalysisPlugin, ParameterDef};
use image::{Rgb, RgbImage};
use jpeg_encoder::{Encoder, ColorType};
use std::sync::atomic::{AtomicI32, AtomicU32, Ordering};

/// Error Level Analysis plugin based on GIMP-Forensics by BernardobL,
/// detecting compression artifacts via re-compression and gamma-corrected histogram stretching.
pub struct GimpForensicsElaPlugin {
    quality: AtomicI32,
    gamma: AtomicU32,
}

impl ImageAnalysisPlugin for GimpForensicsElaPlugin {
    fn name(&self) -> &str {
        "GIMP Forensics ELA"
    }

    fn description(&self) -> &str {
        "Error Level Analysis implementation based on GIMP-Forensics (sourceforge.net/projects/gimp-forensics/) by Bernardo Bulgarelli Labronici. The image is re-saved at a lower JPEG quality and the absolute difference between original and re-compressed versions is computed via per-pixel channel-wise subtraction. The plugin then applies GIMP-style histogram stretching: it scans all pixels to find the global minimum and maximum error values (using per-pixel max across RGB channels), then maps the [min, max] range to full [0, 255] with a gamma correction curve. The gamma parameter controls the mid-tone distribution: gamma < 1 brightens mid-tones, gamma > 1 darkens them. Brighter regions in the output indicate higher error levels that may correspond to tampered areas with a different JPEG compression history."
    }

    fn reference(&self) -> &str {
        "https://sourceforge.net/projects/gimp-forensics/"
    }

    fn parameters(&self) -> Vec<ParameterDef> {
        vec![
            ParameterDef { name: "Quality", min: 1.0, max: 100.0, default: 75.0, is_boolean: false },
            ParameterDef { name: "Gamma", min: 1.0, max: 500.0, default: 100.0, is_boolean: false },
        ]
    }

    fn set_parameter(&self, name: &str, value: f64) {
        match name {
            "Quality" => self.quality.store(value.round() as i32, Ordering::Relaxed),
            "Gamma" => self.gamma.store((value.round() as u32).max(1), Ordering::Relaxed),
            _ => {}
        }
    }

    fn get_parameter(&self, name: &str) -> Option<f64> {
        match name {
            "Quality" => Some(self.quality.load(Ordering::Relaxed) as f64),
            "Gamma" => Some(self.gamma.load(Ordering::Relaxed) as f64),
            _ => None,
        }
    }

    fn process(&self, img: &RgbImage) -> Option<RgbImage> {
        let (width, height) = img.dimensions();
        let quality = self.quality.load(Ordering::Relaxed).clamp(1, 100) as u8;
        let gamma = self.gamma.load(Ordering::Relaxed).clamp(1, 500) as f64 / 100.0;

        // Recompress image at target quality
        let rgb_data: Vec<u8> = img.pixels().flat_map(|p| p.0).collect();
        let mut buf = Vec::new();
        let encoder = Encoder::new(&mut buf, quality);
        encoder.encode(&rgb_data, width as u16, height as u16, ColorType::Rgb).ok()?;
        let compressed = image::load_from_memory_with_format(&buf, image::ImageFormat::Jpeg).ok()?.to_rgb8();

        // Compute absolute difference per channel, take per-pixel max
        let mut diff_pixels = vec![0f64; (width * height) as usize];

        // Find global min/max error values
        let mut min_val = f64::MAX;
        let mut max_val = f64::MIN;

        for y in 0..height {
            for x in 0..width {
                let p_orig = img.get_pixel(x, y);
                let p_comp = compressed.get_pixel(x, y);
                let r = (p_orig[0] as i32 - p_comp[0] as i32).abs() as f64;
                let g = (p_orig[1] as i32 - p_comp[1] as i32).abs() as f64;
                let b = (p_orig[2] as i32 - p_comp[2] as i32).abs() as f64;
                let per_pixel_max = r.max(g).max(b);
                let idx = (y * width + x) as usize;
                diff_pixels[idx] = per_pixel_max;
                if per_pixel_max < min_val { min_val = per_pixel_max; }
                if per_pixel_max > max_val { max_val = per_pixel_max; }
            }
        }

        // Apply gamma-corrected histogram stretch to [0, 255]
        let range = max_val - min_val;
        let gamma_inv = 1.0 / gamma;

        // Output grayscale result
        let mut output = RgbImage::new(width, height);

        for y in 0..height {
            for x in 0..width {
                let idx = (y * width + x) as usize;
                let normalized = if range > 0.0 {
                    ((diff_pixels[idx] - min_val) / range).powf(gamma_inv)
                } else {
                    0.0
                };
                let val = (normalized * 255.0).round().min(255.0) as u8;
                output.put_pixel(x, y, Rgb([val, val, val]));
            }
        }

        Some(output)
    }
}

plugin_api::declare_plugin!(GimpForensicsElaPlugin, create_gimp_forensics_ela_plugin);

fn create_gimp_forensics_ela_plugin() -> GimpForensicsElaPlugin {
    GimpForensicsElaPlugin {
        quality: AtomicI32::new(75),
        gamma: AtomicU32::new(100),
    }
}
