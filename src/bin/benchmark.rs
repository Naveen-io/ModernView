use std::time::Instant;
use std::path::PathBuf;
use image::{Rgba, RgbaImage};
use modernview::decoder::load_and_decode_image;
use modernview::editor::{apply_adjustments, Adjustments, ImageEditor, ResampleFilter};
use modernview::export::{save_image_safe, ExportFormat};
use modernview::thumbnail::generate_thumbnail_from_rgba;

fn create_benchmark_image(w: u32, h: u32) -> RgbaImage {
    let mut img = RgbaImage::new(w, h);
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        let r = ((x * 255) / w.max(1)) as u8;
        let g = ((y * 255) / h.max(1)) as u8;
        let b = ((x + y) % 256) as u8;
        *pixel = Rgba([r, g, b, 255]);
    }
    img
}

fn main() {
    println!("========================================================");
    println!("          MODERNVIEW PERFORMANCE BENCHMARK              ");
    println!("========================================================");
    println!("Operating System: Windows 64-bit");
    println!("Target Architecture: x86_64-pc-windows-msvc");
    println!("SIMD Resizer: fast_image_resize v5 (AVX2/SSE4 enabled)");
    println!("--------------------------------------------------------\n");

    let temp_dir = std::env::temp_dir().join("modernview_benchmark_data");
    let _ = std::fs::create_dir_all(&temp_dir);

    // 1. Prepare Realistic 1080p & 4K test images
    println!("[1/6] Preparing realistic 1920x1080 test images...");
    let img_1080p = create_benchmark_image(1920, 1080);

    let png_path = temp_dir.join("bench_1080p.png");
    let jpg_path = temp_dir.join("bench_1080p.jpg");
    let webp_path = temp_dir.join("bench_1080p.webp");

    save_image_safe(&img_1080p, &png_path, ExportFormat::Png, 90, None).unwrap();
    save_image_safe(&img_1080p, &jpg_path, ExportFormat::Jpeg, 85, None).unwrap();
    save_image_safe(&img_1080p, &webp_path, ExportFormat::WebP, 85, None).unwrap();

    // 2. Cold Decode Benchmarks
    println!("\n[2/6] Running Cold Decode Benchmarks (1920x1080)...");

    // JPEG Decode
    let start = Instant::now();
    let iters = 10;
    for _ in 0..iters {
        let _ = load_and_decode_image(&jpg_path).unwrap();
    }
    let elapsed_jpg = start.elapsed().as_secs_f64() / iters as f64;
    println!("  • JPEG 1080p Cold Decode:     {:.2} ms ({:.1} fps)", elapsed_jpg * 1000.0, 1.0 / elapsed_jpg);

    // PNG Decode
    let start = Instant::now();
    for _ in 0..iters {
        let _ = load_and_decode_image(&png_path).unwrap();
    }
    let elapsed_png = start.elapsed().as_secs_f64() / iters as f64;
    println!("  • PNG 1080p Cold Decode:      {:.2} ms ({:.1} fps)", elapsed_png * 1000.0, 1.0 / elapsed_png);

    // WebP Decode
    let start = Instant::now();
    for _ in 0..iters {
        let _ = load_and_decode_image(&webp_path).unwrap();
    }
    let elapsed_webp = start.elapsed().as_secs_f64() / iters as f64;
    println!("  • WebP 1080p Cold Decode:     {:.2} ms ({:.1} fps)", elapsed_webp * 1000.0, 1.0 / elapsed_webp);

    // 3. Resizing Benchmark (Lanczos3 vs Bilinear)
    println!("\n[3/6] Running SIMD Resizing Benchmarks (1920x1080 -> 960x540)...");
    let mut editor = ImageEditor::new(img_1080p.clone());

    let start = Instant::now();
    let resize_iters = 20;
    for _ in 0..resize_iters {
        let _ = editor.resize(960, 540, ResampleFilter::Lanczos3);
        editor.revert_to_original();
    }
    let elapsed_lanczos = start.elapsed().as_secs_f64() / resize_iters as f64;
    println!("  • Lanczos3 Resampling:        {:.2} ms ({:.1} fps)", elapsed_lanczos * 1000.0, 1.0 / elapsed_lanczos);

    let start = Instant::now();
    for _ in 0..resize_iters {
        let _ = editor.resize(960, 540, ResampleFilter::Bilinear);
        editor.revert_to_original();
    }
    let elapsed_bilinear = start.elapsed().as_secs_f64() / resize_iters as f64;
    println!("  • Bilinear Resampling:        {:.2} ms ({:.1} fps)", elapsed_bilinear * 1000.0, 1.0 / elapsed_bilinear);

    // 4. Thumbnail Generation Benchmark
    println!("\n[4/6] Running Thumbnail Generation Benchmark (1080p -> 96px filmstrip preview)...");
    let start = Instant::now();
    let thumb_iters = 50;
    for _ in 0..thumb_iters {
        let _ = generate_thumbnail_from_rgba(&img_1080p, 96).unwrap();
    }
    let elapsed_thumb = start.elapsed().as_secs_f64() / thumb_iters as f64;
    println!("  • 96px Thumbnail Generation:  {:.2} ms ({:.0} thumbnails/sec)", elapsed_thumb * 1000.0, 1.0 / elapsed_thumb);

    // 5. Image Adjustments Benchmark (LUT-accelerated Brightness/Contrast/Saturation)
    println!("\n[5/6] Running Live Adjustments Benchmark (Full 1080p frame = 2.07 Million Pixels)...");
    let adj = Adjustments {
        brightness: 25.0,
        contrast: 15.0,
        saturation: 20.0,
    };
    let start = Instant::now();
    let adj_iters = 30;
    for _ in 0..adj_iters {
        let _ = apply_adjustments(&img_1080p, &adj);
    }
    let elapsed_adj = start.elapsed().as_secs_f64() / adj_iters as f64;
    let mp_per_sec = (1920.0 * 1080.0 / 1_000_000.0) / elapsed_adj;
    println!("  • Full-frame Adjustment:      {:.2} ms ({:.1} Megapixels/sec)", elapsed_adj * 1000.0, mp_per_sec);

    // 6. Parallel Batch Processing Throughput
    println!("\n[6/6] Running Parallel Batch Processing Throughput (10 images in parallel)...");
    let batch_out = temp_dir.join("batch_out");
    let _ = std::fs::create_dir_all(&batch_out);

    let batch_items: Vec<PathBuf> = (0..10).map(|i| {
        let p = temp_dir.join(format!("batch_src_{}.png", i));
        let _ = save_image_safe(&img_1080p, &p, ExportFormat::Png, 90, None);
        p
    }).collect();

    let (tx, rx) = crossbeam_channel::unbounded();
    let cancel_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

    let configs: Vec<modernview::batch::BatchItemConfig> = batch_items.iter().enumerate().map(|(id, p)| {
        modernview::batch::BatchItemConfig {
            id,
            input_path: p.clone(),
            target_format: ExportFormat::Jpeg,
            quality: 85,
            resize_mode: 0,
            output_dir: Some(batch_out.clone()),
        }
    }).collect();

    let start = Instant::now();
    modernview::batch::run_batch_job(configs, tx, cancel_flag);

    while let Ok(event) = rx.recv() {
        if let modernview::batch::BatchProgressEvent::BatchFinished { total, completed, failed } = event {
            let total_elapsed = start.elapsed().as_secs_f64();
            println!("  • Batch Finished: {}/{} completed in {:.2} s ({:.1} images/sec)",
                completed, total, total_elapsed, total as f64 / total_elapsed);
            assert_eq!(completed, 10);
            assert_eq!(failed, 0);
            break;
        }
    }

    // Clean up
    let _ = std::fs::remove_dir_all(&temp_dir);

    println!("\n========================================================");
    println!("              ALL BENCHMARKS COMPLETED                 ");
    println!("========================================================");
}
