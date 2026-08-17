use std::io::{Read, Seek, SeekFrom, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{Emitter, Manager};

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

static RENDER_CANCEL: AtomicBool = AtomicBool::new(false);

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
pub struct ShakeEffect {
    #[serde(rename = "A0", alias = "a0")]
    pub a0: f64,
    pub omega: f64,
    pub k: f64,
    pub seed: u32,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct ZoomEffect {
    pub scale_start: f64,
    pub scale_end: f64,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct SegmentEffects {
    #[serde(default = "default_shake")]
    pub shake: ShakeEffect,
    #[serde(default = "default_zoom")]
    pub zoom: ZoomEffect,
    #[serde(default)]
    pub reverse: bool,
}

fn default_shake() -> ShakeEffect {
    ShakeEffect {
        a0: 0.0,
        omega: 0.0,
        k: 0.0,
        seed: 0,
    }
}

fn default_zoom() -> ZoomEffect {
    ZoomEffect {
        scale_start: 1.0,
        scale_end: 1.0,
    }
}

fn default_segment_effects() -> SegmentEffects {
    SegmentEffects {
        shake: default_shake(),
        zoom: default_zoom(),
        reverse: false,
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct PlanSegment {
    pub t0: f64,
    pub t1: f64,
    pub s0: f64,
    pub s1: f64,
    pub curve: String,
    #[serde(default = "default_segment_effects")]
    pub effects: SegmentEffects,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct OneFramer {
    pub t: f64,
    #[serde(rename = "type")]
    pub framer_type: String,
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
    #[serde(default)]
    pub motion_blur: bool,
    #[serde(default)]
    pub one_framers: Vec<OneFramer>,
    pub segments: Vec<PlanSegment>,
}

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
pub struct CropInfo {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub out_w: u32,
    pub out_h: u32,
}

#[inline(always)]
pub fn mirror_coordinate(coord: i64, max_dim: usize) -> usize {
    if max_dim <= 1 {
        return 0;
    }
    let dim = max_dim as i64;
    let mut c = coord;
    if c < 0 {
        c = -c;
    }
    if c >= dim {
        let diff = c - dim;
        c = dim - 1 - diff;
    }
    c.clamp(0, dim - 1) as usize
}

#[inline(always)]
pub fn sample_pixel_mirrored(frame_data: &[u8], width: usize, height: usize, x: i64, y: i64) -> [u8; 3] {
    let w_i = width as i64;
    let h_i = height as i64;
    let (mx, my) = if x >= 0 && x < w_i && y >= 0 && y < h_i {
        (x as usize, y as usize)
    } else {
        (mirror_coordinate(x, width), mirror_coordinate(y, height))
    };
    let idx = (my * width + mx) * 3;
    if idx + 2 < frame_data.len() {
        [frame_data[idx], frame_data[idx + 1], frame_data[idx + 2]]
    } else {
        [0, 0, 0]
    }
}

pub fn evaluate_curve(curve_name: &str, x: f64) -> f64 {
    let x_clamped = x.clamp(0.0, 1.0);
    match curve_name.to_lowercase().as_str() {
        "snap" => 1.0 - (1.0 - x_clamped).powi(3),
        "saddle" => x_clamped.powi(2) * (3.0 - 2.0 * x_clamped),
        _ => x_clamped,
    }
}

pub fn evaluate_curve_derivative(curve_name: &str, x: f64) -> f64 {
    let x_clamped = x.clamp(0.0, 1.0);
    match curve_name.to_lowercase().as_str() {
        "snap" => 3.0 * (1.0 - x_clamped).powi(2),
        "saddle" => 6.0 * x_clamped * (1.0 - x_clamped),
        _ => 1.0,
    }
}

pub fn compute_shake_envelope(t_rel: f64, duration: f64, fps: f64) -> f64 {
    if duration <= 1e-6 || fps <= 0.0 {
        return 0.0;
    }
    let frame_dur = 1.0 / fps;
    let transition_dur = 2.0 * frame_dur;
    if transition_dur <= 1e-6 {
        return 1.0;
    }

    let buildup = if t_rel <= transition_dur {
        (t_rel / transition_dur).clamp(0.0, 1.0)
    } else {
        1.0
    };

    let decay = if t_rel >= duration - transition_dur {
        ((duration - t_rel) / transition_dur).clamp(0.0, 1.0)
    } else {
        1.0
    };

    (buildup * decay).clamp(0.0, 1.0)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransformParams {
    pub dx: f64,
    pub dy: f64,
    pub scale: f64,
    pub tilt_rad: f64,
}

pub fn compute_transform_params(
    effects: &SegmentEffects,
    t_rel: f64,
    seg_dur: f64,
    fps: f64,
) -> TransformParams {
    let env = compute_shake_envelope(t_rel, seg_dur, fps);
    let damping = (-effects.shake.k * t_rel).exp();
    let seed = effects.shake.seed;

    let phi_x = ((seed % 360) as f64) * std::f64::consts::PI / 180.0;
    let phi_y = (((seed.wrapping_mul(17)) % 360) as f64) * std::f64::consts::PI / 180.0;
    let phi_z = (((seed.wrapping_mul(31)) % 360) as f64) * std::f64::consts::PI / 180.0;
    let phi_tilt = (((seed.wrapping_mul(47)) % 360) as f64) * std::f64::consts::PI / 180.0;

    let omega_t = effects.shake.omega * t_rel;

    let a0 = effects.shake.a0;
    let dx = a0 * damping * (omega_t + phi_x).sin() * env;
    let dy = a0 * damping * (omega_t + phi_y).sin() * env;
    let dz = (a0 / 100.0) * damping * (omega_t + phi_z).sin() * env;
    let d_tilt_deg = (a0 / 5.0) * damping * (omega_t + phi_tilt).sin() * env;
    let tilt_rad = d_tilt_deg * std::f64::consts::PI / 180.0;

    let x = (t_rel / seg_dur.max(1e-6)).clamp(0.0, 1.0);
    let base_scale = effects.zoom.scale_start + (effects.zoom.scale_end - effects.zoom.scale_start) * x;
    let total_scale = (base_scale * (1.0 + dz)).max(0.1);

    TransformParams {
        dx,
        dy,
        scale: total_scale,
        tilt_rad,
    }
}

pub fn apply_transform_stack_cropped(
    frame_in: &[u8],
    frame_crop_out: &mut [u8],
    src_width: usize,
    src_height: usize,
    crop_x: u32,
    crop_y: u32,
    crop_width: u32,
    crop_height: u32,
    params: TransformParams,
) {
    let cx = (src_width as f64) / 2.0;
    let cy = (src_height as f64) / 2.0;

    if params.dx.abs() < 1e-4
        && params.dy.abs() < 1e-4
        && (params.scale - 1.0).abs() < 1e-4
        && params.tilt_rad.abs() < 1e-4
    {
        let row_src_stride = src_width * 3;
        let row_crop_stride = (crop_width * 3) as usize;
        for row in 0..crop_height {
            let src_y = (crop_y + row) as usize;
            let src_start = src_y * row_src_stride + (crop_x * 3) as usize;
            let src_end = src_start + row_crop_stride;
            let dst_start = (row as usize) * row_crop_stride;
            let dst_end = dst_start + row_crop_stride;
            frame_crop_out[dst_start..dst_end].copy_from_slice(&frame_in[src_start..src_end]);
        }
        return;
    }

    let inv_s = 1.0 / params.scale;
    let cos_t = params.tilt_rad.cos();
    let sin_t = params.tilt_rad.sin();

    let step_x_to_xs = inv_s * cos_t;
    let step_x_to_ys = -inv_s * sin_t;

    let step_xs_fp = (step_x_to_xs * 65536.0).round() as i32;
    let step_ys_fp = (step_x_to_ys * 65536.0).round() as i32;

    let w_i32 = src_width as i32;
    let h_i32 = src_height as i32;

    let cw = crop_width as usize;
    let ch = crop_height as usize;

    for yd in 0..ch {
        let yd_full = (crop_y as usize) + yd;
        let yd_rel = (yd_full as f64) - cy;
        let xd_start_rel = (crop_x as f64) - cx;

        let base_xs = cx - params.dx + inv_s * (xd_start_rel * cos_t + yd_rel * sin_t);
        let base_ys = cy - params.dy + inv_s * (-xd_start_rel * sin_t + yd_rel * cos_t);

        let mut xs_fp = (base_xs * 65536.0 + 32768.0) as i32;
        let mut ys_fp = (base_ys * 65536.0 + 32768.0) as i32;

        let row_out_start = yd * cw * 3;
        let row_out = &mut frame_crop_out[row_out_start..row_out_start + cw * 3];

        for xd in 0..cw {
            let xs = xs_fp >> 16;
            let ys = ys_fp >> 16;
            let out_idx = xd * 3;

            if xs >= 0 && xs < w_i32 && ys >= 0 && ys < h_i32 {
                let in_idx = ((ys as usize) * src_width + (xs as usize)) * 3;
                row_out[out_idx] = frame_in[in_idx];
                row_out[out_idx + 1] = frame_in[in_idx + 1];
                row_out[out_idx + 2] = frame_in[in_idx + 2];
            } else {
                let pixel = sample_pixel_mirrored(frame_in, src_width, src_height, xs as i64, ys as i64);
                row_out[out_idx] = pixel[0];
                row_out[out_idx + 1] = pixel[1];
                row_out[out_idx + 2] = pixel[2];
            }

            xs_fp += step_xs_fp;
            ys_fp += step_ys_fp;
        }
    }
}

pub fn apply_transform_stack(
    frame_in: &[u8],
    frame_out: &mut [u8],
    width: usize,
    height: usize,
    params: TransformParams,
) {
    if params.dx.abs() < 1e-4
        && params.dy.abs() < 1e-4
        && (params.scale - 1.0).abs() < 1e-4
        && params.tilt_rad.abs() < 1e-4
    {
        frame_out.copy_from_slice(frame_in);
        return;
    }

    let cx = (width as f64) / 2.0;
    let cy = (height as f64) / 2.0;
    let inv_s = 1.0 / params.scale;
    let cos_t = params.tilt_rad.cos();
    let sin_t = params.tilt_rad.sin();

    let step_x_to_xs = inv_s * cos_t;
    let step_x_to_ys = -inv_s * sin_t;

    let step_xs_fp = (step_x_to_xs * 65536.0).round() as i32;
    let step_ys_fp = (step_x_to_ys * 65536.0).round() as i32;

    let w_i32 = width as i32;
    let h_i32 = height as i32;

    for yd in 0..height {
        let yd_rel = (yd as f64) - cy;
        let base_xs = cx - params.dx + inv_s * (yd_rel * sin_t);
        let base_ys = cy - params.dy + inv_s * (yd_rel * cos_t);

        let mut xs_fp = (base_xs * 65536.0 + 32768.0) as i32;
        let mut ys_fp = (base_ys * 65536.0 + 32768.0) as i32;

        let row_out_start = yd * width * 3;
        let row_out = &mut frame_out[row_out_start..row_out_start + width * 3];

        for xd in 0..width {
            let xs = xs_fp >> 16;
            let ys = ys_fp >> 16;
            let out_idx = xd * 3;

            if xs >= 0 && xs < w_i32 && ys >= 0 && ys < h_i32 {
                let in_idx = ((ys as usize) * width + (xs as usize)) * 3;
                row_out[out_idx] = frame_in[in_idx];
                row_out[out_idx + 1] = frame_in[in_idx + 1];
                row_out[out_idx + 2] = frame_in[in_idx + 2];
            } else {
                let pixel = sample_pixel_mirrored(frame_in, width, height, xs as i64, ys as i64);
                row_out[out_idx] = pixel[0];
                row_out[out_idx + 1] = pixel[1];
                row_out[out_idx + 2] = pixel[2];
            }

            xs_fp += step_xs_fp;
            ys_fp += step_ys_fp;
        }
    }
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
            cache: std::collections::HashMap::new(),
            order: std::collections::VecDeque::new(),
            capacity,
        }
    }

    pub fn get_frame(&mut self, frame_idx: u64, buf: &mut [u8]) -> Result<(), String> {
        if let Some(cached) = self.cache.get(&frame_idx) {
            buf.copy_from_slice(cached);
            return Ok(());
        }

        let offset = frame_idx * (self.frame_bytes as u64);
        self.file.seek(SeekFrom::Start(offset))
            .map_err(|e| format!("Failed to seek cache for frame {frame_idx}: {e}"))?;
        self.file.read_exact(buf)
            .map_err(|e| format!("Failed to read frame {frame_idx} from cache: {e}"))?;

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

pub fn compute_motion_blur_frames(v: f64, motion_blur_enabled: bool) -> usize {
    if !motion_blur_enabled || v <= 1.0 {
        1
    } else {
        (1 + v.floor() as usize).min(4)
    }
}

#[inline(always)]
pub fn blend_full_frames(frames: &[&[u8]], out: &mut [u8]) {
    let n = frames.len();
    if n == 0 {
        return;
    }
    if n == 1 {
        out.copy_from_slice(frames[0]);
        return;
    }
    if n == 2 {
        let f0 = frames[0];
        let f1 = frames[1];
        for (i, o) in out.iter_mut().enumerate() {
            *o = ((f0[i] as u16 + f1[i] as u16) >> 1) as u8;
        }
        return;
    }
    if n == 4 {
        let f0 = frames[0];
        let f1 = frames[1];
        let f2 = frames[2];
        let f3 = frames[3];
        for (i, o) in out.iter_mut().enumerate() {
            *o = ((f0[i] as u16 + f1[i] as u16 + f2[i] as u16 + f3[i] as u16) >> 2) as u8;
        }
        return;
    }
    let n_u32 = n as u32;
    for (i, out_byte) in out.iter_mut().enumerate() {
        let mut sum = 0u32;
        for frame in frames {
            sum += frame[i] as u32;
        }
        *out_byte = (sum / n_u32) as u8;
    }
}

pub const ONE_FRAMER_TYPES: [&str; 6] = [
    "FLASH_WHITE",
    "FLASH_BLACK",
    "INVERT",
    "TINT_SCENE",
    "OFFSET_BLUR",
    "RADIAL_BLUR",
];

pub fn deterministic_hash_pos(t: f64, salt: u64) -> u64 {
    let bits = (t * 10000.0).round() as i64;
    let mut h = (bits as u64) ^ 0x517cc1b727220a95 ^ salt;
    h = h.wrapping_mul(0x6c62272e07bb0142).wrapping_add(1);
    h ^= h >> 33;
    h = h.wrapping_mul(0x62a9d9ed799705f5);
    h ^= h >> 28;
    h
}

pub fn apply_one_framer_flash_white(frame_in: &[u8], frame_out: &mut [u8]) {
    for (out, &inp) in frame_out.iter_mut().zip(frame_in.iter()) {
        *out = (((inp as u32) * 2 + 2040) / 10).min(255) as u8;
    }
}

pub fn apply_one_framer_flash_black(frame_in: &[u8], frame_out: &mut [u8]) {
    for (out, &inp) in frame_out.iter_mut().zip(frame_in.iter()) {
        *out = (((inp as u32) * 2) / 10) as u8;
    }
}

pub fn apply_one_framer_invert(frame_in: &[u8], frame_out: &mut [u8]) {
    for (out, &inp) in frame_out.iter_mut().zip(frame_in.iter()) {
        *out = 255 - inp;
    }
}

pub fn apply_one_framer_tint_scene(
    frame_in: &[u8],
    frame_out: &mut [u8],
    width: usize,
    height: usize,
) {
    let cx = (width / 2) as i64;
    let cy = (height / 2) as i64;
    let mut sum_r = 0u64;
    let mut sum_g = 0u64;
    let mut sum_b = 0u64;
    let mut count = 0u64;

    for dy in -2..=2 {
        for dx in -2..=2 {
            let px = sample_pixel_mirrored(frame_in, width, height, cx + dx, cy + dy);
            sum_r += px[0] as u64;
            sum_g += px[1] as u64;
            sum_b += px[2] as u64;
            count += 1;
        }
    }
    let avg_r = if count > 0 { (sum_r / count) as u32 } else { 128 };
    let avg_g = if count > 0 { (sum_g / count) as u32 } else { 128 };
    let avg_b = if count > 0 { (sum_b / count) as u32 } else { 128 };

    let mut lut_r = [0u8; 256];
    let mut lut_g = [0u8; 256];
    let mut lut_b = [0u8; 256];
    for i in 0..256 {
        lut_r[i] = (((i as u32) * 4 + avg_r * 6) / 10).min(255) as u8;
        lut_g[i] = (((i as u32) * 4 + avg_g * 6) / 10).min(255) as u8;
        lut_b[i] = (((i as u32) * 4 + avg_b * 6) / 10).min(255) as u8;
    }

    for (chunk_in, chunk_out) in frame_in.chunks_exact(3).zip(frame_out.chunks_exact_mut(3)) {
        chunk_out[0] = lut_r[chunk_in[0] as usize];
        chunk_out[1] = lut_g[chunk_in[1] as usize];
        chunk_out[2] = lut_b[chunk_in[2] as usize];
    }
}

pub fn apply_one_framer_offset_blur(
    frame_in: &[u8],
    frame_out: &mut [u8],
    width: usize,
    height: usize,
) {
    let row_stride = width * 3;
    let ext_len = width + 128;
    let mut pref_r = vec![0u32; ext_len + 1];
    let mut pref_g = vec![0u32; ext_len + 1];
    let mut pref_b = vec![0u32; ext_len + 1];

    for y in 0..height {
        let row_in_offset = y * row_stride;
        let row_in = &frame_in[row_in_offset..row_in_offset + row_stride];
        let row_out = &mut frame_out[row_in_offset..row_in_offset + row_stride];

        for i in 0..ext_len {
            let orig_x = (i as i64) - 64;
            let mx = mirror_coordinate(orig_x, width);
            let idx = mx * 3;
            pref_r[i + 1] = pref_r[i] + row_in[idx] as u32;
            pref_g[i + 1] = pref_g[i] + row_in[idx + 1] as u32;
            pref_b[i + 1] = pref_b[i] + row_in[idx + 2] as u32;
        }

        for x in 0..width {
            let start_idx = 64 + x + 15;
            let end_idx = 64 + x + 45;

            let sum_r = pref_r[end_idx + 1] - pref_r[start_idx];
            let sum_g = pref_g[end_idx + 1] - pref_g[start_idx];
            let sum_b = pref_b[end_idx + 1] - pref_b[start_idx];

            let out_idx = x * 3;
            row_out[out_idx] = (sum_r / 31) as u8;
            row_out[out_idx + 1] = (sum_g / 31) as u8;
            row_out[out_idx + 2] = (sum_b / 31) as u8;
        }
    }
}

pub fn apply_one_framer_radial_blur(
    frame_in: &[u8],
    frame_out: &mut [u8],
    width: usize,
    height: usize,
) {
    let cx_i = (width / 2) as i32;
    let cy_i = (height / 2) as i32;
    let w_i = width as i32;
    let h_i = height as i32;

    for y in 0..height {
        let vy = (y as i32) - cy_i;
        let dy_fp = (vy * 65536) / 25;
        let y_fp = ((y as i32) * 65536) + 32768;

        let row_out_offset = y * width * 3;
        let row_out = &mut frame_out[row_out_offset..row_out_offset + width * 3];

        for x in 0..width {
            let vx = (x as i32) - cx_i;
            let dx_fp = (vx * 65536) / 25;
            let x_fp = ((x as i32) * 65536) + 32768;

            let mut acc_r = 0u32;
            let mut acc_g = 0u32;
            let mut acc_b = 0u32;

            for k in 0..10 {
                let xs = (x_fp - (k as i32) * dx_fp) >> 16;
                let ys = (y_fp - (k as i32) * dy_fp) >> 16;

                if xs >= 0 && xs < w_i && ys >= 0 && ys < h_i {
                    let idx = ((ys as usize) * width + (xs as usize)) * 3;
                    acc_r += frame_in[idx] as u32;
                    acc_g += frame_in[idx + 1] as u32;
                    acc_b += frame_in[idx + 2] as u32;
                } else {
                    let px = sample_pixel_mirrored(frame_in, width, height, xs as i64, ys as i64);
                    acc_r += px[0] as u32;
                    acc_g += px[1] as u32;
                    acc_b += px[2] as u32;
                }
            }

            let out_idx = x * 3;
            row_out[out_idx] = (acc_r / 10) as u8;
            row_out[out_idx + 1] = (acc_g / 10) as u8;
            row_out[out_idx + 2] = (acc_b / 10) as u8;
        }
    }
}

pub fn apply_one_framer(
    framer_type: &str,
    frame_in: &[u8],
    frame_out: &mut [u8],
    width: usize,
    height: usize,
) {
    match framer_type {
        "FLASH_WHITE" => apply_one_framer_flash_white(frame_in, frame_out),
        "FLASH_BLACK" => apply_one_framer_flash_black(frame_in, frame_out),
        "INVERT" => apply_one_framer_invert(frame_in, frame_out),
        "TINT_SCENE" => apply_one_framer_tint_scene(frame_in, frame_out, width, height),
        "OFFSET_BLUR" => apply_one_framer_offset_blur(frame_in, frame_out, width, height),
        "RADIAL_BLUR" => apply_one_framer_radial_blur(frame_in, frame_out, width, height),
        _ => frame_out.copy_from_slice(frame_in),
    }
}

pub fn generate_one_framers(
    style: &str,
    segments: &[PlanSegment],
    downbeats: &[f64],
    fps: u32,
    target_duration: f64,
) -> Vec<OneFramer> {
    if fps == 0 || segments.is_empty() {
        return Vec::new();
    }
    let dt = 1.0 / (fps as f64);
    let mut raw_candidates: Vec<OneFramer> = Vec::new();
    let style_upper = style.to_uppercase();

    // 1. Cuts: boundaries between segments
    for (seg_idx, seg) in segments.iter().enumerate() {
        let t_cut = seg.t0;

        let place_cut = match style_upper.as_str() {
            "HARD" => true,
            "SMOOTH" => deterministic_hash_pos(t_cut, 100) % 2 == 0,
            "HYBRID" => true,
            _ => true,
        };

        if place_cut {
            let offsets = [-2.0 * dt, -1.0 * dt, 0.0, 1.0 * dt];
            for (idx, &off) in offsets.iter().enumerate() {
                let t_pos = t_cut + off;
                let t_rounded = (t_pos * (fps as f64)).round() / (fps as f64);
                let seed = deterministic_hash_pos(t_rounded, (seg_idx as u64) * 10 + (idx as u64) + 1);
                let framer_type = ONE_FRAMER_TYPES[(seed % 6) as usize].to_string();
                raw_candidates.push(OneFramer {
                    t: t_rounded,
                    framer_type,
                });
            }
        }
    }

    // 2. Downbeats
    for (db_idx, &db) in downbeats.iter().enumerate() {
        let place_downbeat = match style_upper.as_str() {
            "HARD" => true,
            "SMOOTH" => true,
            "HYBRID" => deterministic_hash_pos(db, 200) % 2 == 0,
            _ => true,
        };

        if place_downbeat {
            let t_rounded = (db * (fps as f64)).round() / (fps as f64);
            let seed = deterministic_hash_pos(t_rounded, (db_idx as u64) * 100 + 777);
            let framer_type = ONE_FRAMER_TYPES[(seed % 6) as usize].to_string();
            raw_candidates.push(OneFramer {
                t: t_rounded,
                framer_type,
            });
        }
    }

    // Filter within [0.0, target_duration]
    let mut valid_framers: Vec<OneFramer> = raw_candidates
        .into_iter()
        .filter(|f| f.t >= -1e-6 && f.t <= target_duration + 1e-6)
        .collect();

    // Sort ascending by t
    valid_framers.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap_or(std::cmp::Ordering::Equal));

    // Deduplicate by frame index
    let mut deduped: Vec<OneFramer> = Vec::new();
    let mut seen_frames = std::collections::HashSet::new();
    for framer in valid_framers {
        let frame_idx = (framer.t * (fps as f64)).round() as i64;
        if seen_frames.insert(frame_idx) {
            deduped.push(framer);
        }
    }

    deduped
}

pub fn compute_crop_to_fill(src_w: u32, src_h: u32, aspect_w: u32, aspect_h: u32) -> CropInfo {
    let aspect_w = if aspect_w == 0 { 1080 } else { aspect_w };
    let aspect_h = if aspect_h == 0 { 1080 } else { aspect_h };

    let src_ar = (src_w as f64) / (src_h as f64);
    let target_ar = (aspect_w as f64) / (aspect_h as f64);

    let (crop_w, crop_h, crop_x, crop_y) = if src_ar > target_ar {
        let ch = src_h;
        let mut cw = ((src_h as f64) * target_ar).round() as u32;
        cw = (cw.min(src_w)) & !1;
        let cx = (src_w - cw) / 2;
        (cw, ch, cx, 0)
    } else {
        let cw = src_w;
        let mut ch = ((src_w as f64) / target_ar).round() as u32;
        ch = (ch.min(src_h)) & !1;
        let cy = (src_h - ch) / 2;
        (cw, ch, 0, cy)
    };

    let (out_w, out_h) = if aspect_w >= aspect_h {
        let ow = 1080u32;
        let mut oh = ((1080.0 / target_ar).round() as u32) & !1;
        if oh == 0 {
            oh = 2;
        }
        (ow, oh)
    } else {
        let oh = 1080u32;
        let mut ow = ((1080.0 * target_ar).round() as u32) & !1;
        if ow == 0 {
            ow = 2;
        }
        (ow, oh)
    };

    CropInfo {
        x: crop_x,
        y: crop_y,
        width: crop_w,
        height: crop_h,
        out_w,
        out_h,
    }
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

fn get_ffmpeg_binary(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    // 1. Check in app_data_dir/binaries/ffmpeg.exe
    if let Ok(data_dir) = app.path().app_data_dir() {
        let in_data = data_dir.join("binaries").join("ffmpeg.exe");
        if in_data.exists() {
            return Ok(in_data);
        }
        let in_data_root = data_dir.join("ffmpeg.exe");
        if in_data_root.exists() {
            return Ok(in_data_root);
        }
    }

    // 2. Check local binaries dir
    let direct = get_binary_path(app, "ffmpeg.exe");
    if direct.exists() {
        return Ok(direct);
    }

    // 3. Check system PATH
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        if let Ok(output) = std::process::Command::new("where.exe")
            .arg("ffmpeg")
            .creation_flags(CREATE_NO_WINDOW)
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if let Some(first_line) = stdout.lines().next() {
                    let path = std::path::PathBuf::from(first_line.trim());
                    if path.exists() {
                        return Ok(path);
                    }
                }
            }
        }
    }

    // 4. Download and extract ffmpeg-release-essentials
    download_and_extract_ffmpeg(app)
}

fn download_and_extract_ffmpeg(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to resolve app data directory: {e}"))?;
    let bin_dir = data_dir.join("binaries");
    std::fs::create_dir_all(&bin_dir)
        .map_err(|e| format!("Failed to create directory {}: {e}", bin_dir.display()))?;

    let ffmpeg_dest = bin_dir.join("ffmpeg.exe");
    if ffmpeg_dest.exists() {
        return Ok(ffmpeg_dest);
    }

    // Check temp extracted folder if already downloaded
    let temp_extracted = std::env::temp_dir().join("ffmpeg-essentials");
    if temp_extracted.exists() {
        if let Ok(entries) = std::fs::read_dir(&temp_extracted) {
            for entry in entries.flatten() {
                let bin = entry.path().join("bin").join("ffmpeg.exe");
                if bin.exists() {
                    let _ = std::fs::copy(&bin, &ffmpeg_dest);
                    if ffmpeg_dest.exists() {
                        return Ok(ffmpeg_dest);
                    }
                }
            }
        }
    }

    // Fallback: download essentials zip from gyan.dev
    let zip_url = "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip";
    let zip_dest = data_dir.join("ffmpeg-download.zip");

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let script = format!(
            "[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12; Invoke-WebRequest -Uri '{}' -OutFile '{}'; Expand-Archive -Path '{}' -DestinationPath '{}' -Force",
            zip_url,
            zip_dest.display(),
            zip_dest.display(),
            bin_dir.display()
        );
        let status = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", &script])
            .creation_flags(CREATE_NO_WINDOW)
            .status()
            .map_err(|e| format!("Failed to download ffmpeg: {e}"))?;

        if !status.success() {
            return Err("Failed to download or extract FFmpeg".to_string());
        }

        if let Ok(entries) = std::fs::read_dir(&bin_dir) {
            for entry in entries.flatten() {
                let p = entry.path().join("bin").join("ffmpeg.exe");
                if p.exists() {
                    let _ = std::fs::copy(&p, &ffmpeg_dest);
                    let _ = std::fs::remove_file(&zip_dest);
                    return Ok(ffmpeg_dest);
                }
            }
        }
    }

    if ffmpeg_dest.exists() {
        Ok(ffmpeg_dest)
    } else {
        Err("FFmpeg binary could not be located".to_string())
    }
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
    downbeats: &[f64],
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

    // 3. Style presets for shake, zoom, and reverse remap
    let (a0, omega, k, zoom_max) = match style.to_uppercase().as_str() {
        "SMOOTH" => (3.0, 8.0, 2.0, 1.05),
        "HYBRID" => (5.0, 12.0, 2.5, 1.10),
        _ => (8.0, 15.0, 3.0, 1.15), // HARD default
    };

    // 4. Build segments and handle video loops
    let mut segments = Vec::new();
    let mut s_cursor = 0.0;
    let mut loops = 0u32;
    let mut downbeat_count = 0usize;
    let mut seg_index = 0usize;

    for win in filtered_bounds.windows(2) {
        let t0 = win[0];
        let t1 = win[1];

        let (curve_name, r) = match style.to_uppercase().as_str() {
            "SMOOTH" => ("saddle".to_string(), 0.75),
            "HYBRID" => {
                if seg_index % 2 == 0 {
                    ("snap".to_string(), 1.0)
                } else {
                    ("saddle".to_string(), 0.75)
                }
            }
            _ => ("snap".to_string(), 1.0), // HARD default
        };

        // Check if segment falls on a downbeat
        let is_on_downbeat = downbeats.iter().any(|&d| d >= t0 - 1e-4 && d < t1 - 1e-4);
        let mut reverse_this_segment = false;

        if is_on_downbeat {
            downbeat_count += 1;
            match style.to_uppercase().as_str() {
                "HARD" => {
                    if downbeat_count % 5 == 1 {
                        reverse_this_segment = true;
                    }
                }
                "HYBRID" => {
                    if downbeat_count % 10 == 1 {
                        reverse_this_segment = true;
                    }
                }
                _ => {} // SMOOTH = 0% reverse
            }
        }

        let mut seg_t0 = t0;
        let seg_t1 = t1;

        while seg_t0 < seg_t1 - 1e-9 {
            let dt = seg_t1 - seg_t0;
            let span = r * dt;

            // Zoom continuity: alternating between 1.0 and zoom_max
            let (scale_start, scale_end) = if seg_index % 2 == 0 {
                (1.0, zoom_max)
            } else {
                (zoom_max, 1.0)
            };

            let seed = ((seg_index as u32).wrapping_mul(1664525).wrapping_add(1013904223)) ^ 0x5bf03635;

            let effects = SegmentEffects {
                shake: ShakeEffect {
                    a0,
                    omega,
                    k,
                    seed,
                },
                zoom: ZoomEffect {
                    scale_start,
                    scale_end,
                },
                reverse: reverse_this_segment,
            };

            if s_cursor + span <= video_duration + 1e-9 {
                let mut s0 = s_cursor;
                let mut s1 = s_cursor + span;
                s_cursor += span;

                if (s_cursor - video_duration).abs() < 1e-9 {
                    s_cursor = 0.0;
                    loops += 1;
                }

                if reverse_this_segment {
                    std::mem::swap(&mut s0, &mut s1);
                }

                segments.push(PlanSegment {
                    t0: (seg_t0 * 10000.0).round() / 10000.0,
                    t1: (seg_t1 * 10000.0).round() / 10000.0,
                    s0: (s0 * 10000.0).round() / 10000.0,
                    s1: (s1 * 10000.0).round() / 10000.0,
                    curve: curve_name.clone(),
                    effects,
                });
                seg_index += 1;
                break;
            } else {
                // Loop wrap happens within this segment
                let t_wrap = seg_t0 + (video_duration - s_cursor) / r;
                let mut s0 = s_cursor;
                let mut s1 = video_duration;

                if reverse_this_segment {
                    std::mem::swap(&mut s0, &mut s1);
                }

                segments.push(PlanSegment {
                    t0: (seg_t0 * 10000.0).round() / 10000.0,
                    t1: (t_wrap * 10000.0).round() / 10000.0,
                    s0: (s0 * 10000.0).round() / 10000.0,
                    s1: (s1 * 10000.0).round() / 10000.0,
                    curve: curve_name.clone(),
                    effects,
                });

                loops += 1;
                s_cursor = 0.0;
                seg_t0 = t_wrap;
                seg_index += 1;
            }
        }
    }

    let target_dur = (target * 1000.0).round() / 1000.0;
    let one_framers = generate_one_framers(style, &segments, downbeats, fps, target_dur);

    Ok(ProjectPlan {
        schema_version: 2,
        style: style.to_uppercase(),
        fps,
        aspect: AspectRatio {
            w: aspect_w,
            h: aspect_h,
        },
        borderless: true,
        bpm: (bpm * 100.0).round() / 100.0,
        target_duration: target_dur,
        video_duration: (video_duration * 1000.0).round() / 1000.0,
        audio_duration: (audio_duration * 1000.0).round() / 1000.0,
        loops,
        motion_blur: true,
        one_framers,
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

#[tauri::command]
fn cancel_render() -> Result<(), String> {
    RENDER_CANCEL.store(true, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
fn open_target_folder(path: String) -> Result<(), String> {
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
async fn run_render_pipeline(
    app: tauri::AppHandle,
    plan_json: String,
    scene_path: String,
    audio_path: String,
) -> Result<String, String> {
    RENDER_CANCEL.store(false, Ordering::SeqCst);

    let plan: ProjectPlan = serde_json::from_str(&plan_json)
        .map_err(|e| format!("Invalid plan JSON: {e}"))?;

    let ffmpeg_bin = get_ffmpeg_binary(&app)?;

    // 1. Probe scene info
    let scene_info = probe_media(scene_path.clone())?;
    if scene_info.width == 0 || scene_info.height == 0 {
        return Err("Invalid scene dimensions for rendering".to_string());
    }

    let src_w = scene_info.width;
    let src_h = scene_info.height;
    let src_fps = if scene_info.fps > 0.0 { scene_info.fps } else { 30.0 };
    let frame_bytes = (src_w * src_h * 3) as usize;

    let estimated_frames = (scene_info.duration * src_fps).ceil() as u64;
    let estimated_cache_size = estimated_frames * (frame_bytes as u64);

    // Safeguard: 4 GB maximum cache size
    const MAX_CACHE_BYTES: u64 = 4 * 1024 * 1024 * 1024;
    if estimated_cache_size > MAX_CACHE_BYTES {
        return Err("source too heavy for beta renderer".to_string());
    }

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
        cmd.args([
            "-y",
            "-i", &scene_path,
            "-f", "rawvideo",
            "-pix_fmt", "rgb24",
            "-an",
            &raw_cache_file.to_string_lossy(),
        ]);
        #[cfg(target_os = "windows")]
        cmd.creation_flags(CREATE_NO_WINDOW);
        cmd.spawn()
            .map_err(|e| format!("Failed to spawn ffmpeg decoder: {e}"))?
    };

    // Monitor decode
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
    let out_mp4_path = out_dir.join(format!("cia_jugg_{timestamp}.mp4"));

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
        "-c:v", "libx264",
        "-pix_fmt", "yuv420p",
        "-crf", "18",
        "-preset", "veryfast",
        "-c:a", "aac",
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
    let cropped_frame_bytes = (crop.width * crop.height * 3) as usize;
    let mut cropped_buf = vec![0u8; cropped_frame_bytes];
    let mut blend_frames_storage = vec![vec![0u8; frame_bytes]; 4];

    for i in 0..total_output_frames {
        if RENDER_CANCEL.load(Ordering::SeqCst) {
            let _ = encode_child.kill();
            let _ = std::fs::remove_file(&raw_cache_file);
            let _ = std::fs::remove_file(&out_mp4_path);
            return Err("Render cancelled by user".to_string());
        }

        let t = (i as f64) / output_fps;

        // Find segment in plan.segments
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

        // 2 & 3. Transform Stack + Crop
        let transform_params = compute_transform_params(&seg.effects, t_rel, seg_dur, output_fps);
        apply_transform_stack_cropped(
            full_frame_ptr,
            &mut cropped_buf,
            src_w as usize,
            src_h as usize,
            crop.x,
            crop.y,
            crop.width,
            crop.height,
            transform_params,
        );

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

    // Cleanup raw cache
    let _ = std::fs::remove_file(&raw_cache_file);

    if !encode_status.success() {
        return Err("FFmpeg video encoding failed".to_string());
    }

    let _ = app.emit("render-progress", RenderProgressPayload {
        phase: "ENCODING".to_string(),
        percent: 100,
        current_frame: total_output_frames as u32,
        total_frames: total_output_frames as u32,
        message: "Render completed successfully".to_string(),
    });

    Ok(out_mp4_path.to_string_lossy().to_string())
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
            save_plan,
            cancel_render,
            open_target_folder,
            run_render_pipeline
        ])
        .run(tauri::generate_context!())
        .expect("error while running cia app");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shake_envelope_boundaries() {
        // (i) Shake: oscillation amortie aux bornes
        // t=0 -> env=0
        // t=2 frames -> env=1
        // t=end-2 frames -> env=1
        // t=end -> env=0
        let fps = 16.0;
        let duration = 2.0; // 2 seconds
        let dt_frame = 1.0 / fps; // 0.0625s
        let two_frames = 2.0 * dt_frame; // 0.125s

        let env_start = compute_shake_envelope(0.0, duration, fps);
        assert_eq!(env_start, 0.0, "Envelope at t=0 must be 0.0");

        let env_2_frames = compute_shake_envelope(two_frames, duration, fps);
        assert!((env_2_frames - 1.0).abs() < 1e-6, "Envelope at t=2 frames must be 1.0 (got {})", env_2_frames);

        let env_mid = compute_shake_envelope(duration / 2.0, duration, fps);
        assert!((env_mid - 1.0).abs() < 1e-6, "Envelope in mid segment must be 1.0 (got {})", env_mid);

        let env_end_minus_2 = compute_shake_envelope(duration - two_frames, duration, fps);
        assert!((env_end_minus_2 - 1.0).abs() < 1e-6, "Envelope at t=end-2 frames must be 1.0 (got {})", env_end_minus_2);

        let env_end = compute_shake_envelope(duration, duration, fps);
        assert_eq!(env_end, 0.0, "Envelope at t=end must be 0.0");
    }

    #[test]
    fn test_zoom_continuity() {
        // (ii) Zoom continuité: scale_end segment N = scale_start segment N+1
        let beats = vec![0.5, 1.0, 1.5, 2.0, 2.5, 3.0];
        let downbeats = vec![1.0, 2.0, 3.0];

        for style in ["HARD", "SMOOTH", "HYBRID"] {
            let plan = create_plan_internal(style, 16, &beats, &downbeats, 5.0, 3.5, 1080, 1080, 120.0).unwrap();

            for win in plan.segments.windows(2) {
                let seg_n = &win[0];
                let seg_n1 = &win[1];
                assert_eq!(
                    seg_n.effects.zoom.scale_end,
                    seg_n1.effects.zoom.scale_start,
                    "Zoom continuity broken between segments in style {}",
                    style
                );
            }
        }
    }

    #[test]
    fn test_reverse_remap_planner() {
        // (iii) Reverse: s1 < s0 pour les segments inversés, s0 < s1 pour les normaux
        let beats = vec![
            0.42, 1.14, 1.88, 2.60, 3.32, 4.04, 4.76, 5.50, 6.20, 6.94, 7.64, 8.38, 9.10, 9.82,
            10.54, 11.26, 11.98, 12.70, 13.42, 14.14,
        ];
        let downbeats = vec![2.60, 5.50, 8.38, 11.26, 14.14];

        let hard_plan = create_plan_internal("HARD", 16, &beats, &downbeats, 10.773, 14.315, 1080, 1080, 83.33).unwrap();
        let mut reverse_found = false;

        for seg in &hard_plan.segments {
            if seg.effects.reverse {
                assert!(seg.s1 < seg.s0, "Reversed segment must have s1 < s0 (got s0={}, s1={})", seg.s0, seg.s1);
                reverse_found = true;
            } else {
                assert!(seg.s0 < seg.s1, "Normal segment must have s0 < s1 (got s0={}, s1={})", seg.s0, seg.s1);
            }
        }
        assert!(reverse_found, "HARD style on fixture must contain at least one reversed downbeat segment");

        // SMOOTH style must have 0% reverse
        let smooth_plan = create_plan_internal("SMOOTH", 16, &beats, &downbeats, 10.773, 14.315, 1080, 1080, 83.33).unwrap();
        for seg in &smooth_plan.segments {
            assert!(!seg.effects.reverse, "SMOOTH style must not contain reversed segments");
            assert!(seg.s0 < seg.s1);
        }
    }

    #[test]
    fn test_mirror_coordinate_and_sample_pixel_mirrored() {
        let w = 100usize;
        let h = 100usize;

        assert_eq!(mirror_coordinate(-5, w), 5);
        assert_eq!(mirror_coordinate(0, w), 0);
        assert_eq!(mirror_coordinate((w - 1) as i64, w), w - 1);
        assert_eq!(mirror_coordinate((w + 3) as i64, w), w - 4);

        assert_eq!(mirror_coordinate(-5, h), 5);
        assert_eq!(mirror_coordinate(0, h), 0);
        assert_eq!(mirror_coordinate((h - 1) as i64, h), h - 1);
        assert_eq!(mirror_coordinate((h + 3) as i64, h), h - 4);

        let mut test_image = vec![0u8; w * h * 3];
        let idx_left = (10 * w + 5) * 3;
        test_image[idx_left] = 255;
        test_image[idx_left + 1] = 128;
        test_image[idx_left + 2] = 64;

        let sampled_left = sample_pixel_mirrored(&test_image, w, h, -5, 10);
        assert_eq!(sampled_left, [255, 128, 64]);

        let idx_right = (10 * w + (w - 4)) * 3;
        test_image[idx_right] = 10;
        test_image[idx_right + 1] = 20;
        test_image[idx_right + 2] = 30;

        let sampled_right = sample_pixel_mirrored(&test_image, w, h, (w + 3) as i64, 10);
        assert_eq!(sampled_right, [10, 20, 30]);

        let idx_top = (5 * w + 10) * 3;
        test_image[idx_top] = 40;
        test_image[idx_top + 1] = 50;
        test_image[idx_top + 2] = 60;

        let sampled_top = sample_pixel_mirrored(&test_image, w, h, 10, -5);
        assert_eq!(sampled_top, [40, 50, 60]);

        let idx_bottom = ((h - 4) * w + 10) * 3;
        test_image[idx_bottom] = 70;
        test_image[idx_bottom + 1] = 80;
        test_image[idx_bottom + 2] = 90;

        let sampled_bottom = sample_pixel_mirrored(&test_image, w, h, 10, (h + 3) as i64);
        assert_eq!(sampled_bottom, [70, 80, 90]);
    }

    #[test]
    fn test_motion_blur_frame_blending_logic() {
        assert_eq!(compute_motion_blur_frames(1.0, true), 1);
        assert_eq!(compute_motion_blur_frames(0.5, true), 1);
        assert_eq!(compute_motion_blur_frames(3.2, true), 4);
        assert_eq!(compute_motion_blur_frames(2.1, true), 3);
        assert_eq!(compute_motion_blur_frames(5.0, true), 4);
        assert_eq!(compute_motion_blur_frames(3.2, false), 1);

        let f1 = vec![100u8, 100u8, 100u8];
        let f2 = vec![150u8, 150u8, 150u8];
        let f3 = vec![200u8, 200u8, 200u8];
        let f4 = vec![250u8, 250u8, 250u8];
        let frames: Vec<&[u8]> = vec![&f1, &f2, &f3, &f4];
        let mut out = vec![0u8; 3];
        blend_full_frames(&frames, &mut out);
        assert_eq!(out, vec![175, 175, 175]);
    }

    #[test]
    fn test_schema_v1_and_v2_parsing_and_retrocompat() {
        let v1_json = r#"{
            "schema_version": 1,
            "style": "HARD",
            "fps": 16,
            "aspect": { "w": 1080, "h": 1080 },
            "borderless": true,
            "bpm": 120.0,
            "target_duration": 10.0,
            "video_duration": 10.0,
            "audio_duration": 10.0,
            "loops": 0,
            "segments": [
                {
                    "t0": 0.0,
                    "t1": 10.0,
                    "s0": 0.0,
                    "s1": 10.0,
                    "curve": "snap"
                }
            ]
        }"#;

        let parsed_v1: ProjectPlan = serde_json::from_str(v1_json).expect("Schema v1 must parse cleanly");
        assert_eq!(parsed_v1.schema_version, 1);
        assert_eq!(parsed_v1.motion_blur, false);
        assert_eq!(parsed_v1.segments[0].effects.reverse, false);
        assert_eq!(parsed_v1.one_framers.len(), 0);

        let v2_plan = create_plan_internal("HARD", 16, &[1.0, 2.0], &[1.0], 5.0, 5.0, 1080, 1080, 120.0).unwrap();
        assert_eq!(v2_plan.schema_version, 2);
        assert_eq!(v2_plan.motion_blur, true);
        assert_eq!(v2_plan.segments[0].effects.shake.a0, 8.0);
        assert!(v2_plan.one_framers.len() > 0);

        let v2_serialized = serde_json::to_string(&v2_plan).unwrap();
        assert!(v2_serialized.contains("\"one_framers\":["));
        let v2_deserialized: ProjectPlan = serde_json::from_str(&v2_serialized).unwrap();
        assert_eq!(v2_deserialized.one_framers.len(), v2_plan.one_framers.len());
    }

    #[test]
    fn test_one_framers_library_diff() {
        let width = 64usize;
        let height = 64usize;
        let mut frame_in = vec![0u8; width * height * 3];
        for y in 0..height {
            for x in 0..width {
                let idx = (y * width + x) * 3;
                frame_in[idx] = ((x * 4) % 256) as u8;
                frame_in[idx + 1] = ((y * 4) % 256) as u8;
                frame_in[idx + 2] = (((x + y) * 2) % 256) as u8;
            }
        }

        for framer_type in ONE_FRAMER_TYPES {
            let mut frame_out = vec![0u8; width * height * 3];
            apply_one_framer(framer_type, &frame_in, &mut frame_out, width, height);

            let diff: i64 = frame_in
                .iter()
                .zip(frame_out.iter())
                .map(|(&a, &b)| (a as i64 - b as i64).abs())
                .sum();

            println!("One-Framer [{}] produced total pixel diff: {}", framer_type, diff);
            assert!(diff > 0, "One-framer {} must modify frame (diff > 0)", framer_type);
        }
    }

    #[test]
    fn test_one_framers_auto_placement() {
        let video_duration = 10.773;
        let audio_duration = 14.315;
        let beats = vec![
            0.42, 1.14, 1.88, 2.60, 3.32, 4.04, 4.76, 5.50, 6.20, 6.94, 7.64, 8.38, 9.10, 9.82,
            10.54, 11.26, 11.98, 12.70, 13.42, 14.14,
        ];
        let downbeats = vec![2.60, 5.50, 8.38, 11.26, 14.14];
        let fps = 16u32;
        let bpm = 83.33;

        let plan_hard = create_plan_internal(
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
        .unwrap();

        // 21 segments -> 21 cuts * 4 framers + 5 downbeats * 1 framer = 89 raw candidate framers
        let num_cuts = plan_hard.segments.len();
        let num_downbeats = downbeats.len();
        let total_raw_candidates = num_cuts * 4 + num_downbeats;
        println!(
            "HARD raw placement check: {} cuts * 4 + {} downbeats = {} candidates",
            num_cuts, num_downbeats, total_raw_candidates
        );
        assert_eq!(num_cuts, 21);
        assert_eq!(total_raw_candidates, 89);
        assert!(plan_hard.one_framers.len() > 50, "Valid deduped framers in [0, target] should be substantial");

        // Verify ordering
        for win in plan_hard.one_framers.windows(2) {
            assert!(win[0].t <= win[1].t, "one_framers list must be sorted ascending by t");
            assert!(win[0].t >= 0.0 && win[1].t <= audio_duration + 0.1);
        }

        // Test SMOOTH has 50% cuts
        let plan_smooth = create_plan_internal(
            "SMOOTH",
            fps,
            &beats,
            &downbeats,
            video_duration,
            audio_duration,
            1080,
            1080,
            bpm,
        )
        .unwrap();
        assert!(plan_smooth.one_framers.len() < plan_hard.one_framers.len());

        // Test HYBRID has all cuts + 50% downbeats
        let plan_hybrid = create_plan_internal(
            "HYBRID",
            fps,
            &beats,
            &downbeats,
            video_duration,
            audio_duration,
            1080,
            1080,
            bpm,
        )
        .unwrap();
        assert!(plan_hybrid.one_framers.len() > plan_smooth.one_framers.len());
    }

    #[test]
    fn test_one_framers_reproducibility() {
        let video_duration = 10.773;
        let audio_duration = 14.315;
        let beats = vec![
            0.42, 1.14, 1.88, 2.60, 3.32, 4.04, 4.76, 5.50, 6.20, 6.94, 7.64, 8.38, 9.10, 9.82,
            10.54, 11.26, 11.98, 12.70, 13.42, 14.14,
        ];
        let downbeats = vec![2.60, 5.50, 8.38, 11.26, 14.14];
        let fps = 16u32;
        let bpm = 83.33;

        let plan1 = create_plan_internal("HARD", fps, &beats, &downbeats, video_duration, audio_duration, 1080, 1080, bpm).unwrap();
        let plan2 = create_plan_internal("HARD", fps, &beats, &downbeats, video_duration, audio_duration, 1080, 1080, bpm).unwrap();

        assert_eq!(plan1.one_framers, plan2.one_framers);
        for (f1, f2) in plan1.one_framers.iter().zip(plan2.one_framers.iter()) {
            assert_eq!(f1.t, f2.t);
            assert_eq!(f1.framer_type, f2.framer_type);
        }
    }

    #[test]
    fn test_curves_monotonicity_and_bounds() {
        assert_eq!(evaluate_curve("snap", 0.0), 0.0);
        assert_eq!(evaluate_curve("snap", 1.0), 1.0);
        assert_eq!(evaluate_curve("saddle", 0.0), 0.0);
        assert_eq!(evaluate_curve("saddle", 1.0), 1.0);

        let steps = 1000;
        let mut prev_snap = -1.0;
        let mut prev_saddle = -1.0;

        for i in 0..=steps {
            let x = (i as f64) / (steps as f64);
            let y_snap = evaluate_curve("snap", x);
            let y_saddle = evaluate_curve("saddle", x);

            assert!(y_snap >= 0.0 && y_snap <= 1.0);
            assert!(y_saddle >= 0.0 && y_saddle <= 1.0);

            if i > 0 {
                assert!(y_snap > prev_snap);
                assert!(y_saddle >= prev_saddle);
            }
            prev_snap = y_snap;
            prev_saddle = y_saddle;
        }
    }

    #[test]
    fn test_crop_to_fill_maths() {
        let crop_1_1 = compute_crop_to_fill(1080, 1920, 1080, 1080);
        assert_eq!(crop_1_1.x, 0);
        assert_eq!(crop_1_1.y, 420);
        assert_eq!(crop_1_1.width, 1080);
        assert_eq!(crop_1_1.height, 1080);
        assert_eq!(crop_1_1.out_w, 1080);
        assert_eq!(crop_1_1.out_h, 1080);

        let crop_16_9 = compute_crop_to_fill(1080, 1920, 16, 9);
        assert_eq!(crop_16_9.x, 0);
        assert_eq!(crop_16_9.y, 656);
        assert_eq!(crop_16_9.width, 1080);
        assert_eq!(crop_16_9.height, 608);
        assert_eq!(crop_16_9.out_w, 1080);
        assert_eq!(crop_16_9.out_h, 608);

        let crop_9_16 = compute_crop_to_fill(1080, 1920, 9, 16);
        assert_eq!(crop_9_16.x, 0);
        assert_eq!(crop_9_16.y, 0);
        assert_eq!(crop_9_16.width, 1080);
        assert_eq!(crop_9_16.height, 1920);
        assert_eq!(crop_9_16.out_w, 608);
        assert_eq!(crop_9_16.out_h, 1080);
    }

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

            assert_eq!(plan.segments.first().unwrap().t0, 0.0);
            assert!((plan.segments.last().unwrap().t1 - audio_duration).abs() < 0.01);

            for win in plan.segments.windows(2) {
                assert!((win[0].t1 - win[1].t0).abs() < 1e-4);
            }

            for seg in &plan.segments {
                let s_min = seg.s0.min(seg.s1);
                let s_max = seg.s0.max(seg.s1);
                assert!(s_min >= 0.0);
                assert!(s_max > s_min);
                assert!(s_max <= video_duration + 1e-4);
                let dur = seg.t1 - seg.t0;
                assert!(dur >= min_seg_dur - 1e-4);
            }

            if style == "HARD" || style == "HYBRID" {
                assert!(plan.loops >= 1);
            }
        }
    }

    #[test]
    fn test_benchmark_full_effects_pipeline() {
        let video_path = r"C:\Users\cia\Downloads\jugg video & audio tester\snaptik_7674387013243538721_v3.mp4";
        let audio_path = r"C:\Users\cia\Downloads\jugg video & audio tester\curiosos.mp3";

        if !std::path::Path::new(video_path).exists() || !std::path::Path::new(audio_path).exists() {
            println!("Test files not found, skipping benchmark test.");
            return;
        }

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
            10.773,
            14.315,
            1080,
            1080,
            bpm,
        )
        .expect("Plan generation failed");

        let ffmpeg_bin = if let Ok(output) = std::process::Command::new("where.exe").arg("ffmpeg").output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            stdout.lines().next().map(|s| std::path::PathBuf::from(s.trim())).unwrap_or_default()
        } else {
            std::path::PathBuf::from("ffmpeg.exe")
        };

        let scene_info = probe_media(video_path.to_string()).unwrap();
        let src_w = scene_info.width;
        let src_h = scene_info.height;
        let src_fps = scene_info.fps;
        let frame_bytes = (src_w * src_h * 3) as usize;

        let temp_dir = std::env::temp_dir().join("cia_app_bench_t9");
        std::fs::create_dir_all(&temp_dir).unwrap();
        let raw_cache = temp_dir.join("test_frames.raw");

        let t_decode_start = std::time::Instant::now();
        // Decode cache
        let mut decode_cmd = std::process::Command::new(&ffmpeg_bin);
        decode_cmd.args([
            "-y",
            "-i",
            video_path,
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "-an",
            &raw_cache.to_string_lossy(),
        ]);
        let mut decode_proc = decode_cmd.spawn().expect("Failed to spawn decode");
        let status = decode_proc.wait().expect("Decode failed");
        assert!(status.success());
        let t_decode = t_decode_start.elapsed();

        let total_cached_bytes = std::fs::metadata(&raw_cache).unwrap().len();
        let total_source_frames = (total_cached_bytes / (frame_bytes as u64)) as usize;

        let crop = compute_crop_to_fill(src_w, src_h, plan.aspect.w, plan.aspect.h);
        let output_fps = plan.fps as f64;
        let total_output_frames = (plan.target_duration * output_fps).round() as usize;

        let mut raw_file = std::fs::File::open(&raw_cache).unwrap();
        let cropped_frame_bytes = (crop.width * crop.height * 3) as usize;

        // 1. Baseline T8 Effects Pipeline (Shakes 4-axis, Zoom Continuity, Reverse Remap, Mirror Edges)
        let t_t8_start = std::time::Instant::now();
        let mut t8_reader = CachedFrameReader::new(&mut raw_file, frame_bytes, 16);
        let mut sampled_full_frame = vec![0u8; frame_bytes];
        let mut blend_storage = vec![vec![0u8; frame_bytes]; 4];
        let mut t8_crop = vec![0u8; cropped_frame_bytes];

        for i in 0..total_output_frames {
            let t = (i as f64) / output_fps;
            let seg = plan.segments.iter().find(|s| t >= s.t0 && t <= s.t1).or_else(|| plan.segments.last()).unwrap();
            let seg_dur = (seg.t1 - seg.t0).max(1e-6);
            let t_rel = (t - seg.t0).max(0.0);
            let x = (t_rel / seg_dur).clamp(0.0, 1.0);
            let u = evaluate_curve(&seg.curve, x);
            let u_prime = evaluate_curve_derivative(&seg.curve, x);
            let speed_v = ((seg.s1 - seg.s0).abs() / seg_dur) * u_prime;
            let n_blur = compute_motion_blur_frames(speed_v, plan.motion_blur);

            let src_time = seg.s0 + (seg.s1 - seg.s0) * u;
            let mut base_src_frame = (src_time * src_fps).round() as i64;
            if base_src_frame < 0 { base_src_frame = 0; }
            if base_src_frame >= total_source_frames as i64 { base_src_frame = (total_source_frames - 1) as i64; }

            // Sample Full-Frame
            if n_blur <= 1 {
                t8_reader.get_frame(base_src_frame as u64, &mut sampled_full_frame).unwrap();
            } else {
                let mut slice_ptrs: Vec<&[u8]> = Vec::with_capacity(n_blur);
                for k in 0..n_blur {
                    let f_idx = (base_src_frame + (k as i64)).clamp(0, (total_source_frames - 1) as i64) as u64;
                    t8_reader.get_frame(f_idx, &mut blend_storage[k]).unwrap();
                }
                for k in 0..n_blur {
                    slice_ptrs.push(&blend_storage[k]);
                }
                blend_full_frames(&slice_ptrs, &mut sampled_full_frame);
            }

            // Transform Stack + Crop (Shakes + Zoom + Mirror Edges)
            let params = compute_transform_params(&seg.effects, t_rel, seg_dur, output_fps);
            apply_transform_stack_cropped(
                &sampled_full_frame,
                &mut t8_crop,
                src_w as usize,
                src_h as usize,
                crop.x,
                crop.y,
                crop.width,
                crop.height,
                params,
            );
        }
        let t_t8 = t_t8_start.elapsed();

        // 2. T9 Full Pipeline (T8 Effects + One-Framers Library)
        let t_t9_start = std::time::Instant::now();
        let mut t9_reader = CachedFrameReader::new(&mut raw_file, frame_bytes, 16);
        let mut one_framer_buf = vec![0u8; frame_bytes];
        let mut t9_crop = vec![0u8; cropped_frame_bytes];

        for i in 0..total_output_frames {
            let t = (i as f64) / output_fps;
            let seg = plan.segments.iter().find(|s| t >= s.t0 && t <= s.t1).or_else(|| plan.segments.last()).unwrap();
            let seg_dur = (seg.t1 - seg.t0).max(1e-6);
            let t_rel = (t - seg.t0).max(0.0);
            let x = (t_rel / seg_dur).clamp(0.0, 1.0);
            let u = evaluate_curve(&seg.curve, x);
            let u_prime = evaluate_curve_derivative(&seg.curve, x);
            let speed_v = ((seg.s1 - seg.s0).abs() / seg_dur) * u_prime;
            let n_blur = compute_motion_blur_frames(speed_v, plan.motion_blur);

            let src_time = seg.s0 + (seg.s1 - seg.s0) * u;
            let mut base_src_frame = (src_time * src_fps).round() as i64;
            if base_src_frame < 0 { base_src_frame = 0; }
            if base_src_frame >= total_source_frames as i64 { base_src_frame = (total_source_frames - 1) as i64; }

            // Sample Full-Frame
            if n_blur <= 1 {
                t9_reader.get_frame(base_src_frame as u64, &mut sampled_full_frame).unwrap();
            } else {
                let mut slice_ptrs: Vec<&[u8]> = Vec::with_capacity(n_blur);
                for k in 0..n_blur {
                    let f_idx = (base_src_frame + (k as i64)).clamp(0, (total_source_frames - 1) as i64) as u64;
                    t9_reader.get_frame(f_idx, &mut blend_storage[k]).unwrap();
                }
                for k in 0..n_blur {
                    slice_ptrs.push(&blend_storage[k]);
                }
                blend_full_frames(&slice_ptrs, &mut sampled_full_frame);
            }

            // One-Framers Library effect
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

            // Transform Stack + Crop (Shakes + Zoom + Mirror Edges)
            let params = compute_transform_params(&seg.effects, t_rel, seg_dur, output_fps);
            apply_transform_stack_cropped(
                full_frame_ptr,
                &mut t9_crop,
                src_w as usize,
                src_h as usize,
                crop.x,
                crop.y,
                crop.width,
                crop.height,
                params,
            );
        }
        let t_t9 = t_t9_start.elapsed();

        let t_t8_total = t_decode + t_t8;
        let t_t9_total = t_decode + t_t9;
        let ratio = (t_t9_total.as_secs_f64() / t_t8_total.as_secs_f64()).max(0.01);

        println!("=== T9 ONE-FRAMERS BENCHMARK REPORT ===");
        println!("Total frames rendered: {}", total_output_frames);
        println!("Decode time: {:.3}s", t_decode.as_secs_f64());
        println!("T8 total render pipeline time: {:.3}s", t_t8_total.as_secs_f64());
        println!("T9 Full Effects + One-Framers total render pipeline time: {:.3}s", t_t9_total.as_secs_f64());
        println!("Performance ratio (T9 / T8): {:.3}x", ratio);
        println!("========================================");

        assert!(
            ratio < 1.5,
            "Benchmark check failed: ratio was {:.3}x (expected < 1.5x)",
            ratio
        );
    }
}
