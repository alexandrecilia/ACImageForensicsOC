use plugin_api::{ImageAnalysisPlugin, ParameterDef};
use image::{Rgb, RgbImage};
use jpeg_encoder::{Encoder, ColorType};
use std::sync::atomic::{AtomicI32, AtomicBool, Ordering};

/// Builds a contrast-stretch lookup table that maps the input range [contrast, 255-contrast]
/// to the full output range [0, 255], clipping values outside that range.
fn build_contrast_lut(contrast: i32) -> [u8; 256] {
    let mut lut = [0u8; 256];
    if contrast == 0 {
        for i in 0..=255 { lut[i] = i as u8; }
        return lut;
    }
    let denom = 255 - 2 * contrast;
    if denom == 0 {
        for i in 0..=255 { lut[i] = if (i as i32) <= contrast { 0 } else { 255 }; }
        return lut;
    }
    for v in 0..=255 {
        let vf = v as f32;
        let c = contrast as f32;
        let mapped = ((vf - c) * 255.0 / (255.0 - 2.0 * c)).clamp(0.0, 255.0);
        lut[v as usize] = mapped.round() as u8;
    }
    lut
}

/// Error Level Analysis plugin based on Sherloq by GuidoBartoli, detecting
/// compression inconsistencies by comparing original and re-compressed images.
pub struct SherloqElaPlugin {
    quality: AtomicI32,
    scale: AtomicI32,
    contrast: AtomicI32,
    linear: AtomicBool,
    grayscale: AtomicBool,
}

impl ImageAnalysisPlugin for SherloqElaPlugin {
    fn name(&self) -> &str {
        "Sherloq ELA"
    }

    fn description(&self) -> &str {
        "Error Level Analysis implementation based on Sherloq (github.com/GuidoBartoli/sherloq) by GuidoBartoli. The image is re-compressed at a given JPEG quality, and the absolute difference between original and compressed versions is computed. In non-linear mode (default), the difference is square-root scaled to enhance subtle compression artifacts. A contrast stretch LUT maps the [contrast, 255-contrast] intensity range to full [0, 255] for improved visibility. Brighter regions indicate higher error levels that may correspond to tampered areas with different compression histories."
    }

    fn reference(&self) -> &str {
        "https://github.com/GuidoBartoli/sherloq"
    }

    fn parameters(&self) -> Vec<ParameterDef> {
        vec![
            ParameterDef { name: "Quality", min: 1.0, max: 100.0, default: 75.0, is_boolean: false },
            ParameterDef { name: "Scale", min: 1.0, max: 100.0, default: 50.0, is_boolean: false },
            ParameterDef { name: "Contrast", min: 0.0, max: 100.0, default: 20.0, is_boolean: false },
            ParameterDef { name: "Linear", min: 0.0, max: 1.0, default: 0.0, is_boolean: true },
            ParameterDef { name: "Grayscale", min: 0.0, max: 1.0, default: 0.0, is_boolean: true },
        ]
    }

    fn set_parameter(&self, name: &str, value: f64) {
        match name {
            "Quality" => self.quality.store(value.round() as i32, Ordering::Relaxed),
            "Scale" => self.scale.store(value.round() as i32, Ordering::Relaxed),
            "Contrast" => self.contrast.store(value.round() as i32, Ordering::Relaxed),
            "Linear" => self.linear.store(value > 0.5, Ordering::Relaxed),
            "Grayscale" => self.grayscale.store(value > 0.5, Ordering::Relaxed),
            _ => {}
        }
    }

    fn get_parameter(&self, name: &str) -> Option<f64> {
        match name {
            "Quality" => Some(self.quality.load(Ordering::Relaxed) as f64),
            "Scale" => Some(self.scale.load(Ordering::Relaxed) as f64),
            "Contrast" => Some(self.contrast.load(Ordering::Relaxed) as f64),
            "Linear" => Some(if self.linear.load(Ordering::Relaxed) { 1.0 } else { 0.0 }),
            "Grayscale" => Some(if self.grayscale.load(Ordering::Relaxed) { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    fn process(&self, img: &RgbImage) -> Option<RgbImage> {
        let (width, height) = img.dimensions();
        let quality = self.quality.load(Ordering::Relaxed).clamp(1, 100) as u8;
        let scale = self.scale.load(Ordering::Relaxed).clamp(1, 100);
        let contrast = self.contrast.load(Ordering::Relaxed).clamp(0, 100);
        let linear = self.linear.load(Ordering::Relaxed);
        let grayscale = self.grayscale.load(Ordering::Relaxed);

        let rgb_data: Vec<u8> = img.pixels().flat_map(|p| p.0).collect();
        // Recompress at target quality
        let mut buf = Vec::new();
        let encoder = Encoder::new(&mut buf, quality);
        encoder.encode(&rgb_data, width as u16, height as u16, ColorType::Rgb).ok()?;
        let compressed = image::load_from_memory_with_format(&buf, image::ImageFormat::Jpeg).ok()?.to_rgb8();

        // Build contrast lookup table
        let lut = build_contrast_lut(contrast);
        let mut output = RgbImage::new(width, height);

        // Linear mode: scale absolute difference directly
        // Apply contrast LUT and optional grayscale conversion
        if linear {
            let sf = scale.min(100) as f32;
            for y in 0..height {
                for x in 0..width {
                    let p_orig = img.get_pixel(x, y);
                    let p_comp = compressed.get_pixel(x, y);
                    let mut rgb = [0u8; 3];
                    for c in 0..3 {
                        let diff = (p_orig[c] as i32 - p_comp[c] as i32).abs();
                        let val = (diff as f32 * sf).min(255.0) as u8;
                        rgb[c] = lut[val as usize];
                    }
                    if grayscale {
                        let gray = (0.299 * rgb[0] as f32 + 0.587 * rgb[1] as f32 + 0.114 * rgb[2] as f32) as u8;
                        output.put_pixel(x, y, Rgb([gray, gray, gray]));
                    } else {
                        output.put_pixel(x, y, Rgb(rgb));
                    }
                }
            }
        // Non-linear mode: sqrt scaling to enhance subtle artifacts
        } else {
            let sf = scale as f32 / 20.0;
            for y in 0..height {
                for x in 0..width {
                    let p_orig = img.get_pixel(x, y);
                    let p_comp = compressed.get_pixel(x, y);
                    let mut rgb = [0u8; 3];
                    for c in 0..3 {
                        let diff = (p_orig[c] as f32 / 255.0 - p_comp[c] as f32 / 255.0).abs();
                        let val = (diff.sqrt() * 255.0 * sf).min(255.0) as u8;
                        rgb[c] = lut[val as usize];
                    }
                    if grayscale {
                        let gray = (0.299 * rgb[0] as f32 + 0.587 * rgb[1] as f32 + 0.114 * rgb[2] as f32) as u8;
                        output.put_pixel(x, y, Rgb([gray, gray, gray]));
                    } else {
                        output.put_pixel(x, y, Rgb(rgb));
                    }
                }
            }
        }

        Some(output)
    }
}

plugin_api::declare_plugin!(SherloqElaPlugin, create_sherloq_ela_plugin);

fn create_sherloq_ela_plugin() -> SherloqElaPlugin {
    SherloqElaPlugin {
        quality: AtomicI32::new(75),
        scale: AtomicI32::new(50),
        contrast: AtomicI32::new(20),
        linear: AtomicBool::new(false),
        grayscale: AtomicBool::new(false),
    }
}
