use std::io::{Read, Seek, SeekFrom};
use tauri::{Emitter, Manager};

use crate::effects::{
    apply_ambiance_effects_with_scratch, apply_one_framer, apply_slide_shake,
    apply_transform_stack_cropped, apply_warp_bubble, apply_wave_warp, blend_full_frames,
    compute_motion_blur_frames, compute_shake_envelope, compute_slide_shake_shift,
    compute_transform_params, compute_warp_bubble_env, compute_wave_warp_params, TransformParams,
    TransitionItem,
};
use crate::plan::{compute_effects_count, ProjectPlan};
use crate::probe::{compute_crop_to_fill, get_ffmpeg_binary};

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RenderProgressPayload {
    pub job_id: String,
    pub phase: String, // "DECODING" | "SAMPLING" | "ENCODING"
    pub percent: u32,
    pub current_frame: u32,
    pub total_frames: u32,
    pub message: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RenderStats {
    pub output_path: String,
    pub render_time_secs: f64,
    pub file_size_mb: f64,
    pub target_fps: u32,
    pub effects_count: usize,
    pub actual_fps: f64,
    pub decoded_bytes: u64,
    pub peak_cache_bytes: u64,
    pub peak_frame_ram_bytes: u64,
    pub frame_cache_hits: u64,
    pub frame_cache_misses: u64,
    pub decode_time_secs: f64,
    pub sampling_time_secs: f64,
    pub finalization_time_secs: f64,
}

pub struct CachedFrameReader<'a> {
    file: &'a mut std::fs::File,
    frame_bytes: usize,
    cache: std::collections::HashMap<u64, Vec<u8>>,
    order: std::collections::VecDeque<u64>,
    capacity: usize,
}

impl<'a> CachedFrameReader<'a> {
    pub fn new(file: &'a mut std::fs::File, frame_bytes: usize, capacity: usize) -> Self {
        Self {
            file,
            frame_bytes,
            cache: std::collections::HashMap::with_capacity(capacity),
            order: std::collections::VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn get_frame(&mut self, frame_idx: u64, buf: &mut [u8]) -> Result<(), String> {
        if let Some(cached) = self.cache.get(&frame_idx) {
            buf.copy_from_slice(cached);
            return Ok(());
        }

        let offset = frame_idx * (self.frame_bytes as u64);
        self.file
            .seek(SeekFrom::Start(offset))
            .map_err(|e| format!("Seek failed at frame {frame_idx}: {e}"))?;
        self.file
            .read_exact(buf)
            .map_err(|e| format!("Read failed at frame {frame_idx}: {e}"))?;

        if self.capacity > 0 {
            if self.cache.len() >= self.capacity {
                if let Some(oldest) = self.order.pop_front() {
                    self.cache.remove(&oldest);
                }
            }
            self.cache.insert(frame_idx, buf.to_vec());
            self.order.push_back(frame_idx);
        }
        Ok(())
    }
}

pub fn resolve_unique_output_path(
    base_dir: &std::path::Path,
    base_name: &str,
    ext: &str,
) -> std::path::PathBuf {
    let initial = base_dir.join(format!("{base_name}.{ext}"));
    if !initial.exists() {
        return initial;
    }
    let mut counter = 1u32;
    loop {
        let candidate = base_dir.join(format!("{base_name}-{counter}.{ext}"));
        if !candidate.exists() {
            return candidate;
        }
        counter += 1;
    }
}

pub fn compute_render_stats(
    plan: &ProjectPlan,
    out_mp4_path: &std::path::Path,
    render_time_secs: f64,
) -> RenderStats {
    let file_size_bytes = std::fs::metadata(out_mp4_path)
        .map(|m| m.len())
        .unwrap_or(0);
    let file_size_mb = (file_size_bytes as f64) / (1024.0 * 1024.0);
    let effects_count = compute_effects_count(plan);

    RenderStats {
        output_path: out_mp4_path.to_string_lossy().to_string(),
        render_time_secs: (render_time_secs * 100.0).round() / 100.0,
        file_size_mb: (file_size_mb * 100.0).round() / 100.0,
        target_fps: plan.fps,
        effects_count,
        actual_fps: plan.target_duration * plan.fps as f64 / render_time_secs.max(0.001),
        decoded_bytes: 0,
        peak_cache_bytes: 0,
        peak_frame_ram_bytes: 0,
        frame_cache_hits: 0,
        frame_cache_misses: 0,
        decode_time_secs: 0.0,
        sampling_time_secs: 0.0,
        finalization_time_secs: 0.0,
    }
}

#[tauri::command]
pub fn cancel_render(job_id: String) -> Result<(), String> {
    crate::analysis::cancel(&job_id)?;
    crate::jobs::cancel(&job_id)
}

#[tauri::command]
pub fn open_output_file(path: String) -> Result<(), String> {
    let path = std::fs::canonicalize(&path)
        .map_err(|error| format!("Output file is unavailable: {error}"))?;
    if !path.is_file() {
        return Err("Output path is not a file".into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        #[link(name = "shell32")]
        unsafe extern "system" {
            fn ShellExecuteW(
                window: *mut std::ffi::c_void,
                operation: *const u16,
                file: *const u16,
                parameters: *const u16,
                directory: *const u16,
                show: i32,
            ) -> isize;
        }
        // ShellExecute receives a file path directly, without a shell command string.
        let mut file: Vec<_> = path.as_os_str().encode_wide().collect();
        // Canonical Windows paths use verbatim prefixes; shell associations expect normal paths.
        for (prefix, replacement) in [(r"\\?\UNC\", r"\\"), (r"\\?\", "")] {
            let prefix: Vec<_> = prefix.encode_utf16().collect();
            if file.starts_with(&prefix) {
                file.splice(..prefix.len(), replacement.encode_utf16());
                break;
            }
        }
        file.push(0);
        let operation: Vec<_> = "open".encode_utf16().chain(Some(0)).collect();
        let result = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                operation.as_ptr(),
                file.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1,
            )
        };
        if result <= 32 {
            return Err(format!(
                "Cannot open output in its associated application (Windows error {result})"
            ));
        }
    }
    #[cfg(not(windows))]
    {
        return Err("Opening output files is currently supported on Windows".into());
    }
    #[cfg(windows)]
    Ok(())
}

#[tauri::command]
pub fn open_target_folder(path: String) -> Result<(), String> {
    let p = std::path::Path::new(&path);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let mut cmd = std::process::Command::new("explorer.exe");
        if p.exists() {
            cmd.arg(format!("/select,{}", p.display()));
        } else if let Some(parent) = p.parent() {
            cmd.arg(format!("{}", parent.display()));
        } else {
            cmd.arg(r"C:\Users\cia\Downloads");
        }
        cmd.creation_flags(CREATE_NO_WINDOW);
        cmd.spawn()
            .map_err(|e| format!("Failed to open folder: {e}"))?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        let parent = p.parent().unwrap_or(p);
        let _ = open::that(parent);
    }
    Ok(())
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TextRenderJobSpec {
    pub audio_path: Option<String>,
    pub style_method: String,
    pub text_color: String,
    pub glow_enabled: bool,
    pub glow_intensity: f64,
    pub rapid_word_enabled: bool,
    pub blocks: serde_json::Value,
    pub output_path: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TextRenderProgress {
    pub percent: u32,
    pub current_frame: u32,
    pub total_frames: u32,
    pub message: String,
}

#[tauri::command]
pub async fn render_text_video(
    app: tauri::AppHandle,
    spec: TextRenderJobSpec,
) -> Result<String, String> {
    use std::io::{BufRead, BufReader};
    use std::process::{Command, Stdio};

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let out_video = spec
        .output_path
        .clone()
        .unwrap_or_else(|| format!(r"C:\Users\cia\Downloads\jugg_text_{}.mp4", ts));

    let ffmpeg_bin = crate::probe::get_ffmpeg_binary(&app)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "ffmpeg".to_string());

    let job_json = serde_json::json!({
        "audioPath": spec.audio_path.unwrap_or_default(),
        "outputPath": out_video,
        "styleMethod": spec.style_method,
        "textColor": spec.text_color,
        "glowEnabled": spec.glow_enabled,
        "glowIntensity": spec.glow_intensity,
        "rapidWordEnabled": spec.rapid_word_enabled,
        "blocks": spec.blocks,
        "ffmpegBin": ffmpeg_bin,
    });

    let temp_dir = std::env::temp_dir();
    let job_path = temp_dir.join(format!("text_job_{}.json", ts));
    std::fs::write(
        &job_path,
        serde_json::to_string_pretty(&job_json).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("Failed to write job file: {e}"))?;

    let script_path = crate::probe::find_script_path("text_renderer.py");
    let python_bin = crate::probe::find_python_binary();

    let mut cmd = Command::new(&python_bin);
    cmd.arg(&script_path)
        .arg(&job_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to launch python renderer ({:?}): {e}", python_bin))?;

    let stderr_handle = child.stderr.take().map(|stderr| {
        std::thread::spawn(move || {
            let reader = BufReader::new(stderr);
            let mut err_lines = Vec::new();
            for line in reader.lines().map_while(Result::ok) {
                err_lines.push(line);
            }
            err_lines.join("\n")
        })
    });

    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            if line.starts_with("PROGRESS:") {
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() >= 4 {
                    let pct: u32 = parts[1].parse().unwrap_or(0);
                    let cur: u32 = parts[2].parse().unwrap_or(0);
                    let tot: u32 = parts[3].parse().unwrap_or(0);
                    let _ = app.emit(
                        "render-text-progress",
                        TextRenderProgress {
                            percent: pct,
                            current_frame: cur,
                            total_frames: tot,
                            message: format!("Rendering frame {}/{}", cur, tot),
                        },
                    );
                }
            }
        }
    }

    let status = child
        .wait()
        .map_err(|e| format!("Python render process failed: {e}"))?;
    let _ = std::fs::remove_file(&job_path);
    let stderr_msg = stderr_handle
        .and_then(|h| h.join().ok())
        .unwrap_or_default();

    if !status.success() {
        return Err(format!(
            "Python rendering failed (code {:?}): {}",
            status.code(),
            stderr_msg
        ));
    }

    let _ = app.emit(
        "render-text-progress",
        TextRenderProgress {
            percent: 100,
            current_frame: 100,
            total_frames: 100,
            message: "Render finished successfully".to_string(),
        },
    );

    Ok(out_video)
}

pub struct RenderEnvironment {
    pub ffmpeg: std::path::PathBuf,
    pub ffprobe: std::path::PathBuf,
    pub cache_root: std::path::PathBuf,
    pub output_dir: std::path::PathBuf,
}

#[tauri::command]
pub async fn run_render_pipeline(
    app: tauri::AppHandle,
    plan_json: String,
    scene_path: String,
    audio_path: String,
    echo_trail: bool,
    scene_paths: Option<Vec<String>>,
    job_id: String,
) -> Result<RenderStats, String> {
    let job = crate::jobs::RenderJob::begin(job_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let environment = RenderEnvironment {
            ffmpeg: get_ffmpeg_binary(&app)?,
            ffprobe: crate::probe::get_ffprobe_binary(&app)?,
            cache_root: app.path().app_cache_dir().map_err(|e| e.to_string())?,
            output_dir: app
                .path()
                .app_data_dir()
                .map_err(|e| e.to_string())?
                .join("output"),
        };
        let mut last_phase = String::new();
        let mut last_emit = std::time::Instant::now();
        let mut emit = |payload: RenderProgressPayload| {
            if payload.phase != last_phase
                || payload.percent == 100
                || last_emit.elapsed().as_millis() >= 100
            {
                last_phase.clone_from(&payload.phase);
                last_emit = std::time::Instant::now();
                let _ = app.emit("render-progress", payload);
            }
        };
        render_pipeline_core(
            &environment,
            &job,
            &plan_json,
            &scene_path,
            &audio_path,
            echo_trail,
            scene_paths,
            &mut emit,
        )
    })
    .await
    .map_err(|e| format!("Render worker failed: {e}"))?
}

#[allow(clippy::too_many_arguments)]
pub fn render_pipeline_core(
    environment: &RenderEnvironment,
    job: &crate::jobs::RenderJob,
    plan_json: &str,
    scene_path: &str,
    audio_path: &str,
    echo_trail: bool,
    scene_paths: Option<Vec<String>>,
    emit: &mut dyn FnMut(RenderProgressPayload),
) -> Result<RenderStats, String> {
    job.check()?;
    let start_time = std::time::Instant::now();

    let mut plan: ProjectPlan =
        serde_json::from_str(plan_json).map_err(|e| format!("Invalid plan JSON: {e}"))?;

    // Apply runtime echo/trail toggle
    if let Some(amb) = plan.ambiance.as_mut() {
        amb.echo_trail.enabled = echo_trail;
    }

    crate::plan::validate_plan(&plan)?;
    let ffmpeg_bin = &environment.ffmpeg;
    let all_scene_paths = scene_paths
        .filter(|paths| !paths.is_empty())
        .unwrap_or_else(|| vec![scene_path.to_string()]);
    if let Some(planning) = &plan.planning {
        for stamp in &planning.source_fingerprints {
            let path = all_scene_paths
                .get(stamp.source_id)
                .ok_or("Analyzed source is missing")?;
            if crate::analysis::fingerprint(std::path::Path::new(path))? != stamp.fingerprint {
                return Err("Source changed after motion analysis; generate a new plan".into());
            }
        }
    }
    let mut metadata = Vec::new();
    for path in &all_scene_paths {
        job.check()?;
        let info = crate::probe::probe_media_internal(path, Some(&environment.ffprobe))?;
        if info.width == 0 || info.height == 0 || !info.duration.is_finite() || info.duration <= 0.0
        {
            return Err(format!("Invalid video source: {path}"));
        }
        metadata.push(info);
    }
    let durations: Vec<f64> = metadata.iter().map(|info| info.duration).collect();
    let ranges = match &plan.source_ranges {
        Some(ranges) => ranges.clone(),
        None => crate::sources::migrate_intervals(
            &plan
                .source_intervals
                .clone()
                .unwrap_or_else(|| vec![(0.0, plan.video_duration)]),
            &durations,
        )?,
    };
    crate::sources::validate_ranges(&ranges, &durations)?;
    let selected_duration: f64 = ranges.iter().map(|range| range.duration).sum();
    if (selected_duration - plan.video_duration).abs() > 0.1 {
        return Err("The selected sources do not match the plan timeline".into());
    }
    let scene_info = &metadata[ranges[0].source_id];
    let mut src_w = scene_info.width;
    let mut src_h = scene_info.height;
    let src_fps = if scene_info.fps.is_finite() && scene_info.fps > 0.0 {
        scene_info.fps.min(120.0)
    } else {
        30.0
    };
    // Preserve the existing canvas-based normalization, independent of cache budgets.
    let max_target_dim = plan.aspect.w.max(plan.aspect.h).max(1080);
    if src_w > max_target_dim || src_h > max_target_dim {
        let scale = max_target_dim as f64 / src_w.max(src_h) as f64;
        src_w = ((src_w as f64 * scale) as u32 & !1).max(2);
        src_h = ((src_h as f64 * scale) as u32 & !1).max(2);
    }
    let frame_bytes = (src_w as usize)
        .checked_mul(src_h as usize)
        .and_then(|n| n.checked_mul(3))
        .ok_or("Frame size overflow")?;
    let job_directory = crate::jobs::JobDirectory::create(&environment.cache_root, &job.id)?;
    let mut frame_reader = crate::frame_cache::FrameCache::new(
        environment.ffmpeg.clone(),
        all_scene_paths,
        ranges,
        job_directory.0.clone(),
        src_w,
        src_h,
        src_fps,
        job.cancelled.clone(),
        crate::frame_cache::DEFAULT_RAM_BUDGET,
        crate::frame_cache::DEFAULT_DISK_BUDGET,
    )?;
    let total_source_frames = frame_reader.total_frames() as usize;
    emit(RenderProgressPayload {
        job_id: job.id.clone(),
        phase: "DECODING".into(),
        percent: 0,
        current_frame: 0,
        total_frames: total_source_frames as u32,
        message: "Preparing selected video ranges...".into(),
    });
    // Warm the first block; other blocks are decoded only when sampling requests them.
    frame_reader.get_frame(0, &mut vec![0u8; frame_bytes])?;
    emit(RenderProgressPayload {
        job_id: job.id.clone(),
        phase: "DECODING".into(),
        percent: 100,
        current_frame: 1,
        total_frames: total_source_frames as u32,
        message: "Selected sources ready".into(),
    });
    use std::io::Write;
    // Phase 2 & 3: SAMPLING + ENCODE
    let crop = compute_crop_to_fill(src_w, src_h, plan.aspect.w, plan.aspect.h);
    let output_fps = plan.fps as f64;
    let total_output_frames = (plan.target_duration * output_fps).round() as usize;

    let out_dir = &environment.output_dir;
    std::fs::create_dir_all(out_dir)
        .map_err(|e| format!("Failed to create output directory: {e}"))?;

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let export = &plan.export;
    let codec_lib = match export.codec.to_uppercase().as_str() {
        "H265" | "HEVC" | "H.265" => "libx265",
        "VP9" => "libvpx-vp9",
        _ => "libx264",
    };
    let bitrate_str = format!("{}M", export.bitrate_mbps);
    let file_ext = match export.format.to_uppercase().as_str() {
        "MKV" => "mkv",
        "WEBM" => "webm",
        _ => "mp4",
    };
    let is_webm = export.format.to_uppercase() == "WEBM";
    let audio_codec = if is_webm { "libopus" } else { "aac" };

    let slow_codec = matches!(codec_lib, "libx265" | "libvpx-vp9");
    if slow_codec {
        println!(
            "[T19] WARN: {} encoding is slower than H264 — render may take 2-3x longer",
            codec_lib
        );
    }

    crate::frame_cache::ensure_disk_space(
        out_dir,
        (plan.target_duration * plan.export.bitrate_mbps as f64 * 125000.0) as u64
            + 16 * 1024 * 1024,
    )?;
    let temporary_output =
        crate::process::TemporaryOutput(out_dir.join(format!(".render-{}.{file_ext}", job.id)));
    let out_mp4_path = &temporary_output.0;

    let mut encode_cmd = std::process::Command::new(ffmpeg_bin);
    encode_cmd.args([
        "-y",
        "-f",
        "rawvideo",
        "-pix_fmt",
        "rgb24",
        "-s",
        &format!("{}x{}", crop.width, crop.height),
        "-r",
        &format!("{}", plan.fps),
        "-i",
        "-",
        "-i",
        audio_path,
        "-t",
        &format!("{:.3}", plan.target_duration),
        "-vf",
        &format!("scale={}:{}", crop.out_w, crop.out_h),
        "-c:v",
        codec_lib,
        "-b:v",
        &bitrate_str,
        "-pix_fmt",
        "yuv420p",
        "-c:a",
        audio_codec,
        "-shortest",
        &out_mp4_path.to_string_lossy(),
    ]);
    encode_cmd.stdin(std::process::Stdio::piped());
    encode_cmd.stdout(std::process::Stdio::null());
    encode_cmd.stderr(std::process::Stdio::piped());

    let mut encode_child =
        crate::process::ManagedChild::spawn(&mut encode_cmd, job.cancelled.clone())?;

    let mut encode_stdin = encode_child
        .stdin
        .take()
        .ok_or_else(|| "Failed to open encoder stdin".to_string())?;

    let has_dissolve = plan.segments.iter().any(|segment| {
        segment
            .effects
            .dissolve_shake
            .as_ref()
            .is_some_and(|effect| effect.pct > 0.0)
    });
    let mut sampled_full_frame = vec![0u8; frame_bytes];
    let mut one_framer_buf = vec![
        0u8;
        if plan.one_framers.is_empty() {
            0
        } else {
            frame_bytes
        }
    ];
    let mut transition_buf = vec![
        0u8;
        if plan.transitions.is_empty() {
            0
        } else {
            frame_bytes
        }
    ];
    let mut ambiance_buf = vec![
        0u8;
        if plan.ambiance.is_some() {
            frame_bytes
        } else {
            0
        }
    ];
    let cropped_frame_bytes = (crop.width * crop.height * 3) as usize;
    let mut cropped_buf = vec![0u8; cropped_frame_bytes];
    let mut dissolve_buf = vec![0u8; if has_dissolve { cropped_frame_bytes } else { 0 }];
    let mut ghost_full = vec![0u8; if has_dissolve { frame_bytes } else { 0 }];
    let has_time_slice = plan
        .creative_events
        .iter()
        .any(|event| event.kind == crate::creative::CreativeKind::TimeSlice);
    let has_print_effects = plan
        .creative_events
        .iter()
        .any(|event| event.kind != crate::creative::CreativeKind::TimeSlice);
    let mut slice_source = vec![0u8; if has_time_slice { frame_bytes } else { 0 }];
    let mut slice_output = vec![0u8; if has_time_slice { frame_bytes } else { 0 }];
    let mut creative_output = vec![
        0u8;
        if has_print_effects {
            cropped_frame_bytes
        } else {
            0
        }
    ];
    let mut creative_index = 0;
    let sampling_start = std::time::Instant::now();
    let initial_decode_time = frame_reader.decode_time;
    let mut blend_frames_storage =
        vec![vec![0u8; frame_bytes]; if plan.motion_blur { 4 } else { 0 }];
    let echo_k = plan
        .ambiance
        .as_ref()
        .filter(|amb| amb.echo_trail.enabled)
        .map(|amb| (amb.echo_trail.k as usize).clamp(1, 4))
        .unwrap_or(0)
        .min(4);
    let mut echo_ring = vec![vec![128u8; frame_bytes]; echo_k];
    let mut echo_head = 0;
    let mut ambiance_scratch = crate::effects::AmbianceScratch::default();
    let amb = plan.ambiance.as_ref();
    let vig_strength = amb.map(|a| a.vignette.strength).unwrap_or(0.3);
    let scanline_opacity = amb.map(|a| a.scanlines.opacity).unwrap_or(0.15);
    let rx_full = (src_w as f64) / 2.0;
    let ry_full = (src_h as f64) / 2.0;
    let r_max_full = (rx_full * rx_full + ry_full * ry_full).sqrt();
    let mut vignette_lut = vec![
        0u8;
        if amb.is_some() {
            (src_w * src_h) as usize
        } else {
            0
        }
    ];
    for vy in 0..if amb.is_some() { src_h as usize } else { 0 } {
        let dy = (vy as f64) - ry_full;
        for vx in 0..(src_w as usize) {
            let dx = (vx as f64) - rx_full;
            let r = (dx * dx + dy * dy).sqrt();
            let factor = 1.0 - vig_strength * (r / r_max_full).powi(2);
            vignette_lut[vy * (src_w as usize) + vx] = (factor.clamp(0.0, 1.0) * 255.0) as u8;
        }
    }

    for i in 0..total_output_frames {
        job.check()?;

        let t = (i as f64) / output_fps;
        while creative_index < plan.creative_events.len()
            && plan.creative_events[creative_index].t1 <= t
        {
            creative_index += 1;
        }
        let mut creative = [None; crate::creative::CreativeKind::COUNT];
        for event in plan.creative_events[creative_index..]
            .iter()
            .take_while(|event| event.t0 <= t)
        {
            if t < event.t1 && t > event.t0 && event.strength > 0.0 {
                creative[event.kind.slot()] = Some(event);
            }
        }
        let creative_gain =
            1.0 / (1.0 + 0.25 * creative.iter().flatten().count().saturating_sub(1) as f64);

        let seg = plan
            .segments
            .iter()
            .find(|s| t >= s.t0 && t < s.t1)
            .or_else(|| plan.segments.last())
            .ok_or("No render segment at this time")?;

        let seg_dur = (seg.t1 - seg.t0).max(1e-6);
        let t_rel = (t - seg.t0).max(0.0);
        let x = (t_rel / seg_dur).clamp(0.0, 1.0);
        let (u, u_prime) = seg.sample_curve(x);

        let speed_v = ((seg.s1 - seg.s0).abs() / seg_dur) * u_prime;
        let n_blur = compute_motion_blur_frames(speed_v, plan.motion_blur);

        let src_time = seg.s0 + (seg.s1 - seg.s0) * u;
        let (base_src_frame, range_first, range_last) =
            frame_reader.frame_at_time(if seg.s1 < seg.s0 {
                (src_time - 1e-7).max(0.0)
            } else {
                src_time
            });
        let base_src_frame = base_src_frame as i64;
        // 1. Sample Full-Frame
        if n_blur <= 1 {
            frame_reader.get_frame(base_src_frame as u64, &mut sampled_full_frame)?;
        } else {
            let mut slice_ptrs: Vec<&[u8]> = Vec::with_capacity(n_blur);
            for (k, buffer) in blend_frames_storage.iter_mut().enumerate().take(n_blur) {
                let f_idx = (base_src_frame + (k as i64))
                    .clamp(range_first as i64, range_last as i64)
                    as u64;
                frame_reader.get_frame(f_idx, buffer)?;
            }
            for buffer in blend_frames_storage.iter().take(n_blur) {
                slice_ptrs.push(buffer);
            }
            blend_full_frames(&slice_ptrs, &mut sampled_full_frame);
        }

        if let Some(event) = creative[0] {
            let (shot_first, _, _) = frame_reader.frame_at_time(seg.s0.min(seg.s1) + 1e-7);
            let (shot_last, _, _) =
                frame_reader.frame_at_time((seg.s0.max(seg.s1) - 1e-7).max(0.0));
            let first = range_first.max(shot_first) as i64;
            let last = range_last.min(shot_last) as i64;
            let delay = (src_fps * (0.08 + (event.seed % 5) as f64 * 0.015))
                .round()
                .max(1.0) as i64;
            let direction = if seg.s1 < seg.s0 { 1 } else { -1 };
            let alternate = if first <= last {
                (base_src_frame + direction * delay).clamp(first, last)
            } else {
                base_src_frame
            } as u64;
            frame_reader.get_frame(alternate, &mut slice_source)?;
            crate::creative::apply(
                event.kind,
                &sampled_full_frame,
                Some(&slice_source),
                &mut slice_output,
                src_w as usize,
                src_h as usize,
                event.phase(t),
                event.seed,
                event.strength * creative_gain,
            );
            std::mem::swap(&mut sampled_full_frame, &mut slice_output);
        }

        // 1.5. One-Framers Library effect if active
        let active_framer = plan
            .one_framers
            .iter()
            .find(|f| (t - f.t).abs() < (0.5 / output_fps) + 1e-6);

        let full_frame_ptr = if let Some(framer) = active_framer {
            apply_one_framer(
                &framer.framer_type,
                &sampled_full_frame,
                &mut one_framer_buf,
                src_w as usize,
                src_h as usize,
            );
            &one_framer_buf
        } else {
            &sampled_full_frame
        };

        // 1.75. Transitions (Warp Bubble, Wave Warp, Slide Shake) if active
        let mut active_trans: Option<(&TransitionItem, f64)> = None;
        for trans in &plan.transitions {
            let t_frames = (t - trans.t) * output_fps;
            match trans.transition_type.as_str() {
                "WARP_BUBBLE" => {
                    if t_frames.abs() <= 2.0 + 1e-4 {
                        active_trans = Some((trans, t_frames));
                        break;
                    }
                }
                "WAVE_WARP" => {
                    if (-1e-4..=6.0 + 1e-4).contains(&t_frames) {
                        active_trans = Some((trans, t_frames));
                        break;
                    }
                }
                "SLIDE_SHAKE" if t_frames.abs() <= 3.0 + 1e-4 => {
                    active_trans = Some((trans, t_frames));
                    break;
                }
                _ => {}
            }
        }

        let trans_frame_ptr = if let Some((trans, _t_frames)) = active_trans {
            match trans.transition_type.as_str() {
                "WARP_BUBBLE" => {
                    let env_a = compute_warp_bubble_env(t, trans.t, output_fps)
                        * transition_param(trans, "amplitude", 0.5)
                        / 0.5;
                    apply_warp_bubble(
                        full_frame_ptr,
                        &mut transition_buf,
                        src_w as usize,
                        src_h as usize,
                        env_a,
                        transition_param(trans, "frequency", 1.2),
                    );
                    &transition_buf
                }
                "WAVE_WARP" => {
                    let (h_t, k, v, t_fr) =
                        compute_wave_warp_params(t, trans.t, output_fps, src_h as usize);
                    apply_wave_warp(
                        full_frame_ptr,
                        &mut transition_buf,
                        src_w as usize,
                        src_h as usize,
                        h_t * transition_param(trans, "height", 280.0) / 280.0,
                        k,
                        transition_param(trans, "speed", v),
                        t_fr,
                    );
                    &transition_buf
                }
                "SLIDE_SHAKE" => {
                    let shift_x = compute_slide_shake_shift(t, trans.t, output_fps)
                        * transition_param(trans, "amplitude", 100.0)
                        / 100.0;
                    apply_slide_shake(
                        full_frame_ptr,
                        &mut transition_buf,
                        src_w as usize,
                        src_h as usize,
                        shift_x,
                    );
                    &transition_buf
                }
                _ => full_frame_ptr,
            }
        } else {
            full_frame_ptr
        };

        // 2. T11 Ambiance Effects
        let ambiance_frame_ptr = if let Some(amb) = plan.ambiance.as_ref() {
            apply_ambiance_effects_with_scratch(
                trans_frame_ptr,
                &mut ambiance_buf,
                src_w as usize,
                src_h as usize,
                amb,
                &mut echo_ring,
                &mut echo_head,
                &vignette_lut,
                scanline_opacity,
                t,
                seg,
                output_fps,
                &mut ambiance_scratch,
            );
            &ambiance_buf as &[u8]
        } else {
            trans_frame_ptr
        };

        // 3. Transform Stack + Crop
        let transform_params = compute_transform_params(&seg.effects, t_rel, seg_dur, output_fps);
        apply_transform_stack_cropped(
            ambiance_frame_ptr,
            &mut cropped_buf,
            src_w as usize,
            src_h as usize,
            crop.x,
            crop.y,
            crop.width,
            crop.height,
            transform_params,
        );

        // 3.5 T14 Dissolve shake
        if let Some(ref ds) = seg.effects.dissolve_shake {
            if ds.pct > 0.0 {
                let env = compute_shake_envelope(t_rel, seg_dur, output_fps);
                let alpha = (ds.pct / 100.0 * env).clamp(0.0, 0.5);
                if alpha > 1e-4 {
                    let ghost_frame_idx =
                        (base_src_frame + 2).clamp(range_first as i64, range_last as i64) as u64;
                    frame_reader.get_frame(ghost_frame_idx, &mut ghost_full)?;
                    {
                        apply_transform_stack_cropped(
                            &ghost_full,
                            &mut dissolve_buf,
                            src_w as usize,
                            src_h as usize,
                            crop.x,
                            crop.y,
                            crop.width,
                            crop.height,
                            TransformParams {
                                dx: 0.0,
                                dy: 0.0,
                                scale: 1.0,
                                tilt_rad: 0.0,
                                skew_x: 0.0,
                                scale_y: 1.0,
                                scale_x: 1.0,
                                barrel_k: 0.0,
                            },
                        );
                        let alpha_fp = (alpha * 256.0) as u32;
                        let inv_fp = 256 - alpha_fp;
                        for (c, g) in cropped_buf.iter_mut().zip(dissolve_buf.iter()) {
                            *c = ((*c as u32 * inv_fp + *g as u32 * alpha_fp) >> 8) as u8;
                        }
                    }
                }
            }
        }

        // Optical treatment precedes print accents; the paper aperture stays crisp on top.
        for event in [
            creative[3],
            creative[4],
            creative[1],
            creative[2],
            creative[5],
        ]
        .iter()
        .flatten()
        {
            crate::creative::apply(
                event.kind,
                &cropped_buf,
                None,
                &mut creative_output,
                crop.width as usize,
                crop.height as usize,
                event.phase(t),
                event.seed,
                event.strength * creative_gain,
            );
            std::mem::swap(&mut cropped_buf, &mut creative_output);
        }

        // 4. Pipe to encoder
        if let Err(error) = encode_stdin.write_all(&cropped_buf) {
            drop(encode_stdin);
            let (_, stderr) = encode_child.wait()?;
            job.check()?;
            return Err(format!("Failed writing frame {i}: {error}. {stderr}"));
        }

        if i % 8 == 0 || i == total_output_frames - 1 {
            let pct = ((i as f64) / (total_output_frames as f64) * 100.0) as u32;
            emit(RenderProgressPayload {
                job_id: job.id.clone(),
                phase: "SAMPLING".to_string(),
                percent: pct,
                current_frame: (i + 1) as u32,
                total_frames: total_output_frames as u32,
                message: format!(
                    "Remapping & encoding frame {}/{}",
                    i + 1,
                    total_output_frames
                ),
            });
        }
    }

    drop(encode_stdin);

    emit(RenderProgressPayload {
        job_id: job.id.clone(),
        phase: "ENCODING".to_string(),
        percent: 99,
        current_frame: total_output_frames as u32,
        total_frames: total_output_frames as u32,
        message: "Finalizing MP4 container and audio muxing...".to_string(),
    });

    let finalize_start = std::time::Instant::now();
    let encode_result = encode_child.wait_success("Video encoding");
    job.check()?;
    encode_result?;
    let out_mp4_path = crate::process::publish_output(
        out_mp4_path,
        out_dir,
        &format!("cia_jugg_{timestamp}"),
        file_ext,
    )?;

    let render_time_secs = start_time.elapsed().as_secs_f64();
    let mut stats = compute_render_stats(&plan, &out_mp4_path, render_time_secs);

    stats.decoded_bytes = frame_reader.decoded_bytes;
    stats.peak_cache_bytes = frame_reader.peak_disk_bytes;
    stats.peak_frame_ram_bytes = frame_reader.peak_ram_bytes;
    stats.frame_cache_hits = frame_reader.cache_hits;
    stats.frame_cache_misses = frame_reader.cache_misses;
    stats.decode_time_secs = frame_reader.decode_time.as_secs_f64();
    stats.finalization_time_secs = finalize_start.elapsed().as_secs_f64();
    stats.sampling_time_secs = (sampling_start.elapsed().as_secs_f64()
        - stats.finalization_time_secs
        - (frame_reader.decode_time - initial_decode_time).as_secs_f64())
    .max(0.0);
    emit(RenderProgressPayload {
        job_id: job.id.clone(),
        phase: "ENCODING".to_string(),
        percent: 100,
        current_frame: total_output_frames as u32,
        total_frames: total_output_frames as u32,
        message: "Render completed successfully".to_string(),
    });

    Ok(stats)
}

pub fn transition_param(transition: &TransitionItem, key: &str, default: f64) -> f64 {
    transition
        .params
        .get(key)
        .and_then(|value| value.as_f64())
        .filter(|n| n.is_finite())
        .unwrap_or(default)
}
