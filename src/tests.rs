#[cfg(test)]
mod tests {
    use crate::compressor::{compress_to_target, format_size};
    use image::{DynamicImage, ImageBuffer, Rgba};

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(500), "500 B");
        assert_eq!(format_size(1536), "1.5 KB");
        assert_eq!(format_size(1024 * 1024 * 2), "2.00 MB");
    }

    #[test]
    fn test_compression_target() {
        let raw: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(400, 300);
        let dyn_img = DynamicImage::ImageRgba8(raw);
        let target = 15 * 1024; // 15 KB
        let (buf, fmt) = compress_to_target(&dyn_img, target, false).expect("Compression failed");
        assert!(buf.len() as u64 <= target + 1024);
        assert_eq!(fmt, "jpeg");
    }

    #[test]
    fn test_bulk_proportional_math() {
        let sizes = vec![1_000_000u64, 1_000_000u64, 1_000_000u64, 1_000_000u64]; // 4 MB total
        let total_orig: u64 = sizes.iter().sum();
        let desired_bulk_target = 2_000_000u64; // 2 MB total
        let ratio = desired_bulk_target as f64 / total_orig as f64;
        let targets: Vec<u64> = sizes.iter().map(|s| (*s as f64 * ratio) as u64).collect();
        let target_sum: u64 = targets.iter().sum();
        assert_eq!(target_sum, desired_bulk_target);
        for t in targets {
            assert_eq!(t, 500_000);
        }
    }
}
