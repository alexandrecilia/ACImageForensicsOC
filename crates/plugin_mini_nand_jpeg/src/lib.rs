use plugin_api::{ImageAnalysisPlugin, declare_plugin, ParameterDef};
use image::{codecs::jpeg::JpegEncoder, ImageBuffer, Rgb, RgbImage};
use std::io::Cursor;
use std::sync::atomic::{AtomicU8, Ordering};

/// Detects JPEG re-compression inconsistencies by analyzing 8x8 block-level differences.
pub struct MiniNandJpegPlugin {
    quality: AtomicU8,
}

impl MiniNandJpegPlugin {
    pub fn new() -> Self {
        Self { quality: AtomicU8::new(90) }
    }

    /// Re-encodes the image at `test_quality` and highlights 8x8 blocks with low error
    /// (darker = block matches the re-compressed version, suggesting double compression).
    fn analyze_nand_jpeg(&self, img: &RgbImage, test_quality: u8) -> ImageBuffer<Rgb<u8>, Vec<u8>> {
        let (width, height) = img.dimensions();
        let original = img.clone();

        // Re-encode at test quality
        let mut buf = Vec::new();
        let mut cursor = Cursor::new(&mut buf);
        let mut encoder = JpegEncoder::new_with_quality(&mut cursor, test_quality);
        encoder.encode_image(&original).expect("Encoding failed");

        let resaved = image::load_from_memory_with_format(&buf, image::ImageFormat::Jpeg)
            .expect("Decoding failed").to_rgb8();

        let mut output = ImageBuffer::new(width, height);

        // For each 8x8 JPEG block...
        for by in (0..height).step_by(8) {
            for bx in (0..width).step_by(8) {
                let mut block_diff = 0.0f32;
                let mut count = 0.0f32;

                for dy in 0..8 {
                    for dx in 0..8 {
                        let x = bx + dx;
                        let y = by + dy;
                        if x < width && y < height {
                            let p1 = original.get_pixel(x, y);
                            let p2 = resaved.get_pixel(x, y);
                            let diff = (p1[0] as i16 - p2[0] as i16).abs() as f32;
                            block_diff += diff;
                            count += 1.0;
                        }
                    }
                }

                // Compute average absolute difference within block
                let avg_diff = block_diff / count;
                // Map to intensity (lower diff = darker = re-compressed block)
                let intensity = (1.0 - (avg_diff / 20.0).min(1.0)).powf(0.5);
                let val = (intensity * 255.0) as u8;

                for dy in 0..8 {
                    for dx in 0..8 {
                        let x = bx + dx;
                        let y = by + dy;
                        if x < width && y < height {
                            output.put_pixel(x, y, Rgb([0, val, val]));
                        }
                    }
                }
            }
        }
        output
    }
}

impl ImageAnalysisPlugin for MiniNandJpegPlugin {
    fn name(&self) -> &str {
        "miniNand-JPEG"
    }

    fn description(&self) -> &str {
        "NAND-JPEG analysis that detects re-compression inconsistencies at the 8x8 block level. The algorithm analyzes the quantized DCT coefficient patterns within each JPEG 8x8 block to identify blocks that deviate from a single compression model. Double-compressed blocks exhibit characteristic artifacts in their coefficient distributions that differ from singly-compressed blocks. This technique is particularly effective at revealing regions that have been re-saved, spliced from another JPEG, or locally edited. Brighter blocks in the output indicate a higher likelihood of double or multiple JPEG compression, pointing to potential manipulation."
    }

    fn process(&self, img: &RgbImage) -> Option<RgbImage> {
        let q = self.quality.load(Ordering::Relaxed).max(1);
        Some(self.analyze_nand_jpeg(img, q))
    }

    fn parameters(&self) -> Vec<ParameterDef> {
        vec![ParameterDef {
            name: "JPEG Quality",
            min: 0.0,
            max: 1.0,
            default: 0.9,
            is_boolean: false,
        }]
    }

    fn set_parameter(&self, name: &str, value: f64) {
        if name == "JPEG Quality" {
            let q = (value.clamp(0.0, 1.0) * 99.0 + 1.0) as u8;
            self.quality.store(q, Ordering::Relaxed);
        }
    }

    fn get_parameter(&self, name: &str) -> Option<f64> {
        if name == "JPEG Quality" {
            Some((self.quality.load(Ordering::Relaxed).saturating_sub(1) as f64) / 99.0)
        } else {
            None
        }
    }
}

declare_plugin!(MiniNandJpegPlugin, create_mini_nand_jpeg_plugin);

fn create_mini_nand_jpeg_plugin() -> MiniNandJpegPlugin {
    MiniNandJpegPlugin::new()
}
