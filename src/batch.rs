use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use crossbeam_channel::Sender;
use rayon::prelude::*;
use crate::decoder::load_and_decode_image;
use crate::editor::{ResampleFilter, ImageEditor};
use crate::export::{save_image_safe, ExportFormat};

#[derive(Debug, Clone)]
pub struct BatchItemConfig {
    pub id: usize,
    pub input_path: PathBuf,
    pub target_format: ExportFormat,
    pub quality: u8,
    pub resize_mode: usize, // 0: None, 1: Max 1920, 2: Max 1280, 3: Scale 50%, 4: Scale 75%
    pub output_dir: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub enum BatchProgressEvent {
    ItemStarted { id: usize },
    ItemCompleted { id: usize, output_path: PathBuf },
    ItemFailed { id: usize, error: String },
    BatchFinished { total: usize, completed: usize, failed: usize },
}

pub struct BatchJobController {
    pub cancel_flag: Arc<AtomicBool>,
}

impl BatchJobController {
    pub fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel_flag.load(Ordering::SeqCst)
    }
}

pub fn run_batch_job(
    items: Vec<BatchItemConfig>,
    event_tx: Sender<BatchProgressEvent>,
    cancel_flag: Arc<AtomicBool>,
) {
    std::thread::spawn(move || {
        let total = items.len();
        let mut completed = 0;
        let mut failed = 0;

        // Process items with Rayon parallel iterator in chunks
        let results: Vec<(usize, Result<PathBuf, String>)> = items
            .into_par_iter()
            .map(|config| {
                if cancel_flag.load(Ordering::SeqCst) {
                    return (config.id, Err("Cancelled by user".to_string()));
                }

                let _ = event_tx.send(BatchProgressEvent::ItemStarted { id: config.id });

                let res = process_single_item(&config);
                match &res {
                    Ok(out_p) => {
                        let _ = event_tx.send(BatchProgressEvent::ItemCompleted {
                            id: config.id,
                            output_path: out_p.clone(),
                        });
                    }
                    Err(err) => {
                        let _ = event_tx.send(BatchProgressEvent::ItemFailed {
                            id: config.id,
                            error: err.clone(),
                        });
                    }
                }

                (config.id, res)
            })
            .collect();

        for (_, res) in results {
            if res.is_ok() {
                completed += 1;
            } else {
                failed += 1;
            }
        }

        let _ = event_tx.send(BatchProgressEvent::BatchFinished {
            total,
            completed,
            failed,
        });
    });
}

fn process_single_item(config: &BatchItemConfig) -> Result<PathBuf, String> {
    let decoded = load_and_decode_image(&config.input_path).map_err(|e| e.to_string())?;

    let mut editor = ImageEditor::new(decoded.rgba);

    // Apply resize if requested
    match config.resize_mode {
        1 => {
            // Max 1920x1080
            let (w, h) = (editor.working_image().width(), editor.working_image().height());
            if w > 1920 || h > 1080 {
                let scale = (1920.0 / w as f64).min(1080.0 / h as f64);
                let nw = ((w as f64 * scale).round() as u32).max(1);
                let nh = ((h as f64 * scale).round() as u32).max(1);
                editor.resize(nw, nh, ResampleFilter::Lanczos3)?;
            }
        }
        2 => {
            // Max 1280x720
            let (w, h) = (editor.working_image().width(), editor.working_image().height());
            if w > 1280 || h > 720 {
                let scale = (1280.0 / w as f64).min(720.0 / h as f64);
                let nw = ((w as f64 * scale).round() as u32).max(1);
                let nh = ((h as f64 * scale).round() as u32).max(1);
                editor.resize(nw, nh, ResampleFilter::Lanczos3)?;
            }
        }
        3 => {
            // Scale 50%
            let (w, h) = (editor.working_image().width(), editor.working_image().height());
            let nw = ((w as f64 * 0.5).round() as u32).max(1);
            let nh = ((h as f64 * 0.5).round() as u32).max(1);
            editor.resize(nw, nh, ResampleFilter::Lanczos3)?;
        }
        4 => {
            // Scale 75%
            let (w, h) = (editor.working_image().width(), editor.working_image().height());
            let nw = ((w as f64 * 0.75).round() as u32).max(1);
            let nh = ((h as f64 * 0.75).round() as u32).max(1);
            editor.resize(nw, nh, ResampleFilter::Lanczos3)?;
        }
        _ => {}
    }

    // Determine output file path
    let stem = config
        .input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");

    let out_dir = match &config.output_dir {
        Some(dir) => dir.clone(),
        None => config
            .input_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from(".")),
    };

    let target_ext = config.target_format.to_extension();
    let new_filename = if config.output_dir.is_some() {
        format!("{}.{}", stem, target_ext)
    } else {
        format!("{}_converted.{}", stem, target_ext)
    };

    let target_path = out_dir.join(new_filename);

    save_image_safe(
        editor.working_image(),
        &target_path,
        config.target_format,
        config.quality,
        None,
    )?;

    Ok(target_path)
}
