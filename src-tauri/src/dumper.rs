use std::io::Read;
use tauri::{Emitter, Manager};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

use crate::beat::{detect_beats_internal, get_binary_path_opt, BeatResult};
use crate::probe::{probe_media_internal};

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LabStats {
    pub mean: [f64; 3], // [L, a, b]
    pub std: [f64; 3],  // [L_std, a_std, b_std]
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SceneItem {
    pub start: f64,
    pub end: f64,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SceneResult {
    pub cuts: Vec<f64>,
    pub scenes: Vec<SceneItem>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DumpSegment {
    pub start: f64,
    pub end: f64,
    pub lab: LabStats,
    pub mad_mean: f64,
    pub mad_peak: f64,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DumpAnalysis {
    pub schema_version: u32,
    pub source: String,
    pub duration: f64,
    pub fps: f64,
    pub cuts: Vec<f64>,
    pub scenes: Vec<SceneItem>,
    pub beats: BeatResult,
    pub cut_beat_sync: f64,
    pub segments: Vec<DumpSegment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json_path: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DumpProgressPayload {
    pub phase: String, // "SCENES" | "BEATS" | "PROFILES"
    pub percent: u32,
    pub message: String,
}

// --- Math & Metric Calculation Functions ---

pub fn compute_luminance_mad(prev_rgb: &[u8], curr_rgb: &[u8], width: usize, height: usize) -> f64 {
    let num_pixels = width * height;
    if num_pixels == 0 || prev_rgb.len() < num_pixels * 3 || curr_rgb.len() < num_pixels * 3 {
        return 0.0;
    }

    let mut sum_diff: u64 = 0;
    for i in 0..num_pixels {
        let idx = i * 3;
        let r0 = prev_rgb[idx] as i32;
        let g0 = prev_rgb[idx + 1] as i32;
        let b0 = prev_rgb[idx + 2] as i32;
        let y0 = (299 * r0 + 587 * g0 + 114 * b0 + 500) / 1000;

        let r1 = curr_rgb[idx] as i32;
        let g1 = curr_rgb[idx + 1] as i32;
        let b1 = curr_rgb[idx + 2] as i32;
        let y1 = (299 * r1 + 587 * g1 + 114 * b1 + 500) / 1000;

        sum_diff += (y0 - y1).unsigned_abs() as u64;
    }

    sum_diff as f64 / num_pixels as f64
}

fn srgb_to_linear(c: f64) -> f64 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn f_cielab(t: f64) -> f64 {
    let delta = 6.0 / 29.0;
    let delta_cubed = delta * delta * delta;
    if t > delta_cubed {
        t.cbrt()
    } else {
        t / (3.0 * delta * delta) + 4.0 / 29.0
    }
}

pub fn rgb_to_cielab(r_u8: u8, g_u8: u8, b_u8: u8) -> [f64; 3] {
    let r = srgb_to_linear(r_u8 as f64 / 255.0);
    let g = srgb_to_linear(g_u8 as f64 / 255.0);
    let b = srgb_to_linear(b_u8 as f64 / 255.0);

    // Standard sRGB D65 matrix
    let x = 0.4124564 * r + 0.3575761 * g + 0.1804375 * b;
    let y = 0.2126729 * r + 0.7151522 * g + 0.0721750 * b;
    let z = 0.0193339 * r + 0.1191920 * g + 0.9503041 * b;

    // D65 reference white
    let x_n = 0.95047;
    let y_n = 1.00000;
    let z_n = 1.08883;

    let fx = f_cielab(x / x_n);
    let fy = f_cielab(y / y_n);
    let fz = f_cielab(z / z_n);

    let l = 116.0 * fy - 16.0;
    let a = 500.0 * (fx - fy);
    let b_star = 200.0 * (fy - fz);

    [l, a, b_star]
}

pub fn downsample_and_compute_lab_stats(frame_rgb: &[u8], src_w: usize, src_h: usize) -> LabStats {
    const GRID_SIZE: usize = 64;
    const NUM_SAMPLES: usize = GRID_SIZE * GRID_SIZE;

    let mut labs: Vec<[f64; 3]> = Vec::with_capacity(NUM_SAMPLES);
    let mut sum_l = 0.0;
    let mut sum_a = 0.0;
    let mut sum_b = 0.0;

    for gy in 0..GRID_SIZE {
        let sy0 = (gy * src_h) / GRID_SIZE;
        let sy1 = (((gy + 1) * src_h) / GRID_SIZE).max(sy0 + 1);

        for gx in 0..GRID_SIZE {
            let sx0 = (gx * src_w) / GRID_SIZE;
            let sx1 = (((gx + 1) * src_w) / GRID_SIZE).max(sx0 + 1);

            // Area average RGB in cell
            let mut cell_r = 0u64;
            let mut cell_g = 0u64;
            let mut cell_b = 0u64;
            let mut count = 0u64;

            for y in sy0..sy1 {
                for x in sx0..sx1 {
                    let idx = (y * src_w + x) * 3;
                    if idx + 2 < frame_rgb.len() {
                        cell_r += frame_rgb[idx] as u64;
                        cell_g += frame_rgb[idx + 1] as u64;
                        cell_b += frame_rgb[idx + 2] as u64;
                        count += 1;
                    }
                }
            }

            let (r, g, b) = match count.checked_div(1) {
                Some(c) if c > 0 => (
                    (cell_r / c) as u8,
                    (cell_g / c) as u8,
                    (cell_b / c) as u8,
                ),
                _ => (128, 128, 128),
            };

            let lab = rgb_to_cielab(r, g, b);
            sum_l += lab[0];
            sum_a += lab[1];
            sum_b += lab[2];
            labs.push(lab);
        }
    }

    let mean_l = sum_l / NUM_SAMPLES as f64;
    let mean_a = sum_a / NUM_SAMPLES as f64;
    let mean_b = sum_b / NUM_SAMPLES as f64;

    let mut var_l = 0.0;
    let mut var_a = 0.0;
    let mut var_b = 0.0;

    for lab in &labs {
        let dl = lab[0] - mean_l;
        let da = lab[1] - mean_a;
        let db = lab[2] - mean_b;
        var_l += dl * dl;
        var_a += da * da;
        var_b += db * db;
    }

    let std_l = (var_l / NUM_SAMPLES as f64).sqrt();
    let std_a = (var_a / NUM_SAMPLES as f64).sqrt();
    let std_b = (var_b / NUM_SAMPLES as f64).sqrt();

    LabStats {
        mean: [
            (mean_l * 100.0).round() / 100.0,
            (mean_a * 100.0).round() / 100.0,
            (mean_b * 100.0).round() / 100.0,
        ],
        std: [
            (std_l * 100.0).round() / 100.0,
            (std_a * 100.0).round() / 100.0,
            (std_b * 100.0).round() / 100.0,
        ],
    }
}

pub fn compute_cut_beat_sync(cuts: &[f64], beats: &[f64], fps: f64) -> f64 {
    if cuts.is_empty() {
        return 1.0;
    }
    if beats.is_empty() {
        return 0.0;
    }

    let threshold = 0.5 / fps.max(1.0);
    let mut synced_count = 0usize;

    for &cut in cuts {
        let mut min_diff = f64::MAX;
        for &beat in beats {
            let diff = (cut - beat).abs();
            if diff < min_diff {
                min_diff = diff;
            }
        }
        if min_diff <= threshold + 1e-4 {
            synced_count += 1;
        }
    }

    ((synced_count as f64 / cuts.len() as f64) * 10000.0).round() / 10000.0
}

// --- Scene Detection & Pipeline Implementations ---

pub fn detect_scenes_internal(
    app: Option<&tauri::AppHandle>,
    video_path: &str,
) -> Result<SceneResult, String> {
    let video_file = std::path::Path::new(video_path);
    if !video_file.exists() {
        return Err(format!("Video file not found: {video_path}"));
    }

    let bin_path = get_binary_path_opt(app, "scenedetect.exe");
    if !bin_path.exists() {
        return Err(format!(
            "scenedetect binary not found at {}",
            bin_path.display()
        ));
    }

    let mut cmd = std::process::Command::new(&bin_path);
    cmd.arg(video_path);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to execute scenedetect binary: {e}"))?;

    if !output.status.success() {
        let err_str = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Scene detection failed: {err_str}"));
    }

    let stdout_str = String::from_utf8_lossy(&output.stdout);
    let json_val: serde_json::Value = serde_json::from_str(stdout_str.trim())
        .map_err(|e| format!("Failed to parse scenedetect JSON: {e} (stdout: {stdout_str})"))?;

    let cuts: Vec<f64> = json_val
        .get("cuts")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    let scenes: Vec<SceneItem> = json_val
        .get("scenes")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    Ok(SceneResult { cuts, scenes })
}

pub fn run_dump_pipeline_internal(
    app: Option<&tauri::AppHandle>,
    video_path: &str,
) -> Result<DumpAnalysis, String> {
    let video_file = std::path::Path::new(video_path);
    if !video_file.exists() {
        return Err(format!("Video file not found: {video_path}"));
    }

    let ffmpeg_bin = "ffmpeg";

    // 0. Probe Media Info
    let media_info = probe_media_internal(video_path, None)?;
    let duration = media_info.duration;
    let original_fps = if media_info.fps > 0.0 {
        media_info.fps
    } else {
        30.0
    };
    // Profile pass: cap fps at 30
    let analysis_fps = original_fps.min(30.0);

    // Phase 1: SCENES (0..30%)
    if let Some(app_handle) = app {
        let _ = app_handle.emit(
            "dump-progress",
            DumpProgressPayload {
                phase: "SCENES".to_string(),
                percent: 5,
                message: "Running adaptive scene detection...".to_string(),
            },
        );
    }

    let scene_res = detect_scenes_internal(app, video_path)?;
    let cuts = scene_res.cuts;
    let mut scenes = scene_res.scenes;

    // Normalize scenes if none detected
    if scenes.is_empty() {
        scenes.push(SceneItem {
            start: 0.0,
            end: duration,
        });
    }

    if let Some(app_handle) = app {
        let _ = app_handle.emit(
            "dump-progress",
            DumpProgressPayload {
                phase: "SCENES".to_string(),
                percent: 30,
                message: format!("Detected {} cuts ({} scenes)", cuts.len(), scenes.len()),
            },
        );
    }

    // Phase 2: BEATS (30..60%)
    if let Some(app_handle) = app {
        let _ = app_handle.emit(
            "dump-progress",
            DumpProgressPayload {
                phase: "BEATS".to_string(),
                percent: 35,
                message: "Extracting audio and tracking beats...".to_string(),
            },
        );
    }

    let cache_dir = if let Some(app_handle) = app {
        app_handle
            .path()
            .app_cache_dir()
            .unwrap_or_else(|_| std::env::temp_dir())
    } else {
        std::env::temp_dir()
    };
    std::fs::create_dir_all(&cache_dir)
        .map_err(|e| format!("Failed to create cache dir: {e}"))?;

    let temp_audio_path = cache_dir.join(format!("dump_audio_{}.wav", std::process::id()));

    let mut extract_cmd = std::process::Command::new(ffmpeg_bin);
    extract_cmd.args([
        "-y",
        "-i",
        video_path,
        "-vn",
        "-acodec",
        "pcm_s16le",
        "-ar",
        "44100",
        "-ac",
        "2",
        &temp_audio_path.to_string_lossy(),
    ]);
    extract_cmd.stdout(std::process::Stdio::null());
    extract_cmd.stderr(std::process::Stdio::null());
    #[cfg(target_os = "windows")]
    extract_cmd.creation_flags(CREATE_NO_WINDOW);

    let extract_status = extract_cmd.status();
    let beats = if extract_status.map(|s| s.success()).unwrap_or(false)
        && temp_audio_path.exists()
    {
        let beat_res = detect_beats_internal(
            app,
            &temp_audio_path.to_string_lossy(),
        )
        .unwrap_or_else(|_| BeatResult {
            bpm: 0.0,
            beats: vec![],
            downbeats: vec![],
        });
        let _ = std::fs::remove_file(&temp_audio_path);
        beat_res
    } else {
        BeatResult {
            bpm: 0.0,
            beats: vec![],
            downbeats: vec![],
        }
    };

    let cut_beat_sync = compute_cut_beat_sync(&cuts, &beats.beats, analysis_fps);

    if let Some(app_handle) = app {
        let _ = app_handle.emit(
            "dump-progress",
            DumpProgressPayload {
                phase: "BEATS".to_string(),
                percent: 60,
                message: format!(
                    "BPM: {:.1} · Cut-Beat Sync: {:.0}%",
                    beats.bpm,
                    cut_beat_sync * 100.0
                ),
            },
        );
    }

    // Phase 3: PROFILES (60..100%)
    if let Some(app_handle) = app {
        let _ = app_handle.emit(
            "dump-progress",
            DumpProgressPayload {
                phase: "PROFILES".to_string(),
                percent: 65,
                message: "Decoding and computing luminance MAD & LAB signatures...".to_string(),
            },
        );
    }

    let decode_w: u32 = 640;
    let decode_h: u32 = ((media_info.height as f64 * (640.0 / media_info.width as f64)) as u32) & !1;
    let frame_bytes = (decode_w * decode_h * 3) as usize;

    let mut decode_cmd = std::process::Command::new(ffmpeg_bin);
    decode_cmd.args([
        "-y",
        "-i",
        video_path,
        "-r",
        &format!("{:.3}", analysis_fps),
        "-vf",
        &format!("scale={}:{}", decode_w, decode_h),
        "-f",
        "rawvideo",
        "-pix_fmt",
        "rgb24",
        "-an",
        "-",
    ]);
    decode_cmd.stdout(std::process::Stdio::piped());
    decode_cmd.stderr(std::process::Stdio::null());
    #[cfg(target_os = "windows")]
    decode_cmd.creation_flags(CREATE_NO_WINDOW);

    let mut decode_child = decode_cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn profile decoder: {e}"))?;
    let mut decode_stdout = decode_child
        .stdout
        .take()
        .ok_or_else(|| "Failed to capture decoder stdout".to_string())?;

    let mut boundaries = vec![0.0];
    for &c in &cuts {
        if c > 0.0 && c < duration {
            boundaries.push(c);
        }
    }
    boundaries.push(duration);
    boundaries.dedup();

    struct FrameMetric {
        t: f64,
        mad: f64,
        lab: LabStats,
    }

    let mut frame_metrics: Vec<FrameMetric> = Vec::new();
    let mut prev_frame: Vec<u8> = vec![0u8; frame_bytes];
    let mut curr_frame: Vec<u8> = vec![0u8; frame_bytes];
    let mut frame_idx = 0usize;

    let total_est_frames = (duration * analysis_fps).ceil() as usize;

    while decode_stdout.read_exact(&mut curr_frame).is_ok() {
        let t = frame_idx as f64 / analysis_fps;
        let mad = if frame_idx == 0 {
            0.0
        } else {
            compute_luminance_mad(&prev_frame, &curr_frame, decode_w as usize, decode_h as usize)
        };

        let lab = downsample_and_compute_lab_stats(
            &curr_frame,
            decode_w as usize,
            decode_h as usize,
        );

        frame_metrics.push(FrameMetric { t, mad, lab });
        prev_frame.copy_from_slice(&curr_frame);
        frame_idx += 1;

        if frame_idx.is_multiple_of(15) && total_est_frames > 0 {
            let pct = 60 + (((frame_idx as f64 / total_est_frames as f64) * 35.0).min(35.0) as u32);
            if let Some(app_handle) = app {
                let _ = app_handle.emit(
                    "dump-progress",
                    DumpProgressPayload {
                        phase: "PROFILES".to_string(),
                        percent: pct,
                        message: format!("Profiled frame {}/{}", frame_idx, total_est_frames),
                    },
                );
            }
        }
    }

    let _ = decode_child.wait();

    // Aggregate metrics per segment
    let mut segments: Vec<DumpSegment> = Vec::new();
    for win in boundaries.windows(2) {
        let seg_start = win[0];
        let seg_end = win[1];

        let seg_frames: Vec<&FrameMetric> = frame_metrics
            .iter()
            .filter(|f| f.t >= seg_start - 1e-4 && f.t < seg_end + 1e-4)
            .collect();

        if seg_frames.is_empty() {
            segments.push(DumpSegment {
                start: seg_start,
                end: seg_end,
                lab: LabStats {
                    mean: [0.0, 0.0, 0.0],
                    std: [0.0, 0.0, 0.0],
                },
                mad_mean: 0.0,
                mad_peak: 0.0,
            });
            continue;
        }

        let n = seg_frames.len() as f64;
        let mut sum_l_m = 0.0;
        let mut sum_a_m = 0.0;
        let mut sum_b_m = 0.0;
        let mut sum_l_s = 0.0;
        let mut sum_a_s = 0.0;
        let mut sum_b_s = 0.0;
        let mut sum_mad = 0.0;
        let mut peak_mad = 0.0f64;

        for f in &seg_frames {
            sum_l_m += f.lab.mean[0];
            sum_a_m += f.lab.mean[1];
            sum_b_m += f.lab.mean[2];
            sum_l_s += f.lab.std[0];
            sum_a_s += f.lab.std[1];
            sum_b_s += f.lab.std[2];
            sum_mad += f.mad;
            if f.mad > peak_mad {
                peak_mad = f.mad;
            }
        }

        segments.push(DumpSegment {
            start: (seg_start * 1000.0).round() / 1000.0,
            end: (seg_end * 1000.0).round() / 1000.0,
            lab: LabStats {
                mean: [
                    ((sum_l_m / n) * 100.0).round() / 100.0,
                    ((sum_a_m / n) * 100.0).round() / 100.0,
                    ((sum_b_m / n) * 100.0).round() / 100.0,
                ],
                std: [
                    ((sum_l_s / n) * 100.0).round() / 100.0,
                    ((sum_a_s / n) * 100.0).round() / 100.0,
                    ((sum_b_s / n) * 100.0).round() / 100.0,
                ],
            },
            mad_mean: ((sum_mad / n) * 100.0).round() / 100.0,
            mad_peak: (peak_mad * 100.0).round() / 100.0,
        });
    }

    let dump_dir = if let Some(app_handle) = app {
        app_handle
            .path()
            .app_data_dir()
            .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default())
            .join("dump")
    } else {
        std::env::temp_dir().join("cia_dump")
    };
    std::fs::create_dir_all(&dump_dir)
        .map_err(|e| format!("Failed to create dump dir: {e}"))?;

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let json_file_path = dump_dir.join(format!("analysis_{timestamp}.json"));

    let analysis = DumpAnalysis {
        schema_version: 1,
        source: video_path.to_string(),
        duration: (duration * 1000.0).round() / 1000.0,
        fps: (original_fps * 100.0).round() / 100.0,
        cuts,
        scenes,
        beats,
        cut_beat_sync,
        segments,
        json_path: Some(json_file_path.to_string_lossy().to_string()),
    };

    let json_content = serde_json::to_string_pretty(&analysis)
        .map_err(|e| format!("Failed to serialize analysis JSON: {e}"))?;

    std::fs::write(&json_file_path, json_content)
        .map_err(|e| format!("Failed to write analysis JSON file: {e}"))?;

    if let Some(app_handle) = app {
        let _ = app_handle.emit(
            "dump-progress",
            DumpProgressPayload {
                phase: "PROFILES".to_string(),
                percent: 100,
                message: "Analysis completed successfully".to_string(),
            },
        );
    }

    Ok(analysis)
}

// --- Tauri Commands ---

#[tauri::command]
pub fn detect_scenes(app: tauri::AppHandle, video_path: String) -> Result<SceneResult, String> {
    detect_scenes_internal(Some(&app), &video_path)
}

#[tauri::command]
pub async fn run_dump_pipeline(
    app: tauri::AppHandle,
    video_path: String,
) -> Result<DumpAnalysis, String> {
    run_dump_pipeline_internal(Some(&app), &video_path)
}
