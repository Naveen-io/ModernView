use image::RgbaImage;
use std::path::{Path, PathBuf};
use crate::decoder::load_and_decode_image;
use crate::metadata::ImageMetadata;

#[derive(Default)]
pub struct CompareEngine {
    pub path_a: Option<PathBuf>,
    pub image_a: Option<RgbaImage>,
    pub meta_a: Option<ImageMetadata>,

    pub path_b: Option<PathBuf>,
    pub image_b: Option<RgbaImage>,
    pub meta_b: Option<ImageMetadata>,

    pub zoom: f32,
    pub pan_x: f32,
    pub pan_y: f32,
}

impl CompareEngine {
    pub fn new() -> Self {
        Self {
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
            ..Default::default()
        }
    }

    pub fn set_image_a(&mut self, path: &Path) -> Result<(), String> {
        let decoded = load_and_decode_image(path).map_err(|e| e.to_string())?;
        self.path_a = Some(path.to_path_buf());
        self.meta_a = Some(decoded.metadata);
        self.image_a = Some(decoded.rgba);
        Ok(())
    }

    pub fn set_image_b(&mut self, path: &Path) -> Result<(), String> {
        let decoded = load_and_decode_image(path).map_err(|e| e.to_string())?;
        self.path_b = Some(path.to_path_buf());
        self.meta_b = Some(decoded.metadata);
        self.image_b = Some(decoded.rgba);
        Ok(())
    }

    pub fn swap(&mut self) {
        std::mem::swap(&mut self.path_a, &mut self.path_b);
        std::mem::swap(&mut self.image_a, &mut self.image_b);
        std::mem::swap(&mut self.meta_a, &mut self.meta_b);
    }

    pub fn reset_view(&mut self) {
        self.zoom = 1.0;
        self.pan_x = 0.0;
        self.pan_y = 0.0;
    }
}
