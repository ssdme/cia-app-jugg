use tauri::Manager;

pub mod audio;
pub mod batch;
pub mod beat;
pub mod composition;
pub mod dumper;
pub mod effects;
pub mod export;
pub mod nle;
pub mod plan;
pub mod presets;
pub mod preview;
pub mod probe;
pub mod render;
pub mod timeline;

pub use audio::*;
pub use batch::*;
pub use beat::*;
pub use composition::*;
pub use dumper::*;
pub use effects::*;
pub use export::*;
pub use nle::*;
pub use plan::*;
pub use presets::*;
pub use preview::*;
pub use probe::*;
pub use render::*;
pub use timeline::*;

const PROJECT_REPOSITORY_URL: &str = "https://github.com/ssdme/cia-app-jugg";
const ABOUT_URLS: [&str; 9] = [
    PROJECT_REPOSITORY_URL,
    "https://github.com/CPJKU/beat_this",
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
            probe::probe_media,
            beat::detect_beats,
            plan::generate_plan,
            plan::save_plan,
            render::cancel_render,
            render::open_target_folder,
            render::run_render_pipeline,
            render::render_final_jugg,
            preview::get_effect_previews,
            plan::cmd_get_style_defaults,
            dumper::detect_scenes,
            dumper::run_dump_pipeline,
            dumper::apply_dumper_project,
            dumper::generate_remap_plan,
            dumper::run_one_click_jugg,
            composition::check_gpu_status,
            composition::segment_character,
            composition::save_composition_project,
            composition::render_composition,
            composition::render_mesh_preview,
            composition::get_default_composition_ops_cmd,
            presets::save_preset,
            presets::load_preset,
            presets::list_presets,
            presets::save_project_state,
            presets::load_project_state,
            export::queue_render_job,
            export::get_queue_status,
            export::cancel_render_job,
            timeline::get_scrub_frame,
            timeline::get_time_curve_velocities,
            nle::export_for_nle,
            batch::start_batch_job,
            batch::get_batch_status,
            batch::list_batch_jobs,
        ])
        .run(tauri::generate_context!())
        .expect("error while running cia app");
}

#[cfg(test)]
mod tests;
