use image::RgbImage;

/// Definition of an adjustable parameter for a plugin
#[derive(Clone)]
pub struct ParameterDef {
    pub name: &'static str,
    pub min: f64,
    pub max: f64,
    pub default: f64,
    pub is_boolean: bool,
}

/// Trait that all image analysis plugins must implement
pub trait ImageAnalysisPlugin: Send + Sync {
    /// Unique plugin name
    fn name(&self) -> &str;
    
    /// Plugin description
    fn description(&self) -> &str;
    
    /// Process an image and return the result
    fn process(&self, img: &RgbImage) -> Option<RgbImage>;
    
    /// Return the list of adjustable parameters
    fn parameters(&self) -> Vec<ParameterDef> {
        Vec::new()
    }
    
    /// Modify a parameter (normalized value 0.0 to 1.0)
    fn set_parameter(&self, _name: &str, _value: f64) {}
    
    /// Get the normalized value of a parameter (0.0 to 1.0)
    fn get_parameter(&self, _name: &str) -> Option<f64> {
        None
    }

    /// Bibliographic reference or URL of the algorithm
    fn reference(&self) -> &str {
        ""
    }
}

/// Type for the plugin export function
pub type PluginCreate = unsafe extern "C" fn() -> *mut dyn ImageAnalysisPlugin;

/// Macro to export a plugin
#[macro_export]
macro_rules! declare_plugin {
    ($plugin_type:ty, $constructor:path) => {
        #[no_mangle]
        pub extern "C" fn create_plugin() -> *mut dyn $crate::ImageAnalysisPlugin {
            let object = $constructor();
            let boxed: Box<dyn $crate::ImageAnalysisPlugin> = Box::new(object);
            Box::into_raw(boxed)
        }
    };
}
