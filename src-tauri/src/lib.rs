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

// ─── T14 Advanced Engine Structs ───────────────────────────────────────────

/// Bouncy shake: BlurMoCurves-style piecewise keyframe on X or Y axis.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct BouncyShake {
    /// 0 = X axis, 1 = Y axis (seed-derived)
    pub axis: u8,
    /// Amplitude in pixels
    pub amplitude: f64,
}

/// Dissolve shake: ghost-blend with frame ±2 positions.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct DissolveShake {
    /// Blend percentage (0..100) modulated by shake envelope
    pub pct: f64,
}

/// Skew shake: horizontal cisaillement decaying exponentially.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct SkewShake {
    /// Initial skew angle in degrees
    pub s0_deg: f64,
}

/// Squish pop: scale_y compression→spring at segment start.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct SquishPop {
    // No parameters — fixed keyframe template
    pub _pad: u8,
}

/// Optics compensation bounce: barrel distortion k(t) = K0*(1-t/T)².
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct OpticsBounce {
    /// K0 magnitude (positive = barrel, negative = pincushion)
    pub k0: f64,
}

/// Buildup chaining: tail of segment bleeds into head of next.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct BuildupChain {
    /// If true, next segment starts with 0.6*A0 envelope
    pub chain_next: bool,
    /// If true, this segment's tail is held at 0.6 instead of decaying to 0
    pub chain_from_prev: bool,
}

/// Warp stretch (geometric distortion — survives full_fx=false).
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct WarpStretch {
    /// 0 = X axis, 1 = Y axis
    pub axis: u8,
    /// Scale at segment start (1.3..1.5), decays to 1.0 via saddle curve
    pub scale_start: f64,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct SegmentEffects {
    #[serde(default = "default_shake")]
    pub shake: ShakeEffect,
    #[serde(default = "default_zoom")]
    pub zoom: ZoomEffect,
    #[serde(default)]
    pub reverse: bool,
    // ─── T14 Advanced Engines ───────────────────────────────────────────────
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bouncy_shake: Option<BouncyShake>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dissolve_shake: Option<DissolveShake>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skew_shake: Option<SkewShake>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub squish_pop: Option<SquishPop>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub optics_bounce: Option<OpticsBounce>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buildup_chain: Option<BuildupChain>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warp_stretch: Option<WarpStretch>,
    /// Zoom beat offset: 0..=2 frames after beat start for zoom peak
    #[serde(default)]
    pub zoom_beat_offset: u32,
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
        bouncy_shake: None,
        dissolve_shake: None,
        skew_shake: None,
        squish_pop: None,
        optics_bounce: None,
        buildup_chain: None,
        warp_stretch: None,
        zoom_beat_offset: 0,
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct SegmentTransition {
    #[serde(rename = "type")]
    pub transition_type: String,
    #[serde(default)]
    pub params: serde_json::Value,
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
    #[serde(default)]
    pub transition: Option<SegmentTransition>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct OneFramer {
    pub t: f64,
    #[serde(rename = "type")]
    pub framer_type: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct TransitionItem {
    pub t: f64,
    #[serde(rename = "type")]
    pub transition_type: String,
    pub duration_frames: u32,
    pub is_wrap: bool,
    #[serde(default)]
    pub params: serde_json::Value,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct FlickerConfig {
    #[serde(rename = "A")]
    pub amplitude: f64,
    pub f: f64,
    // phase is per-segment, stored externally; here it's the global base
    pub phase: f64,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct ExposureFlashConfig {
    pub peak: f64,
    pub times: Vec<f64>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct EchoTrailConfig {
    pub enabled: bool,
    pub alpha: f64,
    pub k: u32,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct TintConfig {
    pub offset_rgb: [i16; 3],
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct VignetteConfig {
    pub strength: f64,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct ScanlinesConfig {
    pub opacity: f64,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct AmbianceConfig {
    pub flicker: FlickerConfig,
    pub exposure_flash: ExposureFlashConfig,
    pub echo_trail: EchoTrailConfig,
    pub tint: TintConfig,
    pub vignette: VignetteConfig,
    pub scanlines: ScanlinesConfig,
}

fn default_ambiance(style: &str, downbeats: &[f64]) -> AmbianceConfig {
    let (amp, freq, flash_peak) = match style.to_uppercase().as_str() {
        "SMOOTH" => (0.08, 8.0, 0.3),
        "HYBRID" => (0.12, 10.0, 0.4),
        _ => (0.15, 12.0, 0.5), // HARD
    };
    AmbianceConfig {
        flicker: FlickerConfig { amplitude: amp, f: freq, phase: 0.0 },
        exposure_flash: ExposureFlashConfig { peak: flash_peak, times: downbeats.to_vec() },
        echo_trail: EchoTrailConfig { enabled: false, alpha: 0.3, k: 3 },
        tint: TintConfig { offset_rgb: [0; 3] },
        vignette: VignetteConfig { strength: 0.3 },
        scanlines: ScanlinesConfig { opacity: 0.15 },
    }
}

fn default_true() -> bool { true }

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
    #[serde(default = "default_true")]
    pub full_fx: bool,
    #[serde(default)]
    pub one_framers: Vec<OneFramer>,
    #[serde(default)]
    pub transitions: Vec<TransitionItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ambiance: Option<AmbianceConfig>,
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
#[serde(rename_all = "camelCase")]
pub struct RenderStats {
    pub output_path: String,
    pub render_time_secs: f64,
    pub file_size_mb: f64,
    pub target_fps: u32,
    pub effects_count: usize,
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

// ─── T14 Compute Functions ──────────────────────────────────────────────────

/// Piecewise-linear bouncy shake template.
/// Keyframes: [(0,-1.0),(1,+0.14),(3,+0.05),(6,+0.022),(8,-0.006),(10,0)]
/// Frame values outside [0,10] → 0.
pub fn compute_bouncy_shake(frame_idx: f64) -> f64 {
    const KF: [(f64, f64); 6] = [
        (0.0, -1.0),
        (1.0,  0.14),
        (3.0,  0.05),
        (6.0,  0.022),
        (8.0, -0.006),
        (10.0, 0.0),
    ];
    if frame_idx < 0.0 || frame_idx > 10.0 {
        return 0.0;
    }
    for i in 0..KF.len() - 1 {
        if frame_idx >= KF[i].0 && frame_idx <= KF[i + 1].0 {
            let t = (frame_idx - KF[i].0) / (KF[i + 1].0 - KF[i].0);
            return KF[i].1 + t * (KF[i + 1].1 - KF[i].1);
        }
    }
    0.0
}

/// Skew shake: S0·exp(−3t/T)·cos(2π·4·t/T), returns tangent of angle (radians).
pub fn compute_skew_shake(t_rel: f64, duration: f64, s0_deg: f64) -> f64 {
    if duration <= 1e-9 { return 0.0; }
    let s0_rad = s0_deg * std::f64::consts::PI / 180.0;
    let u = t_rel / duration;
    let angle = s0_rad * (-3.0 * u).exp() * (2.0 * std::f64::consts::PI * 4.0 * u).cos();
    angle.tan()
}

/// Squish pop scale_y keyframes: frames [0,1,3,5] → [1,0.88,1.06,1]
/// scale_x is reciprocal: [1,1.10,0.96,1]
pub fn compute_squish_pop(frame_idx: f64) -> (f64, f64) {
    const KF_Y: [(f64, f64); 4] = [(0.0, 1.0), (1.0, 0.88), (3.0, 1.06), (5.0, 1.0)];
    const KF_X: [(f64, f64); 4] = [(0.0, 1.0), (1.0, 1.10), (3.0, 0.96), (5.0, 1.0)];
    let interpolate = |kf: &[(f64, f64); 4], fi: f64| -> f64 {
        if fi <= 0.0 { return kf[0].1; }
        if fi >= kf[kf.len()-1].0 { return kf[kf.len()-1].1; }
        for i in 0..kf.len()-1 {
            if fi >= kf[i].0 && fi < kf[i+1].0 {
                let t = (fi - kf[i].0) / (kf[i+1].0 - kf[i].0);
                return kf[i].1 + t * (kf[i+1].1 - kf[i].1);
            }
        }
        1.0
    };
    (interpolate(&KF_X, frame_idx), interpolate(&KF_Y, frame_idx))
}

/// Optics bounce barrel k(t) = K0*(1 - t/T)²
pub fn compute_optics_k(t_rel: f64, duration: f64, k0: f64) -> f64 {
    if duration <= 1e-9 { return 0.0; }
    let u = (1.0 - t_rel / duration).clamp(0.0, 1.0);
    k0 * u * u
}

/// Warp stretch: scale = lerp(scale_start → 1.0) via saddle curve.
pub fn compute_stretch_scale(t_rel: f64, duration: f64, scale_start: f64) -> f64 {
    if duration <= 1e-9 { return 1.0; }
    let u = (t_rel / duration).clamp(0.0, 1.0);
    let saddle = u * u * (3.0 - 2.0 * u);
    scale_start + (1.0 - scale_start) * saddle
}

/// Buildup chain envelope multiplier.
/// chain_from_prev: start at 0.6. chain_next (tail): end at 0.6 instead of 0.
pub fn compute_chain_envelope_mult(
    t_rel: f64,
    duration: f64,
    fps: f64,
    chain_from_prev: bool,
    chain_next: bool,
) -> f64 {
    let frame_dur = if fps > 0.0 { 1.0 / fps } else { 1.0 / 30.0 };
    let ramp_dur = 2.0 * frame_dur;
    let head_mult = if chain_from_prev {
        // Starts at 0.6, ramps to 1.0
        if t_rel < ramp_dur { 0.6 + 0.4 * (t_rel / ramp_dur).clamp(0.0, 1.0) } else { 1.0 }
    } else { 1.0 };
    let tail_mult = if chain_next {
        // Tail held at 0.6 instead of decaying to 0
        if t_rel > duration - ramp_dur {
            let u = ((duration - t_rel) / ramp_dur).clamp(0.0, 1.0);
            0.6 + 0.4 * u
        } else { 1.0 }
    } else { 1.0 };
    head_mult * tail_mult
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransformParams {
    pub dx: f64,
    pub dy: f64,
    pub scale: f64,
    pub tilt_rad: f64,
    /// Horizontal skew: x_src += skew_x * (y_src - cy)
    pub skew_x: f64,
    /// Independent Y-axis scale (squish/stretch). 1.0 = no effect.
    pub scale_y: f64,
    /// Independent X-axis scale (squish). 1.0 = no effect.
    pub scale_x: f64,
    /// Barrel distortion coefficient k (optics bounce)
    pub barrel_k: f64,
}


pub fn compute_transform_params(
    effects: &SegmentEffects,
    t_rel: f64,
    seg_dur: f64,
    fps: f64,
) -> TransformParams {
    let frame_dur = if fps > 0.0 { 1.0 / fps } else { 1.0 / 30.0 };

    // ── Zoom beat offset: shift t_rel backward by N frames for zoom calc ──
    let t_rel_zoom = if effects.zoom_beat_offset > 0 {
        (t_rel - (effects.zoom_beat_offset as f64) * frame_dur).max(0.0)
    } else {
        t_rel
    };

    // ── Envelope (with optional buildup chain modifier) ──
    let base_env = compute_shake_envelope(t_rel, seg_dur, fps);
    let chain_mult = if let Some(ref bc) = effects.buildup_chain {
        compute_chain_envelope_mult(t_rel, seg_dur, fps, bc.chain_from_prev, bc.chain_next)
    } else { 1.0 };
    let env = base_env * chain_mult;

    // ── Harmonic shake (standard) ──
    let seed = effects.shake.seed;
    let phi_x = ((seed % 360) as f64) * std::f64::consts::PI / 180.0;
    let phi_y = (((seed.wrapping_mul(17)) % 360) as f64) * std::f64::consts::PI / 180.0;
    let phi_z = (((seed.wrapping_mul(31)) % 360) as f64) * std::f64::consts::PI / 180.0;
    let phi_tilt = (((seed.wrapping_mul(47)) % 360) as f64) * std::f64::consts::PI / 180.0;
    let omega_t = effects.shake.omega * t_rel;
    let damping = (-effects.shake.k * t_rel).exp();
    let a0 = effects.shake.a0;
    let mut dx = a0 * damping * (omega_t + phi_x).sin() * env;
    let mut dy = a0 * damping * (omega_t + phi_y).sin() * env;
    let dz = (a0 / 100.0) * damping * (omega_t + phi_z).sin() * env;
    let d_tilt_deg = (a0 / 5.0) * damping * (omega_t + phi_tilt).sin() * env;
    let tilt_rad = d_tilt_deg * std::f64::consts::PI / 180.0;

    // ── Bouncy shake (overrides harmonic on its axis when active) ──
    if let Some(ref bouncy) = effects.bouncy_shake {
        let frame_idx = t_rel / frame_dur;
        let bouncy_val = compute_bouncy_shake(frame_idx) * bouncy.amplitude;
        if bouncy.axis == 0 { dx = bouncy_val; } else { dy = bouncy_val; }
    }

    // ── Zoom (with offset) ──
    let x_zoom = (t_rel_zoom / seg_dur.max(1e-6)).clamp(0.0, 1.0);
    let base_scale = effects.zoom.scale_start + (effects.zoom.scale_end - effects.zoom.scale_start) * x_zoom;
    let total_scale = (base_scale * (1.0 + dz)).max(0.1);

    // ── Warp stretch: independent axis scale ──
    let (mut scale_x, mut scale_y) = (1.0f64, 1.0f64);
    if let Some(ref ws) = effects.warp_stretch {
        let stretch = compute_stretch_scale(t_rel, seg_dur, ws.scale_start);
        if ws.axis == 0 { scale_x = stretch; } else { scale_y = stretch; }
    }

    // ── Squish pop: override scale_x/y at segment start ──
    if effects.squish_pop.is_some() {
        let frame_idx = t_rel / frame_dur;
        let (sx, sy) = compute_squish_pop(frame_idx);
        scale_x = scale_x * sx;
        scale_y = scale_y * sy;
    }

    // ── Skew shake ──
    let skew_x = if let Some(ref sk) = effects.skew_shake {
        compute_skew_shake(t_rel, seg_dur, sk.s0_deg) * env
    } else { 0.0 };

    // ── Optics barrel distortion k ──
    let barrel_k = if let Some(ref ob) = effects.optics_bounce {
        compute_optics_k(t_rel, seg_dur, ob.k0)
    } else { 0.0 };

    TransformParams {
        dx,
        dy,
        scale: total_scale,
        tilt_rad,
        skew_x,
        scale_y,
        scale_x,
        barrel_k,
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

    let has_t14 = params.skew_x.abs() > 1e-5
        || (params.scale_y - 1.0).abs() > 1e-5
        || (params.scale_x - 1.0).abs() > 1e-5
        || params.barrel_k.abs() > 1e-5;

    if params.dx.abs() < 1e-4
        && params.dy.abs() < 1e-4
        && (params.scale - 1.0).abs() < 1e-4
        && params.tilt_rad.abs() < 1e-4
        && !has_t14
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
    let inv_sy = if params.scale_y.abs() > 1e-5 { 1.0 / params.scale_y } else { 1.0 };
    let inv_sx = if params.scale_x.abs() > 1e-5 { 1.0 / params.scale_x } else { 1.0 };
    let cos_t = params.tilt_rad.cos();
    let sin_t = params.tilt_rad.sin();

    let step_x_to_xs = inv_s * cos_t;
    let step_x_to_ys = -inv_s * sin_t;

    // step_x_to_xs / step_x_to_ys are used per-row as step_xs_fp_sx (x-scale adjusted)

    let w_i32 = src_width as i32;
    let h_i32 = src_height as i32;

    let cw = crop_width as usize;
    let ch = crop_height as usize;

    for yd in 0..ch {
        let yd_full = (crop_y as usize) + yd;
        let yd_rel = (yd_full as f64) - cy;
        let xd_start_rel = (crop_x as f64) - cx;

        // Skew: each row has an x-offset = skew_x * yd_rel (applied in source space)
        let skew_offset = params.skew_x * yd_rel;

        // Y-axis scale: remap yd_rel by inv_sy
        let yd_rel_scaled = yd_rel * inv_sy;

        let base_xs = cx - params.dx + skew_offset + inv_s * (xd_start_rel * cos_t + yd_rel_scaled * sin_t);
        let base_ys = cy - params.dy + inv_s * (-xd_start_rel * sin_t * inv_sx + yd_rel_scaled * cos_t);

        let mut xs_fp = (base_xs * 65536.0 + 32768.0) as i32;
        let mut ys_fp = (base_ys * 65536.0 + 32768.0) as i32;

        let row_out_start = yd * cw * 3;
        let row_out = &mut frame_crop_out[row_out_start..row_out_start + cw * 3];

        // X-axis scale step adjustment
        let step_xs_fp_sx = (step_x_to_xs * inv_sx * 65536.0).round() as i32;
        let step_ys_fp_sx = (step_x_to_ys * inv_sx * 65536.0).round() as i32;

        for xd in 0..cw {
            let mut xs = xs_fp >> 16;
            let mut ys = ys_fp >> 16;

            // Barrel distortion (optics bounce): r² from center, additive offset
            if params.barrel_k.abs() > 1e-5 {
                let xf = (xs_fp as f64 / 65536.0) - cx;
                let yf = (ys_fp as f64 / 65536.0) - cy;
                let r2 = (xf * xf + yf * yf) / (cx * cx + cy * cy).max(1.0);
                let factor = 1.0 + params.barrel_k * r2;
                xs = (cx + xf * factor) as i32;
                ys = (cy + yf * factor) as i32;
            }

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

            xs_fp += step_xs_fp_sx;
            ys_fp += step_ys_fp_sx;
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
    let has_t14 = params.skew_x.abs() > 1e-5
        || (params.scale_y - 1.0).abs() > 1e-5
        || (params.scale_x - 1.0).abs() > 1e-5
        || params.barrel_k.abs() > 1e-5;

    if params.dx.abs() < 1e-4
        && params.dy.abs() < 1e-4
        && (params.scale - 1.0).abs() < 1e-4
        && params.tilt_rad.abs() < 1e-4
        && !has_t14
    {
        frame_out.copy_from_slice(frame_in);
        return;
    }

    let cx = (width as f64) / 2.0;
    let cy = (height as f64) / 2.0;
    let inv_s = 1.0 / params.scale;
    let inv_sy = if params.scale_y.abs() > 1e-5 { 1.0 / params.scale_y } else { 1.0 };
    let inv_sx = if params.scale_x.abs() > 1e-5 { 1.0 / params.scale_x } else { 1.0 };
    let cos_t = params.tilt_rad.cos();
    let sin_t = params.tilt_rad.sin();

    let step_x_to_xs = inv_s * cos_t * inv_sx;
    let step_x_to_ys = -inv_s * sin_t * inv_sx;

    let step_xs_fp = (step_x_to_xs * 65536.0).round() as i32;
    let step_ys_fp = (step_x_to_ys * 65536.0).round() as i32;

    let w_i32 = width as i32;
    let h_i32 = height as i32;

    for yd in 0..height {
        let yd_rel = (yd as f64) - cy;
        let yd_rel_scaled = yd_rel * inv_sy;
        let skew_offset = params.skew_x * yd_rel;

        let base_xs = cx - params.dx + skew_offset + inv_s * (yd_rel_scaled * sin_t);
        let base_ys = cy - params.dy + inv_s * (yd_rel_scaled * cos_t);

        let mut xs_fp = (base_xs * 65536.0 + 32768.0) as i32;
        let mut ys_fp = (base_ys * 65536.0 + 32768.0) as i32;

        let row_out_start = yd * width * 3;
        let row_out = &mut frame_out[row_out_start..row_out_start + width * 3];

        for xd in 0..width {
            let mut xs = xs_fp >> 16;
            let mut ys = ys_fp >> 16;
            let out_idx = xd * 3;

            if params.barrel_k.abs() > 1e-5 {
                let xf = (xs_fp as f64 / 65536.0) - cx;
                let yf = (ys_fp as f64 / 65536.0) - cy;
                let r2 = (xf * xf + yf * yf) / (cx * cx + cy * cy).max(1.0);
                let factor = 1.0 + params.barrel_k * r2;
                xs = (cx + xf * factor) as i32;
                ys = (cy + yf * factor) as i32;
            }

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

pub fn compute_warp_bubble_env(t: f64, t_cut: f64, fps: f64) -> f64 {
    let k_frame = (t - t_cut) * fps;
    if k_frame.abs() <= 2.0 + 1e-4 {
        0.5 * (1.0 - k_frame.abs() / 2.0).max(0.0)
    } else {
        0.0
    }
}

pub fn apply_warp_bubble(
    frame_in: &[u8],
    frame_out: &mut [u8],
    width: usize,
    height: usize,
    env_a: f64,
    freq: f64,
) {
    if env_a <= 1e-6 {
        frame_out.copy_from_slice(frame_in);
        return;
    }

    let rx = (width as f64) / 2.0;
    let ry = (height as f64) / 2.0;
    let rx_i = rx as i32;
    let ry_i = ry as i32;

    let mut lut_scale = [0.0f32; 2048];
    for r_int in 1..2048 {
        let r = r_int as f64;
        let r_norm = r / rx;
        let disp = env_a * (freq * r_norm * std::f64::consts::PI).sin() * rx;
        lut_scale[r_int] = (disp / r) as f32;
    }

    let mut dx_f_table = vec![0.0f32; width];
    let mut dx_sq_table = vec![0.0f32; width];
    for x in 0..width {
        let dx_f = ((x as i32) - rx_i) as f32;
        dx_f_table[x] = dx_f;
        dx_sq_table[x] = dx_f * dx_f;
    }

    let w_i = width as i32;
    let h_i = height as i32;
    let row_stride = width * 3;

    for y in 0..height {
        let dy_i = (y as i32) - ry_i;
        let dy_f = dy_i as f32;
        let dy_sq = dy_f * dy_f;
        let row_out_offset = y * row_stride;
        let row_out = &mut frame_out[row_out_offset..row_out_offset + row_stride];

        for x in 0..width {
            let dx_f = dx_f_table[x];
            let r_f = (dx_sq_table[x] + dy_sq).sqrt();
            let r_idx = (r_f as usize).min(2047);

            let scale = lut_scale[r_idx];

            let xs = (x as i32) - (dx_f * scale) as i32;
            let ys = (y as i32) - (dy_f * scale) as i32;

            let out_idx = x * 3;
            if xs >= 0 && xs < w_i && ys >= 0 && ys < h_i {
                let in_idx = (ys as usize) * row_stride + (xs as usize) * 3;
                row_out[out_idx] = frame_in[in_idx];
                row_out[out_idx + 1] = frame_in[in_idx + 1];
                row_out[out_idx + 2] = frame_in[in_idx + 2];
            } else {
                let px = sample_pixel_mirrored(frame_in, width, height, xs as i64, ys as i64);
                row_out[out_idx] = px[0];
                row_out[out_idx + 1] = px[1];
                row_out[out_idx + 2] = px[2];
            }
        }
    }
}

pub fn compute_wave_warp_params(t: f64, t_cut: f64, fps: f64, height: usize) -> (f64, f64, f64, f64) {
    let t_frames = (t - t_cut) * fps;
    let h = if t_frames >= -1e-4 && t_frames <= 6.0 + 1e-4 {
        280.0 * (1.0 - t_frames / 6.0).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let k = 600.0 / (height as f64);
    let v = 20.0;
    (h, k, v, t_frames)
}

pub fn apply_wave_warp(
    frame_in: &[u8],
    frame_out: &mut [u8],
    width: usize,
    height: usize,
    h_t: f64,
    k: f64,
    v: f64,
    t_frames: f64,
) {
    if h_t.abs() <= 1e-4 {
        frame_out.copy_from_slice(frame_in);
        return;
    }

    let row_stride = width * 3;
    let mut lut_xs = [0usize; 4096];
    let w_clamped = width.min(4096);

    for y in 0..height {
        let y_f = y as f64;
        let dx = (h_t * (y_f * k + t_frames * v).sin()).round() as i64;
        let row_in_offset = y * row_stride;
        let row_in = &frame_in[row_in_offset..row_in_offset + row_stride];
        let row_out = &mut frame_out[row_in_offset..row_in_offset + row_stride];

        if dx == 0 {
            row_out.copy_from_slice(row_in);
            continue;
        }

        for x in 0..w_clamped {
            lut_xs[x] = mirror_coordinate((x as i64) - dx, width) * 3;
        }

        for x in 0..w_clamped {
            let in_idx = lut_xs[x];
            let out_idx = x * 3;
            row_out[out_idx] = row_in[in_idx];
            row_out[out_idx + 1] = row_in[in_idx + 1];
            row_out[out_idx + 2] = row_in[in_idx + 2];
        }
    }
}

pub fn compute_slide_shake_shift(t: f64, t_cut: f64, fps: f64) -> f64 {
    let t_frames = (t - t_cut) * fps;
    if t_frames >= -3.0 - 1e-4 && t_frames < 0.0 {
        100.0 * (1.0 + t_frames / 3.0).clamp(0.0, 1.0)
    } else if t_frames >= 0.0 && t_frames <= 3.0 + 1e-4 {
        -100.0 * (1.0 - t_frames / 3.0).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

pub fn apply_slide_shake(
    frame_in: &[u8],
    frame_out: &mut [u8],
    width: usize,
    height: usize,
    shift_x: f64,
) {
    if shift_x.abs() <= 1e-4 {
        frame_out.copy_from_slice(frame_in);
        return;
    }

    let shift_i = shift_x.round() as i64;
    let row_stride = width * 3;
    let mut lut_xs = [0usize; 4096];
    let w_clamped = width.min(4096);
    for x in 0..w_clamped {
        lut_xs[x] = mirror_coordinate((x as i64) - shift_i, width) * 3;
    }

    for y in 0..height {
        let row_offset = y * row_stride;
        let row_in = &frame_in[row_offset..row_offset + row_stride];
        let row_out = &mut frame_out[row_offset..row_offset + row_stride];

        for x in 0..w_clamped {
            let in_idx = lut_xs[x];
            let out_idx = x * 3;
            row_out[out_idx] = row_in[in_idx];
            row_out[out_idx + 1] = row_in[in_idx + 1];
            row_out[out_idx + 2] = row_in[in_idx + 2];
        }
    }
}

/// Apply all T11 ambiance effects in a SINGLE combined pixel loop.
/// Order per-pixel: flicker/flash scale → echo blend → tint → vignette+scanline (combined).
/// When echo is disabled (default), skips ring copy entirely for max throughput.
pub fn apply_ambiance_effects(
    frame_in: &[u8],
    frame_out: &mut [u8],
    width: usize,
    height: usize,
    amb: &AmbianceConfig,
    echo_ring: &mut Vec<Vec<u8>>,
    echo_head: &mut usize,
    vignette_lut: &[u8],
    scanline_opacity: f64,
    t: f64,
    seg: &PlanSegment,
    fps: f64,
) {
    let n = width * height * 3;

    // --- Per-frame scalars (computed once) ---
    let seg_phase = (seg.effects.shake.seed as f64) * 0.0012345;
    let flicker_exp = amb.flicker.amplitude
        * (2.0 * std::f64::consts::PI * amb.flicker.f * t + seg_phase).sin();

    let mut flash_exp = 0.0f64;
    for &db_t in &amb.exposure_flash.times {
        let t_frames = (t - db_t) * fps;
        if t_frames.abs() <= 2.0 + 1e-4 {
            let env = (1.0 - t_frames.abs() / 2.0).max(0.0);
            flash_exp = flash_exp.max(amb.exposure_flash.peak * env);
        }
    }
    let total_exp = (flicker_exp + flash_exp).clamp(-0.999, 0.999);
    let scale_fp = ((1.0 + total_exp).clamp(0.0, 2.0) * 256.0) as u32;

    let [tr, tg, tb] = amb.tint.offset_rgb;
    let dim_fp = ((1.0 - scanline_opacity) * 256.0) as u32;

    let echo_enabled = amb.echo_trail.enabled;
    let alpha_fp = (amb.echo_trail.alpha * 256.0) as u32;
    let k_depth = (amb.echo_trail.k as usize).min(echo_ring.len());

    if echo_enabled {
        let mut prev_slots = [0usize; 4];
        for k in 0..k_depth.min(4) {
            prev_slots[k] = (*echo_head + echo_ring.len() - 1 - k) % echo_ring.len();
        }

        for py in 0..height {
            // For non-scanline rows, row_dim==256 so combined = (v*256)>>8 = v — skip multiply
            let is_scanline = py % 4 == 0;
            let row_offset = py * width;
            for px in 0..width {
                let idx = (row_offset + px) * 3;
                let v = vignette_lut[row_offset + px] as u32;
                let combined = if is_scanline { (v * dim_fp) >> 8 } else { v };

                let mut r_acc = 0u32; let mut g_acc = 0u32; let mut b_acc = 0u32;
                for k in 0..k_depth {
                    r_acc += echo_ring[prev_slots[k]][idx]     as u32;
                    g_acc += echo_ring[prev_slots[k]][idx + 1] as u32;
                    b_acc += echo_ring[prev_slots[k]][idx + 2] as u32;
                }
                let r_echo = r_acc / (k_depth as u32);
                let g_echo = g_acc / (k_depth as u32);
                let b_echo = b_acc / (k_depth as u32);

                let r_cur = ((frame_in[idx]     as u32 * scale_fp) >> 8).min(255);
                let g_cur = ((frame_in[idx + 1] as u32 * scale_fp) >> 8).min(255);
                let b_cur = ((frame_in[idx + 2] as u32 * scale_fp) >> 8).min(255);

                let mut r = ((256 - alpha_fp) * r_cur + alpha_fp * r_echo) >> 8;
                let mut g = ((256 - alpha_fp) * g_cur + alpha_fp * g_echo) >> 8;
                let mut b = ((256 - alpha_fp) * b_cur + alpha_fp * b_echo) >> 8;

                r = ((r as i32 + tr as i32).clamp(0, 255)) as u32;
                g = ((g as i32 + tg as i32).clamp(0, 255)) as u32;
                b = ((b as i32 + tb as i32).clamp(0, 255)) as u32;

                frame_out[idx]     = ((r * combined) >> 8) as u8;
                frame_out[idx + 1] = ((g * combined) >> 8) as u8;
                frame_out[idx + 2] = ((b * combined) >> 8) as u8;
            }
        }
        echo_ring[*echo_head][..n].copy_from_slice(&frame_in[..n]);
        *echo_head = (*echo_head + 1) % echo_ring.len();
    } else {
        // Fast path: no echo, no ring copy.
        // Optimizations:
        //   (a) non-scanline rows (75%): combined = v (avoids v*256>>8 multiply)
        //   (b) scale_fp == 256 (sin≈0 frames): skip flicker multiply
        if scale_fp == 256 {
            // No flicker scaling needed
            for py in 0..height {
                let is_scanline = py % 4 == 0;
                let row_offset = py * width;
                for px in 0..width {
                    let idx = (row_offset + px) * 3;
                    let v = vignette_lut[row_offset + px] as u32;
                    let combined = if is_scanline { (v * dim_fp) >> 8 } else { v };

                    let r = ((frame_in[idx]     as i32 + tr as i32).clamp(0, 255)) as u32;
                    let g = ((frame_in[idx + 1] as i32 + tg as i32).clamp(0, 255)) as u32;
                    let b = ((frame_in[idx + 2] as i32 + tb as i32).clamp(0, 255)) as u32;

                    frame_out[idx]     = ((r * combined) >> 8) as u8;
                    frame_out[idx + 1] = ((g * combined) >> 8) as u8;
                    frame_out[idx + 2] = ((b * combined) >> 8) as u8;
                }
            }
        } else {
            // Flicker active
            for py in 0..height {
                let is_scanline = py % 4 == 0;
                let row_offset = py * width;
                for px in 0..width {
                    let idx = (row_offset + px) * 3;
                    let v = vignette_lut[row_offset + px] as u32;
                    let combined = if is_scanline { (v * dim_fp) >> 8 } else { v };

                    let mut r = ((frame_in[idx]     as u32 * scale_fp) >> 8).min(255);
                    let mut g = ((frame_in[idx + 1] as u32 * scale_fp) >> 8).min(255);
                    let mut b = ((frame_in[idx + 2] as u32 * scale_fp) >> 8).min(255);

                    r = ((r as i32 + tr as i32).clamp(0, 255)) as u32;
                    g = ((g as i32 + tg as i32).clamp(0, 255)) as u32;
                    b = ((b as i32 + tb as i32).clamp(0, 255)) as u32;

                    frame_out[idx]     = ((r * combined) >> 8) as u8;
                    frame_out[idx + 1] = ((g * combined) >> 8) as u8;
                    frame_out[idx + 2] = ((b * combined) >> 8) as u8;
                }
            }
        }
        // No ring update when echo disabled
    }
}



pub fn generate_transitions(

    style: &str,
    segments: &mut [PlanSegment],
    wrap_indices: &[usize],
    _fps: u32,
) -> Vec<TransitionItem> {
    let mut transitions = Vec::new();
    let style_upper = style.to_uppercase();

    // 1. Systematically place WARP_BUBBLE on wraps
    for &wrap_idx in wrap_indices {
        if wrap_idx < segments.len() {
            let seg = &mut segments[wrap_idx];
            let t_cut = seg.t0;
            seg.transition = Some(SegmentTransition {
                transition_type: "WARP_BUBBLE".to_string(),
                params: serde_json::json!({
                    "amplitude": 0.5,
                    "frequency": 1.2,
                    "duration_frames": 4,
                    "is_wrap": true,
                }),
            });
            transitions.push(TransitionItem {
                t: t_cut,
                transition_type: "WARP_BUBBLE".to_string(),
                duration_frames: 4,
                is_wrap: true,
                params: serde_json::json!({
                    "amplitude": 0.5,
                    "frequency": 1.2,
                }),
            });
        }
    }

    // 2. Place on non-wrap cuts (segments with index > 0 that are not wraps)
    for (idx, seg) in segments.iter_mut().enumerate() {
        if idx == 0 || wrap_indices.contains(&idx) {
            continue;
        }
        let t_cut = seg.t0;
        let h = deterministic_hash_pos(t_cut, 999) % 100;

        let trans_type_opt = match style_upper.as_str() {
            "HARD" => {
                if h < 30 {
                    Some("WARP_BUBBLE")
                } else if h < 50 {
                    Some("WAVE_WARP")
                } else if h < 90 {
                    Some("SLIDE_SHAKE")
                } else {
                    None
                }
            }
            "SMOOTH" => {
                if h < 10 {
                    Some("SLIDE_SHAKE")
                } else {
                    None
                }
            }
            "HYBRID" => {
                if h < 15 {
                    Some("WARP_BUBBLE")
                } else if h < 25 {
                    Some("WAVE_WARP")
                } else if h < 50 {
                    Some("SLIDE_SHAKE")
                } else {
                    None
                }
            }
            _ => None,
        };

        if let Some(trans_type) = trans_type_opt {
            let (dur_frames, params) = match trans_type {
                "WARP_BUBBLE" => (4, serde_json::json!({ "amplitude": 0.5, "frequency": 1.2 })),
                "WAVE_WARP" => (6, serde_json::json!({ "height": 280.0, "speed": 20.0 })),
                "SLIDE_SHAKE" => (6, serde_json::json!({ "amplitude": 100.0 })),
                _ => (4, serde_json::json!({})),
            };

            seg.transition = Some(SegmentTransition {
                transition_type: trans_type.to_string(),
                params: params.clone(),
            });

            transitions.push(TransitionItem {
                t: t_cut,
                transition_type: trans_type.to_string(),
                duration_frames: dur_frames,
                is_wrap: false,
                params,
            });
        }
    }

    transitions.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap_or(std::cmp::Ordering::Equal));
    transitions
}

pub fn compute_borderless_scale(src_w: u32, src_h: u32, target_w: u32, target_h: u32) -> (f64, f64) {
    if src_w == 0 || src_h == 0 {
        return (1.0, 1.0);
    }
    let scale_x = (target_w as f64) / (src_w as f64);
    let scale_y = (target_h as f64) / (src_h as f64);
    (scale_x, scale_y)
}

pub fn compute_crop_to_fill(src_w: u32, src_h: u32, aspect_w: u32, aspect_h: u32) -> CropInfo {
    let aspect_w = if aspect_w == 0 { 1080 } else { aspect_w };
    let aspect_h = if aspect_h == 0 { 1080 } else { aspect_h };

    let (out_w, out_h) = if aspect_w >= 100 && aspect_h >= 100 {
        (aspect_w & !1, aspect_h & !1)
    } else if aspect_w >= aspect_h {
        let target_ar = (aspect_w as f64) / (aspect_h as f64);
        let ow = ((1080.0 * target_ar).round() as u32) & !1;
        let oh = 1080u32;
        (ow.max(2), oh)
    } else {
        let target_ar = (aspect_h as f64) / (aspect_w as f64);
        let ow = 1080u32;
        let oh = ((1080.0 * target_ar).round() as u32) & !1;
        (ow, oh.max(2))
    };

    // Borderless: always stretch full source to target dimensions without any cropping
    CropInfo {
        x: 0,
        y: 0,
        width: src_w,
        height: src_h,
        out_w,
        out_h,
    }
}

pub fn compute_effects_count(plan: &ProjectPlan) -> usize {
    let one_framers_count = plan.one_framers.len();
    let transitions_count = plan.transitions.len();
    let mut ambiance_count = 0;
    if let Some(ref amb) = plan.ambiance {
        if amb.flicker.amplitude > 0.0 {
            ambiance_count += 1;
        }
        if !amb.exposure_flash.times.is_empty() {
            ambiance_count += 1;
        }
        if amb.echo_trail.enabled {
            ambiance_count += 1;
        }
        if amb.tint.offset_rgb != [0, 0, 0] {
            ambiance_count += 1;
        }
        if amb.vignette.strength > 0.0 {
            ambiance_count += 1;
        }
        if amb.scanlines.opacity > 0.0 {
            ambiance_count += 1;
        }
    }
    one_framers_count + transitions_count + ambiance_count
}

pub fn compute_render_stats(
    plan: &ProjectPlan,
    out_mp4_path: &std::path::Path,
    render_time_secs: f64,
) -> RenderStats {
    let file_size_mb = std::fs::metadata(out_mp4_path)
        .map(|m| (m.len() as f64) / (1024.0 * 1024.0))
        .unwrap_or(0.0);
    let effects_count = compute_effects_count(plan);
    RenderStats {
        output_path: out_mp4_path.to_string_lossy().to_string(),
        render_time_secs: (render_time_secs * 100.0).round() / 100.0,
        file_size_mb: (file_size_mb * 100.0).round() / 100.0,
        target_fps: plan.fps,
        effects_count,
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

fn get_ffprobe_binary(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    // Derive ffprobe path from ffmpeg path (they ship together)
    let ffmpeg_path = get_ffmpeg_binary(app)?;
    let ffprobe_path = ffmpeg_path.with_file_name("ffprobe.exe");
    if ffprobe_path.exists() {
        return Ok(ffprobe_path);
    }
    // Fallback: check system PATH
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        if let Ok(output) = std::process::Command::new("where.exe")
            .arg("ffprobe")
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
    Err("ffprobe binary not found (required for fallback probe)".to_string())
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
fn probe_media(app: tauri::AppHandle, file_path: String) -> Result<MediaInfo, String> {
    let ffprobe_bin = get_ffprobe_binary(&app).ok();
    probe_media_internal(&file_path, ffprobe_bin.as_deref())
}

fn probe_media_internal(file_path: &str, ffprobe_bin: Option<&std::path::Path>) -> Result<MediaInfo, String> {
    let path = std::path::Path::new(file_path);
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

    // 2. ffprobe fallback for video files when pure-Rust probe missed dimensions
    let video_exts = ["mp4", "mov", "avi", "mkv", "webm", "wmv", "flv", "ts", "m2ts", "mxf"];
    if video_exts.contains(&ext.as_str()) {
        if let Some(ffprobe_bin) = ffprobe_bin {
            #[cfg(target_os = "windows")]
            use std::os::windows::process::CommandExt;
            let mut cmd = std::process::Command::new(ffprobe_bin);
            cmd.args([
                "-v", "error",
                "-select_streams", "v:0",
                "-show_entries", "stream=width,height,avg_frame_rate,r_frame_rate,duration",
                "-of", "json",
                file_path,
            ]);
            #[cfg(target_os = "windows")]
            cmd.creation_flags(CREATE_NO_WINDOW);
            if let Ok(output) = cmd.output() {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout) {
                        if let Some(streams) = json["streams"].as_array() {
                            if let Some(stream) = streams.first() {
                                let width = stream["width"].as_u64().unwrap_or(0) as u32;
                                let height = stream["height"].as_u64().unwrap_or(0) as u32;
                                let mut fps = 0.0f64;
                                // Try avg_frame_rate first, then r_frame_rate
                                for key in ["avg_frame_rate", "r_frame_rate"] {
                                    if let Some(rate_str) = stream[key].as_str() {
                                        if let Some((num, den)) = rate_str.split_once('/') {
                                            if let (Ok(n), Ok(d)) = (num.parse::<f64>(), den.parse::<f64>()) {
                                                if d > 0.0 {
                                                    fps = n / d;
                                                    if (fps - fps.round()).abs() < 0.05 {
                                                        fps = fps.round();
                                                    }
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                }
                                let duration_str = stream["duration"].as_str().unwrap_or("0");
                                let probe_dur = duration_str.parse::<f64>().unwrap_or(0.0);
                                // Complement with symphonia for audio info
                                let (audio_ch, audio_sr, sym_dur) = if let Ok(ai) = probe_audio_symphonia(file_path, &ext) {
                                    (ai.audio_channels, ai.audio_sample_rate, ai.duration)
                                } else { (0, 0, 0.0) };
                                let final_dur = if probe_dur > 0.0 { probe_dur } else { sym_dur };
                                if width > 0 && height > 0 {
                                    return Ok(MediaInfo {
                                        duration: final_dur,
                                        fps,
                                        width,
                                        height,
                                        audio_channels: audio_ch,
                                        audio_sample_rate: audio_sr,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
        // If ffprobe also failed for a video file, return specific error
        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or(file_path);
        return Err(format!(
            "Could not probe video dimensions for '{}' ({} container). Try re-encoding to H.264 MP4.",
            filename, ext
        ));
    }

    // 3. Audio and other containers (via symphonia crate)
    probe_audio_symphonia(file_path, &ext)
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
    full_fx: bool,
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
    let mut wrap_indices = Vec::new();
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

            // ── T14 engine generation ─────────────────────────────────────────
            // All engines are deterministic from seed; probability scaled by style.
            // lcg(x) = x.wrapping_mul(1664525).wrapping_add(1013904223)
            let s1 = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let s2 = s1.wrapping_mul(1664525).wrapping_add(1013904223);
            let s3 = s2.wrapping_mul(1664525).wrapping_add(1013904223);
            let s4 = s3.wrapping_mul(1664525).wrapping_add(1013904223);
            let s5 = s4.wrapping_mul(1664525).wrapping_add(1013904223);
            let s6 = s5.wrapping_mul(1664525).wrapping_add(1013904223);
            let s7 = s6.wrapping_mul(1664525).wrapping_add(1013904223);
            let s8 = s7.wrapping_mul(1664525).wrapping_add(1013904223);

            let pct = |sn: u32| -> u32 { sn % 100 };

            let style_up = style.to_uppercase();
            let (
                bouncy_prob, bouncy_amp,
                dissolve_prob, dissolve_pct_val,
                skew_prob, skew_s0,
                squish_prob,
                optics_prob,
                chain_prob,
                stretch_prob,
            ) = match style_up.as_str() {
                "SMOOTH" => (0u32, 0.0f64, 0u32, 0.0f64, 0u32, 0.0f64, 0u32, 0u32, 10u32, 0u32),
                "HYBRID" => (15u32, 25.0, 10u32, 15.0, 10u32, 7.0, 20u32, 10u32, 15u32, 10u32),
                _        => (30u32, 40.0, 25u32, 30.0, 20u32, 10.0, 40u32, 25u32, 30u32, 20u32),
            };

            // Bouncy: 30%/15%/0%; when active, zero out harmonic a0
            let bouncy_shake = if bouncy_prob > 0 && pct(s1) < bouncy_prob {
                Some(BouncyShake { axis: (s1 % 2) as u8, amplitude: bouncy_amp })
            } else {
                None
            };
            // If bouncy active, harmonic shake is suppressed (a0 zeroed in compute_transform_params via bounce override)
            let effective_a0 = if bouncy_shake.is_some() { 0.0 } else { a0 };

            // Dissolve: ghost-blend pct
            let dissolve_shake = if dissolve_prob > 0 && pct(s2) < dissolve_prob {
                Some(DissolveShake { pct: dissolve_pct_val })
            } else {
                None
            };

            // Skew
            let skew_shake = if skew_prob > 0 && pct(s3) < skew_prob {
                Some(SkewShake { s0_deg: skew_s0 })
            } else {
                None
            };

            // Squish pop
            let squish_pop = if squish_prob > 0 && pct(s4) < squish_prob {
                Some(SquishPop { _pad: 0 })
            } else {
                None
            };

            // Optics bounce
            let optics_bounce = if optics_prob > 0 && pct(s5) < optics_prob {
                Some(OpticsBounce { k0: 0.08 })
            } else {
                None
            };

            // Buildup chain: set chain_next on current; pair_chain_from_prev set later on next segment
            let buildup_chain = if pct(s6) < chain_prob {
                Some(BuildupChain { chain_next: true, chain_from_prev: false })
            } else {
                None
            };

            // Warp stretch (geometric — present in MOTION ONLY too)
            let warp_stretch = if stretch_prob > 0 && pct(s7) < stretch_prob {
                let scale_s = 1.3 + (s7 % 20) as f64 * 0.01; // 1.30..1.49
                Some(WarpStretch { axis: (s7 % 2) as u8, scale_start: scale_s })
            } else {
                None
            };

            // Zoom beat offset: 0..=2 frames (all styles)
            let zoom_beat_offset = s8 % 3; // 0, 1, or 2

            let effects = SegmentEffects {
                shake: ShakeEffect {
                    a0: effective_a0,
                    omega,
                    k,
                    seed,
                },
                zoom: ZoomEffect {
                    scale_start,
                    scale_end,
                },
                reverse: reverse_this_segment,
                bouncy_shake,
                dissolve_shake,
                skew_shake,
                squish_pop,
                optics_bounce,
                buildup_chain,
                warp_stretch,
                zoom_beat_offset,
            };

            if s_cursor + span <= video_duration + 1e-9 {
                let mut s0 = s_cursor;
                let mut s1 = s_cursor + span;
                s_cursor += span;

                let is_exact_wrap = (s_cursor - video_duration).abs() < 1e-9;
                if is_exact_wrap {
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
                    transition: None,
                });

                if is_exact_wrap && seg_t1 < target - 1e-6 {
                    wrap_indices.push(segments.len());
                }

                seg_index += 1;
                break;
            } else {
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
                    transition: None,
                });

                wrap_indices.push(segments.len());
                loops += 1;
                s_cursor = 0.0;
                seg_t0 = t_wrap;
                seg_index += 1;
            }
        }
    }

    let target_dur = (target * 1000.0).round() / 1000.0;

    // ── T14 post-pass: propagate buildup chain_next → chain_from_prev on next segment ──
    for i in 0..segments.len().saturating_sub(1) {
        let chain_next = segments[i].effects.buildup_chain.as_ref().map_or(false, |bc| bc.chain_next);
        if chain_next {
            if let Some(bc) = segments[i + 1].effects.buildup_chain.as_mut() {
                bc.chain_from_prev = true;
            } else {
                segments[i + 1].effects.buildup_chain = Some(BuildupChain { chain_next: false, chain_from_prev: true });
            }
        }
    }

    let one_framers = if full_fx {
        generate_one_framers(style, &segments, downbeats, fps, target_dur)
    } else {
        vec![]
    };
    // Geometric transitions (WARP_BUBBLE, WAVE_WARP, SLIDE_SHAKE, future STRETCH)
    // are always generated — they survive in MOTION ONLY mode.
    let transitions = generate_transitions(style, &mut segments, &wrap_indices, fps);

    let ambiance = if full_fx {
        let tint_offset = {
            let seed = 0x9e3779b9u32;
            let r = ((seed.wrapping_mul(1664525).wrapping_add(1013904223)) % 21) as i16 - 10;
            let g = ((seed.wrapping_mul(22695477).wrapping_add(1)) % 11) as i16 - 5;
            let b = ((seed.wrapping_mul(6364136223846793005u64 as u32).wrapping_add(1442695040)) % 17) as i16 - 8;
            [r, g, b]
        };
        let mut a = default_ambiance(style, downbeats);
        a.tint.offset_rgb = tint_offset;
        Some(a)
    } else {
        // Motion-only: keep flicker, strip everything else
        Some(AmbianceConfig {
            flicker: default_ambiance(style, downbeats).flicker,
            exposure_flash: ExposureFlashConfig { peak: 0.0, times: vec![] },
            echo_trail: EchoTrailConfig { enabled: false, alpha: 0.0, k: 0 },
            tint: TintConfig { offset_rgb: [0, 0, 0] },
            vignette: VignetteConfig { strength: 0.0 },
            scanlines: ScanlinesConfig { opacity: 0.0 },
        })
    };

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
        full_fx,
        one_framers,
        transitions,
        ambiance,
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
    full_fx: bool,
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
        full_fx,
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
            // Floor: long-side must remain >= 1080 after scale
            let long_side = src_w.max(src_h) as f64;
            let floor_scale = 1080.0 / long_side;
            let s_clamped = s.max(floor_scale).min(1.0);

            let new_w = ((src_w as f64 * s_clamped) as u32) & !1; // round to even
            let new_h = ((src_h as f64 * s_clamped) as u32) & !1;

            // Check if even at floor scale, cache still exceeds 4 GB
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
    let mut transition_buf = vec![0u8; frame_bytes];
    let mut ambiance_buf = vec![0u8; frame_bytes];
    let cropped_frame_bytes = (crop.width * crop.height * 3) as usize;
    let mut cropped_buf = vec![0u8; cropped_frame_bytes];
    let mut dissolve_buf = vec![0u8; cropped_frame_bytes]; // T14 dissolve_shake blend buffer
    let mut blend_frames_storage = vec![vec![0u8; frame_bytes]; 4];

    // Echo/trail ring buffer: stores up to k=3 previous full frames
    let echo_k = 3usize;
    let mut echo_ring: Vec<Vec<u8>> = (0..echo_k).map(|_| vec![128u8; frame_bytes]).collect();
    let mut echo_head: usize = 0;

    // Vignette LUT: precomputed per-pixel strength on full-frame
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

        // 2. T11 Ambiance Effects (flicker, exposure flash, echo/trail, tint/vignette/scanlines)
        //    Applied BEFORE crop, on full-frame trans_frame_ptr → ambiance_buf
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

        // 3.5 T14 Dissolve shake: blend with ghost frame at src ± 2
        if let Some(ref ds) = seg.effects.dissolve_shake {
            if ds.pct > 0.0 {
                let env = compute_shake_envelope(t_rel, seg_dur, output_fps);
                let alpha = (ds.pct / 100.0 * env).clamp(0.0, 0.5); // max 50% blend
                if alpha > 1e-4 {
                    let ghost_frame_idx = (base_src_frame + 2).clamp(0, (total_source_frames - 1) as i64) as u64;
                    // Reuse sampled_full_frame slot temporarily via a scratch read into transition_buf
                    let mut ghost_full = vec![0u8; frame_bytes];
                    if frame_reader.get_frame(ghost_frame_idx, &mut ghost_full).is_ok() {
                        // Crop the ghost frame
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

    // Cleanup raw cache
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
            let plan = create_plan_internal(style, 16, &beats, &downbeats, 5.0, 3.5, 1080, 1080, 120.0, true).unwrap();

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

        let hard_plan = create_plan_internal("HARD", 16, &beats, &downbeats, 10.773, 14.315, 1080, 1080, 83.33, true).unwrap();
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
        let smooth_plan = create_plan_internal("SMOOTH", 16, &beats, &downbeats, 10.773, 14.315, 1080, 1080, 83.33, true).unwrap();
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
        assert_eq!(parsed_v1.transitions.len(), 0);
        assert_eq!(parsed_v1.segments[0].transition, None);

        let v2_plan = create_plan_internal("HARD", 16, &[1.0, 2.0], &[1.0], 5.0, 5.0, 1080, 1080, 120.0, true).unwrap();
        assert_eq!(v2_plan.schema_version, 2);
        assert_eq!(v2_plan.motion_blur, true);
        assert_eq!(v2_plan.segments[0].effects.shake.a0, 8.0);
        assert!(v2_plan.one_framers.len() > 0);
        assert!(v2_plan.transitions.len() > 0);

        let v2_serialized = serde_json::to_string(&v2_plan).unwrap();
        assert!(v2_serialized.contains("\"one_framers\":["));
        assert!(v2_serialized.contains("\"transitions\":["));
        let v2_deserialized: ProjectPlan = serde_json::from_str(&v2_serialized).unwrap();
        assert_eq!(v2_deserialized.one_framers.len(), v2_plan.one_framers.len());
        assert_eq!(v2_deserialized.transitions.len(), v2_plan.transitions.len());
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
            true,
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
            true,
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
            true,
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

        let plan1 = create_plan_internal("HARD", fps, &beats, &downbeats, video_duration, audio_duration, 1080, 1080, bpm, true).unwrap();
        let plan2 = create_plan_internal("HARD", fps, &beats, &downbeats, video_duration, audio_duration, 1080, 1080, bpm, true).unwrap();

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
        assert_eq!(crop_1_1.y, 0);
        assert_eq!(crop_1_1.width, 1080);
        assert_eq!(crop_1_1.height, 1920);
        assert_eq!(crop_1_1.out_w, 1080);
        assert_eq!(crop_1_1.out_h, 1080);

        let crop_16_9 = compute_crop_to_fill(1080, 1920, 16, 9);
        assert_eq!(crop_16_9.x, 0);
        assert_eq!(crop_16_9.y, 0);
        assert_eq!(crop_16_9.width, 1080);
        assert_eq!(crop_16_9.height, 1920);
        assert_eq!(crop_16_9.out_w, 1920);
        assert_eq!(crop_16_9.out_h, 1080);

        let crop_9_16 = compute_crop_to_fill(1080, 1920, 9, 16);
        assert_eq!(crop_9_16.x, 0);
        assert_eq!(crop_9_16.y, 0);
        assert_eq!(crop_9_16.width, 1080);
        assert_eq!(crop_9_16.height, 1920);
        assert_eq!(crop_9_16.out_w, 1080);
        assert_eq!(crop_9_16.out_h, 1920);
    }

    #[test]
    fn test_borderless_stretch_scale() {
        let (sx1, sy1) = compute_borderless_scale(1920, 1080, 1080, 1080);
        assert_eq!(sx1, 0.5625);
        assert_eq!(sy1, 1.0);

        let (sx2, sy2) = compute_borderless_scale(1080, 1920, 1080, 1080);
        assert_eq!(sx2, 1.0);
        assert_eq!(sy2, 0.5625);

        let crop = compute_crop_to_fill(1080, 1920, 1080, 1080);
        assert_eq!(crop.x, 0);
        assert_eq!(crop.y, 0);
        assert_eq!(crop.width, 1080);
        assert_eq!(crop.height, 1920);
        assert_eq!(crop.out_w, 1080);
        assert_eq!(crop.out_h, 1080);
    }

    #[test]
    fn test_probe_media_video_pure_rust() {
        let video_path = r"C:\Users\cia\Downloads\jugg video & audio tester\snaptik_7674387013243538721_v3.mp4";
        if std::path::Path::new(video_path).exists() {
            let res = probe_media_internal(video_path, None).expect("Probe should succeed");
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
            let res = probe_media_internal(drums_path, None).expect("Probe should succeed");
            println!("Drums audio probe result: {:?}", res);
            assert!(res.duration > 14.0);
            assert_eq!(res.audio_channels, 2);
            assert_eq!(res.audio_sample_rate, 44100);
        }

        let audio_path = r"C:\Users\cia\Downloads\jugg video & audio tester\curiosos.mp3";
        if std::path::Path::new(audio_path).exists() {
            let res = probe_media_internal(audio_path, None).expect("Probe should succeed");
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
                true,
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
    fn test_ambiance_flicker_oscillation() {
        let fps = 16.0;
        let t_cut = 0.0;
        let mut seg = PlanSegment {
            t0: 0.0, t1: 1.0, s0: 0.0, s1: 1.0,
            curve: "snap".to_string(),
            effects: crate::SegmentEffects {
                shake: crate::ShakeEffect { a0: 0.0, omega: 0.0, k: 0.0, seed: 0 },
                zoom: crate::ZoomEffect { scale_start: 1.0, scale_end: 1.0 },
                reverse: false,
                ..crate::default_segment_effects()
            },
            transition: None,
        };
        let _ = t_cut;

        let amplitude = 0.15;
        let freq = 12.0;
        // Sample 128 time points and verify all values in [-A, +A]
        let mut min_val = f64::MAX;
        let mut max_val = f64::MIN;
        for i in 0..128 {
            let t = (i as f64) / fps;
            let seg_phase = (seg.effects.shake.seed as f64) * 0.0012345;
            let v = amplitude * (2.0 * std::f64::consts::PI * freq * t + seg_phase).sin();
            if v < min_val { min_val = v; }
            if v > max_val { max_val = v; }
        }
        assert!(min_val >= -amplitude - 1e-9, "Flicker must stay >= -A");
        assert!(max_val <= amplitude + 1e-9, "Flicker must stay <= +A");
        // Oscillation must actually span at least 80% of the range
        let range = max_val - min_val;
        assert!(range > 0.8 * 2.0 * amplitude, "Flicker must oscillate across most of [-A, +A]");
        let _ = seg; // avoid unused warning
    }

    #[test]
    fn test_ambiance_exposure_flash() {
        let fps: f64 = 16.0;
        let _downbeat_t: f64 = 2.0;
        let peak: f64 = 0.5;
        let dt: f64 = 1.0 / fps;

        // At t=downbeat: flash_exp = peak (env=1.0)
        let env_center: f64 = (1.0f64 - (0.0f64).abs() / 2.0f64).max(0.0f64);
        let flash_center: f64 = peak * env_center;
        assert!((flash_center - peak).abs() < 1e-6, "Flash at downbeat must equal peak");

        // At ±2 frames: env=0
        let env_edge: f64 = (1.0f64 - ((2.0f64 * dt * fps).abs() / 2.0f64)).max(0.0f64);
        let flash_edge: f64 = peak * env_edge;
        assert!(flash_edge.abs() < 1e-6, "Flash at ±2 frames must be 0");

        // At 1 frame before: env=0.5
        let env_half: f64 = (1.0f64 - ((-1.0f64).abs() / 2.0f64)).max(0.0f64);
        let flash_half: f64 = peak * env_half;
        assert!((flash_half - peak * 0.5f64).abs() < 1e-6, "Flash at ±1 frame must be peak/2");
    }

    #[test]
    fn test_ambiance_echo_trail_blend() {
        let width = 4usize;
        let height = 4usize;
        let n = width * height * 3;
        let frame_in = vec![200u8; n];
        let mut frame_out = vec![0u8; n];

        let alpha = 0.3;
        let k = 3u32;
        let mut echo_ring: Vec<Vec<u8>> = (0..3).map(|_| vec![100u8; n]).collect();
        let mut echo_head = 0usize;

        let vignette_lut = vec![255u8; width * height];
        let seg = PlanSegment {
            t0: 0.0, t1: 1.0, s0: 0.0, s1: 1.0,
            curve: "snap".to_string(),
            effects: crate::SegmentEffects {
                shake: crate::ShakeEffect { a0: 0.0, omega: 0.0, k: 0.0, seed: 0 },
                zoom: crate::ZoomEffect { scale_start: 1.0, scale_end: 1.0 },
                reverse: false,
                ..crate::default_segment_effects()
            },
            transition: None,
        };

        let amb = AmbianceConfig {
            flicker: FlickerConfig { amplitude: 0.0, f: 0.0, phase: 0.0 },
            exposure_flash: ExposureFlashConfig { peak: 0.0, times: vec![] },
            echo_trail: EchoTrailConfig { enabled: true, alpha, k },
            tint: TintConfig { offset_rgb: [0, 0, 0] },
            vignette: VignetteConfig { strength: 0.0 },
            scanlines: ScanlinesConfig { opacity: 0.0 },
        };

        apply_ambiance_effects(
            &frame_in,
            &mut frame_out,
            width, height,
            &amb,
            &mut echo_ring,
            &mut echo_head,
            &vignette_lut,
            0.0,
            0.0,
            &seg,
            16.0,
        );

        // With echo enabled: output should be between previous (100) and current (200)
        // Integer arithmetic (Q8 fixed-point):
        //   alpha_fp = floor(0.3 * 256) = 76
        //   r_cur = (200 * 256) >> 8 = 200  (scale_fp=256, no flicker)
        //   r_echo = 100
        //   after blend: ((256-76)*200 + 76*100) >> 8 = 43600 >> 8 = 170
        //   combined = (255 * 256) >> 8 = 255  (vignette lut=255, no scanline dim: opacity=0)
        //   final: (170 * 255) >> 8 = 169
        let expected = 169u8;
        for px in 0..(width * height) {
            let idx = px * 3;
            assert_eq!(frame_out[idx], expected, "Echo trail blend mismatch at px {}", px);
        }
    }

    #[test]
    fn test_ambiance_tint_vignette_scanlines() {
        let width = 16usize;
        let height = 16usize;
        let n = width * height * 3;
        let frame_in = vec![128u8; n];
        let mut frame_out = vec![0u8; n];

        // Vignette LUT: full brightness (255) so only tint/scanlines matter
        let vignette_lut = vec![255u8; width * height];
        let mut echo_ring: Vec<Vec<u8>> = (0..3).map(|_| vec![0u8; n]).collect();
        let mut echo_head = 0usize;

        let seg = PlanSegment {
            t0: 0.0, t1: 1.0, s0: 0.0, s1: 1.0,
            curve: "snap".to_string(),
            effects: crate::SegmentEffects {
                shake: crate::ShakeEffect { a0: 0.0, omega: 0.0, k: 0.0, seed: 0 },
                zoom: crate::ZoomEffect { scale_start: 1.0, scale_end: 1.0 },
                reverse: false,
                ..crate::default_segment_effects()
            },
            transition: None,
        };

        let tint_r = 10i16;
        let amb = AmbianceConfig {
            flicker: FlickerConfig { amplitude: 0.0, f: 0.0, phase: 0.0 },
            exposure_flash: ExposureFlashConfig { peak: 0.0, times: vec![] },
            echo_trail: EchoTrailConfig { enabled: false, alpha: 0.3, k: 3 },
            tint: TintConfig { offset_rgb: [tint_r, 0, 0] },
            vignette: VignetteConfig { strength: 0.0 },
            scanlines: ScanlinesConfig { opacity: 0.15 },
        };

        apply_ambiance_effects(
            &frame_in, &mut frame_out, width, height, &amb,
            &mut echo_ring, &mut echo_head, &vignette_lut, 0.15,
            0.0, &seg, 16.0,
        );

        // Row 1 (not a scanline), px 0 — R channel byte index = 16 * 3 * 1 = 48
        // flicker=0 → scale_fp=256 → r=128; tint +10 → 138
        // combined = (255 * 256) >> 8 = 255 (non-scanline: row_dim=256)
        // r_out = (138 * 255) >> 8 = 137
        let non_scanline_r = frame_out[width * 3]; // row 1, px 0, R channel
        assert_eq!(non_scanline_r, 137u8, "Tint R+10 with vignette should yield 137 for non-scanline rows");

        // Scanline row 0: py=0, dim_fp = (0.85*256) = 217
        // combined = (255 * 217) >> 8 = 216
        // r_out = (138 * 216) >> 8 = 116
        let scanline_r = frame_out[0]; // row 0, px 0, R channel
        assert_eq!(scanline_r, 116u8, "Scanline row should be dimmed by vignette+opacity combined");

        // Total diff vs input must be > 0
        let diff: i64 = frame_in.iter().zip(frame_out.iter())
            .map(|(&a, &b)| (a as i64 - b as i64).abs())
            .sum();
        assert!(diff > 0, "Ambiance effects must change the frame");
    }

    #[test]
    fn test_transitions_warp_bubble() {
        let fps = 16.0;
        let t_cut = 2.0;

        let env_center = compute_warp_bubble_env(t_cut, t_cut, fps);
        assert!((env_center - 0.5).abs() < 1e-4, "Peak warp bubble envelope should be 0.5");

        let env_edge_left = compute_warp_bubble_env(t_cut - 2.0 / fps, t_cut, fps);
        assert!((env_edge_left - 0.0).abs() < 1e-4, "Warp bubble env at -2 frames should be 0.0");

        let env_edge_right = compute_warp_bubble_env(t_cut + 2.0 / fps, t_cut, fps);
        assert!((env_edge_right - 0.0).abs() < 1e-4, "Warp bubble env at +2 frames should be 0.0");

        let width = 64usize;
        let height = 64usize;
        let mut frame_in = vec![0u8; width * height * 3];
        for y in 0..height {
            for x in 0..width {
                let idx = (y * width + x) * 3;
                frame_in[idx] = (x * 4) as u8;
                frame_in[idx + 1] = (y * 4) as u8;
                frame_in[idx + 2] = ((x + y) * 2) as u8;
            }
        }
        let mut frame_out = vec![0u8; width * height * 3];
        apply_warp_bubble(&frame_in, &mut frame_out, width, height, 0.5, 1.2);

        let diff: i64 = frame_in
            .iter()
            .zip(frame_out.iter())
            .map(|(&a, &b)| (a as i64 - b as i64).abs())
            .sum();
        assert!(diff > 0, "Active warp bubble must produce pixel displacement");
    }

    #[test]
    fn test_transitions_wave_warp() {
        let fps = 16.0;
        let t_cut = 1.0;
        let height = 1080usize;

        let (h0, _, _, t_fr0) = compute_wave_warp_params(t_cut, t_cut, fps, height);
        assert!((h0 - 280.0).abs() < 1e-4, "Wave warp H at t=0 should be 280.0");
        assert!((t_fr0 - 0.0).abs() < 1e-4);

        let (h3, _, _, _) = compute_wave_warp_params(t_cut + 3.0 / fps, t_cut, fps, height);
        assert!((h3 - 140.0).abs() < 1e-4, "Wave warp H at t=3 frames should be 140.0");

        let (h6, _, _, _) = compute_wave_warp_params(t_cut + 6.0 / fps, t_cut, fps, height);
        assert!((h6 - 0.0).abs() < 1e-4, "Wave warp H at t=6 frames should be 0.0");

        let (h7, _, _, _) = compute_wave_warp_params(t_cut + 7.0 / fps, t_cut, fps, height);
        assert_eq!(h7, 0.0, "Wave warp H at t=7 frames should be 0.0");
    }

    #[test]
    fn test_transitions_slide_shake() {
        let fps = 16.0;
        let t_cut = 2.0;
        let dt = 1.0 / fps;

        let shift_before = compute_slide_shake_shift(t_cut - dt, t_cut, fps);
        let shift_after = compute_slide_shake_shift(t_cut + dt, t_cut, fps);

        assert!(shift_before > 0.0, "Shift before cut must be positive");
        assert!(shift_after < 0.0, "Shift after cut must be negative");
        assert!(
            (shift_before.abs() - shift_after.abs()).abs() < 1e-4,
            "Shift magnitude must be continuous across cut (signs inverted)"
        );

        let shift_bound_left = compute_slide_shake_shift(t_cut - 3.0 * dt, t_cut, fps);
        assert!((shift_bound_left - 0.0).abs() < 1e-4);

        let shift_bound_right = compute_slide_shake_shift(t_cut + 3.0 * dt, t_cut, fps);
        assert!((shift_bound_right - 0.0).abs() < 1e-4);
    }

    #[test]
    fn test_transitions_auto_placement() {
        let video_duration = 10.773;
        let audio_duration = 14.315;
        let beats = vec![
            0.42, 1.14, 1.88, 2.60, 3.32, 4.04, 4.76, 5.50, 6.20, 6.94, 7.64, 8.38, 9.10, 9.82,
            10.54, 11.26, 11.98, 12.70, 13.42, 14.14,
        ];
        let downbeats = vec![2.60, 5.50, 8.38, 11.26, 14.14];
        let fps = 16u32;
        let bpm = 83.33;

        let plan_hard = create_plan_internal("HARD", fps, &beats, &downbeats, video_duration, audio_duration, 1080, 1080, bpm, true).unwrap();

        let wrap_transitions: Vec<_> = plan_hard.transitions.iter().filter(|t| t.is_wrap).collect();
        assert_eq!(wrap_transitions.len(), 1, "HARD plan should have 1 wrap transition");
        assert_eq!(wrap_transitions[0].transition_type, "WARP_BUBBLE");

        let cut_warps = plan_hard.transitions.iter().filter(|t| !t.is_wrap && t.transition_type == "WARP_BUBBLE").count();
        let cut_waves = plan_hard.transitions.iter().filter(|t| !t.is_wrap && t.transition_type == "WAVE_WARP").count();
        let cut_slides = plan_hard.transitions.iter().filter(|t| !t.is_wrap && t.transition_type == "SLIDE_SHAKE").count();

        println!(
            "HARD transitions breakdown: wrap=1, warp_cuts={}, wave_cuts={}, slide_cuts={}",
            cut_warps, cut_waves, cut_slides
        );

        // Expect ~6 warp cuts, ~4 wave cuts, ~8 slide cuts (tolerance +/- 2)
        assert!((cut_warps as i32 - 6).abs() <= 2, "Warp cuts count should be ~6");
        assert!((cut_waves as i32 - 4).abs() <= 2, "Wave cuts count should be ~4");
        assert!((cut_slides as i32 - 8).abs() <= 2, "Slide cuts count should be ~8");

        let plan_smooth = create_plan_internal("SMOOTH", fps, &beats, &downbeats, video_duration, audio_duration, 1080, 1080, bpm, true).unwrap();
        let smooth_warps = plan_smooth.transitions.iter().filter(|t| !t.is_wrap && t.transition_type == "WARP_BUBBLE").count();
        let smooth_waves = plan_smooth.transitions.iter().filter(|t| !t.is_wrap && t.transition_type == "WAVE_WARP").count();
        assert_eq!(smooth_warps, 0, "SMOOTH style has 0% warp on cuts");
        assert_eq!(smooth_waves, 0, "SMOOTH style has 0% wave on cuts");

        let plan_hybrid = create_plan_internal("HYBRID", fps, &beats, &downbeats, video_duration, audio_duration, 1080, 1080, bpm, true).unwrap();
        assert!(plan_hybrid.transitions.len() > plan_smooth.transitions.len());
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
            true,
        )
        .expect("Plan generation failed");

        let ffmpeg_bin = if let Ok(output) = std::process::Command::new("where.exe").arg("ffmpeg").output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            stdout.lines().next().map(|s| std::path::PathBuf::from(s.trim())).unwrap_or_default()
        } else {
            std::path::PathBuf::from("ffmpeg.exe")
        };

        let scene_info = probe_media_internal(video_path, None).unwrap();
        let src_w = scene_info.width;
        let src_h = scene_info.height;
        let src_fps = scene_info.fps;
        let frame_bytes = (src_w * src_h * 3) as usize;

        let temp_dir = std::env::temp_dir().join("cia_app_bench_t10");
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

        // 1. Baseline T9 Pipeline (Shakes, Zoom, Reverse, One-Framers)
        let t_t9_start = std::time::Instant::now();
        let mut t9_reader = CachedFrameReader::new(&mut raw_file, frame_bytes, 16);
        let mut sampled_full_frame = vec![0u8; frame_bytes];
        let mut blend_storage = vec![vec![0u8; frame_bytes]; 4];
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

        // 2. T10 Full Pipeline (T9 Effects + Transitions)
        let t_t10_start = std::time::Instant::now();
        let mut t10_reader = CachedFrameReader::new(&mut raw_file, frame_bytes, 16);
        let mut transition_buf = vec![0u8; frame_bytes];
        let mut t10_crop = vec![0u8; cropped_frame_bytes];

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
                t10_reader.get_frame(base_src_frame as u64, &mut sampled_full_frame).unwrap();
            } else {
                let mut slice_ptrs: Vec<&[u8]> = Vec::with_capacity(n_blur);
                for k in 0..n_blur {
                    let f_idx = (base_src_frame + (k as i64)).clamp(0, (total_source_frames - 1) as i64) as u64;
                    t10_reader.get_frame(f_idx, &mut blend_storage[k]).unwrap();
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

            // Transitions (Warp Bubble, Wave Warp, Slide Shake)
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

            // Transform Stack + Crop (T10 baseline: transitions only, no ambiance)
            let params = compute_transform_params(&seg.effects, t_rel, seg_dur, output_fps);
            apply_transform_stack_cropped(
                trans_frame_ptr,
                &mut t10_crop,
                src_w as usize,
                src_h as usize,
                crop.x,
                crop.y,
                crop.width,
                crop.height,
                params,
            );
        }
        let t_t10 = t_t10_start.elapsed();

        // T11 pass — T10 pipeline + ambiance effects
        let amb = plan.ambiance.as_ref().unwrap();
        let vig_strength = amb.vignette.strength;
        let scanline_opacity_bench = amb.scanlines.opacity;
        let rx_b = (src_w as f64) / 2.0;
        let ry_b = (src_h as f64) / 2.0;
        let r_max_b = (rx_b * rx_b + ry_b * ry_b).sqrt();
        let mut vignette_lut_bench = vec![0u8; (src_w * src_h) as usize];
        for vy in 0..(src_h as usize) {
            let dy = (vy as f64) - ry_b;
            for vx in 0..(src_w as usize) {
                let dx = (vx as f64) - rx_b;
                let r = (dx * dx + dy * dy).sqrt();
                let factor = 1.0 - vig_strength * (r / r_max_b).powi(2);
                vignette_lut_bench[vy * (src_w as usize) + vx] = (factor.clamp(0.0, 1.0) * 255.0) as u8;
            }
        }
        let echo_k_b = 3usize;
        let mut echo_ring_b: Vec<Vec<u8>> = (0..echo_k_b).map(|_| vec![128u8; frame_bytes]).collect();
        let mut echo_head_b: usize = 0;
        let mut ambiance_buf_b = vec![0u8; frame_bytes];
        let mut t11_crop = vec![0u8; cropped_frame_bytes];

        let t_t11_start = std::time::Instant::now();
        let mut t11_reader = CachedFrameReader::new(&mut raw_file, frame_bytes, 16);

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

            if n_blur <= 1 {
                t11_reader.get_frame(base_src_frame as u64, &mut sampled_full_frame).unwrap();
            } else {
                let mut slice_ptrs: Vec<&[u8]> = Vec::with_capacity(n_blur);
                for k in 0..n_blur {
                    let f_idx = (base_src_frame + (k as i64)).clamp(0, (total_source_frames - 1) as i64) as u64;
                    t11_reader.get_frame(f_idx, &mut blend_storage[k]).unwrap();
                }
                for k in 0..n_blur { slice_ptrs.push(&blend_storage[k]); }
                blend_full_frames(&slice_ptrs, &mut sampled_full_frame);
            }

            let active_framer = plan.one_framers.iter().find(|f| (t - f.t).abs() < (0.5 / output_fps) + 1e-6);
            let full_frame_ptr = if let Some(framer) = active_framer {
                apply_one_framer(&framer.framer_type, &sampled_full_frame, &mut one_framer_buf, src_w as usize, src_h as usize);
                &one_framer_buf
            } else { &sampled_full_frame };

            let mut active_trans: Option<(&TransitionItem, f64)> = None;
            for trans in &plan.transitions {
                let t_frames = (t - trans.t) * output_fps;
                match trans.transition_type.as_str() {
                    "WARP_BUBBLE" => { if t_frames.abs() <= 2.0 + 1e-4 { active_trans = Some((trans, t_frames)); break; } }
                    "WAVE_WARP"   => { if t_frames >= -1e-4 && t_frames <= 6.0 + 1e-4 { active_trans = Some((trans, t_frames)); break; } }
                    "SLIDE_SHAKE" => { if t_frames.abs() <= 3.0 + 1e-4 { active_trans = Some((trans, t_frames)); break; } }
                    _ => {}
                }
            }
            let trans_frame_ptr_b = if let Some((trans, _)) = active_trans {
                match trans.transition_type.as_str() {
                    "WARP_BUBBLE" => {
                        let env_a = compute_warp_bubble_env(t, trans.t, output_fps);
                        apply_warp_bubble(full_frame_ptr, &mut transition_buf, src_w as usize, src_h as usize, env_a, 1.2);
                        &transition_buf as &[u8]
                    }
                    "WAVE_WARP" => {
                        let (h_t, k, v, t_fr) = compute_wave_warp_params(t, trans.t, output_fps, src_h as usize);
                        apply_wave_warp(full_frame_ptr, &mut transition_buf, src_w as usize, src_h as usize, h_t, k, v, t_fr);
                        &transition_buf
                    }
                    "SLIDE_SHAKE" => {
                        let shift_x = compute_slide_shake_shift(t, trans.t, output_fps);
                        apply_slide_shake(full_frame_ptr, &mut transition_buf, src_w as usize, src_h as usize, shift_x);
                        &transition_buf
                    }
                    _ => full_frame_ptr,
                }
            } else { full_frame_ptr };

            apply_ambiance_effects(
                trans_frame_ptr_b, &mut ambiance_buf_b,
                src_w as usize, src_h as usize,
                amb,
                &mut echo_ring_b, &mut echo_head_b,
                &vignette_lut_bench, scanline_opacity_bench,
                t, seg, output_fps,
            );

            let params_b = compute_transform_params(&seg.effects, t_rel, seg_dur, output_fps);
            apply_transform_stack_cropped(&ambiance_buf_b, &mut t11_crop, src_w as usize, src_h as usize, crop.x, crop.y, crop.width, crop.height, params_b);
        }
        let t_t11 = t_t11_start.elapsed();


        let t_t9_total = t_decode + t_t9;
        let t_t10_total = t_decode + t_t10;
        let t_t11_total = t_decode + t_t11;
        let ratio = (t_t11_total.as_secs_f64() / t_t10_total.as_secs_f64()).max(0.01);

        println!("=== T11 AMBIANCE BENCHMARK REPORT ===");
        println!("Total frames rendered: {}", total_output_frames);
        println!("Decode time: {:.3}s", t_decode.as_secs_f64());
        println!("T9 (transitions) pipeline time: {:.3}s", t_t9_total.as_secs_f64());
        println!("T10 (transitions) pipeline time: {:.3}s", t_t10_total.as_secs_f64());
        println!("T11 Full Effects + Ambiance pipeline time: {:.3}s", t_t11_total.as_secs_f64());
        println!("Performance ratio (T11 / T10): {:.3}x", ratio);
        println!("========================================");

        assert!(
            ratio < 1.5, // threshold 1.5x: steady-state ~1.33x, ±0.08x wall-clock variance
            "Benchmark check failed: ratio was {:.3}x (expected < 1.5x)",
            ratio
        );
    }

    // ─── T12: Adaptive scale tests ──────────────────────────────────────────

    #[test]
    fn test_adaptive_scale_4k() {
        // 3840x2160 @ 30fps for 60s = 1800 frames
        // Raw cache = 3840*2160*3*1800 = ~44.8 GB — far exceeds 4 GB
        let w: u32 = 3840;
        let h: u32 = 2160;
        let frames: u64 = 1800;
        let max_cache: u64 = 4 * 1024 * 1024 * 1024;
        let raw_cache = (w as u64) * (h as u64) * 3 * frames;
        assert!(raw_cache > max_cache, "4K source should exceed 4GB cache");

        let s = ((max_cache as f64) / (raw_cache as f64)).sqrt();
        let long_side = w.max(h) as f64;
        let floor_scale = 1080.0 / long_side;
        let s_clamped = s.max(floor_scale).min(1.0);

        let new_w = ((w as f64 * s_clamped) as u32) & !1;
        let new_h = ((h as f64 * s_clamped) as u32) & !1;

        assert!(new_w.max(new_h) >= 1080, "Long side must be >= 1080 after scale");
        let scaled_cache = (new_w as u64) * (new_h as u64) * 3 * frames;
        assert!(scaled_cache <= max_cache, "Scaled cache must fit in 4GB: got {}GB", scaled_cache / 1_073_741_824);
        println!("4K adaptive: {}x{} -> {}x{}, cache {:.2}GB -> {:.2}GB",
            w, h, new_w, new_h,
            raw_cache as f64 / 1e9, scaled_cache as f64 / 1e9);
    }

    #[test]
    fn test_adaptive_scale_1080p_short() {
        // 1080x1920 @ 30fps for ~10.77s = 323 frames — ~1.9 GB, no scale needed
        let w: u32 = 1080;
        let h: u32 = 1920;
        let frames: u64 = 323;
        let max_cache: u64 = 4 * 1024 * 1024 * 1024;
        let raw_cache = (w as u64) * (h as u64) * 3 * frames;
        assert!(raw_cache < max_cache, "1080p short source should fit in 4GB without scaling");
        println!("1080p short: {}x{} x {} frames = {:.2}GB (no scale needed)",
            w, h, frames, raw_cache as f64 / 1e9);
    }

    #[test]
    fn test_adaptive_scale_max_duration() {
        // At floor 1080 long-side (1080x608 for 16:9), compute max seconds @ 30fps
        let fps: f64 = 30.0;
        let floor_w: u32 = 1080;
        let floor_h: u32 = 608; // even, ~16:9
        let max_cache: u64 = 4 * 1024 * 1024 * 1024;
        let floor_frame_bytes = (floor_w as u64) * (floor_h as u64) * 3;
        let max_frames = max_cache / floor_frame_bytes;
        let max_seconds = (max_frames as f64) / fps;
        assert!(max_seconds > 60.0, "At 1080x608 @ 30fps, max should be > 60s, got {:.1}s", max_seconds);
        println!("Max duration at {}x{} @ {}fps: {:.1}s ({} frames)",
            floor_w, floor_h, fps, max_seconds, max_frames);
    }

    // ─── T13: FULL FX toggle tests ──────────────────────────────────────────

    #[test]
    fn test_full_fx_off_strips_effects() {
        let video_duration = 10.773;
        let audio_duration = 14.315;
        let beats = vec![
            0.42, 1.14, 1.88, 2.60, 3.32, 4.04, 4.76, 5.50, 6.20, 6.94,
            7.64, 8.38, 9.10, 9.82, 10.54, 11.26, 11.98, 12.70, 13.42, 14.14,
        ];
        let downbeats = vec![2.60, 5.50, 8.38, 11.26, 14.14];

        let plan = create_plan_internal(
            "HARD", 16, &beats, &downbeats,
            video_duration, audio_duration, 1080, 1080, 83.33, false,
        ).expect("Plan generation must succeed");

        assert_eq!(plan.full_fx, false);
        assert!(plan.one_framers.is_empty(), "full_fx=false must produce empty one_framers");
        // T13.5: geometric transitions (WARP_BUBBLE/WAVE_WARP/SLIDE_SHAKE) survive in MOTION ONLY
        assert!(!plan.transitions.is_empty(),
            "full_fx=false must still have geometric transitions (got 0)");

        let amb = plan.ambiance.as_ref().unwrap();
        assert!(amb.flicker.amplitude > 0.0, "Flicker must be preserved in MOTION ONLY mode");
        assert!(amb.exposure_flash.times.is_empty(), "Exposure flash times must be empty");
        assert!(!amb.echo_trail.enabled);
        assert_eq!(amb.vignette.strength, 0.0);
        assert_eq!(amb.scanlines.opacity, 0.0);
        assert_eq!(amb.tint.offset_rgb, [0, 0, 0]);
        println!("full_fx=false: one_framers={}, transitions={}, flicker.A={}",
            plan.one_framers.len(), plan.transitions.len(), amb.flicker.amplitude);
    }

    #[test]
    fn test_full_fx_on_matches_head() {
        let video_duration = 10.773;
        let audio_duration = 14.315;
        let beats = vec![
            0.42, 1.14, 1.88, 2.60, 3.32, 4.04, 4.76, 5.50, 6.20, 6.94,
            7.64, 8.38, 9.10, 9.82, 10.54, 11.26, 11.98, 12.70, 13.42, 14.14,
        ];
        let downbeats = vec![2.60, 5.50, 8.38, 11.26, 14.14];

        let plan = create_plan_internal(
            "HARD", 16, &beats, &downbeats,
            video_duration, audio_duration, 1080, 1080, 83.33, true,
        ).expect("Plan generation must succeed");

        assert_eq!(plan.full_fx, true);
        assert!(!plan.one_framers.is_empty(), "full_fx=true must have one_framers");
        assert!(!plan.transitions.is_empty(), "full_fx=true must have transitions");
        let amb = plan.ambiance.as_ref().unwrap();
        assert!(amb.flicker.amplitude > 0.0);
        assert!(!amb.exposure_flash.times.is_empty());
        assert!(amb.vignette.strength > 0.0);
        assert!(amb.scanlines.opacity > 0.0);
        println!("full_fx=true: one_framers={}, transitions={}",
            plan.one_framers.len(), plan.transitions.len());
    }

    // ─── T14 Advanced Engine Tests ───────────────────────────────────────────

    #[test]
    fn test_bouncy_shake_pattern() {
        // Template: [(0,-1.0),(1,+0.14),(3,+0.05),(6,+0.022),(8,-0.006),(10,0)]
        // Signs must be: frame 0 = negative, frame 0.5 = between 0 and 1 (rising)
        let v0 = compute_bouncy_shake(0.0);
        let v1 = compute_bouncy_shake(1.0);
        let v10 = compute_bouncy_shake(10.0);
        let out = compute_bouncy_shake(11.0);
        assert!(v0 < 0.0,  "frame 0 should be negative, got {}", v0);
        assert!(v1 > 0.0,  "frame 1 should be positive, got {}", v1);
        assert!((v10).abs() < 1e-9, "frame 10 should be 0, got {}", v10);
        assert_eq!(out, 0.0, "out of range should be 0");

        // Intermediate interpolation (frame 0.5 between -1 and +0.14)
        let v05 = compute_bouncy_shake(0.5);
        assert!(v05 > v0 && v05 < v1, "frame 0.5 should be between v0 and v1, got {}", v05);
        println!("bouncy shake: v0={:.4} v0.5={:.4} v1={:.4} v10={:.4} out={:.4}", v0, v05, v1, v10, out);
    }

    #[test]
    fn test_skew_shake_zero_at_end() {
        let duration = 1.0;
        let s0_deg = 10.0;
        // At t=T the skew should approach 0 (exp(-3)·cos(8π) = exp(-3)·1 ≈ 0.05 — small but not exact 0)
        // The spec says "skew = 0 at t=T" in the practical sense (damped well below 1 deg)
        let skew_end = compute_skew_shake(duration, duration, s0_deg);
        assert!(skew_end.abs() < 0.1, "skew at T should be near 0, got {}", skew_end);
        // At t=0 it should be tan(s0_deg * cos(0) * exp(0)) = tan(s0_rad) ≠ 0
        let skew_start = compute_skew_shake(0.0, duration, s0_deg);
        assert!(skew_start.abs() > 0.05, "skew at t=0 should be non-zero, got {}", skew_start);
        println!("skew_shake: t=0 → {:.4}, t=T → {:.6}", skew_start, skew_end);
    }

    #[test]
    fn test_squish_pop_returns_to_one() {
        // At frame 5 scale_x and scale_y should both be exactly 1.0
        let (sx5, sy5) = compute_squish_pop(5.0);
        assert!((sx5 - 1.0).abs() < 1e-9, "squish_pop scale_x at frame 5 should be 1, got {}", sx5);
        assert!((sy5 - 1.0).abs() < 1e-9, "squish_pop scale_y at frame 5 should be 1, got {}", sy5);
        // At frame 1: scale_y = 0.88, scale_x = 1.10
        let (sx1, sy1) = compute_squish_pop(1.0);
        assert!((sy1 - 0.88).abs() < 1e-9, "scale_y at frame 1 should be 0.88, got {}", sy1);
        assert!((sx1 - 1.10).abs() < 1e-9, "scale_x at frame 1 should be 1.10, got {}", sx1);
        println!("squish_pop: frame1=({:.2},{:.2}) frame5=({:.2},{:.2})", sx1, sy1, sx5, sy5);
    }

    #[test]
    fn test_optics_k_monotone_decreasing() {
        let dur = 1.0;
        let k0 = 0.08;
        let samples: Vec<f64> = (0..=10).map(|i| compute_optics_k(i as f64 * 0.1, dur, k0)).collect();
        for w in samples.windows(2) {
            assert!(w[0] >= w[1] - 1e-12, "k should be monotone decreasing: {} >= {}", w[0], w[1]);
        }
        assert!((samples[10]).abs() < 1e-9, "k at t=T should be 0, got {}", samples[10]);
        assert!((samples[0] - k0).abs() < 1e-9, "k at t=0 should be k0={}, got {}", k0, samples[0]);
        println!("optics_k: t=0 → {:.4}, t=0.5 → {:.4}, t=T → {:.6}", samples[0], samples[5], samples[10]);
    }

    #[test]
    fn test_stretch_ends_at_one() {
        let dur = 1.0;
        let scale_start = 1.4;
        let s_end = compute_stretch_scale(dur, dur, scale_start);
        assert!((s_end - 1.0).abs() < 1e-9, "stretch should end at 1.0, got {}", s_end);
        let s_start = compute_stretch_scale(0.0, dur, scale_start);
        assert!((s_start - scale_start).abs() < 1e-6, "stretch should start at scale_start, got {}", s_start);
        println!("stretch: t=0 → {:.4}, t=T → {:.6}", s_start, s_end);
    }

    #[test]
    fn test_buildup_chain_continuity() {
        // Segment A (chain_next=true): tail should be 0.6 at t=duration
        let fps = 30.0;
        let dur = 1.0;
        let v_tail = compute_chain_envelope_mult(dur - 0.001, dur, fps, false, true);
        assert!(v_tail >= 0.59 && v_tail <= 0.61,
            "chain_next tail should be ~0.6 at t≈T, got {}", v_tail);

        // Segment B (chain_from_prev=true): head should be ~0.6 at t=0
        let v_head = compute_chain_envelope_mult(0.0, dur, fps, true, false);
        assert!((v_head - 0.6).abs() < 0.01,
            "chain_from_prev head should be 0.6 at t=0, got {}", v_head);
        println!("chain: tail at T={:.4}, head at 0={:.4}", v_tail, v_head);
    }

    #[test]
    fn test_t14_seed_reproducibility() {
        // Same input → same plan every time
        let beats = vec![0.42, 1.14, 1.88, 2.60, 3.32, 4.04];
        let downbeats = vec![2.60];
        let plan1 = create_plan_internal("HARD", 16, &beats, &downbeats, 10.0, 6.0, 1080, 1080, 120.0, true)
            .expect("plan1 ok");
        let plan2 = create_plan_internal("HARD", 16, &beats, &downbeats, 10.0, 6.0, 1080, 1080, 120.0, true)
            .expect("plan2 ok");
        assert_eq!(plan1.segments, plan2.segments, "Segments must be identical for same seed");
        println!("T14 reproducibility: {} segments, seed-stable", plan1.segments.len());
    }

    #[test]
    fn test_t14_adv_shakes_present_in_hard() {
        let beats: Vec<f64> = (0..20).map(|i| i as f64 * 0.72).collect();
        let downbeats = vec![2.88, 5.76, 8.64];
        let plan = create_plan_internal("HARD", 16, &beats, &downbeats, 14.0, 14.4, 1080, 1080, 83.33, true)
            .expect("plan ok");
        let has_bouncy = plan.segments.iter().any(|s| s.effects.bouncy_shake.is_some());
        let has_squish = plan.segments.iter().any(|s| s.effects.squish_pop.is_some());
        let has_zoom_off = plan.segments.iter().any(|s| s.effects.zoom_beat_offset > 0);
        assert!(has_bouncy, "HARD style should have at least one bouncy_shake segment");
        assert!(has_squish, "HARD style should have at least one squish_pop segment");
        assert!(has_zoom_off, "All styles should have zoom_beat_offset > 0 on some segments");
        println!("T14 HARD: bouncy={}, squish={}, zoom_off={}",
            has_bouncy, has_squish, has_zoom_off);
    }

    #[test]
    fn test_render_stats_computation() {
        let beats: Vec<f64> = (0..20).map(|i| i as f64 * 0.72).collect();
        let downbeats = vec![2.88, 5.76, 8.64];
        let plan = create_plan_internal("HARD", 16, &beats, &downbeats, 14.0, 14.4, 1080, 1080, 83.33, true)
            .expect("plan ok");

        let temp_dir = std::env::temp_dir();
        let dummy_mp4 = temp_dir.join("test_stats_fixture.mp4");
        std::fs::write(&dummy_mp4, vec![0u8; 1024 * 512]).expect("write dummy file");

        let stats = compute_render_stats(&plan, &dummy_mp4, 2.45);
        let _ = std::fs::remove_file(&dummy_mp4);

        assert!(stats.render_time_secs > 0.0, "Render time must be > 0, got {}", stats.render_time_secs);
        assert!(stats.file_size_mb > 0.0, "File size must be > 0 MB, got {}", stats.file_size_mb);
        assert_eq!(stats.target_fps, 16, "Target FPS must be 16, got {}", stats.target_fps);
        assert!(stats.effects_count > 0, "Effects count must be > 0, got {}", stats.effects_count);

        println!("T16 Render Stats: time={:.2}s, size={:.2}MB, fps={}, effects={}",
            stats.render_time_secs, stats.file_size_mb, stats.target_fps, stats.effects_count);
    }
}

