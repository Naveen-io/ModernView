use image::{ColorType, ImageEncoder, ImageFormat, RgbImage, RgbaImage};
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Png,
    Jpeg,
    WebP,
    Bmp,
    Tiff,
    Ico,
}

impl ExportFormat {
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "jpg" | "jpeg" => ExportFormat::Jpeg,
            "webp" => ExportFormat::WebP,
            "bmp" => ExportFormat::Bmp,
            "tif" | "tiff" => ExportFormat::Tiff,
            "ico" => ExportFormat::Ico,
            _ => ExportFormat::Png,
        }
    }

    pub fn to_extension(&self) -> &'static str {
        match self {
            ExportFormat::Png => "png",
            ExportFormat::Jpeg => "jpg",
            ExportFormat::WebP => "webp",
            ExportFormat::Bmp => "bmp",
            ExportFormat::Tiff => "tiff",
            ExportFormat::Ico => "ico",
        }
    }

    pub fn supports_alpha(&self) -> bool {
        match self {
            ExportFormat::Png | ExportFormat::WebP | ExportFormat::Tiff | ExportFormat::Ico => true,
            ExportFormat::Jpeg | ExportFormat::Bmp => false,
        }
    }
}

pub fn flatten_rgba_to_rgb(rgba: &RgbaImage, bg_color: [u8; 3]) -> RgbImage {
    let (w, h) = (rgba.width(), rgba.height());
    let mut rgb = RgbImage::new(w, h);

    for (src_p, dst_p) in rgba.pixels().zip(rgb.pixels_mut()) {
        let alpha = src_p[3] as f32 / 255.0;
        let inv_alpha = 1.0 - alpha;

        let r = (src_p[0] as f32 * alpha + bg_color[0] as f32 * inv_alpha).round() as u8;
        let g = (src_p[1] as f32 * alpha + bg_color[1] as f32 * inv_alpha).round() as u8;
        let b = (src_p[2] as f32 * alpha + bg_color[2] as f32 * inv_alpha).round() as u8;

        *dst_p = image::Rgb([r, g, b]);
    }

    rgb
}

pub fn has_transparency(rgba: &RgbaImage) -> bool {
    rgba.pixels().any(|p| p[3] < 255)
}

pub fn save_image_safe(
    image: &RgbaImage,
    target_path: &Path,
    format: ExportFormat,
    quality: u8,
    flatten_bg: Option<[u8; 3]>,
) -> Result<(), String> {
    let parent = target_path.parent().unwrap_or_else(|| Path::new("."));
    if !parent.exists() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Failed to create directory: {}", e))?;
    }

    // Write to a temporary file first in the same directory for atomic rename
    let tmp_filename = format!(
        ".modernview_tmp_{}_{}.tmp",
        std::process::id(),
        target_path.file_name().and_then(|f| f.to_str()).unwrap_or("out")
    );
    let tmp_path = parent.join(tmp_filename);

    let file = File::create(&tmp_path).map_err(|e| format!("Cannot create file: {}", e))?;
    let mut writer = BufWriter::new(file);

    let bg = flatten_bg.unwrap_or([255, 255, 255]);

    let res = match format {
        ExportFormat::Png => {
            let encoder = image::codecs::png::PngEncoder::new(writer);
            encoder.write_image(
                image.as_raw(),
                image.width(),
                image.height(),
                ColorType::Rgba8.into(),
            )
        }
        ExportFormat::Jpeg => {
            let rgb = flatten_rgba_to_rgb(image, bg);
            let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(writer, quality.clamp(1, 100));
            encoder.write_image(
                rgb.as_raw(),
                rgb.width(),
                rgb.height(),
                ColorType::Rgb8.into(),
            )
        }
        ExportFormat::WebP => {
            // Encode WebP via image crate or dynamic image write
            let dyn_img = image::DynamicImage::ImageRgba8(image.clone());
            dyn_img.write_to(&mut writer, ImageFormat::WebP)
        }
        ExportFormat::Bmp => {
            let rgb = flatten_rgba_to_rgb(image, bg);
            let mut encoder = image::codecs::bmp::BmpEncoder::new(&mut writer);
            encoder.encode(
                rgb.as_raw(),
                rgb.width(),
                rgb.height(),
                ColorType::Rgb8.into(),
            )
        }
        ExportFormat::Tiff => {
            let encoder = image::codecs::tiff::TiffEncoder::new(writer);
            encoder.write_image(
                image.as_raw(),
                image.width(),
                image.height(),
                ColorType::Rgba8.into(),
            )
        }
        ExportFormat::Ico => {
            let encoder = image::codecs::ico::IcoEncoder::new(writer);
            encoder.write_image(
                image.as_raw(),
                image.width(),
                image.height(),
                ColorType::Rgba8.into(),
            )
        }
    };

    if let Err(e) = res {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(format!("Encoding failed: {}", e));
    }

    // Atomic rename to final target path
    if let Err(e) = std::fs::rename(&tmp_path, target_path) {
        // Fallback for cross-device or permission replacement on Windows
        if std::fs::copy(&tmp_path, target_path).is_ok() {
            let _ = std::fs::remove_file(&tmp_path);
            Ok(())
        } else {
            let _ = std::fs::remove_file(&tmp_path);
            Err(format!("Failed to finalize image file: {}", e))
        }
    } else {
        Ok(())
    }
}
