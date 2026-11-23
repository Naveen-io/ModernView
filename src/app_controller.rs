use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use crossbeam_channel::unbounded;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use crate::batch::{run_batch_job, BatchItemConfig, BatchJobController, BatchProgressEvent};
use crate::browser::FolderBrowser;
use crate::cache::{create_shared_cache, SharedImageCache};
use crate::compare::CompareEngine;
use crate::decoder::{load_and_decode_image, rgba_to_slint_image};
use crate::editor::{Adjustments, ImageEditor, ResampleFilter};
use crate::export::{save_image_safe, ExportFormat};
use crate::settings::AppSettings;
use crate::thumbnail::generate_thumbnail_from_rgba;
use crate::{AppWindow, BatchQueueItem, FilmstripItem, RecentItem};

pub struct AppState {
    pub browser: FolderBrowser,
    pub editor: Option<ImageEditor>,
    pub cache: SharedImageCache,
    pub compare: CompareEngine,
    pub settings: AppSettings,
    pub current_filepath: Option<PathBuf>,
    pub batch_files: Vec<PathBuf>,
    pub batch_controller: Option<BatchJobController>,
    pub zoom: f32,
    pub pan_x: f32,
    pub pan_y: f32,
}

pub struct AppController {
    app: AppWindow,
    state: Rc<RefCell<AppState>>,
}

impl AppController {
    pub fn new(app: AppWindow, initial_file: Option<PathBuf>) -> Self {
        let settings = AppSettings::load();
        let cache = create_shared_cache(settings.cache_limit_mb);

        let state = Rc::new(RefCell::new(AppState {
            browser: FolderBrowser::new(),
            editor: None,
            cache,
            compare: CompareEngine::new(),
            settings,
            current_filepath: None,
            batch_files: Vec::new(),
            batch_controller: None,
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
        }));

        let controller = Self { app, state };
        controller.init(initial_file);
        controller
    }

    fn init(&self, initial_file: Option<PathBuf>) {
        self.apply_initial_settings();
        self.wire_callbacks();

        if let Some(path) = initial_file {
            self.load_image_file(&path);
        } else {
            // Check if there are recent files to suggest
            let state = self.state.borrow();
            if let Some(recent) = state.settings.recent_files.first() {
                let p = PathBuf::from(recent);
                if p.exists() {
                    drop(state);
                    self.load_image_file(&p);
                }
            }
        }
    }

    fn apply_initial_settings(&self) {
        let state = self.state.borrow();
        self.app.set_opt_dark_mode(state.settings.dark_mode);
        self.app.set_opt_default_zoom(state.settings.default_zoom_mode as i32);
        self.app.set_filmstrip_open(state.settings.filmstrip_visible);
        self.app.set_inspector_open(state.settings.inspector_visible);
        self.app.set_opt_cache_limit_mb(state.settings.cache_limit_mb as f32);
        self.app.set_opt_default_format(state.settings.default_export_format as i32);
        self.app.set_opt_default_quality(state.settings.default_jpeg_quality as f32);
        self.app.set_opt_confirm_overwrite(state.settings.confirm_overwrite);

        self.update_recent_files_ui();
    }

    fn update_recent_files_ui(&self) {
        let state = self.state.borrow();
        let items: Vec<RecentItem> = state
            .settings
            .recent_files
            .iter()
            .map(|p_str| {
                let p = Path::new(p_str);
                let fname = p
                    .file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    .unwrap_or_else(|| p_str.clone());
                RecentItem {
                    path: SharedString::from(p_str.clone()),
                    filename: SharedString::from(fname),
                }
            })
            .collect();

        self.app.set_recent_items(ModelRc::new(VecModel::from(items)));
    }

    pub fn load_image_file(&self, path: &Path) {
        if !path.exists() {
            self.show_toast(&format!("File not found: {}", path.display()), true);
            return;
        }

        self.app.set_is_loading(true);

        // Update browser folder context
        {
            let mut state = self.state.borrow_mut();
            state.browser.set_file(path);
            state.current_filepath = Some(path.to_path_buf());
            state.settings.add_recent(&path.to_string_lossy());
        }
        self.update_recent_files_ui();

        // Check memory cache first
        let cached = {
            let state = self.state.borrow();
            if let Ok(mut c) = state.cache.lock() {
                c.get(path)
            } else {
                None
            }
        };

        let decoded = match cached {
            Some((rgba, meta)) => {
                crate::decoder::DecodedImage {
                    width: rgba.width(),
                    height: rgba.height(),
                    rgba,
                    metadata: meta,
                }
            }
            None => {
                match load_and_decode_image(path) {
                    Ok(img) => {
                        let state = self.state.borrow();
                        if let Ok(mut c) = state.cache.lock() {
                            c.insert(path.to_path_buf(), img.rgba.clone(), img.metadata.clone());
                        }
                        img
                    }
                    Err(e) => {
                        self.app.set_is_loading(false);
                        self.show_toast(&format!("Failed to open image: {}", e), true);
                        return;
                    }
                }
            }
        };

        // Construct editor state
        let slint_img = rgba_to_slint_image(&decoded.rgba);
        let (w, h) = (decoded.width, decoded.height);

        {
            let mut state = self.state.borrow_mut();
            state.editor = Some(ImageEditor::new(decoded.rgba.clone()));
            state.zoom = 1.0;
            state.pan_x = 0.0;
            state.pan_y = 0.0;

            // Also configure comparison slot A if empty
            if state.compare.image_a.is_none() {
                let _ = state.compare.set_image_a(path);
            }
        }

        // Update UI properties
        self.app.set_current_image(slint_img);
        self.app.set_img_width(w as f32);
        self.app.set_img_height(h as f32);
        self.app.set_filename(SharedString::from(&decoded.metadata.file_name));
        self.app.set_filepath(SharedString::from(&decoded.metadata.file_path));
        self.app.set_file_format(SharedString::from(&decoded.metadata.format_name));
        self.app.set_file_size_str(SharedString::from(&decoded.metadata.file_size_str));
        self.app.set_dimensions_str(SharedString::from(&decoded.metadata.dimensions_str));
        self.app.set_megapixels_str(SharedString::from(&decoded.metadata.megapixels_str));
        self.app.set_aspect_ratio_str(SharedString::from(&decoded.metadata.aspect_ratio_str));
        self.app.set_color_type_str(SharedString::from(&decoded.metadata.color_type_str));

        self.app.set_window_title(SharedString::from(format!(
            "{} — {} — ModernView",
            decoded.metadata.file_name, decoded.metadata.dimensions_str
        )));

        // EXIF properties
        self.app.set_has_exif(decoded.metadata.has_exif);
        self.app.set_exif_camera(SharedString::from(&decoded.metadata.camera_model));
        self.app.set_exif_lens(SharedString::from(&decoded.metadata.lens_model));
        self.app.set_exif_aperture(SharedString::from(&decoded.metadata.aperture));
        self.app.set_exif_shutter(SharedString::from(&decoded.metadata.shutter_speed));
        self.app.set_exif_iso(SharedString::from(&decoded.metadata.iso));
        self.app.set_exif_focal(SharedString::from(&decoded.metadata.focal_length));
        self.app.set_exif_date(SharedString::from(&decoded.metadata.date_taken));

        // Editor properties
        self.app.set_edit_resize_w(SharedString::from(w.to_string()));
        self.app.set_edit_resize_h(SharedString::from(h.to_string()));
        self.app.set_edit_brightness(0.0);
        self.app.set_edit_contrast(0.0);
        self.app.set_edit_saturation(0.0);
        self.app.set_edit_can_undo(false);
        self.app.set_edit_can_redo(false);
        self.app.set_edit_has_unsaved(false);
        self.app.set_edit_is_cropping(false);

        // Reset view / Zoom
        let default_zoom = self.state.borrow().settings.default_zoom_mode;
        if default_zoom == 0 {
            self.fit_to_window();
        } else {
            self.actual_size();
        }

        self.update_browser_navigation_ui();
        self.populate_filmstrip();

        self.app.set_is_loading(false);
    }

    fn update_browser_navigation_ui(&self) {
        let state = self.state.borrow();
        self.app.set_can_prev(state.browser.can_prev());
        self.app.set_can_next(state.browser.can_next());
        self.app.set_folder_image_index(state.browser.current_index.unwrap_or(0) as i32);
        self.app.set_folder_image_total(state.browser.total_count() as i32);
        if let Some(ref folder) = state.browser.current_folder {
            self.app.set_current_folder(SharedString::from(folder.to_string_lossy().as_ref()));
        }
    }

    fn populate_filmstrip(&self) {
        let state = self.state.borrow();
        let cur_idx = state.browser.current_index.unwrap_or(0);
        let files: Vec<PathBuf> = state.browser.file_list.clone();
        drop(state);

        let mut items = Vec::new();
        for (i, p) in files.iter().enumerate() {
            let fname = p
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_default();

            // Generate or fetch thumbnail (lazy / lightweight)
            let thumb = if i == cur_idx {
                if let Some(ref ed) = self.state.borrow().editor {
                    if let Ok(rgba) = generate_thumbnail_from_rgba(ed.working_image(), 96) {
                        rgba_to_slint_image(&rgba)
                    } else {
                        slint::Image::default()
                    }
                } else {
                    slint::Image::default()
                }
            } else {
                slint::Image::default()
            };

            items.push(FilmstripItem {
                index: i as i32,
                filename: SharedString::from(fname),
                path: SharedString::from(p.to_string_lossy().as_ref()),
                thumbnail: thumb,
                is_selected: i == cur_idx,
            });
        }

        self.app.set_filmstrip_items(ModelRc::new(VecModel::from(items)));
    }

    pub fn fit_to_window(&self) {
        let mut state = self.state.borrow_mut();
        state.zoom = 1.0;
        state.pan_x = 0.0;
        state.pan_y = 0.0;
        self.app.set_zoom_level(1.0);
        self.app.set_pan_offset_x(0.0);
        self.app.set_pan_offset_y(0.0);
    }

    pub fn actual_size(&self) {
        let mut state = self.state.borrow_mut();
        state.zoom = 1.0;
        state.pan_x = 0.0;
        state.pan_y = 0.0;
        self.app.set_zoom_level(1.0);
        self.app.set_pan_offset_x(0.0);
        self.app.set_pan_offset_y(0.0);
    }

    pub fn zoom_in(&self) {
        let mut state = self.state.borrow_mut();
        state.zoom = (state.zoom * 1.25).min(25.0);
        self.app.set_zoom_level(state.zoom);
    }

    pub fn zoom_out(&self) {
        let mut state = self.state.borrow_mut();
        state.zoom = (state.zoom * 0.8).max(0.05);
        self.app.set_zoom_level(state.zoom);
    }

    pub fn show_toast(&self, message: &str, is_error: bool) {
        self.app.set_toast_message(SharedString::from(message));
        self.app.set_toast_is_error(is_error);
        self.app.set_toast_visible(true);
    }

    fn update_editor_ui(&self) {
        let state = self.state.borrow();
        if let Some(ref ed) = state.editor {
            let rendered = ed.render_preview();
            let slint_img = rgba_to_slint_image(&rendered);
            let (w, h) = (rendered.width(), rendered.height());

            self.app.set_current_image(slint_img);
            self.app.set_img_width(w as f32);
            self.app.set_img_height(h as f32);
            self.app.set_dimensions_str(SharedString::from(format!("{} × {} px", w, h)));
            self.app.set_edit_resize_w(SharedString::from(w.to_string()));
            self.app.set_edit_resize_h(SharedString::from(h.to_string()));
            self.app.set_edit_can_undo(ed.can_undo());
            self.app.set_edit_can_redo(ed.can_redo());
            self.app.set_edit_has_unsaved(ed.has_unsaved_changes());
        }
    }

    fn wire_callbacks(&self) {
        let app_weak = self.app.as_weak();
        let state_rc = Rc::clone(&self.state);

        // Open File
        {
            let app_w = app_weak.clone();
            let s_rc = Rc::clone(&state_rc);
            self.app.on_open_file(move || {
                if let Some(file) = rfd::FileDialog::new()
                    .add_filter("Image Files", &["jpg", "jpeg", "png", "webp", "gif", "bmp", "tiff", "ico"])
                    .pick_file()
                {
                    if let Some(app) = app_w.upgrade() {
                        let ctrl = AppController { app, state: Rc::clone(&s_rc) };
                        ctrl.load_image_file(&file);
                    }
                }
            });
        }

        // Open Folder
        {
            let app_w = app_weak.clone();
            let s_rc = Rc::clone(&state_rc);
            self.app.on_open_folder(move || {
                if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                    let mut s = s_rc.borrow_mut();
                    s.browser.scan_folder(&folder, None);
                    if let Some(first) = s.browser.file_list.first().cloned() {
                        drop(s);
                        if let Some(app) = app_w.upgrade() {
                            let ctrl = AppController { app, state: Rc::clone(&s_rc) };
                            ctrl.load_image_file(&first);
                        }
                    } else if let Some(app) = app_w.upgrade() {
                        let ctrl = AppController { app, state: Rc::clone(&s_rc) };
                        ctrl.show_toast("No supported images found in this folder", false);
                    }
                }
            });
        }

        // Navigation: Next / Prev
        {
            let app_w = app_weak.clone();
            let s_rc = Rc::clone(&state_rc);
            self.app.on_next_image(move || {
                let target = {
                    let mut s = s_rc.borrow_mut();
                    s.browser.next_index();
                    s.browser.current_file().cloned()
                };
                if let Some(path) = target {
                    if let Some(app) = app_w.upgrade() {
                        let ctrl = AppController { app, state: Rc::clone(&s_rc) };
                        ctrl.load_image_file(&path);
                    }
                }
            });
        }

        {
            let app_w = app_weak.clone();
            let s_rc = Rc::clone(&state_rc);
            self.app.on_prev_image(move || {
                let target = {
                    let mut s = s_rc.borrow_mut();
                    s.browser.prev_index();
                    s.browser.current_file().cloned()
                };
                if let Some(path) = target {
                    if let Some(app) = app_w.upgrade() {
                        let ctrl = AppController { app, state: Rc::clone(&s_rc) };
                        ctrl.load_image_file(&path);
                    }
                }
            });
        }

        // Filmstrip index select
        {
            let app_w = app_weak.clone();
            let s_rc = Rc::clone(&state_rc);
            self.app.on_select_filmstrip_index(move |idx| {
                let target = {
                    let mut s = s_rc.borrow_mut();
                    s.browser.set_index(idx as usize).cloned()
                };
                if let Some(path) = target {
                    if let Some(app) = app_w.upgrade() {
                        let ctrl = AppController { app, state: Rc::clone(&s_rc) };
                        ctrl.load_image_file(&path);
                    }
                }
            });
        }

        // Zoom & Pan
        {
            let app_w = app_weak.clone();
            let s_rc = Rc::clone(&state_rc);
            self.app.on_zoom_in(move || {
                if let Some(app) = app_w.upgrade() {
                    let ctrl = AppController { app, state: Rc::clone(&s_rc) };
                    ctrl.zoom_in();
                }
            });
        }

        {
            let app_w = app_weak.clone();
            let s_rc = Rc::clone(&state_rc);
            self.app.on_zoom_out(move || {
                if let Some(app) = app_w.upgrade() {
                    let ctrl = AppController { app, state: Rc::clone(&s_rc) };
                    ctrl.zoom_out();
                }
            });
        }

        {
            let app_w = app_weak.clone();
            let s_rc = Rc::clone(&state_rc);
            self.app.on_fit_window(move || {
                if let Some(app) = app_w.upgrade() {
                    let ctrl = AppController { app, state: Rc::clone(&s_rc) };
                    ctrl.fit_to_window();
                }
            });
        }

        {
            let app_w = app_weak.clone();
            let s_rc = Rc::clone(&state_rc);
            self.app.on_actual_size(move || {
                if let Some(app) = app_w.upgrade() {
                    let ctrl = AppController { app, state: Rc::clone(&s_rc) };
                    ctrl.actual_size();
                }
            });
        }

        {
            let app_w = app_weak.clone();
            let s_rc = Rc::clone(&state_rc);
            self.app.on_pan_delta(move |dx, dy| {
                let mut s = s_rc.borrow_mut();
                s.pan_x += dx;
                s.pan_y += dy;
                if let Some(app) = app_w.upgrade() {
                    app.set_pan_offset_x(s.pan_x);
                    app.set_pan_offset_y(s.pan_y);
                }
            });
        }

        // Recent file select
        {
            let app_w = app_weak.clone();
            let s_rc = Rc::clone(&state_rc);
            self.app.on_select_recent_file(move |path_str| {
                let p = PathBuf::from(path_str.as_str());
                if let Some(app) = app_w.upgrade() {
                    let ctrl = AppController { app, state: Rc::clone(&s_rc) };
                    ctrl.load_image_file(&p);
                }
            });
        }

        // Favorites folder select
        {
            let app_w = app_weak.clone();
            let s_rc = Rc::clone(&state_rc);
            self.app.on_select_favorite_folder(move |fav_name| {
                let target_dir = match fav_name.as_str() {
                    "pictures" => directories::UserDirs::new().and_then(|u| u.picture_dir().map(|p| p.to_path_buf())),
                    "downloads" => directories::UserDirs::new().and_then(|u| u.download_dir().map(|p| p.to_path_buf())),
                    "desktop" => directories::UserDirs::new().and_then(|u| u.desktop_dir().map(|p| p.to_path_buf())),
                    _ => None,
                };

                if let Some(dir) = target_dir {
                    let mut s = s_rc.borrow_mut();
                    s.browser.scan_folder(&dir, None);
                    if let Some(first) = s.browser.file_list.first().cloned() {
                        drop(s);
                        if let Some(app) = app_w.upgrade() {
                            let ctrl = AppController { app, state: Rc::clone(&s_rc) };
                            ctrl.load_image_file(&first);
                        }
                    } else if let Some(app) = app_w.upgrade() {
                        let ctrl = AppController { app, state: Rc::clone(&s_rc) };
                        ctrl.show_toast("No supported images in this folder", false);
                    }
                }
            });
        }

        // Editor Transforms
        {
            let app_w = app_weak.clone();
            let s_rc = Rc::clone(&state_rc);
            self.app.on_rotate_90_cw(move || {
                {
                    let mut s = s_rc.borrow_mut();
                    if let Some(ref mut ed) = s.editor {
                        ed.rotate_90_cw();
                    }
                }
                if let Some(app) = app_w.upgrade() {
                    let ctrl = AppController { app, state: Rc::clone(&s_rc) };
                    ctrl.update_editor_ui();
                }
            });
        }

        {
            let app_w = app_weak.clone();
            let s_rc = Rc::clone(&state_rc);
            self.app.on_rotate_90_ccw(move || {
                {
                    let mut s = s_rc.borrow_mut();
                    if let Some(ref mut ed) = s.editor {
                        ed.rotate_90_ccw();
                    }
                }
                if let Some(app) = app_w.upgrade() {
                    let ctrl = AppController { app, state: Rc::clone(&s_rc) };
                    ctrl.update_editor_ui();
                }
            });
        }

        {
            let app_w = app_weak.clone();
            let s_rc = Rc::clone(&state_rc);
            self.app.on_rotate_180(move || {
                {
                    let mut s = s_rc.borrow_mut();
                    if let Some(ref mut ed) = s.editor {
                        ed.rotate_180();
                    }
                }
                if let Some(app) = app_w.upgrade() {
                    let ctrl = AppController { app, state: Rc::clone(&s_rc) };
                    ctrl.update_editor_ui();
                }
            });
        }

        {
            let app_w = app_weak.clone();
            let s_rc = Rc::clone(&state_rc);
            self.app.on_flip_horizontal(move || {
                {
                    let mut s = s_rc.borrow_mut();
                    if let Some(ref mut ed) = s.editor {
                        ed.flip_horizontal();
                    }
                }
                if let Some(app) = app_w.upgrade() {
                    let ctrl = AppController { app, state: Rc::clone(&s_rc) };
                    ctrl.update_editor_ui();
                }
            });
        }

        {
            let app_w = app_weak.clone();
            let s_rc = Rc::clone(&state_rc);
            self.app.on_flip_vertical(move || {
                {
                    let mut s = s_rc.borrow_mut();
                    if let Some(ref mut ed) = s.editor {
                        ed.flip_vertical();
                    }
                }
                if let Some(app) = app_w.upgrade() {
                    let ctrl = AppController { app, state: Rc::clone(&s_rc) };
                    ctrl.update_editor_ui();
                }
            });
        }

        // Crop callbacks
        {
            let app_w = app_weak.clone();
            self.app.on_toggle_crop(move || {
                if let Some(app) = app_w.upgrade() {
                    let cur = app.get_edit_is_cropping();
                    app.set_edit_is_cropping(!cur);
                }
            });
        }

        {
            let app_w = app_weak.clone();
            self.app.on_set_crop_aspect(move |preset| {
                if let Some(app) = app_w.upgrade() {
                    app.set_edit_crop_aspect_preset(preset);
                }
            });
        }

        {
            let app_w = app_weak.clone();
            let s_rc = Rc::clone(&state_rc);
            self.app.on_apply_crop(move || {
                {
                    let mut s = s_rc.borrow_mut();
                    if let Some(ref mut ed) = s.editor {
                        // Standard crop box default 0.1 to 0.9 (80% crop)
                        let _ = ed.crop(0.1, 0.1, 0.8, 0.8);
                    }
                }
                if let Some(app) = app_w.upgrade() {
                    app.set_edit_is_cropping(false);
                    let ctrl = AppController { app, state: Rc::clone(&s_rc) };
                    ctrl.update_editor_ui();
                    ctrl.show_toast("Crop applied", false);
                }
            });
        }

        {
            let app_w = app_weak.clone();
            self.app.on_cancel_crop(move || {
                if let Some(app) = app_w.upgrade() {
                    app.set_edit_is_cropping(false);
                }
            });
        }

        // Resize callbacks
        {
            let app_w = app_weak.clone();
            let s_rc = Rc::clone(&state_rc);
            self.app.on_apply_resize(move |target_w, target_h, filter_idx| {
                let filter = match filter_idx {
                    1 => ResampleFilter::Bilinear,
                    2 => ResampleFilter::Nearest,
                    _ => ResampleFilter::Lanczos3,
                };
                let res = {
                    let mut s = s_rc.borrow_mut();
                    if let Some(ref mut ed) = s.editor {
                        ed.resize(target_w as u32, target_h as u32, filter)
                    } else {
                        Err("No image active".to_string())
                    }
                };

                if let Some(app) = app_w.upgrade() {
                    let ctrl = AppController { app, state: Rc::clone(&s_rc) };
                    match res {
                        Ok(()) => {
                            ctrl.update_editor_ui();
                            ctrl.show_toast(&format!("Resized to {} × {} px", target_w, target_h), false);
                        }
                        Err(e) => {
                            ctrl.show_toast(&format!("Resize failed: {}", e), true);
                        }
                    }
                }
            });
        }

        {
            let app_w = app_weak.clone();
            let s_rc = Rc::clone(&state_rc);
            self.app.on_set_resize_scale(move |scale| {
                if let Some(app) = app_w.upgrade() {
                    let s = s_rc.borrow();
                    if let Some(ref ed) = s.editor {
                        let cur_w = ed.working_image().width() as f32;
                        let cur_h = ed.working_image().height() as f32;
                        let new_w = (cur_w * scale).round() as u32;
                        let new_h = (cur_h * scale).round() as u32;
                        app.set_edit_resize_w(SharedString::from(new_w.to_string()));
                        app.set_edit_resize_h(SharedString::from(new_h.to_string()));
                    }
                }
            });
        }

        // Adjustments: Brightness, Contrast, Saturation
        {
            let app_w = app_weak.clone();
            let s_rc = Rc::clone(&state_rc);
            self.app.on_brightness_changed(move |v| {
                {
                    let mut s = s_rc.borrow_mut();
                    if let Some(ref mut ed) = s.editor {
                        let mut adj = ed.adjustments().clone();
                        adj.brightness = v;
                        ed.set_adjustments(adj);
                    }
                }
                if let Some(app) = app_w.upgrade() {
                    app.set_edit_brightness(v);
                    let ctrl = AppController { app, state: Rc::clone(&s_rc) };
                    ctrl.update_editor_ui();
                }
            });
        }

        {
            let app_w = app_weak.clone();
            let s_rc = Rc::clone(&state_rc);
            self.app.on_contrast_changed(move |v| {
                {
                    let mut s = s_rc.borrow_mut();
                    if let Some(ref mut ed) = s.editor {
                        let mut adj = ed.adjustments().clone();
                        adj.contrast = v;
                        ed.set_adjustments(adj);
                    }
                }
                if let Some(app) = app_w.upgrade() {
                    app.set_edit_contrast(v);
                    let ctrl = AppController { app, state: Rc::clone(&s_rc) };
                    ctrl.update_editor_ui();
                }
            });
        }

        {
            let app_w = app_weak.clone();
            let s_rc = Rc::clone(&state_rc);
            self.app.on_saturation_changed(move |v| {
                {
                    let mut s = s_rc.borrow_mut();
                    if let Some(ref mut ed) = s.editor {
                        let mut adj = ed.adjustments().clone();
                        adj.saturation = v;
                        ed.set_adjustments(adj);
                    }
                }
                if let Some(app) = app_w.upgrade() {
                    app.set_edit_saturation(v);
                    let ctrl = AppController { app, state: Rc::clone(&s_rc) };
                    ctrl.update_editor_ui();
                }
            });
        }

        {
            let app_w = app_weak.clone();
            let s_rc = Rc::clone(&state_rc);
            self.app.on_reset_adjustments(move || {
                {
                    let mut s = s_rc.borrow_mut();
                    if let Some(ref mut ed) = s.editor {
                        ed.set_adjustments(Adjustments::default());
                    }
                }
                if let Some(app) = app_w.upgrade() {
                    app.set_edit_brightness(0.0);
                    app.set_edit_contrast(0.0);
                    app.set_edit_saturation(0.0);
                    let ctrl = AppController { app, state: Rc::clone(&s_rc) };
                    ctrl.update_editor_ui();
                }
            });
        }

        // History: Undo / Redo / Revert
        {
            let app_w = app_weak.clone();
            let s_rc = Rc::clone(&state_rc);
            self.app.on_undo(move || {
                let success = {
                    let mut s = s_rc.borrow_mut();
                    s.editor.as_mut().map(|ed| ed.undo()).unwrap_or(false)
                };
                if success {
                    if let Some(app) = app_w.upgrade() {
                        let ctrl = AppController { app, state: Rc::clone(&s_rc) };
                        ctrl.update_editor_ui();
                    }
                }
            });
        }

        {
            let app_w = app_weak.clone();
            let s_rc = Rc::clone(&state_rc);
            self.app.on_redo(move || {
                let success = {
                    let mut s = s_rc.borrow_mut();
                    s.editor.as_mut().map(|ed| ed.redo()).unwrap_or(false)
                };
                if success {
                    if let Some(app) = app_w.upgrade() {
                        let ctrl = AppController { app, state: Rc::clone(&s_rc) };
                        ctrl.update_editor_ui();
                    }
                }
            });
        }

        {
            let app_w = app_weak.clone();
            let s_rc = Rc::clone(&state_rc);
            self.app.on_revert_original(move || {
                {
                    let mut s = s_rc.borrow_mut();
                    if let Some(ref mut ed) = s.editor {
                        ed.revert_to_original();
                    }
                }
                if let Some(app) = app_w.upgrade() {
                    let ctrl = AppController { app, state: Rc::clone(&s_rc) };
                    ctrl.update_editor_ui();
                    ctrl.show_toast("Reverted all modifications to original", false);
                }
            });
        }

        // Save & Export
        {
            let app_w = app_weak.clone();
            let s_rc = Rc::clone(&state_rc);
            self.app.on_save_overwrite(move || {
                let (has_file, confirm_needed) = {
                    let s = s_rc.borrow();
                    (s.current_filepath.is_some(), s.settings.confirm_overwrite)
                };

                if let Some(app) = app_w.upgrade() {
                    if !has_file {
                        // If no file loaded, trigger Save As
                        let ctrl = AppController { app, state: Rc::clone(&s_rc) };
                        ctrl.trigger_save_as();
                    } else if confirm_needed {
                        app.set_confirm_dialog_open(true);
                    } else {
                        let ctrl = AppController { app, state: Rc::clone(&s_rc) };
                        ctrl.perform_save_overwrite();
                    }
                }
            });
        }

        {
            let app_w = app_weak.clone();
            let s_rc = Rc::clone(&state_rc);
            self.app.on_confirm_overwrite_action(move || {
                if let Some(app) = app_w.upgrade() {
                    let ctrl = AppController { app, state: Rc::clone(&s_rc) };
                    ctrl.perform_save_overwrite();
                }
            });
        }

        {
            let app_w = app_weak.clone();
            let s_rc = Rc::clone(&state_rc);
            self.app.on_save_as(move || {
                if let Some(app) = app_w.upgrade() {
                    let ctrl = AppController { app, state: Rc::clone(&s_rc) };
                    ctrl.trigger_save_as();
                }
            });
        }

        // Compare mode callbacks
        {
            let app_w = app_weak.clone();
            let s_rc = Rc::clone(&state_rc);
            self.app.on_swap_compare_images(move || {
                {
                    let mut s = s_rc.borrow_mut();
                    s.compare.swap();
                }
                if let Some(app) = app_w.upgrade() {
                    let ctrl = AppController { app, state: Rc::clone(&s_rc) };
                    ctrl.update_compare_ui();
                }
            });
        }

        {
            let app_w = app_weak.clone();
            let s_rc = Rc::clone(&state_rc);
            self.app.on_fit_both_compare(move || {
                {
                    let mut s = s_rc.borrow_mut();
                    s.compare.reset_view();
                }
                if let Some(app) = app_w.upgrade() {
                    app.set_zoom_level(1.0);
                    app.set_pan_offset_x(0.0);
                    app.set_pan_offset_y(0.0);
                }
            });
        }

        // Batch processing callbacks
        {
            let app_w = app_weak.clone();
            let s_rc = Rc::clone(&state_rc);
            self.app.on_batch_add_files(move || {
                if let Some(files) = rfd::FileDialog::new()
                    .add_filter("Images", &["jpg", "jpeg", "png", "webp", "bmp", "tiff", "ico"])
                    .pick_files()
                {
                    let mut s = s_rc.borrow_mut();
                    for f in files {
                        if !s.batch_files.contains(&f) {
                            s.batch_files.push(f);
                        }
                    }
                    drop(s);
                    if let Some(app) = app_w.upgrade() {
                        let ctrl = AppController { app, state: Rc::clone(&s_rc) };
                        ctrl.update_batch_queue_ui();
                    }
                }
            });
        }

        {
            let app_w = app_weak.clone();
            let s_rc = Rc::clone(&state_rc);
            self.app.on_batch_add_folder(move || {
                if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                    let mut browser = FolderBrowser::new();
                    browser.scan_folder(&folder, None);
                    let mut s = s_rc.borrow_mut();
                    for f in browser.file_list {
                        if !s.batch_files.contains(&f) {
                            s.batch_files.push(f);
                        }
                    }
                    drop(s);
                    if let Some(app) = app_w.upgrade() {
                        let ctrl = AppController { app, state: Rc::clone(&s_rc) };
                        ctrl.update_batch_queue_ui();
                    }
                }
            });
        }

        {
            let app_w = app_weak.clone();
            let s_rc = Rc::clone(&state_rc);
            self.app.on_batch_clear(move || {
                {
                    let mut s = s_rc.borrow_mut();
                    s.batch_files.clear();
                }
                if let Some(app) = app_w.upgrade() {
                    let ctrl = AppController { app, state: Rc::clone(&s_rc) };
                    ctrl.update_batch_queue_ui();
                }
            });
        }

        {
            let app_w = app_weak.clone();
            self.app.on_batch_browse_output(move || {
                if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                    if let Some(app) = app_w.upgrade() {
                        app.set_batch_output_dir(SharedString::from(folder.to_string_lossy().as_ref()));
                    }
                }
            });
        }

        {
            let app_w = app_weak.clone();
            let s_rc = Rc::clone(&state_rc);
            self.app.on_batch_start(move |format_idx, quality, resize_mode| {
                if let Some(app) = app_w.upgrade() {
                    let ctrl = AppController { app, state: Rc::clone(&s_rc) };
                    ctrl.start_batch_job(format_idx as usize, quality as u8, resize_mode as usize);
                }
            });
        }

        {
            let s_rc = Rc::clone(&state_rc);
            self.app.on_batch_cancel(move || {
                let s = s_rc.borrow();
                if let Some(ref ctrl) = s.batch_controller {
                    ctrl.cancel();
                }
            });
        }

        {
            let app_w = app_weak.clone();
            let s_rc = Rc::clone(&state_rc);
            self.app.on_batch_open_output(move || {
                let out_dir = {
                    let s = s_rc.borrow();
                    if let Some(ref first) = s.batch_files.first() {
                        first.parent().map(|p| p.to_path_buf())
                    } else {
                        None
                    }
                };

                if let Some(dir) = out_dir {
                    let _ = std::process::Command::new("explorer").arg(dir).spawn();
                } else if let Some(app) = app_w.upgrade() {
                    let ctrl = AppController { app, state: Rc::clone(&s_rc) };
                    ctrl.show_toast("No output directory available", false);
                }
            });
        }

        // Settings save
        {
            let app_w = app_weak.clone();
            let s_rc = Rc::clone(&state_rc);
            self.app.on_save_settings(move || {
                if let Some(app) = app_w.upgrade() {
                    let mut s = s_rc.borrow_mut();
                    s.settings.dark_mode = app.get_opt_dark_mode();
                    s.settings.default_zoom_mode = app.get_opt_default_zoom() as usize;
                    s.settings.cache_limit_mb = app.get_opt_cache_limit_mb() as usize;
                    s.settings.default_export_format = app.get_opt_default_format() as usize;
                    s.settings.default_jpeg_quality = app.get_opt_default_quality() as u8;
                    s.settings.confirm_overwrite = app.get_opt_confirm_overwrite();
                    let _ = s.settings.save();

                    // Update memory cache limit
                    if let Ok(mut c) = s.cache.lock() {
                        c.set_max_mb(s.settings.cache_limit_mb);
                    }

                    let ctrl = AppController { app, state: Rc::clone(&s_rc) };
                    ctrl.show_toast("Settings saved successfully", false);
                }
            });
        }

        {
            let app_w = app_weak.clone();
            let s_rc = Rc::clone(&state_rc);
            self.app.on_register_associations(move || {
                if let Some(app) = app_w.upgrade() {
                    let ctrl = AppController { app, state: Rc::clone(&s_rc) };
                    match crate::windows::register_windows_integration() {
                        Ok(_) => {
                            ctrl.show_toast("✓ ModernView registered in Windows context menu & default apps", false);
                        }
                        Err(e) => {
                            ctrl.show_toast(&format!("Registration failed: {}", e), true);
                        }
                    }
                }
            });
        }
    }

    fn update_compare_ui(&self) {
        let state = self.state.borrow();
        if let Some(ref img_a) = state.compare.image_a {
            let slint_a = rgba_to_slint_image(img_a);
            self.app.set_compare_image_a(slint_a);
            self.app.set_compare_img_a_w(img_a.width() as f32);
            self.app.set_compare_img_a_h(img_a.height() as f32);
            if let Some(ref meta) = state.compare.meta_a {
                self.app.set_compare_name_a(SharedString::from(&meta.file_name));
                self.app.set_compare_specs_a(SharedString::from(format!("({})", meta.dimensions_str)));
            }
        }

        if let Some(ref img_b) = state.compare.image_b {
            let slint_b = rgba_to_slint_image(img_b);
            self.app.set_compare_image_b(slint_b);
            self.app.set_compare_img_b_w(img_b.width() as f32);
            self.app.set_compare_img_b_h(img_b.height() as f32);
            if let Some(ref meta) = state.compare.meta_b {
                self.app.set_compare_name_b(SharedString::from(&meta.file_name));
                self.app.set_compare_specs_b(SharedString::from(format!("({})", meta.dimensions_str)));
            }
        }
    }

    fn update_batch_queue_ui(&self) {
        let state = self.state.borrow();
        let mut items = Vec::new();
        for (id, path) in state.batch_files.iter().enumerate() {
            let fname = path
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_default();
            let size_str = std::fs::metadata(path)
                .map(|m| crate::metadata::format_file_size(m.len()))
                .unwrap_or_else(|_| "-".to_string());

            items.push(BatchQueueItem {
                id: id as i32,
                filename: SharedString::from(fname),
                file_path: SharedString::from(path.to_string_lossy().as_ref()),
                file_size_str: SharedString::from(size_str),
                status: SharedString::from("Queued"),
                is_done: false,
                is_failed: false,
            });
        }

        self.app.set_batch_total(items.len() as i32);
        self.app.set_batch_completed(0);
        self.app.set_batch_failed(0);
        self.app.set_batch_progress(0.0);
        self.app.set_batch_status_msg(SharedString::from(format!(
            "{} images in queue. Click 'Start Batch Processing' to begin.",
            items.len()
        )));
        self.app.set_batch_queue(ModelRc::new(VecModel::from(items)));
    }

    fn start_batch_job(&self, format_idx: usize, quality: u8, resize_mode: usize) {
        let target_format = match format_idx {
            1 => ExportFormat::Png,
            2 => ExportFormat::WebP,
            3 => ExportFormat::Bmp,
            4 => ExportFormat::Tiff,
            _ => ExportFormat::Jpeg,
        };

        let state = self.state.borrow();
        if state.batch_files.is_empty() {
            return;
        }

        let out_dir_str = self.app.get_batch_output_dir().to_string();
        let output_dir = if !out_dir_str.is_empty() {
            Some(PathBuf::from(out_dir_str))
        } else {
            None
        };

        let configs: Vec<BatchItemConfig> = state
            .batch_files
            .iter()
            .enumerate()
            .map(|(id, path)| BatchItemConfig {
                id,
                input_path: path.clone(),
                target_format,
                quality,
                resize_mode,
                output_dir: output_dir.clone(),
            })
            .collect();

        drop(state);

        let cancel_flag = Arc::new(AtomicBool::new(false));
        let (tx, rx) = unbounded();

        {
            let mut s = self.state.borrow_mut();
            s.batch_controller = Some(BatchJobController {
                cancel_flag: Arc::clone(&cancel_flag),
            });
        }

        self.app.set_batch_is_running(true);
        self.app.set_batch_progress(0.0);
        self.app.set_batch_status_msg(SharedString::from("Batch processing running..."));

        run_batch_job(configs, tx, cancel_flag);

        // Timer to drain progress events and update Slint model
        let app_weak = self.app.as_weak();
        let timer = slint::Timer::default();
        let total = self.state.borrow().batch_files.len();

        let mut completed_count = 0;
        let mut failed_count = 0;

        timer.start(slint::TimerMode::Repeated, std::time::Duration::from_millis(60), move || {
            while let Ok(event) = rx.try_recv() {
                match event {
                    BatchProgressEvent::ItemStarted { id } => {
                        if let Some(app) = app_weak.upgrade() {
                            app.set_batch_status_msg(SharedString::from(format!(
                                "Processing file {} of {}...",
                                id + 1,
                                total
                            )));
                        }
                    }
                    BatchProgressEvent::ItemCompleted { id: _, output_path: _ } => {
                        completed_count += 1;
                        if let Some(app) = app_weak.upgrade() {
                            let prog = (completed_count + failed_count) as f32 / total as f32;
                            app.set_batch_progress(prog);
                            app.set_batch_completed(completed_count as i32);
                        }
                    }
                    BatchProgressEvent::ItemFailed { id: _, error: _ } => {
                        failed_count += 1;
                        if let Some(app) = app_weak.upgrade() {
                            let prog = (completed_count + failed_count) as f32 / total as f32;
                            app.set_batch_progress(prog);
                            app.set_batch_failed(failed_count as i32);
                        }
                    }
                    BatchProgressEvent::BatchFinished { total: _, completed, failed } => {
                        if let Some(app) = app_weak.upgrade() {
                            app.set_batch_is_running(false);
                            app.set_batch_progress(1.0);
                            app.set_batch_completed(completed as i32);
                            app.set_batch_failed(failed as i32);
                            app.set_batch_status_msg(SharedString::from(format!(
                                "Batch complete: {} processed successfully, {} failed.",
                                completed, failed
                            )));
                        }
                        return;
                    }
                }
            }
        });
    }

    fn perform_save_overwrite(&self) {
        let (img, path) = {
            let mut s = self.state.borrow_mut();
            if let Some(ref mut ed) = s.editor {
                ed.apply_adjustments_permanently();
                let img = ed.working_image().clone();
                let path = s.current_filepath.clone();
                (Some(img), path)
            } else {
                (None, None)
            }
        };

        if let (Some(img), Some(path)) = (img, path) {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("png");
            let format = ExportFormat::from_extension(ext);
            let quality = self.state.borrow().settings.default_jpeg_quality;

            match save_image_safe(&img, &path, format, quality, None) {
                Ok(()) => {
                    self.show_toast(&format!("Saved image: {}", path.file_name().and_then(|f| f.to_str()).unwrap_or("file")), false);
                    self.load_image_file(&path);
                }
                Err(e) => {
                    self.show_toast(&format!("Save failed: {}", e), true);
                }
            }
        }
    }

    fn trigger_save_as(&self) {
        let img = {
            let mut s = self.state.borrow_mut();
            if let Some(ref mut ed) = s.editor {
                ed.apply_adjustments_permanently();
                Some(ed.working_image().clone())
            } else {
                None
            }
        };

        if let Some(img) = img {
            let default_name = self
                .state
                .borrow()
                .current_filepath
                .as_ref()
                .and_then(|p| p.file_name().and_then(|f| f.to_str()))
                .unwrap_or("exported_image.png")
                .to_string();

            if let Some(target) = rfd::FileDialog::new()
                .set_file_name(&default_name)
                .add_filter("PNG Image", &["png"])
                .add_filter("JPEG Image", &["jpg", "jpeg"])
                .add_filter("WebP Image", &["webp"])
                .add_filter("BMP Image", &["bmp"])
                .add_filter("TIFF Image", &["tiff", "tif"])
                .save_file()
            {
                let ext = target.extension().and_then(|e| e.to_str()).unwrap_or("png");
                let format = ExportFormat::from_extension(ext);
                let quality = self.state.borrow().settings.default_jpeg_quality;

                match save_image_safe(&img, &target, format, quality, None) {
                    Ok(()) => {
                        self.show_toast(&format!("Exported to: {}", target.display()), false);
                        self.load_image_file(&target);
                    }
                    Err(e) => {
                        self.show_toast(&format!("Export failed: {}", e), true);
                    }
                }
            }
        }
    }
}
