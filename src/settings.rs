use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub dark_mode: bool,
    pub default_zoom_mode: usize, // 0: Fit, 1: 100%
    pub filmstrip_visible: bool,
    pub inspector_visible: bool,
    pub cache_limit_mb: usize,
    pub default_export_format: usize, // 0: PNG, 1: JPEG, 2: WebP
    pub default_jpeg_quality: u8,
    pub confirm_overwrite: bool,
    pub recent_files: Vec<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            dark_mode: true,
            default_zoom_mode: 0,
            filmstrip_visible: true,
            inspector_visible: false,
            cache_limit_mb: 256,
            default_export_format: 0,
            default_jpeg_quality: 85,
            confirm_overwrite: true,
            recent_files: Vec::new(),
        }
    }
}

impl AppSettings {
    pub fn config_path() -> Option<PathBuf> {
        directories::ProjectDirs::from("com", "ModernView", "ModernView")
            .map(|proj| proj.config_dir().join("settings.json"))
    }

    pub fn load() -> Self {
        if let Some(path) = Self::config_path() {
            if path.exists() {
                if let Ok(contents) = std::fs::read_to_string(&path) {
                    if let Ok(settings) = serde_json::from_str::<AppSettings>(&contents) {
                        return settings;
                    }
                }
            }
        }
        Self::default()
    }

    pub fn save(&self) -> Result<(), String> {
        if let Some(path) = Self::config_path() {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let data = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
            std::fs::write(&path, data).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub fn add_recent(&mut self, file_path: &str) {
        self.recent_files.retain(|p| p != file_path);
        self.recent_files.insert(0, file_path.to_string());
        if self.recent_files.len() > 10 {
            self.recent_files.truncate(10);
        }
        let _ = self.save();
    }
}
