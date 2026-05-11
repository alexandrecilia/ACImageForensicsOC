# AC Image Forensics

AC Image Forensics is an experimental digital image forensic analysis tool written in Rust. It features a modular plugin architecture, allowing for the dynamic extension of analysis algorithms to detect image manipulations, compression artifacts, and sensor inconsistencies.

## 💡 Genesis & Philosophy

The project was born from a desire to transform scientific publications and existing Python-based forensic tools into a portable executable. While Python is excellent for research, deploying and executing these tools often involves complex environment setups. By porting these algorithms to Rust, the project achieves good performance and stability and ease of deployment: A single binary with dynamic plugins, eliminating the need for Python runtimes or dependency hell.


### Vibe Coding with opencode
This project is a showcase of **"vibe coding"**. Most of the codebase was developed using `opencode`, an AI-powered engineering tool. Instead of traditional manual coding, the implementation was driven by high-level intent, iterative prompting, and real-time feedback loops, allowing for rapid prototyping of complex forensic algorithms directly in Rust.

## 🚀 Features

The application provides a suite of specialized forensic tools organized into a flexible plugin system.

### Analysis Categories

#### 📉 Compression & Quantization
- **Error Level Analysis (ELA)**: Multiple variants (Sherloq, GIMP-style, Forensically-style) to detect re-compression inconsistencies.
- **JPEG Ghost**: Multiple variants (Sherloq, GIMP-style, Alt) to detect "ghost" artifacts from multiple JPEG compression cycles.
- **miniNand-JPEG**: Analyzes 8x8 block-level differences to identify double compression.

#### 🔊 Noise & Texture Analysis
- **Noise Variance**: Detects local noise inconsistency.
- **Texture Inconsistency**: Highlights inpainted or cloned areas via local mean deviation.
- **Noise Analysis**: Isolates high-frequency noise using separable median filters.
- **Noise Kurtosis**: Analyzes the kurtosis of noise distribution to identify tampering.
- **Sensor Noise**: Analyzes specific sensor-level noise patterns.

#### 🌐 Grid & Mosaic Analysis
- **Mosaic Analysis**: Detects CFA (Color Filter Array) coherence and mosaic disruptions.
- **CFA Detector**: Identifies anomalies in the Bayer pattern of the image.

#### 🔍 Advanced Anomaly Detection
- **Zero-shot Anomaly Detection**: Uses local semantic distribution to find statistical anomalies.
- **Gradient Forensics**: Analyzes micro-discontinuities in gradients to reveal AI inpainting.

## 🛠 Technical Architecture

### Core Stack
- **Language**: Rust
- **GUI**: `eframe` / `egui`
- **Image Processing**: `image` crate and `imageproc`
- **Parallelism**: `rayon` for multi-threaded pixel processing

### Plugin System
The application uses a dynamic plugin architecture where each algorithm is compiled as a shared library (`.dll`), loaded at runtime via a common API.

## 📦 Installation & Build

### Prerequisites
- **Rust**: 1.70+

### Building the Project
```bash
cargo build --release
```
The executable `ACImageForensics.exe` and the resulting `plugin_*.dll` files will be located in the `target/release` directory.

## 📖 Usage

1. **Load Image**: Click **"Load JPEG Image"** or drag & drop an image file onto the window.
2. **Select Plugin**: Choose an analysis tool from the list.
3. **Adjust Parameters**: Fine-tune the algorithm using the provided sliders.
4. **Interpret Results**: Bright/Red areas typically indicate potential tampering.

### Image Navigation

- **Left-click + drag**: Pan the image (move around when zoomed in).
- **Right-click + drag**: Draw a selection rectangle to zoom into a specific region.
- **Scroll wheel**: Zoom in and out.

## ⚖️ License & Credits

This project is licensed under the **GNU General Public License v3.0 (GPL-3.0)**.

It is an implementation of various scientific algorithms and open-source tools. We tried to respect the licenses of the original works:

### Publications & Open-Source Tools Cited

- **Farid, H.** (2009). "Exposing Digital Forgeries from JPEG Ghosts", *IEEE Transactions on Information Forensics and Security*. — Implemented in Sherloq JPEG Ghost plugin.
- **Bammey, Q.** (2023). "A Contrario Mosaic Analysis for Image Forensics", *ACIVS 2023*. — https://doi.org/10.1007/978-3-031-45382-3_19 — Implemented in Mosaic Analysis plugin.
- **Nikoukhah, T., Anger, J., Colom, M., Morel, J.-M., & Grompone von Gioi, R.** (2021). "ZERO: a Local JPEG Grid Origin Detector Based on the Number of DCT Zeros and its Applications in Image Forensics", *IPOL*. — https://ipol.im/pub/art/2021/390/ — Implemented in Zero-shot Anomaly Detection plugin.
- **Fridrich, J., Goljan, M., Lisonek, P., & Soukal, D.** (2005). "Writing on Wet Paper", *IEEE Transactions on Signal Processing*, 53(10), 3923–3935. — https://doi.org/10.1109/TSP.2005.855393 — Implemented in CFA Detector plugin.
- **PENet**: "Color Image Steganalysis Based on Pixel Difference Convolution and Enhanced Transformer With Selective Pooling", *TIFS 2024*. — https://github.com/revere7/PENet_Steganalysis — Implemented in PENet SRM Selector plugin.
- **Sherloq** by Guido Bartoli. — https://github.com/GuidoBartoli/sherloq — Implemented in Sherloq ELA and Sherloq JPEG Ghost plugins.
- **GIMP-Forensics** by Bernardo Bulgarelli Labronici. — https://sourceforge.net/projects/gimp-forensics/ — Implemented in GIMP Forensics ELA and GIMP Forensics JPEG Ghost plugins.
- **Forensically** by Jonas Wagner. — https://29a.ch/photo-forensics/ — Implemented in Forensically ELA and Noise Analysis plugins.
- **Wagner, J.** (2015). "Noise Analysis for Image Forensics". — https://29a.ch/2015/08/21/noise-analysis-for-image-forensics — Implemented in Noise Analysis plugin.
- **Noisesniffer** by Marina Gardella et al. (2021). "Noisesniffer: a Fully Automatic Image Forgery Detector Based on Noise Analysis", *IWBF 2021*. — https://github.com/marinagardella/Noisesniffer — Implemented in Noisesniffer Distribution and Noisesniffer Mask plugins.

### Other Implementations

The remaining plugins (Noise Variance, Noise Kurtosis, Texture Inconsistency, Gradient Forensics, Sensor Noise, etc.) are original exploratory implementations, developed as research paths with the assistance of AI (opencode). They do not directly reproduce specific published algorithms but were inspired by general forensic analysis concepts.
