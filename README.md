# Rust Image Compressor 🦀🖼️

A fast, lightweight, and modern desktop application built in Rust with an intuitive native GUI for single and bulk image compression with granular target file size controls.

![Rust](https://img.shields.io/badge/Language-Rust-orange.svg)
![Platform](https://img.shields.io/badge/Platform-Windows%20x64-blue.svg)
![License](https://img.shields.io/badge/License-MIT-green.svg)

---

## Features

- **Native Graphical User Interface**: Snappy and responsive desktop UI powered by `eframe` (egui).
- **Single & Bulk Processing**: Add individual image files or select an entire folder of photos in one click.
- **Per-File Target Size Slider**: Inspect each photo's original dimensions and file size, and adjust an individual target slider (in KB) to compress it down to an exact size limit.
- **Proportional Bulk Target Optimizer**: Have multiple photos (e.g. 5 photos total 4 MB) and need the entire set compressed to under 2 MB? Simply set the global batch target slider to `2.0 MB` and click **"⚡ Terapkan ke Semua Foto"** — the application will automatically calculate and assign the target budget proportionally across all files.
- **Adaptive Multi-Pass Compression Engine**: Employs iterative binary search over encoding quality and intelligent proportional downscaling if quality alone cannot meet the requested target byte budget.
- **Multi-Format Support**:
  - Auto (matches source format)
  - JPEG (`.jpg`)
  - WebP (`.webp`)
  - Supports input images in JPEG, PNG, WebP, and BMP.
- **Custom Output Destination**: Easily select where compressed photos are saved, complete with a button to open the output directory directly.
- **Non-blocking Multi-threaded Architecture**: Background worker threads process files concurrently while keeping the UI fluid and showing live progress indicators and savings percentages.

---

## Quick Start (Pre-built Binaries)

1. Download `rust-image-compressor-windows-x64.zip` from the latest **[Releases](https://github.com/GlitchWorlds/rust-image-compressor/releases)**.
2. Extract the archive.
3. Run `rust-image-compressor.exe`. No installation or external dependencies required.

---

## Building from Source

### Prerequisites
- Rust 1.80+ (`cargo`, `rustc`)
- A C compiler / linker (`gcc` or MSVC build tools)

### Build
```bash
# Clone the repository
git clone https://github.com/GlitchWorlds/rust-image-compressor.git
cd rust-image-compressor

# Build optimized release binary
cargo build --release
```

The resulting executable will be available at `target/release/rust-image-compressor.exe`.

---

## License

This project is licensed under the MIT License.
