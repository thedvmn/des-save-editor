// Hide the console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod item_list;
mod save;
mod settings;
mod tendency_tab;
mod theme;

use eframe::egui;

/// Matches the desktop entry name, which is how Wayland compositors find the icon.
const APP_ID: &str = "des-save-editor";

fn icon() -> egui::IconData {
    eframe::icon_data::from_png_bytes(include_bytes!("../assets/icons/icon-256.png"))
        .expect("bundled icon is a valid PNG")
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Demon's Souls Save Editor")
            .with_app_id(APP_ID)
            .with_icon(icon())
            .with_inner_size([1066.0, 728.0])
            .with_min_inner_size([760.0, 520.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Demon's Souls Save Editor",
        options,
        Box::new(|cc| {
            theme::apply(&cc.egui_ctx);
            let mut app = app::SaveEditorApp::new(settings::Settings::load());
            // Optional: a save folder to open on startup. Launchers sometimes
            // pass unrelated arguments, so anything that isn't a folder is ignored.
            let startup_dir = std::env::args_os()
                .nth(1)
                .map(std::path::PathBuf::from)
                .filter(|p| p.is_dir());
            if let Some(path) = startup_dir {
                app.open_folder(&path);
            }
            Ok(Box::new(app))
        }),
    )
}
