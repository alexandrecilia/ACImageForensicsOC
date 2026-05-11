use eframe::egui;
use image::RgbImage;
use plugin_api::ImageAnalysisPlugin;
use std::path::PathBuf;
use libloading::{Library, Symbol};

/// Wraps a loaded DLL plugin, holding the library handle and the plugin trait object.
struct PluginInstance {
    _library: Library,
    plugin: Box<dyn ImageAnalysisPlugin>,
}

/// Main application state for the forensic image viewer.
struct ImageViewer {
    image_path: Option<PathBuf>,
    original_image: Option<RgbImage>,
    processed_image: Option<RgbImage>,
    texture: Option<egui::TextureHandle>,
    original_texture: Option<egui::TextureHandle>,
    zoom: f32,
    pan: egui::Vec2,
    plugins: Vec<PluginInstance>,
    selected_plugin: usize,
    right_click_down: bool,
    zoom_nearest: bool,
}

/// Default constructor that initializes state and loads all available plugins.
impl Default for ImageViewer {
    fn default() -> Self {
        let mut viewer = Self {
            image_path: None,
            original_image: None,
            processed_image: None,
            texture: None,
            original_texture: None,
            zoom: 1.0,
            pan: egui::Vec2::ZERO,
            plugins: Vec::new(),
            selected_plugin: 0,
            right_click_down: false,
            zoom_nearest: true,
        };
        viewer.load_plugins();
        viewer
    }
}

/// Internal helper methods for plugin loading, image processing, and UI state management.
impl ImageViewer {
    // Loads all available plugins by scanning for plugin_*.dll in the executable directory.
    fn load_plugins(&mut self) {
        let mut search_dirs = Vec::new();

        // Exe directory (primary)
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                search_dirs.push(dir.to_path_buf());
            }
        }
        // Current directory (secondary, for development)
        if let Ok(cwd) = std::env::current_dir() {
            search_dirs.push(cwd);
        }

        // Track by filename only (not full path) to avoid loading the same plugin twice
        let mut seen_names = std::collections::HashSet::new();

        for dir in &search_dirs {
            if !dir.exists() { continue; }
            let entries = match std::fs::read_dir(dir) {
                Ok(e) => e,
                Err(_) => continue,
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let filename = match path.file_name().and_then(|n| n.to_str()) {
                    Some(f) => f.to_string(),
                    None => continue,
                };
                if !filename.starts_with("plugin_") || !filename.ends_with(".dll") {
                    continue;
                }
                // Deduplicate by filename (not path) — same plugin in multiple dirs
                if !seen_names.insert(filename.clone()) {
                    continue;
                }
                // Skip known non-functional DLLs
                if filename == "plugin_penet_steganalysis.dll" { continue; }
                eprintln!("Loading plugin: {}", path.display());
                match unsafe { self.load_plugin(path.to_str().unwrap()) } {
                    Ok(plugin) => {
                        eprintln!("✓ Plugin loaded: {}", plugin.plugin.name());
                        self.plugins.push(plugin);
                    }
                    Err(e) => eprintln!("✗ Error loading {}: {:?}", filename, e),
                }
            }
        }
        eprintln!("Total plugins loaded: {}", self.plugins.len());
        self.plugins.sort_by(|a, b| a.plugin.name().cmp(b.plugin.name()));
    }

    // Loads a single plugin DLL via unsafe FFI, retrieving the `create_plugin` constructor symbol.
    unsafe fn load_plugin(&self, path: &str) -> Result<PluginInstance, Box<dyn std::error::Error>> {
        let lib = Library::new(path)?;
        let constructor: Symbol<plugin_api::PluginCreate> = lib.get(b"create_plugin")?;
        let plugin = Box::from_raw(constructor());
        Ok(PluginInstance {
            _library: lib,
            plugin,
        })
    }

    // Opens a save-file dialog and writes the current processed image to disk.
    fn save_current_result(&self) {
        if let Some(ref img) = self.processed_image {
            let stem = self.image_path
                .as_ref()
                .and_then(|p| p.file_stem())
                .and_then(|s| s.to_str())
                .unwrap_or("image");
            let plugin_name = self.plugins
                .get(self.selected_plugin)
                .map(|p| p.plugin.name())
                .unwrap_or("result");
            let suggested = format!("{}_{}.png", stem, plugin_name);
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("PNG", &["png"])
                .set_file_name(&suggested)
                .save_file()
            {
                if img.save(&path).is_ok() {
                    self.set_status(&format!("Image saved: {}", path.display()));
                } else {
                    self.set_status("Error saving image");
                }
            }
        } else {
            self.set_status("No image to save");
        }
    }

    fn export_all_html(&mut self, ctx: &egui::Context) {
        // Clone the original image; abort if nothing is loaded
        let original = match self.original_image {
            Some(ref img) => img.clone(),
            None => {
                self.set_status("No image loaded");
                return;
            }
        };

        // Pick an output directory via the system folder dialog
        let dir = match rfd::FileDialog::new().pick_folder() {
            Some(d) => d,
            None => return,
        };

        // Derive output subdirectory name from the image filename stem
        let stem = self
            .image_path
            .as_ref()
            .and_then(|p| p.file_stem())
            .and_then(|s| s.to_str())
            .unwrap_or("image");

        let out_dir = dir.join(format!("{}_forensics", stem));
        std::fs::create_dir_all(&out_dir).unwrap_or_default();

        // Build the HTML report header with inline CSS styling
        let mut html = String::new();
        html.push_str(&format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="utf-8"><title>Forensics Report - {}</title>
<style>
  body {{ font-family: system-ui, sans-serif; margin: 20px; background: #f5f5f5; }}
  h1 {{ color: #333; border-bottom: 2px solid #09c; padding-bottom: 8px; }}
  h2 {{ color: #444; }}
  .card {{ background: #fff; border-radius: 8px; padding: 16px; margin: 16px 0; box-shadow: 0 2px 4px rgba(0,0,0,0.1); }}
  .card img {{ max-width: 100%; height: auto; border: 1px solid #ddd; border-radius: 4px; }}
  .desc {{ color: #666; margin: 8px 0; }}
  .ref {{ color: #09c; font-size: 0.9em; }}
  a {{ color: #09c; }}
  .grid {{ display: grid; grid-template-columns: repeat(auto-fill, minmax(400px, 1fr)); gap: 16px; }}
</style></head>
<body>
<h1>Forensics Analysis: {}</h1>
<p>Report generated on {}</p>
"#,
            stem,
            stem,
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
        ));

        let total = self.plugins.len();

        // Run each plugin on the original image and generate a card in the report
        for (i, instance) in self.plugins.iter().enumerate() {
            let name = instance.plugin.name().to_string();
            let desc = instance.plugin.description().to_string();
            let ref_url = instance.plugin.reference().to_string();
            let safe_name = name.replace(|c: char| !c.is_alphanumeric() && c != '-', "_");
            let filename = format!("{:02}_{}.png", i, safe_name);

            let msg = format!("Exporting {}/{}: {} ...", i + 1, total, name);
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(msg.clone()));
            eprintln!("[Export] {}", msg);

            let result = instance.plugin.process(&original);
            if let Some(img) = result {
                let path = out_dir.join(&filename);
                let _ = img.save(&path);
            }

            html.push_str("<div class=\"card\">");
            html.push_str(&format!("<h2>{}</h2>", escape_html(&name)));
            html.push_str(&format!("<img src=\"{}\" alt=\"{}\">", filename, escape_html(&name)));
            html.push_str(&format!("<p class=\"desc\">{}</p>", escape_html(&desc)));
            if !ref_url.is_empty() {
                html.push_str(&format!(
                    "<p class=\"ref\">📖 <a href=\"{}\">{}</a></p>",
                    escape_html(&ref_url),
                    escape_html(&ref_url)
                ));
            }
            html.push_str("</div>\n");
        }

        html.push_str("</div>\n</body>\n</html>");

        let html_path = out_dir.join("report.html");
        // Write the complete HTML report to disk
        if std::fs::write(&html_path, html).is_ok() {
            self.set_status(&format!(
                "HTML report generated: {}",
                html_path.display()
            ));
        } else {
            self.set_status("Error creating HTML report");
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Title("ACImageForensics".to_string()));
    }

    fn set_status(&self, msg: &str) {
        eprintln!("[Status] {}", msg);
    }
}

/// Escapes special HTML characters (&, <, >, ") to their entity equivalents.
fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Implements the eframe::App trait to render the GUI loop via egui.
impl eframe::App for ImageViewer {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Auto-process the image if one is loaded but not yet processed
        if self.processed_image.is_none() && self.original_image.is_some() {
            self.process_image();
        }

        // Create the GPU texture if processing completed but texture not yet created
        if self.processed_image.is_some() && self.texture.is_none() {
            self.create_texture(ui.ctx());
        }

        // Top control bar — independent of the image
        egui::Panel::top("controls").show_inside(ui, |ui| {
            ui.heading("ACImageForensics");

            let dropped_files = ui.input(|i| i.raw.dropped_files.clone());
            if !dropped_files.is_empty() {
                if let Some(dropped) = dropped_files.first() {
                    if let Some(path) = &dropped.path {
                        self.load_image(path);
                        self.process_image();
                        self.texture = None;
                    }
                }
            }

            ui.horizontal(|ui| {
                if ui.button("Load JPEG Image").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("JPEG", &["jpg", "jpeg"])
                        .pick_file()
                    {
                        self.load_image(&path);
                        self.process_image();
                        self.texture = None;
                    }
                }

                if ui.button("💾 Save Result").clicked() {
                    self.save_current_result();
                }

                if ui.button("📊 Run All & Export HTML").clicked() {
                    self.export_all_html(ui.ctx());
                }

                ui.separator();

                if self.plugins.is_empty() {
                    ui.label("No plugin loaded");
                } else {
                    let plugin_names: Vec<String> = self.plugins
                        .iter()
                        .map(|p| p.plugin.name().to_string())
                        .collect();
                    egui::ComboBox::from_id_salt("plugin_selector")
                        .selected_text(&plugin_names[self.selected_plugin])
                        .show_ui(ui, |ui| {
                            for (idx, name) in plugin_names.iter().enumerate() {
                                if ui
                                    .selectable_label(self.selected_plugin == idx, name)
                                    .clicked()
                                {
                                    self.selected_plugin = idx;
                                    self.process_image();
                                    self.texture = None;
                                }
                            }
                        });
                }

                ui.separator();

                let mut smooth = !self.zoom_nearest;
                if ui.checkbox(&mut smooth, "Smooth Zoom").changed() {
                    self.zoom_nearest = !smooth;
                    self.rebuild_textures(ui.ctx());
                }
                if ui.button("❓ Help").clicked() {
                    let _ = webbrowser::open("https://github.com/alexandrecilia/ACImageForensicsOC");
                }
            });

            // Information frame
            if !self.plugins.is_empty() {
                let desc = self.plugins[self.selected_plugin]
                    .plugin
                    .description()
                    .to_string();
                let ref_url = self.plugins[self.selected_plugin]
                    .plugin
                    .reference()
                    .to_string();

                egui::Frame::group(ui.style())
                    .inner_margin(egui::Margin::symmetric(8, 4))
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new(desc).size(12.0));
                        if !ref_url.is_empty() {
                            ui.add(
                                egui::Hyperlink::from_label_and_url("📖 Reference", &ref_url),
                            );
                        }
                    });

                // Adjustable plugin parameters
                let params = self.plugins[self.selected_plugin].plugin.parameters();
                if !params.is_empty() {
                    let mut changed = false;
                    for param in &params {
                        if param.is_boolean {
                            let val = self.plugins[self.selected_plugin]
                                .plugin
                                .get_parameter(param.name)
                                .unwrap_or(param.default);
                            let mut bool_val = val >= 0.5;
                            if ui.checkbox(&mut bool_val, param.name).changed() {
                                self.plugins[self.selected_plugin]
                                    .plugin
                                    .set_parameter(param.name, if bool_val { 1.0 } else { 0.0 });
                                changed = true;
                            }
                        } else {
                            let mut val = self.plugins[self.selected_plugin]
                                .plugin
                                .get_parameter(param.name)
                                .unwrap_or(param.default);
                            if ui
                                .add(egui::Slider::new(&mut val, param.min..=param.max).text(param.name))
                                .changed()
                            {
                                self.plugins[self.selected_plugin]
                                    .plugin
                                    .set_parameter(param.name, val);
                                changed = true;
                            }
                        }
                    }
                    if changed {
                        self.process_image();
                        self.texture = None;
                    }
                }
            }
        });

        // Image display area — separated from controls
        egui::CentralPanel::default().show_inside(ui, |ui| {
            // Ctrl+C: copy displayed image to clipboard
            if ui.input(|i| i.key_pressed(egui::Key::C) && i.modifiers.ctrl && !i.modifiers.alt && !i.modifiers.shift) {
                if let Some(img) = self.current_display_image() {
                    let (w, h) = img.dimensions();
                    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
                    for p in img.pixels() {
                        rgba.push(p[0]);
                        rgba.push(p[1]);
                        rgba.push(p[2]);
                        rgba.push(255);
                    }
                    if let Ok(mut cb) = arboard::Clipboard::new() {
                        let _ = cb.set_image(arboard::ImageData {
                            width: w as usize,
                            height: h as usize,
                            bytes: std::borrow::Cow::Owned(rgba),
                        });
                    }
                }
            }
            if let Some(texture) = &self.texture {
                let img_size = texture.size_vec2();
                let available_size = ui.available_size();

                let response = ui.allocate_response(
                    available_size,
                    egui::Sense::click_and_drag().union(egui::Sense::hover()),
                );

                // Right-click held on image → show original
                if response.hovered() && ui.input(|i| i.pointer.button_down(egui::PointerButton::Secondary)) {
                    self.right_click_down = true;
                }
                if !ui.input(|i| i.pointer.button_down(egui::PointerButton::Secondary)) {
                    self.right_click_down = false;
                }

                if response.dragged_by(egui::PointerButton::Primary) {
                    self.pan -= response.drag_delta();
                }

                let scroll_delta = ui.input(|i| i.smooth_scroll_delta);
                if scroll_delta.y != 0.0 {
                    let zoom_factor = 1.02_f32.powf(scroll_delta.y);
                    let old_zoom = self.zoom;
                    self.zoom *= zoom_factor;

                    if let Some(mouse_pos) = response.hover_pos() {
                        let rect_center = response.rect.center();
                        let rel_pos = mouse_pos - rect_center;
                        let zoom_change = self.zoom / old_zoom;
                        self.pan = rel_pos - (rel_pos - self.pan) * zoom_change;
                    }
                }

                self.zoom = self.zoom.max(0.1).min(10.0);

                // Select texture based on right-click state
                let display_tex = if self.right_click_down {
                    self.original_texture.as_ref().unwrap_or(texture)
                } else {
                    texture
                };

                let zoomed_size = img_size * self.zoom;
                let rect =
                    egui::Rect::from_min_size(response.rect.min - self.pan, zoomed_size);
                ui.painter().image(
                    display_tex.id(),
                    rect,
                    egui::Rect::from_min_max(
                        egui::pos2(0.0, 0.0),
                        egui::pos2(1.0, 1.0),
                    ),
                    egui::Color32::WHITE,
                );
            } else if self.original_image.is_some() {
                ui.label("Processing...");
            } else {
                ui.label(
                    "No image loaded. Drag & drop a JPEG image or click 'Load JPEG Image'.\n\n\
                     Navigation:\n\
                     • Left-click + drag: Pan the image\n\
                     • Right-click + drag: Zoom into a region\n\
                     • Scroll wheel: Zoom in/out",
                );
            }
        });
    }
}

impl ImageViewer {
    /// Loads a JPEG image from disk into memory, resetting zoom and pan state.
    fn load_image(&mut self, path: &PathBuf) {
        if let Ok(img) = image::open(path) {
            let rgb_img = img.to_rgb8();
            self.original_image = Some(rgb_img);
            self.image_path = Some(path.clone());
            self.zoom = 1.0;
            self.pan = egui::Vec2::ZERO;
            self.texture = None;
            self.original_texture = None;
        }
    }

    /// Runs the currently selected plugin on the original image to produce a processed result.
    fn process_image(&mut self) {
        if let Some(ref original) = self.original_image {
            if self.selected_plugin < self.plugins.len() {
                self.processed_image =
                    self.plugins[self.selected_plugin].plugin.process(original);
            }
        }
    }

    /// Rebuilds both GPU textures with the current zoom filter setting.
    fn rebuild_textures(&mut self, ctx: &egui::Context) {
        self.texture = None;
        self.original_texture = None;
        self.create_texture(ctx);
    }

    /// Converts an RgbImage into an egui GPU texture for rendering.
    fn make_texture(ctx: &egui::Context, id: &str, img: &RgbImage, nearest: bool) -> egui::TextureHandle {
        let size = [img.width() as usize, img.height() as usize];
        let pixels: Vec<egui::Color32> = img
            .pixels()
            .map(|p| egui::Color32::from_rgb(p[0], p[1], p[2]))
            .collect();
        let color_image = egui::ColorImage {
            size,
            pixels,
            source_size: egui::vec2(size[0] as f32, size[1] as f32),
        };
        let filter = if nearest { egui::TextureFilter::Nearest } else { egui::TextureFilter::Linear };
        ctx.load_texture(
            id,
            egui::ImageData::Color(std::sync::Arc::new(color_image)),
            egui::TextureOptions { magnification: filter, minification: filter, ..Default::default() },
        )
    }

    /// Creates GPU textures for both the processed and original images.
    fn create_texture(&mut self, ctx: &egui::Context) {
        if let Some(ref img) = self.processed_image {
            self.texture = Some(Self::make_texture(ctx, "processed_image", img, self.zoom_nearest));
        }
        if let Some(ref img) = self.original_image {
            self.original_texture = Some(Self::make_texture(ctx, "original_image", img, self.zoom_nearest));
        }
    }

    /// Returns the currently displayed image (processed or original if right-click is held).
    fn current_display_image(&self) -> Option<&RgbImage> {
        if self.right_click_down {
            self.original_image.as_ref()
        } else {
            self.processed_image.as_ref()
        }
    }
}

/// Application entry point: configures the native window and launches the egui event loop.
fn main() -> eframe::Result<()> {
    let icon_img = image::load_from_memory(include_bytes!("acimageforensics.png"))
        .expect("Failed to load icon")
        .into_rgba8();
    let (w, h) = icon_img.dimensions();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(egui::vec2(800.0, 600.0))
            .with_icon(egui::IconData {
                rgba: icon_img.into_raw(),
                width: w,
                height: h,
            }),
        ..Default::default()
    };
    eframe::run_native(
        "ACImageForensics",
        options,
        Box::new(|_cc| Ok(Box::new(ImageViewer::default()))),
    )
}
