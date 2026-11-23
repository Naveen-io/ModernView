use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use image::RgbaImage;
use crate::metadata::ImageMetadata;

pub struct CachedImage {
    pub rgba: RgbaImage,
    pub metadata: ImageMetadata,
    pub byte_size: usize,
    pub last_accessed: u64,
}

pub struct ImageCache {
    entries: HashMap<PathBuf, CachedImage>,
    max_bytes: usize,
    current_bytes: usize,
    access_counter: u64,
}

impl ImageCache {
    pub fn new(max_mb: usize) -> Self {
        Self {
            entries: HashMap::new(),
            max_bytes: max_mb * 1024 * 1024,
            current_bytes: 0,
            access_counter: 0,
        }
    }

    pub fn set_max_mb(&mut self, max_mb: usize) {
        self.max_bytes = max_mb * 1024 * 1024;
        self.evict_if_needed();
    }

    pub fn get(&mut self, path: &Path) -> Option<(RgbaImage, ImageMetadata)> {
        self.access_counter += 1;
        let counter = self.access_counter;
        if let Some(entry) = self.entries.get_mut(path) {
            entry.last_accessed = counter;
            return Some((entry.rgba.clone(), entry.metadata.clone()));
        }
        None
    }

    pub fn insert(&mut self, path: PathBuf, rgba: RgbaImage, metadata: ImageMetadata) {
        let byte_size = (rgba.width() * rgba.height() * 4) as usize;
        self.access_counter += 1;

        if let Some(old) = self.entries.remove(&path) {
            self.current_bytes = self.current_bytes.saturating_sub(old.byte_size);
        }

        self.current_bytes += byte_size;
        self.entries.insert(
            path,
            CachedImage {
                rgba,
                metadata,
                byte_size,
                last_accessed: self.access_counter,
            },
        );

        self.evict_if_needed();
    }

    fn evict_if_needed(&mut self) {
        while self.current_bytes > self.max_bytes && !self.entries.is_empty() {
            // Find oldest accessed entry
            let oldest_path = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_accessed)
                .map(|(path, _)| path.clone());

            if let Some(path) = oldest_path {
                if let Some(removed) = self.entries.remove(&path) {
                    self.current_bytes = self.current_bytes.saturating_sub(removed.byte_size);
                }
            } else {
                break;
            }
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.current_bytes = 0;
    }
}

pub type SharedImageCache = Arc<Mutex<ImageCache>>;

pub fn create_shared_cache(max_mb: usize) -> SharedImageCache {
    Arc::new(Mutex::new(ImageCache::new(max_mb)))
}
