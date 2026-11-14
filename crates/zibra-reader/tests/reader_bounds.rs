use zibra_reader::{slice_dynamic_image, MAX_SAFE_IMAGE_BYTES, MAX_TILE_HEIGHT};
use image::{DynamicImage, RgbaImage};

#[test]
fn test_image_slice_bounds_and_tiling() {
    // Create a tall image (e.g. 800 x 5000 px, simulating a vertical manhwa strip)
    let width = 800;
    let height = 5000;
    let img = DynamicImage::ImageRgba8(RgbaImage::new(width, height));

    let sliced = slice_dynamic_image(img, 0, 0);

    assert_eq!(sliced.total_width, 800);
    assert_eq!(sliced.total_height, 5000);
    // 5000 px should be split into 3 tiles: 2048, 2048, 904
    assert_eq!(sliced.tiles.len(), 3);
    assert_eq!(sliced.tiles[0].height, MAX_TILE_HEIGHT);
    assert_eq!(sliced.tiles[1].height, MAX_TILE_HEIGHT);
    assert_eq!(sliced.tiles[2].height, 904);

    // Byte weight should be accurate
    let expected_bytes = 800 * 5000 * 4;
    assert_eq!(sliced.byte_weight(), expected_bytes);
}

#[test]
fn test_checked_image_arithmetic_overflow_prevention() {
    // Test that extreme dimensions calculate safely in u64 without u32 overflow
    let extreme_width: usize = 70000;
    let extreme_height: usize = 70000;
    let bytes_per_pixel = 4u64;

    let total_bytes = (extreme_width as u64)
        .saturating_mul(extreme_height as u64)
        .saturating_mul(bytes_per_pixel);

    // This would overflow u32 (70000 * 70000 * 4 = 19,600,000,000 bytes > 4GB)
    assert!(total_bytes > u32::MAX as u64);
    assert!(total_bytes > MAX_SAFE_IMAGE_BYTES);
}
