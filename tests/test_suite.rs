use image::{Rgba, RgbaImage};
use modernview::browser::{is_supported_image, natural_sort_key};
use modernview::cache::ImageCache;
use modernview::decoder::load_and_decode_image;
use modernview::editor::{apply_adjustments, Adjustments, ImageEditor, ResampleFilter};
use modernview::export::{save_image_safe, ExportFormat};
use modernview::metadata::{calculate_aspect_ratio, format_file_size};
use modernview::thumbnail::generate_thumbnail_from_rgba;
use std::path::{Path, PathBuf};

fn create_sample_rgba_image(w: u32, h: u32) -> RgbaImage {
    let mut img = RgbaImage::new(w, h);
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        let r = ((x * 255) / w.max(1)) as u8;
        let g = ((y * 255) / h.max(1)) as u8;
        let b = 128u8;
        let a = if x < w / 2 { 255 } else { 180 }; // half transparent
        *pixel = Rgba([r, g, b, a]);
    }
    img
}

#[test]
fn test_aspect_ratio_and_file_size() {
    assert_eq!(calculate_aspect_ratio(1920, 1080), "16:9");
    assert_eq!(calculate_aspect_ratio(1440, 1080), "4:3");
    assert_eq!(calculate_aspect_ratio(1000, 1000), "1:1");
    assert_eq!(calculate_aspect_ratio(3000, 2000), "3:2");

    assert_eq!(format_file_size(500), "500 B");
    assert_eq!(format_file_size(2048), "2.0 KB");
    assert_eq!(format_file_size(5 * 1024 * 1024), "5.00 MB");
}

#[test]
fn test_editor_transforms_and_history() {
    let img = create_sample_rgba_image(200, 100);
    let mut editor = ImageEditor::new(img);

    assert_eq!(editor.working_image().width(), 200);
    assert_eq!(editor.working_image().height(), 100);

    // Rotate 90 CW: dimensions swap
    editor.rotate_90_cw();
    assert_eq!(editor.working_image().width(), 100);
    assert_eq!(editor.working_image().height(), 200);
    assert!(editor.can_undo());

    // Undo rotation
    assert!(editor.undo());
    assert_eq!(editor.working_image().width(), 200);
    assert_eq!(editor.working_image().height(), 100);
    assert!(editor.can_redo());

    // Redo rotation
    assert!(editor.redo());
    assert_eq!(editor.working_image().width(), 100);
    assert_eq!(editor.working_image().height(), 200);

    // Revert
    editor.revert_to_original();
    assert_eq!(editor.working_image().width(), 200);
    assert_eq!(editor.working_image().height(), 100);
    assert!(!editor.can_undo());
}

#[test]
fn test_editor_crop() {
    let img = create_sample_rgba_image(400, 200);
    let mut editor = ImageEditor::new(img);

    // Crop center 50%
    editor.crop(0.25, 0.25, 0.5, 0.5).unwrap();
    assert_eq!(editor.working_image().width(), 200);
    assert_eq!(editor.working_image().height(), 100);

    // Undo
    editor.undo();
    assert_eq!(editor.working_image().width(), 400);
    assert_eq!(editor.working_image().height(), 200);
}

#[test]
fn test_editor_resize_with_fast_image_resize() {
    let img = create_sample_rgba_image(800, 600);
    let mut editor = ImageEditor::new(img);

    // Lanczos3 resize to 400x300
    editor.resize(400, 300, ResampleFilter::Lanczos3).unwrap();
    assert_eq!(editor.working_image().width(), 400);
    assert_eq!(editor.working_image().height(), 300);

    // Bilinear resize to 1200x900
    editor.resize(1200, 900, ResampleFilter::Bilinear).unwrap();
    assert_eq!(editor.working_image().width(), 1200);
    assert_eq!(editor.working_image().height(), 900);
}

#[test]
fn test_editor_adjustments() {
    let img = create_sample_rgba_image(50, 50);
    let adj = Adjustments {
        brightness: 20.0,
        contrast: 15.0,
        saturation: 10.0,
    };
    let modified = apply_adjustments(&img, &adj);
    assert_eq!(modified.width(), 50);
    assert_eq!(modified.height(), 50);
}

#[test]
fn test_thumbnail_generation() {
    let img = create_sample_rgba_image(1920, 1080);
    let thumb = generate_thumbnail_from_rgba(&img, 96).unwrap();
    assert!(thumb.width() <= 96);
    assert!(thumb.height() <= 96);
    assert_eq!(thumb.width(), 96);
    assert_eq!(thumb.height(), 54); // 16:9 ratio preserved!
}

#[test]
fn test_natural_sort() {
    let mut files = vec!["photo10.png", "photo1.png", "photo2.png", "photo20.png"];
    files.sort_by(|a, b| natural_sort_key(a).cmp(&natural_sort_key(b)));
    assert_eq!(files, vec!["photo1.png", "photo2.png", "photo10.png", "photo20.png"]);
}

#[test]
fn test_supported_extensions() {
    assert!(is_supported_image(Path::new("test.jpg")));
    assert!(is_supported_image(Path::new("test.JPEG")));
    assert!(is_supported_image(Path::new("test.png")));
    assert!(is_supported_image(Path::new("test.webp")));
    assert!(is_supported_image(Path::new("test.bmp")));
    assert!(is_supported_image(Path::new("test.tiff")));
    assert!(is_supported_image(Path::new("test.ico")));
    assert!(!is_supported_image(Path::new("test.txt")));
    assert!(!is_supported_image(Path::new("test.pdf")));
}

#[test]
fn test_image_cache_eviction() {
    let mut cache = ImageCache::new(1); // 1 MB limit
    let img = create_sample_rgba_image(400, 400); // 400*400*4 = 640 KB
    let meta = modernview::metadata::ImageMetadata::default();

    cache.insert(PathBuf::from("img1.png"), img.clone(), meta.clone());
    assert!(cache.get(Path::new("img1.png")).is_some());

    // Inserting second 640 KB image should exceed 1MB and evict img1
    cache.insert(PathBuf::from("img2.png"), img.clone(), meta.clone());
    assert!(cache.get(Path::new("img2.png")).is_some());
}

#[test]
fn test_save_and_decode_formats() {
    let temp_dir = std::env::temp_dir().join("modernview_test_dir");
    let _ = std::fs::create_dir_all(&temp_dir);

    let img = create_sample_rgba_image(100, 80);

    // Test PNG
    let png_path = temp_dir.join("test.png");
    save_image_safe(&img, &png_path, ExportFormat::Png, 90, None).unwrap();
    let decoded_png = load_and_decode_image(&png_path).unwrap();
    assert_eq!(decoded_png.width, 100);
    assert_eq!(decoded_png.height, 80);

    // Test JPEG with transparency flattening
    let jpg_path = temp_dir.join("test.jpg");
    save_image_safe(&img, &jpg_path, ExportFormat::Jpeg, 85, Some([255, 255, 255])).unwrap();
    let decoded_jpg = load_and_decode_image(&jpg_path).unwrap();
    assert_eq!(decoded_jpg.width, 100);
    assert_eq!(decoded_jpg.height, 80);

    // Test BMP
    let bmp_path = temp_dir.join("test.bmp");
    save_image_safe(&img, &bmp_path, ExportFormat::Bmp, 85, Some([255, 255, 255])).unwrap();
    let decoded_bmp = load_and_decode_image(&bmp_path).unwrap();
    assert_eq!(decoded_bmp.width, 100);
    assert_eq!(decoded_bmp.height, 80);

    // Test WebP
    let webp_path = temp_dir.join("test.webp");
    save_image_safe(&img, &webp_path, ExportFormat::WebP, 85, None).unwrap();
    let decoded_webp = load_and_decode_image(&webp_path).unwrap();
    assert_eq!(decoded_webp.width, 100);
    assert_eq!(decoded_webp.height, 80);

    // Clean up
    let _ = std::fs::remove_dir_all(&temp_dir);
}
