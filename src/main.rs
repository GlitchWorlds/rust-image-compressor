#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod compressor;

#[cfg(test)]
mod tests;

use app::CompressorApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([960.0, 680.0])
            .with_min_inner_size([720.0, 480.0])
            .with_title("Rust Image Compressor"),
        ..Default::default()
    };

    eframe::run_native(
        "Rust Image Compressor",
        options,
        Box::new(|cc| Ok(Box::new(CompressorApp::new(cc)))),
    )
}
