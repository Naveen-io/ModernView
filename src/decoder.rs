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
    image::image_dimensions(path).map_err(|e| DecodeError::CorruptData(e.to_string()))
}

pub fn load_and_decode_image(path: &Path) -> Result<DecodedImage, DecodeError> {
    if !path.exists() {
        return Err(DecodeError::FileNotFound(path.to_string_lossy().to_string()));
    }

    let reader = ImageReader::open(path)
        .map_err(|e| DecodeError::IoError(e.to_string()))?
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
