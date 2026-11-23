use fast_image_resize as fr;
use image::RgbaImage;
use std::path::Path;

pub fn generate_thumbnail_from_rgba(src: &RgbaImage, max_dim: u32) -> Result<RgbaImage, String> {
    let (src_w, src_h) = (src.width(), src.height());
    if src_w == 0 || src_h == 0 {
        return Err("Zero dimensions".to_string());
    }

    let scale = (max_dim as f64 / src_w.max(src_h) as f64).min(1.0);
    let dst_w = ((src_w as f64 * scale).round() as u32).max(1);
    let dst_h = ((src_h as f64 * scale).round() as u32).max(1);

    if dst_w == src_w && dst_h == src_h {
        return Ok(src.clone());
    }

    let src_image = fr::images::Image::from_vec_u8(
        src_w,
        src_h,
        src.as_raw().clone(),
        fr::PixelType::U8x4,
    ).map_err(|e| e.to_string())?;

    let mut dst_image = fr::images::Image::new(dst_w, dst_h, fr::PixelType::U8x4);
    let mut resizer = fr::Resizer::new();
    resizer.resize(&src_image, &mut dst_image, &fr::ResizeOptions::default())
        .map_err(|e| e.to_string())?;

    let dst_raw = dst_image.into_vec();
    RgbaImage::from_raw(dst_w, dst_h, dst_raw).ok_or_else(|| "Failed to create RgbaImage".to_string())
}

pub fn generate_thumbnail_from_path(path: &Path, max_dim: u32) -> Result<slint::Image, String> {
    let decoded = crate::decoder::load_and_decode_image(path).map_err(|e| e.to_string())?;
    let thumb_rgba = generate_thumbnail_from_rgba(&decoded.rgba, max_dim)?;
    Ok(crate::decoder::rgba_to_slint_image(&thumb_rgba))
}
