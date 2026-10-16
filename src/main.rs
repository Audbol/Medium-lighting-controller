//! Stage-Lite: a 24-fixture HSI lighting console for LED rigs that works like a
//! Jands Stage CL, with a stage visualizer and Art-Net, sACN or ENTTEC output.
//!
//! Usage: stage-lite [SHOW_FILE]
//! Without a path the show lives in the per-user data folder (see `data_dir`).

// No console window behind the GUI in Windows release builds.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod artnet;
#[cfg(feature = "audio")]
mod audio;
#[cfg(any(feature = "audio", test))]
mod beat;
mod colour;
mod demo;
mod effects;
mod engine;
#[cfg(feature = "enttec")]
mod enttec;
mod fixtures;
mod output;
mod sacn;
mod visualizer;

use std::path::PathBuf;

fn main() -> eframe::Result {
    let data = data_dir();
    let show = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| data.join("default.show.json"));
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Stage-Lite")
            .with_inner_size([1600.0, 1000.0])
            .with_min_inner_size([1280.0, 820.0]),
        ..Default::default()
    };
    eframe::run_native(
        "stage-lite",
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, show, data)))),
    )
}

/// The per-user data folder: `%APPDATA%\Stage-Lite` on Windows,
/// `~/Library/Application Support/Stage-Lite` on macOS, `$XDG_DATA_HOME/stage-lite`
/// (or `~/.local/share/stage-lite`) elsewhere. Falls back to the working directory.
fn data_dir() -> PathBuf {
    let env = |k: &str| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    let dir = if cfg!(windows) {
        env("APPDATA").map(|d| d.join("Stage-Lite"))
    } else if cfg!(target_os = "macos") {
        env("HOME").map(|h| h.join("Library/Application Support/Stage-Lite"))
    } else {
        env("XDG_DATA_HOME")
            .or_else(|| env("HOME").map(|h| h.join(".local/share")))
            .map(|d| d.join("stage-lite"))
    };
    match dir {
        Some(dir) if std::fs::create_dir_all(&dir).is_ok() => dir,
        _ => PathBuf::from("."),
    }
}
