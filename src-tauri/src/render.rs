use std::io::{Read, Seek, SeekFrom, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{Emitter, Manager};

use crate::effects::{
    apply_ambiance_effects, apply_one_framer, apply_slide_shake,
    apply_transform_stack_cropped, apply_warp_bubble, apply_wave_warp,
    blend_full_frames, compute_motion_blur_frames, compute_slide_shake_shift,
    compute_transform_params, compute_warp_bubble_env, compute_wave_warp_params,
    compute_shake_envelope, evaluate_curve, evaluate_curve_derivative,
    TransformParams, TransitionItem,
};
use crate::plan::{compute_effects_count, ProjectPlan};
use crate::probe::{compute_crop_to_fill, get_ffmpeg_binary, probe_media};

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub static RENDER_CANCEL: AtomicBool = AtomicBool::new(false);

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RenderProgressPayload {
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

pub fn resolve_unique_output_path(base_dir: &std::path::Path, base_name: &str, ext: &str) -> std::path::PathBuf {
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
    }
}

#[tauri::command]
pub fn cancel_render() -> Result<(), String> {
    RENDER_CANCEL.store(true, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub fn open_target_folder(path: String) -> Result<(), String> {
    let p = std::path::Path::new(&path);
    if !p.exists() {
        return Err(format!("Path not found: {path}"));
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let mut cmd = std::process::Command::new("explorer.exe");
        cmd.arg(format!("/select,{}", p.display()));
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

#[tauri::command]
pub async fn run_render_pipeline(
    app: tauri::AppHandle,
    plan_json: String,
    scene_path: String,
    audio_path: String,
    echo_trail: bool,
) -> Result<RenderStats, String> {
    RENDER_CANCEL.store(false, Ordering::SeqCst);
    let start_time = std::time::Instant::now();

    let mut plan: ProjectPlan = serde_json::from_str(&plan_json)
        .map_err(|e| format!("Invalid plan JSON: {e}"))?;

    // Apply runtime echo/trail toggle
    if let Some(amb) = plan.ambiance.as_mut() {
        amb.echo_trail.enabled = echo_trail;
    }

    let ffmpeg_bin = get_ffmpeg_binary(&app)?;

    // 1. Probe scene info (with ffprobe fallback)
    let scene_info = probe_media(app.clone(), scene_path.clone())?;
    if scene_info.width == 0 || scene_info.height == 0 {
        return Err("Invalid scene dimensions for rendering".to_string());
    }

    let mut src_w = scene_info.width;
    let mut src_h = scene_info.height;
    let src_fps = if scene_info.fps > 0.0 { scene_info.fps } else { 30.0 };

    let estimated_frames = (scene_info.duration * src_fps).ceil() as u64;
    const MAX_CACHE_BYTES: u64 = 4 * 1024 * 1024 * 1024;

    // Adaptive decode resolution: scale down if cache would exceed 4 GB
    let mut scale_filter: Option<String> = None;
    {
        let raw_frame_bytes = (src_w as u64) * (src_h as u64) * 3;
        let raw_cache = estimated_frames * raw_frame_bytes;
        if raw_cache > MAX_CACHE_BYTES {
            let s = ((MAX_CACHE_BYTES as f64) / (raw_cache as f64)).sqrt();
            let long_side = src_w.max(src_h) as f64;
            let floor_scale = 1080.0 / long_side;
            let s_clamped = s.max(floor_scale).min(1.0);

            let new_w = ((src_w as f64 * s_clamped) as u32) & !1;
            let new_h = ((src_h as f64 * s_clamped) as u32) & !1;

            let floor_frame_bytes = (new_w as u64) * (new_h as u64) * 3;
            let floor_cache = estimated_frames * floor_frame_bytes;
            if floor_cache > MAX_CACHE_BYTES {
                let max_frames_at_floor = MAX_CACHE_BYTES / floor_frame_bytes;
                let max_seconds = (max_frames_at_floor as f64) / src_fps;
                return Err(format!(
                    "Source too long for beta renderer (max ~{:.0}s at this resolution)",
                    max_seconds
                ));
            }

            src_w = new_w;
            src_h = new_h;
            scale_filter = Some(format!("scale={}:{}", new_w, new_h));
        }
    }

    let frame_bytes = (src_w * src_h * 3) as usize;
    let estimated_cache_size = estimated_frames * (frame_bytes as u64);

    let cache_dir = app
        .path()
        .app_cache_dir()
        .unwrap_or_else(|_| std::env::temp_dir());
    std::fs::create_dir_all(&cache_dir)
        .map_err(|e| format!("Failed to create cache dir: {e}"))?;
    let raw_cache_file = cache_dir.join(format!("frames_{}.raw", std::process::id()));

    // Phase 1: DECODE
    let _ = app.emit("render-progress", RenderProgressPayload {
        phase: "DECODING".to_string(),
        percent: 0,
        current_frame: 0,
        total_frames: estimated_frames as u32,
        message: "Decoding source video frames into memory cache...".to_string(),
    });

    #[cfg(target_os = "windows")]
    use std::os::windows::process::CommandExt;

    let mut decode_child = {
        let mut cmd = std::process::Command::new(&ffmpeg_bin);
        cmd.args(["-y", "-i", &scene_path]);
        if let Some(ref vf) = scale_filter {
            cmd.args(["-vf", vf]);
        }
        cmd.args([
            "-f", "rawvideo",
            "-pix_fmt", "rgb24",
            "-an",
            &raw_cache_file.to_string_lossy(),
        ]);
        cmd.stdout(std::process::Stdio::null());
        cmd.stderr(std::process::Stdio::piped());
        #[cfg(target_os = "windows")]
        cmd.creation_flags(CREATE_NO_WINDOW);
        cmd.spawn()
            .map_err(|e| format!("Failed to spawn ffmpeg decoder: {e}"))?
    };

    while let Ok(None) = decode_child.try_wait() {
        if RENDER_CANCEL.load(Ordering::SeqCst) {
            let _ = decode_child.kill();
            let _ = std::fs::remove_file(&raw_cache_file);
            return Err("Render cancelled by user".to_string());
        }
        if let Ok(meta) = std::fs::metadata(&raw_cache_file) {
            let decoded_bytes = meta.len();
            let pct = ((decoded_bytes as f64) / (estimated_cache_size as f64) * 100.0).min(99.0) as u32;
            let current_f = (decoded_bytes / (frame_bytes as u64)) as u32;
            let _ = app.emit("render-progress", RenderProgressPayload {
                phase: "DECODING".to_string(),
                percent: pct,
                current_frame: current_f,
                total_frames: estimated_frames as u32,
                message: format!("Decoded frame {}/{}", current_f, estimated_frames),
            });
        }
        std::thread::sleep(std::time::Duration::from_millis(150));
    }

    let decode_status = decode_child.wait()
        .map_err(|e| format!("Failed to wait for decoder: {e}"))?;
    if !decode_status.success() {
        let _ = std::fs::remove_file(&raw_cache_file);
        return Err("FFmpeg decoding failed".to_string());
    }

    let total_cached_bytes = std::fs::metadata(&raw_cache_file)
        .map_err(|e| format!("Failed to check decoded cache file: {e}"))?
        .len();
    let total_source_frames = (total_cached_bytes / (frame_bytes as u64)) as usize;
    if total_source_frames == 0 {
        let _ = std::fs::remove_file(&raw_cache_file);
        return Err("No video frames were decoded".to_string());
    }

    let _ = app.emit("render-progress", RenderProgressPayload {
        phase: "DECODING".to_string(),
        percent: 100,
        current_frame: total_source_frames as u32,
        total_frames: total_source_frames as u32,
        message: format!("Decoded {} frames successfully", total_source_frames),
    });

    // Phase 2 & 3: SAMPLING + ENCODE
    let crop = compute_crop_to_fill(src_w, src_h, plan.aspect.w, plan.aspect.h);
    let output_fps = plan.fps as f64;
    let total_output_frames = (plan.target_duration * output_fps).round() as usize;

    let out_dir = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default())
        .join("output");
    std::fs::create_dir_all(&out_dir)
        .map_err(|e| format!("Failed to create output directory: {e}"))?;

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let export = &plan.export;
    let codec_lib = match export.codec.to_uppercase().as_str() {
        "H265" | "HEVC" | "H.265" => "libx265",
        "VP9"                     => "libvpx-vp9",
        _                         => "libx264",
    };
    let bitrate_str = format!("{}M", export.bitrate_mbps);
    let file_ext = match export.format.to_uppercase().as_str() {
        "MKV"  => "mkv",
        "WEBM" => "webm",
        _      => "mp4",
    };
    let is_webm = export.format.to_uppercase() == "WEBM";
    let audio_codec = if is_webm { "libopus" } else { "aac" };

    let slow_codec = matches!(codec_lib, "libx265" | "libvpx-vp9");
    if slow_codec {
        println!("[T19] WARN: {} encoding is slower than H264 — render may take 2-3x longer", codec_lib);
    }

    let out_mp4_path = resolve_unique_output_path(&out_dir, &format!("cia_jugg_{timestamp}"), file_ext);

    let mut encode_cmd = std::process::Command::new(&ffmpeg_bin);
    encode_cmd.args([
        "-y",
        "-f", "rawvideo",
        "-pix_fmt", "rgb24",
        "-s", &format!("{}x{}", crop.width, crop.height),
        "-r", &format!("{}", plan.fps),
        "-i", "-",
        "-i", &audio_path,
        "-t", &format!("{:.3}", plan.target_duration),
        "-vf", &format!("scale={}:{}", crop.out_w, crop.out_h),
        "-c:v", codec_lib,
        "-b:v", &bitrate_str,
        "-pix_fmt", "yuv420p",
        "-c:a", audio_codec,
        "-shortest",
        &out_mp4_path.to_string_lossy(),
    ]);
    encode_cmd.stdin(std::process::Stdio::piped());
    encode_cmd.stdout(std::process::Stdio::null());
    encode_cmd.stderr(std::process::Stdio::piped());

    #[cfg(target_os = "windows")]
    encode_cmd.creation_flags(CREATE_NO_WINDOW);

    let mut encode_child = encode_cmd.spawn()
        .map_err(|e| format!("Failed to spawn encoder: {e}"))?;

    let mut encode_stdin = encode_child.stdin.take()
        .ok_or_else(|| "Failed to open encoder stdin".to_string())?;

    let mut raw_file = std::fs::File::open(&raw_cache_file)
        .map_err(|e| format!("Failed to open cache file: {e}"))?;
    let mut frame_reader = CachedFrameReader::new(&mut raw_file, frame_bytes, 16);

    let mut sampled_full_frame = vec![0u8; frame_bytes];
    let mut one_framer_buf = vec![0u8; frame_bytes];
    let mut transition_buf = vec![0u8; frame_bytes];
    let mut ambiance_buf = vec![0u8; frame_bytes];
    let cropped_frame_bytes = (crop.width * crop.height * 3) as usize;
    let mut cropped_buf = vec![0u8; cropped_frame_bytes];
    let mut dissolve_buf = vec![0u8; cropped_frame_bytes];
    let mut blend_frames_storage = vec![vec![0u8; frame_bytes]; 4];

    let echo_k = 3usize;
    let mut echo_ring: Vec<Vec<u8>> = (0..echo_k).map(|_| vec![128u8; frame_bytes]).collect();
    let mut echo_head: usize = 0;

    let amb = plan.ambiance.as_ref();
    let vig_strength = amb.map(|a| a.vignette.strength).unwrap_or(0.3);
    let scanline_opacity = amb.map(|a| a.scanlines.opacity).unwrap_or(0.15);
    let rx_full = (src_w as f64) / 2.0;
    let ry_full = (src_h as f64) / 2.0;
    let r_max_full = (rx_full * rx_full + ry_full * ry_full).sqrt();
    let mut vignette_lut = vec![0u8; (src_w * src_h) as usize];
    for vy in 0..(src_h as usize) {
        let dy = (vy as f64) - ry_full;
        for vx in 0..(src_w as usize) {
            let dx = (vx as f64) - rx_full;
            let r = (dx * dx + dy * dy).sqrt();
            let factor = 1.0 - vig_strength * (r / r_max_full).powi(2);
            vignette_lut[vy * (src_w as usize) + vx] = (factor.clamp(0.0, 1.0) * 255.0) as u8;
        }
    }

    for i in 0..total_output_frames {
        if RENDER_CANCEL.load(Ordering::SeqCst) {
            let _ = encode_child.kill();
            let _ = std::fs::remove_file(&raw_cache_file);
            let _ = std::fs::remove_file(&out_mp4_path);
            return Err("Render cancelled by user".to_string());
        }

        let t = (i as f64) / output_fps;

        let seg = plan.segments
            .iter()
            .find(|s| t >= s.t0 && t <= s.t1)
            .or_else(|| plan.segments.last())
            .unwrap();

        let seg_dur = (seg.t1 - seg.t0).max(1e-6);
        let t_rel = (t - seg.t0).max(0.0);
        let x = (t_rel / seg_dur).clamp(0.0, 1.0);
        let u = evaluate_curve(&seg.curve, x);
        let u_prime = evaluate_curve_derivative(&seg.curve, x);

        let speed_v = ((seg.s1 - seg.s0).abs() / seg_dur) * u_prime;
        let n_blur = compute_motion_blur_frames(speed_v, plan.motion_blur);

        let src_time = seg.s0 + (seg.s1 - seg.s0) * u;
        let mut base_src_frame = (src_time * src_fps).round() as i64;
        if base_src_frame < 0 {
            base_src_frame = 0;
        }
        if base_src_frame >= total_source_frames as i64 {
            base_src_frame = (total_source_frames - 1) as i64;
        }

        // 1. Sample Full-Frame
        if n_blur <= 1 {
            frame_reader.get_frame(base_src_frame as u64, &mut sampled_full_frame)?;
        } else {
            let mut slice_ptrs: Vec<&[u8]> = Vec::with_capacity(n_blur);
            for k in 0..n_blur {
                let f_idx = (base_src_frame + (k as i64)).clamp(0, (total_source_frames - 1) as i64) as u64;
                frame_reader.get_frame(f_idx, &mut blend_frames_storage[k])?;
            }
            for k in 0..n_blur {
                slice_ptrs.push(&blend_frames_storage[k]);
            }
            blend_full_frames(&slice_ptrs, &mut sampled_full_frame);
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
                    if t_frames >= -1e-4 && t_frames <= 6.0 + 1e-4 {
                        active_trans = Some((trans, t_frames));
                        break;
                    }
                }
                "SLIDE_SHAKE" => {
                    if t_frames.abs() <= 3.0 + 1e-4 {
                        active_trans = Some((trans, t_frames));
                        break;
                    }
                }
                _ => {}
            }
        }

        let trans_frame_ptr = if let Some((trans, _t_frames)) = active_trans {
            match trans.transition_type.as_str() {
                "WARP_BUBBLE" => {
                    let env_a = compute_warp_bubble_env(t, trans.t, output_fps);
                    apply_warp_bubble(
                        full_frame_ptr,
                        &mut transition_buf,
                        src_w as usize,
                        src_h as usize,
                        env_a,
                        1.2,
                    );
                    &transition_buf
                }
                "WAVE_WARP" => {
                    let (h_t, k, v, t_fr) = compute_wave_warp_params(t, trans.t, output_fps, src_h as usize);
                    apply_wave_warp(
                        full_frame_ptr,
                        &mut transition_buf,
                        src_w as usize,
                        src_h as usize,
                        h_t,
                        k,
                        v,
                        t_fr,
                    );
                    &transition_buf
                }
                "SLIDE_SHAKE" => {
                    let shift_x = compute_slide_shake_shift(t, trans.t, output_fps);
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
            apply_ambiance_effects(
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
                    let ghost_frame_idx = (base_src_frame + 2).clamp(0, (total_source_frames - 1) as i64) as u64;
                    let mut ghost_full = vec![0u8; frame_bytes];
                    if frame_reader.get_frame(ghost_frame_idx, &mut ghost_full).is_ok() {
                        apply_transform_stack_cropped(
                            &ghost_full,
                            &mut dissolve_buf,
                            src_w as usize, src_h as usize,
                            crop.x, crop.y, crop.width, crop.height,
                            TransformParams { dx: 0.0, dy: 0.0, scale: 1.0, tilt_rad: 0.0,
                                              skew_x: 0.0, scale_y: 1.0, scale_x: 1.0, barrel_k: 0.0 },
                        );
                        let alpha_fp = (alpha * 256.0) as u32;
                        let inv_fp   = 256 - alpha_fp;
                        for (c, g) in cropped_buf.iter_mut().zip(dissolve_buf.iter()) {
                            *c = ((*c as u32 * inv_fp + *g as u32 * alpha_fp) >> 8) as u8;
                        }
                    }
                }
            }
        }

        // 4. Pipe to encoder
        encode_stdin.write_all(&cropped_buf)
            .map_err(|e| format!("Failed to write frame {i} to encoder: {e}"))?;

        if i % 8 == 0 || i == total_output_frames - 1 {
            let pct = ((i as f64) / (total_output_frames as f64) * 100.0) as u32;
            let _ = app.emit("render-progress", RenderProgressPayload {
                phase: "SAMPLING".to_string(),
                percent: pct,
                current_frame: (i + 1) as u32,
                total_frames: total_output_frames as u32,
                message: format!("Remapping & encoding frame {}/{}", i + 1, total_output_frames),
            });
        }
    }

    drop(encode_stdin);

    let _ = app.emit("render-progress", RenderProgressPayload {
        phase: "ENCODING".to_string(),
        percent: 99,
        current_frame: total_output_frames as u32,
        total_frames: total_output_frames as u32,
        message: "Finalizing MP4 container and audio muxing...".to_string(),
    });

    let encode_status = encode_child.wait()
        .map_err(|e| format!("Encoder wait failed: {e}"))?;

    let _ = std::fs::remove_file(&raw_cache_file);

    if !encode_status.success() {
        return Err("FFmpeg video encoding failed".to_string());
    }

    let render_time_secs = start_time.elapsed().as_secs_f64();
    let stats = compute_render_stats(&plan, &out_mp4_path, render_time_secs);

    let _ = app.emit("render-progress", RenderProgressPayload {
        phase: "ENCODING".to_string(),
        percent: 100,
        current_frame: total_output_frames as u32,
        total_frames: total_output_frames as u32,
        message: "Render completed successfully".to_string(),
    });

    Ok(stats)
}
