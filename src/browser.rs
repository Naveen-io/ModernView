use std::path::{Path, PathBuf};

const SUPPORTED_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "webp", "gif", "bmp", "tiff", "tif", "ico",
];

pub fn is_supported_image(path: &Path) -> bool {
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        let ext_lower = ext.to_lowercase();
        SUPPORTED_EXTENSIONS.contains(&ext_lower.as_str())
    } else {
        false
    }
}

pub fn natural_sort_key(s: &str) -> Vec<(bool, String, u64)> {
    let mut parts = Vec::new();
    let mut current_digits = String::new();
    let mut current_chars = String::new();

    for c in s.chars() {
        if c.is_ascii_digit() {
            if !current_chars.is_empty() {
                parts.push((false, current_chars.to_lowercase(), 0));
                current_chars.clear();
            }
            current_digits.push(c);
        } else {
            if !current_digits.is_empty() {
                let num = current_digits.parse::<u64>().unwrap_or(0);
                parts.push((true, String::new(), num));
                current_digits.clear();
            }
            current_chars.push(c);
        }
    }

    if !current_chars.is_empty() {
        parts.push((false, current_chars.to_lowercase(), 0));
    }
    if !current_digits.is_empty() {
        let num = current_digits.parse::<u64>().unwrap_or(0);
        parts.push((true, String::new(), num));
    }

    parts
}

#[derive(Debug, Clone, Default)]
pub struct FolderBrowser {
    pub current_folder: Option<PathBuf>,
    pub file_list: Vec<PathBuf>,
    pub current_index: Option<usize>,
}

impl FolderBrowser {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn scan_folder(&mut self, folder: &Path, initial_file: Option<&Path>) {
        self.current_folder = Some(folder.to_path_buf());
        self.file_list.clear();
        self.current_index = None;

        if let Ok(entries) = std::fs::read_dir(folder) {
            let mut files: Vec<PathBuf> = entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.is_file() && is_supported_image(p))
                .collect();

            // Natural alphanumeric sort
            files.sort_by(|a, b| {
                let name_a = a.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
                let name_b = b.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
                natural_sort_key(&name_a).cmp(&natural_sort_key(&name_b))
            });

            if let Some(target) = initial_file {
                if let Some(pos) = files.iter().position(|p| p == target) {
                    self.current_index = Some(pos);
                }
            }

            if self.current_index.is_none() && !files.is_empty() {
                self.current_index = Some(0);
            }

            self.file_list = files;
        }
    }

    pub fn set_file(&mut self, file: &Path) {
        if let Some(parent) = file.parent() {
            if self.current_folder.as_deref() != Some(parent) {
                self.scan_folder(parent, Some(file));
            } else if let Some(pos) = self.file_list.iter().position(|p| p == file) {
                self.current_index = Some(pos);
            }
        }
    }

    pub fn current_file(&self) -> Option<&PathBuf> {
        self.current_index.and_then(|idx| self.file_list.get(idx))
    }

    pub fn can_prev(&self) -> bool {
        self.current_index.map(|idx| idx > 0).unwrap_or(false)
    }

    pub fn can_next(&self) -> bool {
        self.current_index
            .map(|idx| idx + 1 < self.file_list.len())
            .unwrap_or(false)
    }

    pub fn prev_index(&mut self) -> Option<usize> {
        if let Some(idx) = self.current_index {
            if idx > 0 {
                self.current_index = Some(idx - 1);
                return self.current_index;
            }
        }
        None
    }

    pub fn next_index(&mut self) -> Option<usize> {
        if let Some(idx) = self.current_index {
            if idx + 1 < self.file_list.len() {
                self.current_index = Some(idx + 1);
                return self.current_index;
            }
        }
        None
    }

    pub fn set_index(&mut self, idx: usize) -> Option<&PathBuf> {
        if idx < self.file_list.len() {
            self.current_index = Some(idx);
            self.file_list.get(idx)
        } else {
            None
        }
    }

    pub fn total_count(&self) -> usize {
        self.file_list.len()
    }
}
