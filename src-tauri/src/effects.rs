use crate::probe::{mirror_coordinate, sample_pixel_mirrored};

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

pub fn default_shake() -> ShakeEffect {
    ShakeEffect {
        a0: 0.0,
        omega: 0.0,
        k: 0.0,
        seed: 0,
    }
}

pub fn default_zoom() -> ZoomEffect {
    ZoomEffect {
        scale_start: 1.0,
        scale_end: 1.0,
    }
}

pub fn default_segment_effects() -> SegmentEffects {
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
    #[serde(default)]
    pub invert_bw: bool,
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

pub fn default_ambiance(style: &str, downbeats: &[f64]) -> AmbianceConfig {
    let (amp, freq, flash_peak) = match style.to_uppercase().as_str() {
        "SMOOTH" => (0.08, 8.0, 0.3),
        "HYBRID" => (0.12, 10.0, 0.4),
        _ => (0.15, 12.0, 0.5), // HARD
    };
    AmbianceConfig {
        flicker: FlickerConfig { amplitude: amp, f: freq, phase: 0.0 },
        exposure_flash: ExposureFlashConfig { peak: flash_peak, times: downbeats.to_vec() },
        echo_trail: EchoTrailConfig { enabled: false, alpha: 0.3, k: 3 },
        tint: TintConfig { offset_rgb: [0; 3], invert_bw: false },
        vignette: VignetteConfig { strength: 0.3 },
        scanlines: ScanlinesConfig { opacity: 0.0 },
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

pub fn compute_skew_shake(t_rel: f64, duration: f64, s0_deg: f64) -> f64 {
    if duration <= 1e-9 { return 0.0; }
    let s0_rad = s0_deg * std::f64::consts::PI / 180.0;
    let u = t_rel / duration;
    let angle = s0_rad * (-3.0 * u).exp() * (2.0 * std::f64::consts::PI * 4.0 * u).cos();
    angle.tan()
}

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

pub fn compute_optics_k(t_rel: f64, duration: f64, k0: f64) -> f64 {
    if duration <= 1e-9 { return 0.0; }
    let u = (1.0 - t_rel / duration).clamp(0.0, 1.0);
    k0 * u * u
}

pub fn compute_stretch_scale(t_rel: f64, duration: f64, scale_start: f64) -> f64 {
    if duration <= 1e-9 { return 1.0; }
    let u = (t_rel / duration).clamp(0.0, 1.0);
    let saddle = u * u * (3.0 - 2.0 * u);
    scale_start + (1.0 - scale_start) * saddle
}

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
        if t_rel < ramp_dur { 0.6 + 0.4 * (t_rel / ramp_dur).clamp(0.0, 1.0) } else { 1.0 }
    } else { 1.0 };
    let tail_mult = if chain_next {
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
    pub skew_x: f64,
    pub scale_y: f64,
    pub scale_x: f64,
    pub barrel_k: f64,
}

pub fn compute_transform_params(
    effects: &SegmentEffects,
    t_rel: f64,
    seg_dur: f64,
    fps: f64,
) -> TransformParams {
    let frame_dur = if fps > 0.0 { 1.0 / fps } else { 1.0 / 30.0 };

    let t_rel_zoom = if effects.zoom_beat_offset > 0 {
        (t_rel - (effects.zoom_beat_offset as f64) * frame_dur).max(0.0)
    } else {
        t_rel
    };

    let base_env = compute_shake_envelope(t_rel, seg_dur, fps);
    let chain_mult = if let Some(ref bc) = effects.buildup_chain {
        compute_chain_envelope_mult(t_rel, seg_dur, fps, bc.chain_from_prev, bc.chain_next)
    } else { 1.0 };
    let env = base_env * chain_mult;

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

    if let Some(ref bouncy) = effects.bouncy_shake {
        let frame_idx = t_rel / frame_dur;
        let bouncy_val = compute_bouncy_shake(frame_idx) * bouncy.amplitude;
        if bouncy.axis == 0 { dx = bouncy_val; } else { dy = bouncy_val; }
    }

    let x_zoom = (t_rel_zoom / seg_dur.max(1e-6)).clamp(0.0, 1.0);
    let base_scale = effects.zoom.scale_start + (effects.zoom.scale_end - effects.zoom.scale_start) * x_zoom;
    let total_scale = (base_scale * (1.0 + dz)).max(0.1);

    let (mut scale_x, mut scale_y) = (1.0f64, 1.0f64);
    if let Some(ref ws) = effects.warp_stretch {
        let stretch = compute_stretch_scale(t_rel, seg_dur, ws.scale_start);
        if ws.axis == 0 { scale_x = stretch; } else { scale_y = stretch; }
    }

    if effects.squish_pop.is_some() {
        let frame_idx = t_rel / frame_dur;
        let (sx, sy) = compute_squish_pop(frame_idx);
        scale_x = scale_x * sx;
        scale_y = scale_y * sy;
    }

    let skew_x = if let Some(ref sk) = effects.skew_shake {
        compute_skew_shake(t_rel, seg_dur, sk.s0_deg) * env
    } else { 0.0 };

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

    let w_i32 = src_width as i32;
    let h_i32 = src_height as i32;

    let cw = crop_width as usize;
    let ch = crop_height as usize;

    for yd in 0..ch {
        let yd_full = (crop_y as usize) + yd;
        let yd_rel = (yd_full as f64) - cy;
        let xd_start_rel = (crop_x as f64) - cx;

        let skew_offset = params.skew_x * yd_rel;
        let yd_rel_scaled = yd_rel * inv_sy;

        let base_xs = cx - params.dx + skew_offset + inv_s * (xd_start_rel * cos_t + yd_rel_scaled * sin_t);
        let base_ys = cy - params.dy + inv_s * (-xd_start_rel * sin_t * inv_sx + yd_rel_scaled * cos_t);

        let mut xs_fp = (base_xs * 65536.0 + 32768.0) as i32;
        let mut ys_fp = (base_ys * 65536.0 + 32768.0) as i32;

        let row_out_start = yd * cw * 3;
        let row_out = &mut frame_crop_out[row_out_start..row_out_start + cw * 3];

        let step_xs_fp_sx = (step_x_to_xs * inv_sx * 65536.0).round() as i32;
        let step_ys_fp_sx = (step_x_to_ys * inv_sx * 65536.0).round() as i32;

        for xd in 0..cw {
            let mut xs = xs_fp >> 16;
            let mut ys = ys_fp >> 16;

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
    seg: &crate::plan::PlanSegment,
    fps: f64,
) {
    let n = width * height * 3;

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

                if amb.tint.invert_bw {
                    let gray = ((r * 77 + g * 150 + b * 29) >> 8) as u32;
                    let inv = 255 - gray.min(255);
                    r = inv;
                    g = inv;
                    b = inv;
                }

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
    } else if scale_fp == 256 {
        for py in 0..height {
            let is_scanline = py % 4 == 0;
            let row_offset = py * width;
            for px in 0..width {
                let idx = (row_offset + px) * 3;
                let v = vignette_lut[row_offset + px] as u32;
                let combined = if is_scanline { (v * dim_fp) >> 8 } else { v };

                let mut r = frame_in[idx] as u32;
                let mut g = frame_in[idx + 1] as u32;
                let mut b = frame_in[idx + 2] as u32;

                if amb.tint.invert_bw {
                    let gray = ((r * 77 + g * 150 + b * 29) >> 8) as u32;
                    let inv = 255 - gray.min(255);
                    r = inv;
                    g = inv;
                    b = inv;
                }

                r = ((r as i32 + tr as i32).clamp(0, 255)) as u32;
                g = ((g as i32 + tg as i32).clamp(0, 255)) as u32;
                b = ((b as i32 + tb as i32).clamp(0, 255)) as u32;

                frame_out[idx]     = ((r * combined) >> 8) as u8;
                frame_out[idx + 1] = ((g * combined) >> 8) as u8;
                frame_out[idx + 2] = ((b * combined) >> 8) as u8;
            }
        }
    } else {
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

                if amb.tint.invert_bw {
                    let gray = ((r * 77 + g * 150 + b * 29) >> 8) as u32;
                    let inv = 255 - gray.min(255);
                    r = inv;
                    g = inv;
                    b = inv;
                }

                r = ((r as i32 + tr as i32).clamp(0, 255)) as u32;
                g = ((g as i32 + tg as i32).clamp(0, 255)) as u32;
                b = ((b as i32 + tb as i32).clamp(0, 255)) as u32;

                frame_out[idx]     = ((r * combined) >> 8) as u8;
                frame_out[idx + 1] = ((g * combined) >> 8) as u8;
                frame_out[idx + 2] = ((b * combined) >> 8) as u8;
            }
        }
    }
}
