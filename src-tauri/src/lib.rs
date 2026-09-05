use tauri::Manager;

pub mod beat;
pub mod effects;
pub mod plan;
pub mod preview;
pub mod probe;
pub mod render;

pub use beat::*;
pub use effects::*;
pub use plan::*;
pub use preview::*;
pub use probe::*;
pub use render::*;

const PROJECT_REPOSITORY_URL: &str = "https://github.com/ssdme/cia-app-jugg";
const ABOUT_URLS: [&str; 10] = [
    PROJECT_REPOSITORY_URL,
    "https://vocalremover.org/splitter-ai",
    "https://github.com/CP-JKU/beat_this",
    "https://github.com/microsoft/onnxruntime",
    "https://github.com/pdeljanov/Symphonia",
    "https://github.com/alfg/mp4-rust",
    "https://github.com/FFmpeg/FFmpeg",
    "https://github.com/tauri-apps/tauri",
    "https://github.com/sveltejs/svelte",
    "https://github.com/IBM/plex",
];

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[tauri::command]
fn get_app_version(app: tauri::AppHandle) -> String {
    app.package_info().version.to_string()
}

#[tauri::command]
fn open_about_link(url: String) -> Result<(), String> {
    if !ABOUT_URLS.contains(&url.as_str()) {
        return Err("This About link is not supported".to_string());
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &url])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|error| format!("Failed to open browser: {error}"))?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        open::that(&url).map_err(|error| format!("Failed to open browser: {error}"))?;
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();

    #[cfg(desktop)]
    {
        builder = builder
            .plugin(tauri_plugin_updater::Builder::new().build())
            .plugin(tauri_plugin_process::init());
    }

    builder
        .setup(|app| {
            #[cfg(desktop)]
            {
                if let (Some(window), Some(icon)) =
                    (app.get_webview_window("main"), app.default_window_icon())
                {
                    let _ = window.set_icon(icon.clone());
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_app_version,
            open_about_link,
            probe::pick_file,
            probe::pick_files,
            probe::probe_media,
            probe::transcribe_audio,
            probe::detect_scenes,
            probe::get_scene_clips,
            probe::get_multi_scene_clips,
            beat::detect_beats,
            plan::generate_plan,
            plan::save_plan,
            render::cancel_render,
            render::open_target_folder,
            render::run_render_pipeline,
            render::render_text_video,
            preview::get_effect_previews,
            plan::cmd_get_style_defaults,
            probe::read_media_file_bytes
        ])
        .run(tauri::generate_context!())
        .expect("error while running cia app");
}

#[cfg(test)]
mod tests;
