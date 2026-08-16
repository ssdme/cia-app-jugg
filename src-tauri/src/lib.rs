use tauri::Manager;

const PROJECT_REPOSITORY_URL: &str = "https://github.com/cia213/cia-app";
const ABOUT_URLS: [&str; 9] = [
    PROJECT_REPOSITORY_URL,
    "https://github.com/hzwer/Practical-RIFE",
    "https://github.com/couleur-tweak-tips/smoothie-rs",
    "https://github.com/vapoursynth/vapoursynth",
    "https://github.com/FFmpeg/FFmpeg",
    "https://github.com/tauri-apps/tauri",
    "https://github.com/sveltejs/svelte",
    "https://github.com/IBM/plex",
    "https://github.com/n00mkrad/flowframes",
];

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MediaInfo {
    pub duration: f64,
    pub fps: f64,
    pub width: u32,
    pub height: u32,
    pub audio_channels: u32,
    pub audio_sample_rate: u32,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BeatResult {
    pub bpm: Option<f64>,
    pub beats: Vec<f64>,
    pub downbeats: Vec<f64>,
}

fn parse_fps(val: &str) -> f64 {
    if let Some((num_str, den_str)) = val.split_once('/') {
        if let (Ok(num), Ok(den)) = (num_str.trim().parse::<f64>(), den_str.trim().parse::<f64>()) {
            if den > 0.0 {
                return num / den;
            }
        }
    } else if let Ok(fps) = val.trim().parse::<f64>() {
        return fps;
    }
    0.0
}

fn get_binary_path(app: &tauri::AppHandle, name: &str) -> std::path::PathBuf {
    // 1. Check relative to current exe directory
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let direct = exe_dir.join(name);
            if direct.exists() {
                return direct;
            }
            let in_binaries = exe_dir.join("binaries").join(name);
            if in_binaries.exists() {
                return in_binaries;
            }
        }
    }
    // 2. Check current working dir
    let cwd = std::env::current_dir().unwrap_or_default();
    let direct_cwd = cwd.join("src-tauri").join("binaries").join(name);
    if direct_cwd.exists() {
        return direct_cwd;
    }
    let binaries_cwd = cwd.join("binaries").join(name);
    if binaries_cwd.exists() {
        return binaries_cwd;
    }
    // 3. Fallback to resource_dir
    if let Ok(res_dir) = app.path().resource_dir() {
        let in_res = res_dir.join("binaries").join(name);
        if in_res.exists() {
            return in_res;
        }
    }
    cwd.join("src-tauri").join("binaries").join(name)
}

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
        let _ = open::that(&url);
    }
    Ok(())
}

#[tauri::command]
fn pick_file(kind: String) -> Result<Option<String>, String> {
    let mut dialog = rfd::FileDialog::new();
    match kind.as_str() {
        "video" => {
            dialog = dialog.add_filter(
                "Video Files (*.mp4, *.mkv, *.webm, *.mov, *.avi)",
                &["mp4", "mkv", "webm", "mov", "avi"],
            );
        }
        "audio" => {
            dialog = dialog.add_filter(
                "Audio Files (*.mp3, *.wav, *.flac, *.m4a, *.ogg)",
                &["mp3", "wav", "flac", "m4a", "ogg"],
            );
        }
        _ => {
            dialog = dialog.add_filter(
                "Media Files",
                &[
                    "mp4", "mkv", "webm", "mov", "avi", "mp3", "wav", "flac", "m4a", "ogg",
                ],
            );
        }
    }
    let file = dialog.pick_file();
    Ok(file.map(|path| path.to_string_lossy().to_string()))
}

#[tauri::command]
fn probe_media(file_path: String) -> Result<MediaInfo, String> {
    let path_buf = std::path::PathBuf::from(&file_path);
    if !path_buf.exists() {
        return Err(format!("File not found: {file_path}"));
    }

    #[cfg(target_os = "windows")]
    use std::os::windows::process::CommandExt;

    let mut cmd = std::process::Command::new("ffprobe");
    cmd.args([
        "-v",
        "quiet",
        "-print_format",
        "json",
        "-show_format",
        "-show_streams",
        &file_path,
    ]);

    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = cmd.output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            "ffprobe not found".to_string()
        } else {
            format!("Failed to execute ffprobe: {e}")
        }
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("ffprobe probe failed: {stderr}"));
    }

    let json_str = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&json_str)
        .map_err(|e| format!("Failed to parse ffprobe json output: {e}"))?;

    let mut duration = parsed["format"]["duration"]
        .as_str()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);

    let mut fps = 0.0;
    let mut width = 0u32;
    let mut height = 0u32;
    let mut audio_channels = 0u32;
    let mut audio_sample_rate = 0u32;

    if let Some(streams) = parsed["streams"].as_array() {
        for stream in streams {
            let codec_type = stream["codec_type"].as_str().unwrap_or("");
            if codec_type == "video" && width == 0 {
                width = stream["width"].as_u64().unwrap_or(0) as u32;
                height = stream["height"].as_u64().unwrap_or(0) as u32;
                let r_frame_rate = stream["r_frame_rate"].as_str().unwrap_or("");
                let avg_frame_rate = stream["avg_frame_rate"].as_str().unwrap_or("");
                let parsed_fps = parse_fps(r_frame_rate);
                fps = if parsed_fps > 0.0 {
                    parsed_fps
                } else {
                    parse_fps(avg_frame_rate)
                };
                if duration == 0.0 {
                    duration = stream["duration"]
                        .as_str()
                        .and_then(|s| s.parse::<f64>().ok())
                        .unwrap_or(0.0);
                }
            } else if codec_type == "audio" && audio_channels == 0 {
                audio_channels = stream["channels"].as_u64().unwrap_or(0) as u32;
                audio_sample_rate = stream["sample_rate"]
                    .as_str()
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(0);
                if duration == 0.0 {
                    duration = stream["duration"]
                        .as_str()
                        .and_then(|s| s.parse::<f64>().ok())
                        .unwrap_or(0.0);
                }
            }
        }
    }

    Ok(MediaInfo {
        duration,
        fps,
        width,
        height,
        audio_channels,
        audio_sample_rate,
    })
}

#[tauri::command]
fn detect_beats(app: tauri::AppHandle, audio_path: String) -> Result<BeatResult, String> {
    let audio_file = std::path::Path::new(&audio_path);
    if !audio_file.exists() {
        return Err(format!("Audio file not found: {audio_path}"));
    }

    let bin_path = get_binary_path(&app, "beat_this.exe");
    if !bin_path.exists() {
        return Err(format!(
            "beat_this binary not found at {}",
            bin_path.display()
        ));
    }

    let onnx_path = get_binary_path(&app, "beat_this.onnx");
    if !onnx_path.exists() {
        return Err(format!(
            "beat_this.onnx model not found at {}",
            onnx_path.display()
        ));
    }

    #[cfg(target_os = "windows")]
    use std::os::windows::process::CommandExt;

    let mut cmd = std::process::Command::new(&bin_path);
    cmd.args([
        onnx_path.to_string_lossy().as_ref(),
        audio_file.to_string_lossy().as_ref(),
        "--json",
    ]);

    if let Some(parent_dir) = bin_path.parent() {
        cmd.current_dir(parent_dir);
    }

    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to run beat_this subprocess: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(format!(
            "beat_this failed (code {:?}): {} {}",
            output.status.code(),
            stderr.trim(),
            stdout.trim()
        ));
    }

    let stdout_str = String::from_utf8_lossy(&output.stdout);
    let result: BeatResult = serde_json::from_str(&stdout_str).map_err(|e| {
        format!(
            "Failed to parse beat_this JSON output: {e}. Output was: {}",
            stdout_str.trim()
        )
    })?;

    Ok(result)
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
            pick_file,
            probe_media,
            detect_beats
        ])
        .run(tauri::generate_context!())
        .expect("error while running cia app");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_fps() {
        assert_eq!(parse_fps("30/1"), 30.0);
        assert_eq!(parse_fps("60/1"), 60.0);
        assert!((parse_fps("24000/1001") - 23.976).abs() < 0.001);
    }

    #[test]
    fn test_probe_media_video() {
        let video_path = r"C:\Users\cia\Downloads\jugg video & audio tester\snaptik_7674387013243538721_v3.mp4";
        if std::path::Path::new(video_path).exists() {
            let res = probe_media(video_path.to_string()).expect("Probe should succeed");
            assert!(res.duration > 10.0);
            assert_eq!(res.width, 1080);
            assert_eq!(res.height, 1920);
            assert_eq!(res.fps, 30.0);
        }
    }

    #[test]
    fn test_probe_media_audio() {
        let audio_path = r"C:\Users\cia\Downloads\jugg video & audio tester\audio [drums].mp3";
        if std::path::Path::new(audio_path).exists() {
            let res = probe_media(audio_path.to_string()).expect("Probe should succeed");
            assert!(res.duration > 14.0);
            assert_eq!(res.audio_channels, 2);
            assert_eq!(res.audio_sample_rate, 44100);
        }
    }
}

