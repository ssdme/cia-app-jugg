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

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct AspectRatio {
    pub w: u32,
    pub h: u32,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct PlanSegment {
    pub t0: f64,
    pub t1: f64,
    pub s0: f64,
    pub s1: f64,
    pub curve: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct ProjectPlan {
    pub schema_version: u32,
    pub style: String,
    pub fps: u32,
    pub aspect: AspectRatio,
    pub borderless: bool,
    pub bpm: f64,
    pub target_duration: f64,
    pub video_duration: f64,
    pub audio_duration: f64,
    pub loops: u32,
    pub segments: Vec<PlanSegment>,
}

fn get_binary_path(app: &tauri::AppHandle, name: &str) -> std::path::PathBuf {
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
    let cwd = std::env::current_dir().unwrap_or_default();
    let direct_cwd = cwd.join("src-tauri").join("binaries").join(name);
    if direct_cwd.exists() {
        return direct_cwd;
    }
    let binaries_cwd = cwd.join("binaries").join(name);
    if binaries_cwd.exists() {
        return binaries_cwd;
    }
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
    let path = std::path::Path::new(&file_path);
    if !path.exists() {
        return Err(format!("File not found: {file_path}"));
    }

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    // 1. MP4 / MOV containers (via mp4 crate)
    if ext == "mp4" || ext == "mov" || ext == "m4a" {
        if let Ok(file) = std::fs::File::open(&file_path) {
            let file_size = file.metadata().map(|m| m.len()).unwrap_or(0);
            let reader = std::io::BufReader::new(file);
            if let Ok(mp4_reader) = mp4::Mp4Reader::read_header(reader, file_size) {
                let mut duration = mp4_reader.duration().as_secs_f64();
                let mut width = 0u32;
                let mut height = 0u32;
                let mut fps = 0.0;
                let mut audio_channels = 0u32;
                let mut audio_sample_rate = 0u32;

                for track in mp4_reader.tracks().values() {
                    match track.track_type() {
                        Ok(mp4::TrackType::Video) if width == 0 => {
                            width = track.width() as u32;
                            height = track.height() as u32;
                            let sample_count = track.sample_count();
                            let track_dur = track.duration().as_secs_f64();
                            if track_dur > 0.0 && sample_count > 0 {
                                fps = (sample_count as f64) / track_dur;
                                if (fps - fps.round()).abs() < 0.05 {
                                    fps = fps.round();
                                }
                            }
                            if duration == 0.0 {
                                duration = track_dur;
                            }
                        }
                        Ok(mp4::TrackType::Audio) if audio_channels == 0 => {
                            if let Some(mp4a) = track.trak.mdia.minf.stbl.stsd.mp4a.as_ref() {
                                audio_channels = mp4a.channelcount as u32;
                            }
                            if audio_sample_rate == 0 {
                                audio_sample_rate = track.timescale();
                            }
                        }
                        _ => {}
                    }
                }

                // Complement with symphonia for audio if needed
                if let Ok(audio_info) = probe_audio_symphonia(&file_path, &ext) {
                    if audio_channels == 0 {
                        audio_channels = audio_info.audio_channels;
                    }
                    if audio_sample_rate == 0 {
                        audio_sample_rate = audio_info.audio_sample_rate;
                    }
                    if duration == 0.0 {
                        duration = audio_info.duration;
                    }
                }

                if width > 0 || audio_channels > 0 || duration > 0.0 {
                    return Ok(MediaInfo {
                        duration,
                        fps,
                        width,
                        height,
                        audio_channels,
                        audio_sample_rate,
                    });
                }
            }
        }
    }

    // 2. Audio and other containers (via symphonia crate)
    probe_audio_symphonia(&file_path, &ext)
}

fn probe_audio_symphonia(file_path: &str, ext: &str) -> Result<MediaInfo, String> {
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let file = std::fs::File::open(file_path)
        .map_err(|e| format!("Failed to open file '{file_path}': {e}"))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if !ext.is_empty() {
        hint.with_extension(ext);
    }

    let meta_opts = MetadataOptions::default();
    let fmt_opts = FormatOptions::default();

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &fmt_opts, &meta_opts)
        .map_err(|e| format!("Unsupported format '{ext}': {e}"))?;

    let mut format = probed.format;
    let mut duration = 0.0;
    let mut audio_channels = 0u32;
    let mut audio_sample_rate = 0u32;

    for track in format.tracks() {
        let params = &track.codec_params;
        if let Some(sr) = params.sample_rate {
            if audio_sample_rate == 0 {
                audio_sample_rate = sr;
            }
        }
        if let Some(ch) = params.channels {
            if audio_channels == 0 {
                audio_channels = ch.count() as u32;
            }
        }
        if let (Some(n_frames), Some(tb)) = (params.n_frames, params.time_base) {
            let dur = tb.calc_time(n_frames);
            let d = dur.seconds as f64 + dur.frac;
            if d > duration {
                duration = d;
            }
        } else if let (Some(n_frames), Some(sr)) = (params.n_frames, params.sample_rate) {
            if sr > 0 {
                let d = (n_frames as f64) / (sr as f64);
                if d > duration {
                    duration = d;
                }
            }
        }
    }

    // Fallback: iterate packets to calculate duration if not present in header
    if duration == 0.0 {
        let default_track = format.default_track().cloned();
        if let Some(track) = default_track {
            let tb = track.codec_params.time_base;
            let sr = track.codec_params.sample_rate.unwrap_or(44100);
            let mut total_ts = 0u64;
            while let Ok(packet) = format.next_packet() {
                if packet.track_id() == track.id {
                    total_ts += packet.dur();
                }
            }
            if let Some(tb) = tb {
                let dur = tb.calc_time(total_ts);
                duration = dur.seconds as f64 + dur.frac;
            } else if sr > 0 {
                duration = (total_ts as f64) / (sr as f64);
            }
        }
    }

    Ok(MediaInfo {
        duration,
        fps: 0.0,
        width: 0,
        height: 0,
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

pub fn create_plan_internal(
    style: &str,
    fps: u32,
    beats: &[f64],
    _downbeats: &[f64],
    video_duration: f64,
    audio_duration: f64,
    aspect_w: u32,
    aspect_h: u32,
    bpm: f64,
) -> Result<ProjectPlan, String> {
    if fps == 0 {
        return Err("FPS must be greater than 0".to_string());
    }
    if video_duration <= 0.0 {
        return Err("Video duration must be greater than 0".to_string());
    }
    if audio_duration <= 0.0 {
        return Err("Audio duration must be greater than 0".to_string());
    }

    let target = audio_duration;
    let min_seg_dur = 3.0 / (fps as f64);

    // 1. Initial bounds based on style
    let mut raw_beats: Vec<f64> = beats
        .iter()
        .copied()
        .filter(|&b| b > 0.0 && b < target)
        .collect();
    raw_beats.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let subset_beats: Vec<f64> = match style.to_uppercase().as_str() {
        "SMOOTH" => raw_beats.into_iter().step_by(2).collect(),
        _ => raw_beats, // HARD, HYBRID
    };

    let mut initial_bounds = Vec::with_capacity(subset_beats.len() + 2);
    initial_bounds.push(0.0);
    initial_bounds.extend(subset_beats);
    initial_bounds.push(target);

    // 2. Merge any segment shorter than 3/fps with preceding segment
    let mut filtered_bounds = vec![0.0];
    for &b in &initial_bounds[1..] {
        let last = *filtered_bounds.last().unwrap();
        if b - last < min_seg_dur {
            if (b - target).abs() < 1e-9 {
                if filtered_bounds.len() > 1 {
                    filtered_bounds.pop();
                    filtered_bounds.push(target);
                } else {
                    filtered_bounds.push(target);
                }
            }
        } else {
            filtered_bounds.push(b);
        }
    }
    if *filtered_bounds.last().unwrap() < target {
        if target - *filtered_bounds.last().unwrap() < min_seg_dur && filtered_bounds.len() > 1 {
            filtered_bounds.pop();
        }
        filtered_bounds.push(target);
    }

    // 3. Build segments and handle video loops
    let mut segments = Vec::new();
    let mut s_cursor = 0.0;
    let mut loops = 0u32;

    for (idx, win) in filtered_bounds.windows(2).enumerate() {
        let t0 = win[0];
        let t1 = win[1];

        let (curve_name, r) = match style.to_uppercase().as_str() {
            "SMOOTH" => ("saddle".to_string(), 0.75),
            "HYBRID" => {
                if idx % 2 == 0 {
                    ("snap".to_string(), 1.0)
                } else {
                    ("saddle".to_string(), 0.75)
                }
            }
            _ => ("snap".to_string(), 1.0), // HARD default
        };

        let mut seg_t0 = t0;
        let seg_t1 = t1;

        while seg_t0 < seg_t1 - 1e-9 {
            let dt = seg_t1 - seg_t0;
            let span = r * dt;

            if s_cursor + span <= video_duration + 1e-9 {
                let s0 = s_cursor;
                let s1 = s_cursor + span;
                s_cursor += span;

                if (s_cursor - video_duration).abs() < 1e-9 {
                    s_cursor = 0.0;
                    loops += 1;
                }

                segments.push(PlanSegment {
                    t0: (seg_t0 * 10000.0).round() / 10000.0,
                    t1: (seg_t1 * 10000.0).round() / 10000.0,
                    s0: (s0 * 10000.0).round() / 10000.0,
                    s1: (s1 * 10000.0).round() / 10000.0,
                    curve: curve_name.clone(),
                });
                break;
            } else {
                // Loop wrap happens within this segment
                let t_wrap = seg_t0 + (video_duration - s_cursor) / r;
                let s0 = s_cursor;
                let s1 = video_duration;

                segments.push(PlanSegment {
                    t0: (seg_t0 * 10000.0).round() / 10000.0,
                    t1: (t_wrap * 10000.0).round() / 10000.0,
                    s0: (s0 * 10000.0).round() / 10000.0,
                    s1: (s1 * 10000.0).round() / 10000.0,
                    curve: curve_name.clone(),
                });

                loops += 1;
                s_cursor = 0.0;
                seg_t0 = t_wrap;
            }
        }
    }

    Ok(ProjectPlan {
        schema_version: 1,
        style: style.to_uppercase(),
        fps,
        aspect: AspectRatio {
            w: aspect_w,
            h: aspect_h,
        },
        borderless: true,
        bpm: (bpm * 100.0).round() / 100.0,
        target_duration: (target * 1000.0).round() / 1000.0,
        video_duration: (video_duration * 1000.0).round() / 1000.0,
        audio_duration: (audio_duration * 1000.0).round() / 1000.0,
        loops,
        segments,
    })
}

#[tauri::command]
fn generate_plan(
    style: String,
    fps: u32,
    beats: Vec<f64>,
    downbeats: Vec<f64>,
    video_duration: f64,
    audio_duration: f64,
    aspect_w: u32,
    aspect_h: u32,
    bpm: f64,
) -> Result<String, String> {
    let plan = create_plan_internal(
        &style,
        fps,
        &beats,
        &downbeats,
        video_duration,
        audio_duration,
        aspect_w,
        aspect_h,
        bpm,
    )?;
    serde_json::to_string_pretty(&plan).map_err(|e| format!("Failed to serialize plan: {e}"))
}

#[tauri::command]
fn save_plan(app: tauri::AppHandle, plan_json: String) -> Result<String, String> {
    let _: serde_json::Value =
        serde_json::from_str(&plan_json).map_err(|e| format!("Invalid plan JSON: {e}"))?;

    let data_dir = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());

    std::fs::create_dir_all(&data_dir)
        .map_err(|e| format!("Failed to create data dir {}: {e}", data_dir.display()))?;

    let plan_path = data_dir.join("project.json");
    std::fs::write(&plan_path, plan_json.as_bytes())
        .map_err(|e| format!("Failed to write plan file {}: {e}", plan_path.display()))?;

    Ok(plan_path.to_string_lossy().to_string())
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
            detect_beats,
            generate_plan,
            save_plan
        ])
        .run(tauri::generate_context!())
        .expect("error while running cia app");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_probe_media_video_pure_rust() {
        let video_path = r"C:\Users\cia\Downloads\jugg video & audio tester\snaptik_7674387013243538721_v3.mp4";
        if std::path::Path::new(video_path).exists() {
            let res = probe_media(video_path.to_string()).expect("Probe should succeed");
            println!("Video probe result: {:?}", res);
            assert!(res.duration > 10.0);
            assert_eq!(res.width, 1080);
            assert_eq!(res.height, 1920);
            assert_eq!(res.fps, 30.0);
        }
    }

    #[test]
    fn test_probe_media_audio_pure_rust() {
        let drums_path = r"C:\Users\cia\Downloads\jugg video & audio tester\audio [drums].mp3";
        if std::path::Path::new(drums_path).exists() {
            let res = probe_media(drums_path.to_string()).expect("Probe should succeed");
            println!("Drums audio probe result: {:?}", res);
            assert!(res.duration > 14.0);
            assert_eq!(res.audio_channels, 2);
            assert_eq!(res.audio_sample_rate, 44100);
        }

        let audio_path = r"C:\Users\cia\Downloads\jugg video & audio tester\curiosos.mp3";
        if std::path::Path::new(audio_path).exists() {
            let res = probe_media(audio_path.to_string()).expect("Probe should succeed");
            println!("Target audio probe result: {:?}", res);
            assert!(res.duration > 14.0);
            assert_eq!(res.audio_channels, 2);
            assert_eq!(res.audio_sample_rate, 44100);
        }
    }

    #[test]
    fn test_generate_plan_fixture_invariants() {
        let video_duration = 10.773;
        let audio_duration = 14.315;
        let beats = vec![
            0.42, 1.14, 1.88, 2.60, 3.32, 4.04, 4.76, 5.50, 6.20, 6.94, 7.64, 8.38, 9.10, 9.82,
            10.54, 11.26, 11.98, 12.70, 13.42, 14.14,
        ];
        let downbeats = vec![2.60, 5.50, 8.38, 11.26, 14.14];
        let fps = 16u32;
        let bpm = 83.33;
        let min_seg_dur = 3.0 / (fps as f64);

        for style in ["HARD", "SMOOTH", "HYBRID"] {
            let plan = create_plan_internal(
                style,
                fps,
                &beats,
                &downbeats,
                video_duration,
                audio_duration,
                1080,
                1080,
                bpm,
            )
            .expect("Plan generation must succeed");

            println!("--- Tested Style: {} (Segments: {}, Loops: {}) ---", style, plan.segments.len(), plan.loops);

            // Invariant (i): Contiguous coverage [0, target], first t0=0, last t1=target
            assert_eq!(plan.segments.first().unwrap().t0, 0.0, "First segment must start at t0=0");
            assert!((plan.segments.last().unwrap().t1 - audio_duration).abs() < 0.01, "Last segment must end at target");

            for win in plan.segments.windows(2) {
                assert!(
                    (win[0].t1 - win[1].t0).abs() < 1e-4,
                    "Coverage must be contiguous between segments: {} vs {}",
                    win[0].t1,
                    win[1].t0
                );
            }

            // Invariant (ii): 0 <= s0 < s1 <= video_duration for all segments
            for seg in &plan.segments {
                assert!(seg.s0 >= 0.0, "s0 must be >= 0 (got {})", seg.s0);
                assert!(seg.s1 > seg.s0, "s1 must be > s0 (got s0={}, s1={})", seg.s0, seg.s1);
                assert!(
                    seg.s1 <= video_duration + 1e-4,
                    "s1 must be <= video_duration (got {} vs max {})",
                    seg.s1,
                    video_duration
                );
            }

            // Invariant (iii): No segment < 3/fps
            for seg in &plan.segments {
                let dur = seg.t1 - seg.t0;
                assert!(
                    dur >= min_seg_dur - 1e-4,
                    "Segment duration must be >= 3/fps = {} (got {}) for [{}-{}]",
                    min_seg_dur,
                    dur,
                    seg.t0,
                    seg.t1
                );
            }

            // Invariant (iv): loops >= 1 on this fixture for HARD & HYBRID, and each wrap aligned on a segment boundary
            if style == "HARD" || style == "HYBRID" {
                assert!(
                    plan.loops >= 1,
                    "Fixture video_dur=10.773, audio_dur=14.315 must trigger at least 1 loop in style {} (got {})",
                    style,
                    plan.loops
                );
            }

            // Verify each wrap aligns on a segment boundary (s1 == video_duration, next s0 == 0.0)
            let mut found_wraps = 0;
            for i in 0..plan.segments.len() - 1 {
                if (plan.segments[i].s1 - video_duration).abs() < 0.01 {
                    assert_eq!(
                        plan.segments[i + 1].s0,
                        0.0,
                        "Next segment after wrap must start at s0=0.0"
                    );
                    assert_eq!(
                        plan.segments[i].t1,
                        plan.segments[i + 1].t0,
                        "Wrap cut must align on segment boundary"
                    );
                    found_wraps += 1;
                }
            }
            assert_eq!(found_wraps, plan.loops as usize);
        }
    }

    #[test]
    fn test_save_and_read_project_json() {
        let video_duration = 10.773;
        let audio_duration = 14.315;
        let beats = vec![
            0.42, 1.14, 1.88, 2.60, 3.32, 4.04, 4.76, 5.50, 6.20, 6.94, 7.64, 8.38, 9.10, 9.82,
            10.54, 11.26, 11.98, 12.70, 13.42, 14.14,
        ];
        let downbeats = vec![2.60, 5.50, 8.38, 11.26, 14.14];
        let fps = 16u32;
        let bpm = 83.33;

        let plan = create_plan_internal(
            "HARD",
            fps,
            &beats,
            &downbeats,
            video_duration,
            audio_duration,
            1080,
            1080,
            bpm,
        )
        .expect("Plan creation must succeed");

        let json_str = serde_json::to_string_pretty(&plan).expect("Serialization must succeed");
        println!("=== RAW PROJECT.JSON FIXTURE ===");
        println!("{json_str}");
        println!("===============================");

        let temp_dir = std::env::temp_dir().join("cia_app_test");
        std::fs::create_dir_all(&temp_dir).unwrap();
        let plan_file = temp_dir.join("project.json");
        std::fs::write(&plan_file, json_str.as_bytes()).unwrap();

        let read_back = std::fs::read_to_string(&plan_file).unwrap();
        let parsed: ProjectPlan = serde_json::from_str(&read_back).unwrap();
        assert_eq!(parsed.style, "HARD");
        assert_eq!(parsed.fps, 16);
        assert_eq!(parsed.loops, 1);
        assert_eq!(parsed.segments.len(), 21);
    }
}


