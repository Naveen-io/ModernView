use fast_image_resize as fr;
use image::{imageops, RgbaImage};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResampleFilter {
    Lanczos3,
    Bilinear,
    Nearest,
}

#[derive(Debug, Clone)]
pub struct Adjustments {
    pub brightness: f32, // -100.0 to 100.0
    pub contrast: f32,   // -100.0 to 100.0
    pub saturation: f32, // -100.0 to 100.0
}

impl Default for Adjustments {
    fn default() -> Self {
        Self {
            brightness: 0.0,
            contrast: 0.0,
            saturation: 0.0,
        }
    }
}

impl Adjustments {
    pub fn is_identity(&self) -> bool {
        self.brightness.abs() < 0.1 && self.contrast.abs() < 0.1 && self.saturation.abs() < 0.1
    }
}

pub struct ImageEditor {
    original: RgbaImage,
    working: RgbaImage,
    history: Vec<RgbaImage>,
    redo_stack: Vec<RgbaImage>,
    adjustments: Adjustments,
    max_history: usize,
}

impl ImageEditor {
    pub fn new(image: RgbaImage) -> Self {
        Self {
            original: image.clone(),
            working: image,
            history: Vec::new(),
            redo_stack: Vec::new(),
            adjustments: Adjustments::default(),
            max_history: 12,
        }
    }

    pub fn set_image(&mut self, image: RgbaImage) {
        self.original = image.clone();
        self.working = image;
        self.history.clear();
        self.redo_stack.clear();
        self.adjustments = Adjustments::default();
    }

    pub fn working_image(&self) -> &RgbaImage {
        &self.working
    }

    pub fn adjustments(&self) -> &Adjustments {
        &self.adjustments
    }

    pub fn has_unsaved_changes(&self) -> bool {
        !self.history.is_empty() || !self.adjustments.is_identity()
    }

    pub fn can_undo(&self) -> bool {
        !self.history.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    fn push_history(&mut self) {
        if self.history.len() >= self.max_history {
            self.history.remove(0);
        }
        self.history.push(self.working.clone());
        self.redo_stack.clear();
    }

    pub fn undo(&mut self) -> bool {
        if let Some(prev) = self.history.pop() {
            self.redo_stack.push(self.working.clone());
            self.working = prev;
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self) -> bool {
        if let Some(next) = self.redo_stack.pop() {
            self.history.push(self.working.clone());
            self.working = next;
            true
        } else {
            false
        }
    }

    pub fn revert_to_original(&mut self) {
        self.working = self.original.clone();
        self.history.clear();
        self.redo_stack.clear();
        self.adjustments = Adjustments::default();
    }

    pub fn rotate_90_cw(&mut self) {
        self.push_history();
        self.working = imageops::rotate90(&self.working);
    }

    pub fn rotate_90_ccw(&mut self) {
        self.push_history();
        self.working = imageops::rotate270(&self.working);
    }

    pub fn rotate_180(&mut self) {
        self.push_history();
        self.working = imageops::rotate180(&self.working);
    }

    pub fn flip_horizontal(&mut self) {
        self.push_history();
        self.working = imageops::flip_horizontal(&self.working);
    }

    pub fn flip_vertical(&mut self) {
        self.push_history();
        self.working = imageops::flip_vertical(&self.working);
    }

    pub fn crop(&mut self, norm_x: f32, norm_y: f32, norm_w: f32, norm_h: f32) -> Result<(), String> {
        let (img_w, img_h) = (self.working.width(), self.working.height());
        if img_w == 0 || img_h == 0 {
            return Err("Empty image".to_string());
        }

        let px_x = ((norm_x * img_w as f32).round() as u32).min(img_w.saturating_sub(1));
        let px_y = ((norm_y * img_h as f32).round() as u32).min(img_h.saturating_sub(1));
        let px_w = ((norm_w * img_w as f32).round() as u32).max(1).min(img_w - px_x);
        let px_h = ((norm_h * img_h as f32).round() as u32).max(1).min(img_h - px_y);

        if px_w == 0 || px_h == 0 {
            return Err("Invalid crop region".to_string());
        }

        self.push_history();
        let sub = imageops::crop_imm(&self.working, px_x, px_y, px_w, px_h).to_image();
        self.working = sub;
        Ok(())
    }

    pub fn resize(&mut self, target_w: u32, target_h: u32, filter: ResampleFilter) -> Result<(), String> {
        let (src_w, src_h) = (self.working.width(), self.working.height());
        if target_w == 0 || target_h == 0 {
            return Err("Target dimensions must be greater than zero".to_string());
        }
        if src_w == target_w && src_h == target_h {
            return Ok(());
        }

        let src_image = fr::images::Image::from_vec_u8(
            src_w,
            src_h,
            self.working.as_raw().clone(),
            fr::PixelType::U8x4,
        ).map_err(|e| e.to_string())?;

        let mut dst_image = fr::images::Image::new(target_w, target_h, fr::PixelType::U8x4);
        let mut resizer = fr::Resizer::new();

        let mut options = fr::ResizeOptions::default();
        options.algorithm = match filter {
            ResampleFilter::Lanczos3 => fr::ResizeAlg::Convolution(fr::FilterType::Lanczos3),
            ResampleFilter::Bilinear => fr::ResizeAlg::Convolution(fr::FilterType::Bilinear),
            ResampleFilter::Nearest => fr::ResizeAlg::Nearest,
        };

        resizer.resize(&src_image, &mut dst_image, &options).map_err(|e| e.to_string())?;

        let dst_raw = dst_image.into_vec();
        let new_img = RgbaImage::from_raw(target_w, target_h, dst_raw)
            .ok_or_else(|| "Failed to construct resized image".to_string())?;

        self.push_history();
        self.working = new_img;
        Ok(())
    }

    pub fn set_adjustments(&mut self, adj: Adjustments) {
        self.adjustments = adj;
    }

    pub fn render_preview(&self) -> RgbaImage {
        if self.adjustments.is_identity() {
            self.working.clone()
        } else {
            apply_adjustments(&self.working, &self.adjustments)
        }
    }

    pub fn apply_adjustments_permanently(&mut self) {
        if !self.adjustments.is_identity() {
            self.push_history();
            self.working = apply_adjustments(&self.working, &self.adjustments);
            self.adjustments = Adjustments::default();
        }
    }
}

pub fn apply_adjustments(src: &RgbaImage, adj: &Adjustments) -> RgbaImage {
    let mut out = src.clone();

    // Precalculate brightness + contrast LUT for fast execution
    let mut lut = [0u8; 256];
    let contrast_val = adj.contrast.clamp(-100.0, 100.0);
    let brightness_val = adj.brightness.clamp(-100.0, 100.0);

    let factor = if contrast_val >= 0.0 {
        (259.0 * (contrast_val + 255.0)) / (255.0 * (259.0 - contrast_val))
    } else {
        (255.0 + contrast_val) / 255.0
    };

    for i in 0..256 {
        let mut v = i as f32;
        // Brightness shift
        v += brightness_val * 1.28;
        // Contrast scaling around center 128
        v = factor * (v - 128.0) + 128.0;
        lut[i] = v.clamp(0.0, 255.0).round() as u8;
    }

    let sat_factor = if adj.saturation >= 0.0 {
        1.0 + (adj.saturation / 100.0) * 1.5
    } else {
        (100.0 + adj.saturation) / 100.0
    };

    let needs_sat = (sat_factor - 1.0).abs() > 0.01;

    let raw = out.as_mut();
    for pixel in raw.chunks_exact_mut(4) {
        let mut r = lut[pixel[0] as usize] as f32;
        let mut g = lut[pixel[1] as usize] as f32;
        let mut b = lut[pixel[2] as usize] as f32;

        if needs_sat {
            let gray = 0.299 * r + 0.587 * g + 0.114 * b;
            r = gray + (r - gray) * sat_factor;
            g = gray + (g - gray) * sat_factor;
            b = gray + (b - gray) * sat_factor;
        }

        pixel[0] = r.clamp(0.0, 255.0) as u8;
        pixel[1] = g.clamp(0.0, 255.0) as u8;
        pixel[2] = b.clamp(0.0, 255.0) as u8;
    }

    out
}
