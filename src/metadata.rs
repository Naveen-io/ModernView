use std::fs::File;
use std::io::BufReader;
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct ImageMetadata {
    pub file_name: String,
    pub file_path: String,
    pub file_size_bytes: u64,
    pub file_size_str: String,
    pub format_name: String,
    pub width: u32,
    pub height: u32,
    pub dimensions_str: String,
    pub megapixels_str: String,
    pub aspect_ratio_str: String,
    pub color_type_str: String,

    // EXIF
    pub has_exif: bool,
    pub camera_model: String,
    pub lens_model: String,
    pub aperture: String,
    pub shutter_speed: String,
    pub iso: String,
    pub focal_length: String,
    pub date_taken: String,
    pub orientation: u32,
}

pub fn format_file_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    let b = bytes as f64;
    if b >= GB {
        format!("{:.2} GB", b / GB)
    } else if b >= MB {
        format!("{:.2} MB", b / MB)
    } else if b >= KB {
        format!("{:.1} KB", b / KB)
    } else {
        format!("{} B", bytes)
    }
}

pub fn calculate_aspect_ratio(w: u32, h: u32) -> String {
    if w == 0 || h == 0 {
        return "-".to_string();
    }

    fn gcd(mut a: u32, mut b: u32) -> u32 {
        while b != 0 {
            let t = b;
            b = a % b;
            a = t;
        }
        a
    }

    let divisor = gcd(w, h);
    let rw = w / divisor;
    let rh = h / divisor;

    // Common standard aspect ratios
    let ratio = w as f64 / h as f64;
    if (ratio - 16.0 / 9.0).abs() < 0.02 {
        "16:9".to_string()
    } else if (ratio - 4.0 / 3.0).abs() < 0.02 {
        "4:3".to_string()
    } else if (ratio - 3.0 / 2.0).abs() < 0.02 {
        "3:2".to_string()
    } else if (ratio - 1.0).abs() < 0.01 {
        "1:1".to_string()
    } else if (ratio - 21.0 / 9.0).abs() < 0.02 {
        "21:9".to_string()
    } else if rw <= 20 && rh <= 20 {
        format!("{}:{}", rw, rh)
    } else {
        format!("{:.2}:1", ratio)
    }
}

pub fn read_image_metadata(path: &Path, width: u32, height: u32, color_type: &str) -> ImageMetadata {
    let file_name = path
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    let file_path = path.to_string_lossy().to_string();

    let file_size_bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let file_size_str = format_file_size(file_size_bytes);

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_uppercase();
    let format_name = match ext.as_str() {
        "JPG" | "JPEG" => "JPEG",
        "PNG" => "PNG",
        "WEBP" => "WebP",
        "GIF" => "GIF",
        "BMP" => "BMP",
        "TIFF" | "TIF" => "TIFF",
        "ICO" => "ICO",
        other => other,
    }
    .to_string();

    let dimensions_str = format!("{} × {} px", width, height);
    let mp = (width as f64 * height as f64) / 1_000_000.0;
    let megapixels_str = format!("{:.2} MP", mp);
    let aspect_ratio_str = calculate_aspect_ratio(width, height);
    let color_type_str = if color_type.is_empty() {
        "8-bit RGBA".to_string()
    } else {
        color_type.to_string()
    };

    let mut meta = ImageMetadata {
        file_name,
        file_path,
        file_size_bytes,
        file_size_str,
        format_name,
        width,
        height,
        dimensions_str,
        megapixels_str,
        aspect_ratio_str,
        color_type_str,
        has_exif: false,
        camera_model: String::new(),
        lens_model: String::new(),
        aperture: String::new(),
        shutter_speed: String::new(),
        iso: String::new(),
        focal_length: String::new(),
        date_taken: String::new(),
        orientation: 1,
    };

    // Extract EXIF
    if let Ok(file) = File::open(path) {
        let mut bufreader = BufReader::new(file);
        let exifreader = exif::Reader::new();
        if let Ok(exif) = exifreader.read_from_container(&mut bufreader) {
            meta.has_exif = true;

            // Camera Make / Model
            let make = exif
                .get_field(exif::Tag::Make, exif::In::PRIMARY)
                .map(|f| f.display_value().to_string());
            let model = exif
                .get_field(exif::Tag::Model, exif::In::PRIMARY)
                .map(|f| f.display_value().to_string());
            meta.camera_model = match (make, model) {
                (Some(m), Some(mo)) => {
                    let m_clean = m.trim_matches('"');
                    let mo_clean = mo.trim_matches('"');
                    if mo_clean.starts_with(m_clean) {
                        mo_clean.to_string()
                    } else {
                        format!("{} {}", m_clean, mo_clean)
                    }
                }
                (None, Some(mo)) => mo.trim_matches('"').to_string(),
                (Some(m), None) => m.trim_matches('"').to_string(),
                _ => String::new(),
            };

            // Lens Model
            if let Some(lens) = exif.get_field(exif::Tag::LensModel, exif::In::PRIMARY) {
                meta.lens_model = lens.display_value().to_string().trim_matches('"').to_string();
            }

            // FNumber (Aperture)
            if let Some(fnum) = exif.get_field(exif::Tag::FNumber, exif::In::PRIMARY) {
                meta.aperture = format!("f/{}", fnum.display_value());
            }

            // ExposureTime (Shutter speed)
            if let Some(exp) = exif.get_field(exif::Tag::ExposureTime, exif::In::PRIMARY) {
                meta.shutter_speed = format!("{} s", exp.display_value());
            }

            // PhotographicSensitivity (ISO)
            if let Some(iso) = exif.get_field(exif::Tag::PhotographicSensitivity, exif::In::PRIMARY) {
                meta.iso = format!("ISO {}", iso.display_value());
            }

            // FocalLength
            if let Some(fl) = exif.get_field(exif::Tag::FocalLength, exif::In::PRIMARY) {
                meta.focal_length = format!("{} mm", fl.display_value());
            }

            // DateTimeOriginal
            if let Some(dt) = exif.get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY) {
                meta.date_taken = dt.display_value().to_string().trim_matches('"').to_string();
            }

            // Orientation
            if let Some(orient) = exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY) {
                if let exif::Value::Short(ref v) = orient.value {
                    if let Some(&first) = v.first() {
                        meta.orientation = first as u32;
                    }
                }
            }
        }
    }

    meta
}
