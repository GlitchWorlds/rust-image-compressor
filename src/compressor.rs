use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use image::{imageops::FilterType, DynamicImage, GenericImageView, ImageFormat};
use image::codecs::jpeg::JpegEncoder;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Auto, // Keeps original if JPEG/WebP/PNG, or falls back to JPEG
    Jpeg,
    WebP,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ItemStatus {
    Pending,
    Processing,
    Done { compressed_bytes: u64, out_path: PathBuf },
    Error(String),
}

#[derive(Debug, Clone)]
pub struct ImageItem {
    pub id: usize,
    pub path: PathBuf,
    pub file_name: String,
    pub original_bytes: u64,
    pub dimensions: (u32, u32),
    pub target_bytes: u64,
    pub status: ItemStatus,
}

impl ImageItem {
    pub fn new(id: usize, path: PathBuf) -> Result<Self, String> {
        let metadata = fs::metadata(&path).map_err(|e| format!("Failed to read metadata: {e}"))?;
        let original_bytes = metadata.len();
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "image".to_string());

        // Probe dimensions without full decode if possible or using Reader
        let dimensions = match image::ImageReader::open(&path) {
            Ok(reader) => match reader.into_dimensions() {
                Ok(dims) => dims,
                Err(_) => (0, 0),
            },
            Err(_) => (0, 0),
        };

        // Default target is 50% of original bytes (minimum 10 KB)
        let target_bytes = (original_bytes / 2).max(10 * 1024).min(original_bytes);

        Ok(Self {
            id,
            path,
            file_name,
            original_bytes,
            dimensions,
            target_bytes,
            status: ItemStatus::Pending,
        })
    }
}

pub fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

/// Compress a single image item to output directory
pub fn process_image(
    item: &ImageItem,
    out_dir: &Path,
    format_pref: OutputFormat,
) -> Result<(u64, PathBuf), String> {
    // 1. Open and decode image
    let img = image::ImageReader::open(&item.path)
        .map_err(|e| format!("Cannot open: {e}"))?
        .decode()
        .map_err(|e| format!("Decode failed: {e}"))?;

    let stem = item
        .path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "compressed".to_string());

    let (is_webp, extension) = match format_pref {
        OutputFormat::WebP => (true, "webp"),
        OutputFormat::Jpeg => (false, "jpg"),
        OutputFormat::Auto => {
            let ext = item
                .path
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            if ext == "webp" {
                (true, "webp")
            } else {
                (false, "jpg")
            }
        }
    };

    let out_filename = format!("{}_compressed.{}", stem, extension);
    let out_path = out_dir.join(out_filename);

    let (compressed_bytes_data, _) = compress_to_target(&img, item.target_bytes, is_webp)?;

    fs::write(&out_path, &compressed_bytes_data)
        .map_err(|e| format!("Write failed: {e}"))?;

    let final_size = compressed_bytes_data.len() as u64;
    Ok((final_size, out_path))
}

/// Encode image to memory attempting to fit within target_bytes
pub fn compress_to_target(
    img: &DynamicImage,
    target_bytes: u64,
    prefer_webp: bool,
) -> Result<(Vec<u8>, &'static str), String> {
    let mut current_img = img.clone();
    let (orig_w, orig_h) = current_img.dimensions();
    let mut w = orig_w;
    let mut h = orig_h;

    let mut best_buf = Vec::new();
    let best_fmt = if prefer_webp { "webp" } else { "jpeg" };

    // Try up to 5 downscale iterations if needed
    for attempt in 0..5 {
        let mut low = 5u8;
        let mut high = 95u8;
        let mut found_fit = false;

        // Binary search quality parameter
        while low <= high {
            let mid = (low + high) / 2;
            let encoded = encode_raw(&current_img, mid, prefer_webp)?;
            let len = encoded.len() as u64;

            if len <= target_bytes {
                found_fit = true;
                best_buf = encoded;
                if len >= (target_bytes as f64 * 0.92) as u64 || mid >= 95 {
                    break;
                }
                low = mid + 1;
            } else {
                if mid <= 5 {
                    break;
                }
                high = mid - 1;
            }
        }

        if found_fit || attempt >= 4 {
            break;
        }

        // Downscale dimensions by 20%
        w = ((w as f32) * 0.80).max(64.0) as u32;
        h = ((h as f32) * 0.80).max(64.0) as u32;
        current_img = img.resize_exact(w, h, FilterType::Triangle);
    }

    if best_buf.is_empty() {
        // Fallback lowest quality
        best_buf = encode_raw(&current_img, 10, prefer_webp)?;
    }

    Ok((best_buf, best_fmt))
}

fn encode_raw(img: &DynamicImage, quality: u8, is_webp: bool) -> Result<Vec<u8>, String> {
    let mut buf = Vec::new();
    let mut cursor = Cursor::new(&mut buf);

    if is_webp {
        img.write_to(&mut cursor, ImageFormat::WebP)
            .map_err(|e| format!("WebP encode error: {e}"))?;
    } else {
        let rgb = img.to_rgb8();
        let encoder = JpegEncoder::new_with_quality(&mut cursor, quality);
        image::ImageEncoder::write_image(
            encoder,
            rgb.as_raw(),
            img.width(),
            img.height(),
            image::ExtendedColorType::Rgb8,
        )
        .map_err(|e| format!("JPEG encode error: {e}"))?;
    }

    Ok(buf)
}
