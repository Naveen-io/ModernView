# ModernView

A fast, modern, and lightweight native image viewer and editor for Windows.

![ModernView Preview](assets/modernview-preview.png)

## Overview

ModernView is a desktop image application written in Rust using the Slint UI framework. It is designed to be an efficient, responsive, and elegant utility for everyday photo viewing, editing, and file organization on Windows 10 and 11.

- **Fast Startup & Cold Loading**: Optimized decoders load full-resolution images in milliseconds.
- **Minimal Footprint**: Self-contained compiled native binary without browser or web-view runtimes.
- **Refined Interface**: Dark and light themes inspired by modern minimalist design principles, with clear typography and solid surfaces.
- **Windows Integration**: Native integration with Windows Explorer context menus ("Open With") and system Default Apps.

---

## Features

### Image Viewing
- Smooth cursor-centered zooming and panning.
- One-click fit-to-window and actual-size (1:1 pixel) modes.
- Transparency checkerboard grid with alpha blending support.
- Automatic EXIF orientation normalization for portrait/landscape shots.

### Folder Browsing & Filmstrip
- Horizontal filmstrip at the bottom displaying folder thumbnails.
- Thumbnails generated lazily on background worker threads.
- Natural alphanumeric ordering for numbered image sequences.
- Left-hand sidebar for quick navigation to frequent folders and recent files.

### Image Editing
- Non-destructive adjustments for brightness, contrast, and saturation.
- 90-degree rotations and horizontal/vertical flips.
- Precision crop tool with aspect ratio locks.
- Resampling with Bilinear and Lanczos3 interpolation powered by SIMD acceleration.
- Complete undo and redo history with original revert capability.

### Image Comparison
- Side-by-side split screen to compare two images.
- Synchronized zoom and pan controls.
- One-click image swap and difference inspection.

### Batch Processing
- Multi-threaded batch processor using Rayon work-stealing queues.
- Batch convert, resize, and rename collections of images concurrently.

### Metadata & Inspector
- Collapsible sidebar showing format, resolution, aspect ratio, file size, and color depth.
- Camera EXIF data reader: camera make, lens, exposure, aperture, ISO, and capture date.

---

## Supported Formats

- **JPEG / JPG**: Full reading and quality-controlled encoding.
- **PNG**: Full 32-bit RGBA decoding and lossless export.
- **WebP**: Modern lossy and lossless image viewing and export.
- **BMP**: Standard uncompressed Windows bitmap images.
- **GIF**: Image preview for static and animated image files.
- **TIFF / TIF**: Baseline multi-channel TIFF images.
- **ICO**: Standard Windows icon files.

---

## Performance Benchmarks

Measured on Windows 64-bit (Release build, LTO enabled):

| Operation | Target | Speed |
| :--- | :--- | :--- |
| **WebP Cold Decode** | 1920 × 1080 | ~12.8 ms (77+ fps) |
| **JPEG Cold Decode** | 1920 × 1080 | ~16.8 ms (59+ fps) |
| **PNG Cold Decode** | 1920 × 1080 | ~27.3 ms (36+ fps) |
| **Lanczos3 High-Quality Resample** | 1080p → 540p | ~20.8 ms (47+ fps) |
| **Thumbnail Generation** | 1080p → 96px | ~23.8 ms (42 previews/sec) |
| **Adjustment Calculation** | 2.07M Pixels (LUT) | ~26.9 ms (77 MP/sec) |
| **Parallel Batch Run** | 10 images | ~0.35 s (28+ images/sec) |

---

## Technology Stack

- **Language**: Rust
- **GUI Framework**: Slint (v1.9)
- **Image Processing**: `image` crate (v0.25)
- **SIMD Resizing**: `fast_image_resize` (v5)
- **Metadata**: `kamadak-exif` (v0.5)
- **Concurrency**: `rayon` (v1.10)
- **Dialogs**: `rfd` (v0.15)

---

## Build Instructions

### Prerequisites
- Windows 10 or 11 (64-bit)
- Rust toolchain (stable) with Cargo
- Visual Studio C++ Build Tools (MSVC)

### Building
```powershell
# Clone the repository
git clone https://github.com/Naveen-io/ModernView.git
cd ModernView

# Run tests
cargo test

# Build optimized release binary
cargo build --release
```

The resulting binary will be created at `target\release\modernview.exe`.

---

## Windows Integration & Default App Setup

ModernView can be integrated directly into Windows Explorer on any Windows machine:

1. **One-Click Setup**:
   Simply run `setup.bat` (or `setup.ps1` in PowerShell) from the repository root:
   ```cmd
   setup.bat
   ```
   This automatically installs ModernView to `%LOCALAPPDATA%\Programs\ModernView`, creates Desktop and Start Menu shortcuts, registers the Explorer context menu, and enables Windows Default App compatibility.

2. **Via ModernView Settings**:
   Open ModernView, go to Settings (`Ctrl + ,`), open the **About** tab, and click **Register 'Open With' & Default App Handler**.

3. **Via Standalone Registration Script**:
   ```powershell
   powershell -ExecutionPolicy Bypass -File scripts\register_associations.ps1
   ```

4. **Setting ModernView as Default**:
   - Open Windows **Settings** (`Win + I`) > **Apps** > **Default apps**.
   - Select **ModernView** to associate it with all image formats (.jpg, .png, .webp, .bmp, .gif, .tiff, .ico).

---

## Keyboard Shortcuts

| Shortcut | Action |
| :--- | :--- |
| `Ctrl + O` | Open File |
| `Ctrl + Shift + O` | Open Folder |
| `Ctrl + S` | Save File |
| `Ctrl + Shift + S` | Export As |
| `Ctrl + Z` / `Ctrl + Y` | Undo / Redo |
| `Right Arrow` / `Space` | Next Image |
| `Left Arrow` / `Backspace` | Previous Image |
| `+` / `Mouse Wheel Up` | Zoom In |
| `-` / `Mouse Wheel Down` | Zoom Out |
| `0` | Fit to Window |
| `1` | 100% Actual Size |
| `E` | Toggle Edit Mode |
| `I` | Toggle Inspector Panel |
| `F` / `F11` | Toggle Fullscreen |
| `Ctrl + ,` | Settings |
| `Esc` | Close Active Panel |

---

## Project Status

ModernView is actively maintained. Version 0.1.0 delivers complete core functionality including image decoding, folder navigation, editing, batch processing, comparison, and Windows shell registration.
