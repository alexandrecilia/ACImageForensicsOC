use image::{ImageBuffer, RgbImage};
use imageproc::filter::median_filter;
use plugin_api::{declare_plugin, ImageAnalysisPlugin, ParameterDef};
use std::sync::atomic::{AtomicU8, Ordering};

/// A configurable plugin that extracts and visualizes noise using a separable median filter.
pub struct NoiseAnalysisPlugin {
    amplitude: AtomicU8,
    equalize_hist: AtomicU8,
    opacity: AtomicU8,
}

impl NoiseAnalysisPlugin {
    pub fn new() -> Self {
        Self {
            amplitude: AtomicU8::new(0),
            equalize_hist: AtomicU8::new(255),
            opacity: AtomicU8::new(242),
        }
    }

    /// Applies the full noise analysis pipeline: median filter, noise extraction, amplification, optional equalization, and blending.
    fn process_impl(&self, img: &RgbImage) -> Option<RgbImage> {
        let (width, height) = img.dimensions();

        let amp = self.amplitude.load(Ordering::Relaxed) as f32 + 1.0;
        let eq_hist = self.equalize_hist.load(Ordering::Relaxed) != 0;
        let op = self.opacity.load(Ordering::Relaxed) as f32 / 255.0;

        // Apply separable median filter (horizontal then vertical)
        let median_h = median_filter(img, 1, 0);
        let median_v = median_filter(&median_h, 0, 1);

        // Extract noise = original - median filtered
        let mut noise_data = vec![0i16; (width * height * 3) as usize];
        for y in 0..height {
            for x in 0..width {
                let orig = img.get_pixel(x, y);
                let med = median_v.get_pixel(x, y);
                let idx = (y * width + x) as usize * 3;
                for c in 0..3 {
                    noise_data[idx + c] = orig[c] as i16 - med[c] as i16;
                }
            }
        }

        // Amplify noise and center at 128 (gray)
        let mut noise_viz = vec![0u8; (width * height * 3) as usize];
        for y in 0..height {
            for x in 0..width {
                let idx = (y * width + x) as usize * 3;
                for c in 0..3 {
                    let amplified = noise_data[idx + c] as f32 * amp;
                    let centered = (amplified + 128.0).clamp(0.0, 255.0) as u8;
                    noise_viz[idx + c] = centered;
                }
            }
        }

        // Optionally equalize histogram
        if eq_hist {
            let viz_img = ImageBuffer::from_raw(width, height, noise_viz).unwrap();
            let eq_img = equalize_histogram_rgb(&viz_img);
            noise_viz = eq_img.into_raw();
        }

        // Blend noise overlay with original
        let orig_raw = img.as_raw();
        let mut out_pixels = vec![0u8; (width * height * 3) as usize];
        for i in 0..out_pixels.len() {
            let nv = noise_viz[i] as f32;
            let ov = orig_raw[i] as f32;
            out_pixels[i] = (ov * (1.0 - op) + nv * op).round() as u8;
        }

        Some(ImageBuffer::from_raw(width, height, out_pixels).unwrap())
    }
}

impl ImageAnalysisPlugin for NoiseAnalysisPlugin {
    fn name(&self) -> &str {
        "forensically Noise Analysis"
    }

    fn description(&self) -> &str {
        "Noise analysis using separable median filter. Parameters: Noise Amplitude (1–100), Equalize Histogram (0/1), Opacity (0–1)."
    }

    fn process(&self, img: &RgbImage) -> Option<RgbImage> {
        self.process_impl(img)
    }

    fn parameters(&self) -> Vec<ParameterDef> {
        vec![
            ParameterDef {
                name: "Noise Amplitude",
                min: 1.0,
                max: 100.0,
                default: 1.0,
                is_boolean: false,
            },
            ParameterDef {
                name: "Equalize Histogram",
                min: 0.0,
                max: 1.0,
                default: 1.0,
                is_boolean: true,
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
            "Noise Amplitude" => {
                let v = (value.clamp(1.0, 100.0) - 1.0) as u8;
                self.amplitude.store(v, Ordering::Relaxed);
            }
            "Equalize Histogram" => {
                self.equalize_hist.store(if value >= 0.5 { 255 } else { 0 }, Ordering::Relaxed);
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
            "Noise Amplitude" => Some(self.amplitude.load(Ordering::Relaxed) as f64 + 1.0),
            "Equalize Histogram" => Some(if self.equalize_hist.load(Ordering::Relaxed) != 0 { 1.0 } else { 0.0 }),
            "Opacity" => Some(self.opacity.load(Ordering::Relaxed) as f64 / 255.0),
            _ => None,
        }
    }

    fn reference(&self) -> &str {
        "https://29a.ch/2015/08/21/noise-analysis-for-image-forensics"
    }
}

/// Applies histogram equalization to an RGB image using per-pixel intensity scaling.
fn equalize_histogram_rgb(img: &RgbImage) -> RgbImage {
    let (w, h) = img.dimensions();
    let total = (w * h) as usize;
    let pixels = img.as_raw();

    let mut hist = [0u32; 256];
    for chunk in pixels.chunks(3) {
        let intensity = (chunk[0] as u32 + chunk[1] as u32 + chunk[2] as u32) / 3;
        hist[intensity as usize] += 1;
    }

    let mut cdf = [0u32; 256];
    let mut sum = 0u32;
    for i in 0..256 {
        sum += hist[i];
        cdf[i] = sum;
    }

    let first = cdf.iter().position(|&v| v > 0).unwrap_or(0);
    let scale = 255.0 / (total as f32 - cdf[first] as f32);

    let mut out_pixels = pixels.clone();
    for chunk in out_pixels.chunks_exact_mut(3) {
        let intensity = (chunk[0] as u32 + chunk[1] as u32 + chunk[2] as u32) / 3;
        let eq = ((cdf[intensity as usize].saturating_sub(cdf[first])) as f32 * scale).round() as u8;
        let ratio = if intensity > 0 { eq as f32 / intensity as f32 } else { 1.0 };
        chunk[0] = (chunk[0] as f32 * ratio).clamp(0.0, 255.0) as u8;
        chunk[1] = (chunk[1] as f32 * ratio).clamp(0.0, 255.0) as u8;
        chunk[2] = (chunk[2] as f32 * ratio).clamp(0.0, 255.0) as u8;
    }
    ImageBuffer::from_raw(w, h, out_pixels).unwrap()
}

declare_plugin!(NoiseAnalysisPlugin, create_noise_analysis_plugin);

fn create_noise_analysis_plugin() -> NoiseAnalysisPlugin {
    NoiseAnalysisPlugin::new()
}
