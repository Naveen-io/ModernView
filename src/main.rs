#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(dead_code)]

slint::include_modules!();

mod metadata;
mod decoder;
mod thumbnail;
mod cache;
mod browser;
mod editor;
mod export;
mod batch;
mod compare;
mod settings;
mod windows;
mod app_controller;

use app_controller::AppController;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Check CLI argument for direct file opening (e.g. from Windows Explorer or command line)
    let cli_file = windows::parse_cli_image_path();

    // 2. Initialize Slint Window
    let app = AppWindow::new()?;

    // 3. Initialize Controller and wire state
    let _controller = AppController::new(app.clone_strong(), cli_file);

    // 4. Start Event Loop
    app.run()?;

    Ok(())
}
