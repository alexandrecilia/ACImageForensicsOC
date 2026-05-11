use plugin_api::{ImageAnalysisPlugin, ParameterDef};
use image::{Rgb, RgbImage, DynamicImage};
use std::sync::atomic::{AtomicI32, Ordering};

// ============================================================
// SRM Filter Kernels (from srm_filter_kernel.py)
// All 30 normalized filters exactly as used by PENet.
// ============================================================

/// filter_class_1: 8 filters, 3x3 (kept as-is, no normalization)
const FILTER_CLASS_1: [[[f32; 3]; 3]; 8] = [
    [[1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 0.0]],
    [[0.0, 1.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 0.0]],
    [[0.0, 0.0, 1.0], [0.0, -1.0, 0.0], [0.0, 0.0, 0.0]],
    [[0.0, 0.0, 0.0], [1.0, -1.0, 0.0], [0.0, 0.0, 0.0]],
    [[0.0, 0.0, 0.0], [0.0, -1.0, 1.0], [0.0, 0.0, 0.0]],
    [[0.0, 0.0, 0.0], [0.0, -1.0, 0.0], [1.0, 0.0, 0.0]],
    [[0.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 1.0, 0.0]],
    [[0.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 1.0]],
];

/// filter_class_2 normalized (÷2): 4 filters, 3x3
const FILTER_CLASS_2: [[[f32; 3]; 3]; 4] = [
    [[0.5, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 0.5]],
    [[0.0, 0.5, 0.0], [0.0, -1.0, 0.0], [0.0, 0.5, 0.0]],
    [[0.0, 0.0, 0.5], [0.0, -1.0, 0.0], [0.5, 0.0, 0.0]],
    [[0.0, 0.0, 0.0], [0.5, -1.0, 0.5], [0.0, 0.0, 0.0]],
];

/// filter_class_3 normalized (÷3): 8 filters, 5x5
const FILTER_CLASS_3: [[[f32; 5]; 5]; 8] = [
    [[-1.0/3.0, 0.0, 0.0, 0.0, 0.0],
     [0.0, 1.0, 0.0, 0.0, 0.0],
     [0.0, 0.0, -1.0, 0.0, 0.0],
     [0.0, 0.0, 0.0, 1.0/3.0, 0.0],
     [0.0, 0.0, 0.0, 0.0, 0.0]],
    [[0.0, 0.0, -1.0/3.0, 0.0, 0.0],
     [0.0, 0.0, 1.0, 0.0, 0.0],
     [0.0, 0.0, -1.0, 0.0, 0.0],
     [0.0, 0.0, 1.0/3.0, 0.0, 0.0],
     [0.0, 0.0, 0.0, 0.0, 0.0]],
    [[0.0, 0.0, 0.0, 0.0, -1.0/3.0],
     [0.0, 0.0, 0.0, 1.0, 0.0],
     [0.0, 0.0, -1.0, 0.0, 0.0],
     [0.0, 1.0/3.0, 0.0, 0.0, 0.0],
     [0.0, 0.0, 0.0, 0.0, 0.0]],
    [[0.0, 0.0, 0.0, 0.0, 0.0],
     [0.0, 0.0, 0.0, 0.0, 0.0],
     [0.0, 1.0/3.0, -1.0, 1.0, -1.0/3.0],
     [0.0, 0.0, 0.0, 0.0, 0.0],
     [0.0, 0.0, 0.0, 0.0, 0.0]],
    [[0.0, 0.0, 0.0, 0.0, 0.0],
     [0.0, 1.0/3.0, 0.0, 0.0, 0.0],
     [0.0, 0.0, -1.0, 0.0, 0.0],
     [0.0, 0.0, 0.0, 1.0, 0.0],
     [0.0, 0.0, 0.0, 0.0, -1.0/3.0]],
    [[0.0, 0.0, 0.0, 0.0, 0.0],
     [0.0, 0.0, 1.0/3.0, 0.0, 0.0],
     [0.0, 0.0, -1.0, 0.0, 0.0],
     [0.0, 0.0, 1.0, 0.0, 0.0],
     [0.0, 0.0, -1.0/3.0, 0.0, 0.0]],
    [[0.0, 0.0, 0.0, 0.0, 0.0],
     [0.0, 0.0, 0.0, 1.0/3.0, 0.0],
     [0.0, 0.0, -1.0, 0.0, 0.0],
     [0.0, 1.0, 0.0, 0.0, 0.0],
     [-1.0/3.0, 0.0, 0.0, 0.0, 0.0]],
    [[0.0, 0.0, 0.0, 0.0, 0.0],
     [0.0, 0.0, 0.0, 0.0, 0.0],
     [-1.0/3.0, 1.0, -1.0, 1.0/3.0, 0.0],
     [0.0, 0.0, 0.0, 0.0, 0.0],
     [0.0, 0.0, 0.0, 0.0, 0.0]],
];

/// filter_edge_3x3 normalized (÷4): 4 filters, 3x3
const FILTER_EDGE_3X3: [[[f32; 3]; 3]; 4] = [
    [[-0.25, 0.5, -0.25], [0.5, -1.0, 0.5], [0.0, 0.0, 0.0]],
    [[0.0, 0.5, -0.25], [0.0, -1.0, 0.5], [0.0, 0.5, -0.25]],
    [[0.0, 0.0, 0.0], [0.5, -1.0, 0.5], [-0.25, 0.5, -0.25]],
    [[-0.25, 0.5, 0.0], [0.5, -1.0, 0.0], [-0.25, 0.5, 0.0]],
];

/// filter_edge_5x5 normalized (÷12): 4 filters, 5x5
const FILTER_EDGE_5X5: [[[f32; 5]; 5]; 4] = [
    [[-1.0/12.0, 1.0/6.0, -1.0/6.0, 1.0/6.0, -1.0/12.0],
     [1.0/6.0, -0.5, 2.0/3.0, -0.5, 1.0/6.0],
     [-1.0/6.0, 2.0/3.0, -1.0, 2.0/3.0, -1.0/6.0],
     [0.0, 0.0, 0.0, 0.0, 0.0],
     [0.0, 0.0, 0.0, 0.0, 0.0]],
    [[0.0, 0.0, -1.0/6.0, 1.0/6.0, -1.0/12.0],
     [0.0, 0.0, 2.0/3.0, -0.5, 1.0/6.0],
     [0.0, 0.0, -1.0, 2.0/3.0, -1.0/6.0],
     [0.0, 0.0, 2.0/3.0, -0.5, 1.0/6.0],
     [0.0, 0.0, -1.0/6.0, 1.0/6.0, -1.0/12.0]],
    [[0.0, 0.0, 0.0, 0.0, 0.0],
     [0.0, 0.0, 0.0, 0.0, 0.0],
     [-1.0/6.0, 2.0/3.0, -1.0, 2.0/3.0, -1.0/6.0],
     [1.0/6.0, -0.5, 2.0/3.0, -0.5, 1.0/6.0],
     [-1.0/12.0, 1.0/6.0, -1.0/6.0, 1.0/6.0, -1.0/12.0]],
    [[-1.0/12.0, 1.0/6.0, -1.0/6.0, 0.0, 0.0],
     [1.0/6.0, -0.5, 2.0/3.0, 0.0, 0.0],
     [-1.0/6.0, 2.0/3.0, -1.0, 0.0, 0.0],
     [1.0/6.0, -0.5, 2.0/3.0, 0.0, 0.0],
     [-1.0/12.0, 1.0/6.0, -1.0/6.0, 0.0, 0.0]],
];

/// square_3x3 normalized (÷4)
const SQUARE_3X3: [[f32; 3]; 3] = [
    [-0.25, 0.5, -0.25],
    [0.5, -1.0, 0.5],
    [-0.25, 0.5, -0.25],
];

/// square_5x5 normalized (÷12)
const SQUARE_5X5: [[f32; 5]; 5] = [
    [-1.0/12.0, 1.0/6.0, -1.0/6.0, 1.0/6.0, -1.0/12.0],
    [1.0/6.0, -0.5, 2.0/3.0, -0.5, 1.0/6.0],
    [-1.0/6.0, 2.0/3.0, -1.0, 2.0/3.0, -1.0/6.0],
    [1.0/6.0, -0.5, 2.0/3.0, -0.5, 1.0/6.0],
    [-1.0/12.0, 1.0/6.0, -1.0/6.0, 1.0/6.0, -1.0/12.0],
];

/// Applies a 3×3 SRM filter kernel with clamped TLU truncation to [-5, 5].
fn apply_conv3x3(img: &image::GrayImage, kernel: &[[f32; 3]; 3]) -> image::GrayImage {
    let (w, h) = img.dimensions();
    let wu = w as usize;
    let hu = h as usize;
    let pixels = img.as_raw();
    let mut out = image::GrayImage::new(w, h);
    for y in 0..hu {
        for x in 0..wu {
            let mut sum = 0.0f32;
            for ky in 0..3 {
                for kx in 0..3 {
                    let sx = (x as i32 + kx as i32 - 1).clamp(0, wu as i32 - 1) as usize;
                    let sy = (y as i32 + ky as i32 - 1).clamp(0, hu as i32 - 1) as usize;
                    sum += pixels[sy * wu + sx] as f32 * kernel[ky][kx];
                }
            }
            let v = sum.clamp(-5.0, 5.0); // TLU truncation
            let p = ((v.abs() / 5.0) * 255.0) as u8;
            out.put_pixel(x as u32, y as u32, image::Luma([p]));
        }
    }
    out
}

/// Applies a 5×5 SRM filter kernel with clamped TLU truncation to [-5, 5].
fn apply_conv5x5(img: &image::GrayImage, kernel: &[[f32; 5]; 5]) -> image::GrayImage {
    let (w, h) = img.dimensions();
    let wu = w as usize;
    let hu = h as usize;
    let pixels = img.as_raw();
    let mut out = image::GrayImage::new(w, h);
    for y in 0..hu {
        for x in 0..wu {
            let mut sum = 0.0f32;
            for ky in 0..5 {
                for kx in 0..5 {
                    let sx = (x as i32 + kx as i32 - 2).clamp(0, wu as i32 - 1) as usize;
                    let sy = (y as i32 + ky as i32 - 2).clamp(0, hu as i32 - 1) as usize;
                    sum += pixels[sy * wu + sx] as f32 * kernel[ky][kx];
                }
            }
            let v = sum.clamp(-5.0, 5.0);
            let p = ((v.abs() / 5.0) * 255.0) as u8;
            out.put_pixel(x as u32, y as u32, image::Luma([p]));
        }
    }
    out
}

/// Applies a single SRM high-pass filter from the PENet steganalysis filter bank (selectable index 0-29).
pub struct PENetSrmSelectorPlugin {
    filter_index: AtomicI32,
}

impl ImageAnalysisPlugin for PENetSrmSelectorPlugin {
    fn name(&self) -> &str {
        "PENet SRM Selector"
    }

    fn description(&self) -> &str {
        "Applies a single SRM (Spatial Rich Model) high-pass filter from the PENet steganalysis filter bank, selectable via the Filter Index parameter (0-29). Each of the 30 filters from the PENet architecture captures different directional high-frequency residual patterns in the image luminance channel. Steganographic embedding alters local noise statistics, and different filters reveal different embedding artifacts — some are best at detecting horizontal patterns, others vertical or diagonal. Use the Filter Index slider to browse individual filter responses and identify which filter best highlights potential hidden data in your image. The output maps the absolute truncated filter response to grayscale (black=no response, white=strong response). Reference: github.com/revere7/PENet_Steganalysis."
    }

    fn reference(&self) -> &str {
        "https://github.com/revere7/PENet_Steganalysis"
    }

    fn parameters(&self) -> Vec<ParameterDef> {
        vec![
            ParameterDef {
                name: "Filter Index",
                min: 0.0,
                max: 29.0,
                default: 0.0,
                is_boolean: false,
            },
        ]
    }

    fn set_parameter(&self, name: &str, value: f64) {
        if name == "Filter Index" {
            self.filter_index.store(value.round() as i32, Ordering::Relaxed);
        }
    }

    fn get_parameter(&self, name: &str) -> Option<f64> {
        if name == "Filter Index" {
            Some(self.filter_index.load(Ordering::Relaxed) as f64)
        } else {
            None
        }
    }

    fn process(&self, img: &RgbImage) -> Option<RgbImage> {
        let (width, height) = img.dimensions();
        let idx = self.filter_index.load(Ordering::Relaxed).clamp(0, 29);
        // Convert to grayscale (Y channel)
        let gray = DynamicImage::ImageRgb8(img.clone()).to_luma8();

        // Select filter by index from SRM filter bank
        let result = match idx {
            // Apply convolution with TLU truncation
            0..=7 => {
                apply_conv3x3(&gray, &FILTER_CLASS_1[idx as usize])
            }
            8..=11 => {
                apply_conv3x3(&gray, &FILTER_CLASS_2[idx as usize - 8])
            }
            12..=19 => {
                apply_conv5x5(&gray, &FILTER_CLASS_3[idx as usize - 12])
            }
            20..=23 => {
                apply_conv3x3(&gray, &FILTER_EDGE_3X3[idx as usize - 20])
            }
            24..=27 => {
                apply_conv5x5(&gray, &FILTER_EDGE_5X5[idx as usize - 24])
            }
            28 => {
                apply_conv3x3(&gray, &SQUARE_3X3)
            }
            29 => {
                apply_conv5x5(&gray, &SQUARE_5X5)
            }
            _ => unreachable!(),
        };

        // Map absolute response to grayscale
        let mut output = RgbImage::new(width, height);
        for y in 0..height {
            for x in 0..width {
                let v = result.get_pixel(x, y)[0];
                output.put_pixel(x, y, Rgb([v, v, v]));
            }
        }

        Some(output)
    }
}

plugin_api::declare_plugin!(PENetSrmSelectorPlugin, create_penet_srm_selector_plugin);

fn create_penet_srm_selector_plugin() -> PENetSrmSelectorPlugin {
    PENetSrmSelectorPlugin {
        filter_index: AtomicI32::new(0),
    }
}
