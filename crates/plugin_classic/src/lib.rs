use plugin_api::ImageAnalysisPlugin;
use image::RgbImage;

/// A pass-through plugin that returns the original image unchanged.
pub struct ClassicPlugin;

impl ImageAnalysisPlugin for ClassicPlugin {
    fn name(&self) -> &str {
        "Classic"
    }

    fn description(&self) -> &str {
        "Displays the original unmodified image as a baseline reference. This plugin performs no processing and simply passes through the input image unchanged. It is useful for comparing the output of other forensic filters against the original. Use this plugin to verify that your image was loaded correctly and to serve as a control when evaluating other analysis results."
    }

    // Returns a clone of the original image, performing no processing.
    fn process(&self, img: &RgbImage) -> Option<RgbImage> {
        Some(img.clone())
    }
}

plugin_api::declare_plugin!(ClassicPlugin, create_classic_plugin);

fn create_classic_plugin() -> ClassicPlugin {
    ClassicPlugin
}
