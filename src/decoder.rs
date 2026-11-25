use image::{ImageReader, RgbaImage};
use std::path::Path;
use crate::metadata::{read_image_metadata, ImageMetadata};

#[derive(Debug)]
pub enum DecodeError {
    FileNotFound(String),
    UnsupportedFormat(String),
    CorruptData(String),
    IoError(String),
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecodeError::FileNotFound(p) => write!(f, "File not found: {}", p),
            DecodeError::UnsupportedFormat(e) => write!(f, "Unsupported image format: {}", e),
            DecodeError::CorruptData(e) => write!(f, "Image file corrupted or invalid: {}", e),
            DecodeError::IoError(e) => write!(f, "File access error: {}", e),
        }
    }
}

pub struct DecodedImage {
    pub rgba: RgbaImage,
    pub width: u32,
    pub height: u32,
    pub metadata: ImageMetadata,
}

impl DecodedImage {
    pub fn to_slint_image(&self) -> slint::Image {
        rgba_to_slint_image(&self.rgba)
    }
}

pub fn rgba_to_slint_image(rgba: &RgbaImage) -> slint::Image {
    let width = rgba.width();
    let height = rgba.height();
    let mut buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(width, height);
    buffer.make_mut_bytes().copy_from_slice(rgba.as_raw());
    slint::Image::from_rgba8(buffer)
}

pub fn probe_image_dimensions(path: &Path) -> Result<(u32, u32), DecodeError> {
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        let ext_lower = ext.to_lowercase();
        if ext_lower == "heic" || ext_lower == "heif" {
            if let Ok(data) = std::fs::read(path) {
                let mut decoder = zune_heic::HeifDecoder::new(zune_heic::zune_core::bytestream::ZCursor::new(&data));
                if decoder.decode_headers().is_ok() {
                    if let (Some(w), Some(h)) = (decoder.width(), decoder.height()) {
                        return Ok((w as u32, h as u32));
                    }
                }
            }
        } else if ext_lower == "avif" {
            if let Ok(data) = std::fs::read(path) {
                if let Ok(info) = oxideav_avif::info(&data) {
                    return Ok((info.width, info.height));
                }
            }
        }
    }
    image::image_dimensions(path).map_err(|e| DecodeError::CorruptData(e.to_string()))
}

fn decode_heic(path: &Path) -> Result<DecodedImage, DecodeError> {
    let data = std::fs::read(path).map_err(|e| DecodeError::IoError(e.to_string()))?;
    let mut decoder = zune_heic::HeifDecoder::new(zune_heic::zune_core::bytestream::ZCursor::new(&data));
    decoder.decode_headers().map_err(|e| DecodeError::CorruptData(format!("{:?}", e)))?;

    let width = decoder.width().unwrap_or(0) as u32;
    let height = decoder.height().unwrap_or(0) as u32;
    let colorspace = decoder.colorspace().unwrap_or(zune_heic::zune_core::colorspace::ColorSpace::RGB);

    let pixels = decoder.decode().map_err(|e| DecodeError::CorruptData(format!("{:?}", e)))?;

    let rgba_pixels = match colorspace {
        zune_heic::zune_core::colorspace::ColorSpace::RGB => {
            let mut out = Vec::with_capacity((width * height * 4) as usize);
            for chunk in pixels.chunks_exact(3) {
                out.push(chunk[0]);
                out.push(chunk[1]);
                out.push(chunk[2]);
                out.push(255);
            }
            out
        }
        zune_heic::zune_core::colorspace::ColorSpace::RGBA => pixels,
        zune_heic::zune_core::colorspace::ColorSpace::Luma => {
            let mut out = Vec::with_capacity((width * height * 4) as usize);
            for &lum in &pixels {
                out.push(lum);
                out.push(lum);
                out.push(lum);
                out.push(255);
            }
            out
        }
        zune_heic::zune_core::colorspace::ColorSpace::LumaA => {
            let mut out = Vec::with_capacity((width * height * 4) as usize);
            for chunk in pixels.chunks_exact(2) {
                out.push(chunk[0]);
                out.push(chunk[0]);
                out.push(chunk[0]);
                out.push(chunk[1]);
            }
            out
        }
        _ => {
            if pixels.len() == (width * height * 3) as usize {
                let mut out = Vec::with_capacity((width * height * 4) as usize);
                for chunk in pixels.chunks_exact(3) {
                    out.push(chunk[0]);
                    out.push(chunk[1]);
                    out.push(chunk[2]);
                    out.push(255);
                }
                out
            } else if pixels.len() == (width * height * 4) as usize {
                pixels
            } else {
                return Err(DecodeError::UnsupportedFormat(format!("Unsupported HEIC colorspace {:?}", colorspace)));
            }
        }
    };

    let rgba = RgbaImage::from_raw(width, height, rgba_pixels)
        .ok_or_else(|| DecodeError::CorruptData("Failed to create RGBA buffer from HEIC".to_string()))?;

    let meta = read_image_metadata(path, width, height, "HEIC/HEIF");

    Ok(DecodedImage {
        rgba,
        width,
        height,
        metadata: meta,
    })
}

fn decode_avif(path: &Path) -> Result<DecodedImage, DecodeError> {
    let data = std::fs::read(path).map_err(|e| DecodeError::IoError(e.to_string()))?;
    let img = oxideav_avif::decode(&data)
        .map_err(|e| DecodeError::CorruptData(format!("AVIF decode error: {:?}", e)))?;

    let width = img.width();
    let height = img.height();
    let rgba_raw = img.to_rgba8();

    let rgba = RgbaImage::from_raw(width, height, rgba_raw)
        .ok_or_else(|| DecodeError::CorruptData("Failed to create RGBA buffer from AVIF".to_string()))?;

    let meta = read_image_metadata(path, width, height, "AVIF");

    Ok(DecodedImage {
        rgba,
        width,
        height,
        metadata: meta,
    })
}

pub fn load_and_decode_image(path: &Path) -> Result<DecodedImage, DecodeError> {
    if !path.exists() {
        return Err(DecodeError::FileNotFound(path.to_string_lossy().to_string()));
    }

    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        let ext_lower = ext.to_lowercase();
        if ext_lower == "heic" || ext_lower == "heif" {
            return decode_heic(path);
        } else if ext_lower == "avif" {
            return decode_avif(path);
        }
    }

    let file = std::fs::File::open(path)
        .map_err(|e| DecodeError::IoError(e.to_string()))?;
    let buf_reader = std::io::BufReader::with_capacity(128 * 1024, file);
    let reader = ImageReader::new(buf_reader)
        .with_guessed_format()
        .map_err(|e| DecodeError::UnsupportedFormat(e.to_string()))?;

    let format_str = format!("{:?}", reader.format());
    let mut dynamic_img = reader.decode().map_err(|e| DecodeError::CorruptData(e.to_string()))?;

    // Read metadata and orientation
    let mut meta = read_image_metadata(path, dynamic_img.width(), dynamic_img.height(), &format_str);

    // Apply EXIF orientation if needed
    if meta.has_exif && meta.orientation > 1 {
        dynamic_img = match meta.orientation {
            2 => dynamic_img.fliph(),
            3 => dynamic_img.rotate180(),
            4 => dynamic_img.flipv(),
            5 => dynamic_img.rotate90().fliph(),
            6 => dynamic_img.rotate90(),
            7 => dynamic_img.rotate270().fliph(),
            8 => dynamic_img.rotate270(),
            _ => dynamic_img,
        };
        // Update dimensions in metadata after rotation
        meta.width = dynamic_img.width();
        meta.height = dynamic_img.height();
        meta.dimensions_str = format!("{} × {} px", meta.width, meta.height);
        meta.aspect_ratio_str = crate::metadata::calculate_aspect_ratio(meta.width, meta.height);
    }

    let rgba = dynamic_img.to_rgba8();
    let width = rgba.width();
    let height = rgba.height();

    Ok(DecodedImage {
        rgba,
        width,
        height,
        metadata: meta,
    })
}
