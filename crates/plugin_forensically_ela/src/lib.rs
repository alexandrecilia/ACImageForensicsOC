use image::{codecs::jpeg::JpegEncoder, ImageBuffer, RgbImage};
use plugin_api::{declare_plugin, ImageAnalysisPlugin, ParameterDef};
use std::io::Cursor;
use std::sync::atomic::{AtomicU8, Ordering};

/// Error Level Analysis plugin that re-encodes at a given JPEG quality and highlights
/// regions where compression artifacts differ from the original.
pub struct ForensicallyELAPlugin {
    quality: AtomicU8,
    error_scale: AtomicU8,
    opacity: AtomicU8,
}

impl ForensicallyELAPlugin {
    pub fn new() -> Self {
        Self {
            quality: AtomicU8::new(90),
            error_scale: AtomicU8::new(20),
            opacity: AtomicU8::new(242),
        }
    }

    /// Performs ELA: re-encodes the image as JPEG, then highlights per-pixel differences.
    fn process_impl(&self, img: &RgbImage) -> Option<RgbImage> {
        let (width, height) = img.dimensions();

        let q = self.quality.load(Ordering::Relaxed);
        let scale = self.error_scale.load(Ordering::Relaxed) as f32;
        let op = self.opacity.load(Ordering::Relaxed) as f32 / 255.0;

        // Re-encode at the specified JPEG quality
        let mut buf = Vec::new();
        let mut cursor = Cursor::new(&mut buf);
        let mut encoder = JpegEncoder::new_with_quality(&mut cursor, q);
        encoder.encode_image(img).ok()?;

        let resaved = image::load_from_memory_with_format(&buf, image::ImageFormat::Jpeg)
            .ok()?
            .to_rgb8();

        // Compute per-pixel absolute error, scale it, blend with original
        let orig_raw = img.as_raw();
        let mut out_pixels = vec![0u8; (width * height * 3) as usize];

        for y in 0..height {
            for x in 0..width {
                let idx = (y * width + x) as usize * 3;
                for c in 0..3 {
                    let diff = (orig_raw[idx + c] as i16 - resaved.get_pixel(x, y)[c] as i16).unsigned_abs() as f32;
                    // Scale difference and blend with original at given opacity
                    let scaled = (diff * scale).min(255.0);
                    let blended = orig_raw[idx + c] as f32 * (1.0 - op) + scaled * op;
                    out_pixels[idx + c] = blended.round() as u8;
                }
            }
        }

        Some(ImageBuffer::from_raw(width, height, out_pixels).unwrap())
    }
}

impl ImageAnalysisPlugin for ForensicallyELAPlugin {
    fn name(&self) -> &str {
        "forensicallyELA"
    }

    fn description(&self) -> &str {
        "Error Level Analysis (Forensically) : JPEG Quality (0–100), Error Scale (0–100), Opacity (0–1)."
    }

    fn process(&self, img: &RgbImage) -> Option<RgbImage> {
        self.process_impl(img)
    }

    fn parameters(&self) -> Vec<ParameterDef> {
        vec![
            ParameterDef {
                name: "JPEG Quality",
                min: 0.0,
                max: 100.0,
                default: 90.0,
                is_boolean: false,
            },
            ParameterDef {
                name: "Error Scale",
                min: 0.0,
                max: 100.0,
                default: 20.0,
                is_boolean: false,
            },
            ParameterDef {
                name: "Opacity",
                min: 0.0,
                max: 1.0,
                default: 0.95,
                is_boolean: false,
            },
        ]
    }

    fn set_parameter(&self, name: &str, value: f64) {
        match name {
            "JPEG Quality" => {
                let v = value.clamp(0.0, 100.0) as u8;
                self.quality.store(v, Ordering::Relaxed);
            }
            "Error Scale" => {
                let v = value.clamp(0.0, 100.0) as u8;
                self.error_scale.store(v, Ordering::Relaxed);
            }
            "Opacity" => {
                let v = (value.clamp(0.0, 1.0) * 255.0) as u8;
                self.opacity.store(v, Ordering::Relaxed);
            }
            _ => {}
        }
    }

    fn get_parameter(&self, name: &str) -> Option<f64> {
        match name {
            "JPEG Quality" => Some(self.quality.load(Ordering::Relaxed) as f64),
            "Error Scale" => Some(self.error_scale.load(Ordering::Relaxed) as f64),
            "Opacity" => Some(self.opacity.load(Ordering::Relaxed) as f64 / 255.0),
            _ => None,
        }
    }

    fn reference(&self) -> &str {
        "https://29a.ch/photo-forensics/#error-level-analysis"
    }
}

declare_plugin!(ForensicallyELAPlugin, create_forensically_ela_plugin);

fn create_forensically_ela_plugin() -> ForensicallyELAPlugin {
    ForensicallyELAPlugin::new()
}
