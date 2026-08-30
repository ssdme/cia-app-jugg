use tauri::Manager;
use crate::effects::{
    default_ambiance, default_segment_effects, deterministic_hash_pos, AmbianceConfig,
    AspectRatio, BuildupChain, BouncyShake, DissolveShake, OneFramer, OpticsBounce,
    SegmentEffects, SegmentTransition, ShakeEffect, SkewShake, SquishPop, TransitionItem,
    WarpStretch, ZoomEffect, ONE_FRAMER_TYPES,
};

fn default_true() -> bool {
    true
}

fn default_false() -> bool {
    false
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EffectOverrides {
    #[serde(default = "default_true")]
    pub shakes: bool,
    #[serde(default = "default_true")]
    pub zoom: bool,
    #[serde(default = "default_true")]
    pub flicker: bool,
    #[serde(default = "default_true")]
    pub one_framers: bool,
    #[serde(default = "default_true")]
    pub transitions: bool,
    #[serde(default = "default_true")]
    pub tint: bool,
    #[serde(default = "default_true")]
    pub vignette: bool,
    #[serde(default = "default_false")]
    pub scanlines: bool,
    #[serde(default = "default_false")]
    pub echo_trail: bool,
    #[serde(default = "default_true")]
    pub exposure_flash: bool,
    #[serde(default = "default_true")]
    pub bouncy_shake: bool,
    #[serde(default = "default_true")]
    pub dissolve_shake: bool,
    #[serde(default = "default_true")]
    pub skew_shake: bool,
    #[serde(default = "default_true")]
    pub squish_pop: bool,
    #[serde(default = "default_true")]
    pub optics_bounce: bool,
    #[serde(default = "default_true")]
    pub buildup_chain: bool,
    #[serde(default = "default_true")]
    pub warp_stretch: bool,
    #[serde(default = "default_true")]
    pub zoom_beat_offset: bool,
}

impl Default for EffectOverrides {
    fn default() -> Self {
        Self {
            shakes: true,
            zoom: true,
            flicker: true,
            one_framers: true,
            transitions: true,
            tint: true,
            vignette: true,
            scanlines: false,
            echo_trail: false,
            exposure_flash: true,
            bouncy_shake: true,
            dissolve_shake: true,
            skew_shake: true,
            squish_pop: true,
            optics_bounce: true,
            buildup_chain: true,
            warp_stretch: true,
            zoom_beat_offset: true,
        }
    }
}

pub fn default_effects_for_style(style: &str, full_fx: bool) -> EffectOverrides {
    let style_up = style.to_uppercase();
    let is_smooth = style_up == "SMOOTH";
    let is_hybrid = style_up == "HYBRID";
    let is_hard = !is_smooth && !is_hybrid;

    EffectOverrides {
        shakes: true,
        zoom: true,
        flicker: true,
        one_framers: full_fx && (is_hard || is_hybrid),
        transitions: true,
        tint: full_fx,
        vignette: full_fx,
        scanlines: false,
        echo_trail: false,
        exposure_flash: full_fx && is_hard,
        bouncy_shake: is_hard || is_hybrid,
        dissolve_shake: is_hard || is_hybrid,
        skew_shake: is_hard || is_hybrid,
        squish_pop: is_hard || is_hybrid,
        optics_bounce: is_hard || is_hybrid,
        buildup_chain: true,
        warp_stretch: is_hard || is_hybrid,
        zoom_beat_offset: true,
    }
}

// ─── T18 Custom Style Parameters Engine ─────────────────────────────────────

fn default_hard_shake_a0() -> f64 { 8.0 }
fn default_hard_shake_omega() -> f64 { 15.0 }
fn default_hard_shake_k() -> f64 { 3.0 }
fn default_hard_bouncy_amp() -> f64 { 40.0 }
fn default_hard_dissolve_pct() -> f64 { 30.0 }
fn default_hard_skew_s0() -> f64 { 10.0 }
fn default_squish_scale_y_min() -> f64 { 0.85 }
fn default_squish_scale_x_max() -> f64 { 1.18 }
fn default_optics_k0() -> f64 { 0.08 }
fn default_stretch_scale() -> f64 { 1.40 }
fn default_zoom_start() -> f64 { 1.0 }
fn default_zoom_end() -> f64 { 1.15 }
fn default_zoom_beat_offset() -> u32 { 1 }
fn default_flicker_amplitude() -> f64 { 0.15 }
fn default_flicker_frequency() -> f64 { 12.0 }
fn default_exposure_flash_peak() -> f64 { 0.5 }
fn default_echo_alpha() -> f64 { 0.3 }
fn default_echo_k_depth() -> u32 { 3 }
fn default_vignette_strength() -> f64 { 0.3 }
fn default_scanlines_opacity() -> f64 { 0.15 }
fn default_warp_bubble_amplitude() -> f64 { 0.5 }
fn default_warp_bubble_frequency() -> f64 { 1.2 }
fn default_wave_warp_height() -> f64 { 280.0 }
fn default_wave_warp_speed() -> f64 { 20.0 }
fn default_slide_shake_pixels() -> f64 { 100.0 }

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CustomParams {
    // SHAKES
    #[serde(default = "default_hard_shake_a0")]
    pub shake_a0: f64,
    #[serde(default = "default_hard_shake_omega")]
    pub shake_omega: f64,
    #[serde(default = "default_hard_shake_k")]
    pub shake_k: f64,
    #[serde(default = "default_hard_bouncy_amp")]
    pub bouncy_amplitude: f64,
    #[serde(default = "default_hard_dissolve_pct")]
    pub dissolve_pct: f64,
    #[serde(default = "default_hard_skew_s0")]
    pub skew_s0: f64,
    #[serde(default = "default_squish_scale_y_min")]
    pub squish_scale_y_min: f64,
    #[serde(default = "default_squish_scale_x_max")]
    pub squish_scale_x_max: f64,
    #[serde(default = "default_optics_k0")]
    pub optics_k0: f64,
    #[serde(default = "default_stretch_scale")]
    pub stretch_scale: f64,

    // ZOOM
    #[serde(default = "default_zoom_start")]
    pub zoom_scale_start: f64,
    #[serde(default = "default_zoom_end")]
    pub zoom_scale_end: f64,
    #[serde(default = "default_zoom_beat_offset")]
    pub zoom_beat_offset_frames: u32,

    // AMBIANCE
    #[serde(default = "default_flicker_amplitude")]
    pub flicker_amplitude: f64,
    #[serde(default = "default_flicker_frequency")]
    pub flicker_frequency_hz: f64,
    #[serde(default = "default_exposure_flash_peak")]
    pub exposure_flash_peak: f64,
    #[serde(default = "default_echo_alpha")]
    pub echo_alpha: f64,
    #[serde(default = "default_echo_k_depth")]
    pub echo_k_depth: u32,
    #[serde(default)]
    pub tint_r_offset: i16,
    #[serde(default)]
    pub tint_g_offset: i16,
    #[serde(default)]
    pub tint_b_offset: i16,
    #[serde(default = "default_vignette_strength")]
    pub vignette_strength: f64,
    #[serde(default = "default_scanlines_opacity")]
    pub scanlines_opacity: f64,

    // TRANSITIONS
    #[serde(default = "default_warp_bubble_amplitude")]
    pub warp_bubble_amplitude: f64,
    #[serde(default = "default_warp_bubble_frequency")]
    pub warp_bubble_frequency: f64,
    #[serde(default = "default_wave_warp_height")]
    pub wave_warp_height: f64,
    #[serde(default = "default_wave_warp_speed")]
    pub wave_warp_speed: f64,
    #[serde(default = "default_slide_shake_pixels")]
    pub slide_shake_pixels: f64,
}

impl Default for CustomParams {
    fn default() -> Self {
        get_style_defaults("HARD")
    }
}

pub fn get_style_defaults(style: &str) -> CustomParams {
    let style_up = style.to_uppercase();
    match style_up.as_str() {
        "SMOOTH" => CustomParams {
            shake_a0: 3.0,
            shake_omega: 8.0,
            shake_k: 2.0,
            bouncy_amplitude: 25.0,
            dissolve_pct: 15.0,
            skew_s0: 0.0,
            squish_scale_y_min: 0.92,
            squish_scale_x_max: 1.08,
            optics_k0: 0.04,
            stretch_scale: 1.20,

            zoom_scale_start: 1.0,
            zoom_scale_end: 1.05,
            zoom_beat_offset_frames: 0,

            flicker_amplitude: 0.08,
            flicker_frequency_hz: 8.0,
            exposure_flash_peak: 0.3,
            echo_alpha: 0.2,
            echo_k_depth: 2,
            tint_r_offset: 0,
            tint_g_offset: 0,
            tint_b_offset: 0,
            vignette_strength: 0.2,
            scanlines_opacity: 0.10,

            warp_bubble_amplitude: 0.3,
            warp_bubble_frequency: 1.0,
            wave_warp_height: 180.0,
            wave_warp_speed: 15.0,
            slide_shake_pixels: 60.0,
        },
        "HYBRID" => CustomParams {
            shake_a0: 5.0,
            shake_omega: 12.0,
            shake_k: 2.5,
            bouncy_amplitude: 25.0,
            dissolve_pct: 15.0,
            skew_s0: 7.0,
            squish_scale_y_min: 0.88,
            squish_scale_x_max: 1.14,
            optics_k0: 0.06,
            stretch_scale: 1.30,

            zoom_scale_start: 1.0,
            zoom_scale_end: 1.10,
            zoom_beat_offset_frames: 1,

            flicker_amplitude: 0.12,
            flicker_frequency_hz: 10.0,
            exposure_flash_peak: 0.4,
            echo_alpha: 0.25,
            echo_k_depth: 3,
            tint_r_offset: 0,
            tint_g_offset: 0,
            tint_b_offset: 0,
            vignette_strength: 0.25,
            scanlines_opacity: 0.12,

            warp_bubble_amplitude: 0.4,
            warp_bubble_frequency: 1.1,
            wave_warp_height: 230.0,
            wave_warp_speed: 18.0,
            slide_shake_pixels: 80.0,
        },
        _ => CustomParams {
            shake_a0: 8.0,
            shake_omega: 15.0,
            shake_k: 3.0,
            bouncy_amplitude: 40.0,
            dissolve_pct: 30.0,
            skew_s0: 10.0,
            squish_scale_y_min: 0.85,
            squish_scale_x_max: 1.18,
            optics_k0: 0.08,
            stretch_scale: 1.40,

            zoom_scale_start: 1.0,
            zoom_scale_end: 1.15,
            zoom_beat_offset_frames: 1,

            flicker_amplitude: 0.15,
            flicker_frequency_hz: 12.0,
            exposure_flash_peak: 0.5,
            echo_alpha: 0.3,
            echo_k_depth: 3,
            tint_r_offset: 0,
            tint_g_offset: 0,
            tint_b_offset: 0,
            vignette_strength: 0.3,
            scanlines_opacity: 0.15,

            warp_bubble_amplitude: 0.5,
            warp_bubble_frequency: 1.2,
            wave_warp_height: 280.0,
            wave_warp_speed: 20.0,
            slide_shake_pixels: 100.0,
        },
    }
}

// ─── T19 Export Config ────────────────────────────────────────────────────────

fn default_export_codec() -> String { "H264".to_string() }
fn default_export_bitrate() -> u32 { 12 }
fn default_export_format() -> String { "MP4".to_string() }

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExportConfig {
    #[serde(default = "default_export_codec")]
    pub codec: String,         // "H264" | "H265" | "VP9"
    #[serde(default = "default_export_bitrate")]
    pub bitrate_mbps: u32,     // 5..=50
    #[serde(default = "default_export_format")]
    pub format: String,        // "MP4" | "MKV" | "WEBM"
}

impl Default for ExportConfig {
    fn default() -> Self {
        ExportConfig {
            codec: default_export_codec(),
            bitrate_mbps: default_export_bitrate(),
            format: default_export_format(),
        }
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
    #[serde(default)]
    pub transition: Option<SegmentTransition>,
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
    #[serde(default = "default_true")]
    pub full_fx: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_params: Option<CustomParams>,
    #[serde(default)]
    pub one_framers: Vec<OneFramer>,
    #[serde(default)]
    pub transitions: Vec<TransitionItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ambiance: Option<AmbianceConfig>,
    pub segments: Vec<PlanSegment>,
    #[serde(default)]
    pub export: ExportConfig,
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

pub fn generate_transitions(
    style: &str,
    segments: &mut [PlanSegment],
    wrap_indices: &[usize],
    _fps: u32,
    custom_params: Option<&CustomParams>,
) -> Vec<TransitionItem> {
    let mut transitions = Vec::new();
    let style_upper = style.to_uppercase();

    let wb_amp = custom_params.map_or(0.5, |c| c.warp_bubble_amplitude);
    let wb_freq = custom_params.map_or(1.2, |c| c.warp_bubble_frequency);
    let ww_height = custom_params.map_or(280.0, |c| c.wave_warp_height);
    let ww_speed = custom_params.map_or(20.0, |c| c.wave_warp_speed);
    let ss_amp = custom_params.map_or(100.0, |c| c.slide_shake_pixels);

    // 1. Systematically place WARP_BUBBLE on wraps
    for &wrap_idx in wrap_indices {
        if wrap_idx < segments.len() {
            let seg = &mut segments[wrap_idx];
            let t_cut = seg.t0;
            seg.transition = Some(SegmentTransition {
                transition_type: "WARP_BUBBLE".to_string(),
                params: serde_json::json!({
                    "amplitude": wb_amp,
                    "frequency": wb_freq,
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
                    "amplitude": wb_amp,
                    "frequency": wb_freq,
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
                "WARP_BUBBLE" => (4, serde_json::json!({ "amplitude": wb_amp, "frequency": wb_freq })),
                "WAVE_WARP" => (6, serde_json::json!({ "height": ww_height, "speed": ww_speed })),
                "SLIDE_SHAKE" => (6, serde_json::json!({ "amplitude": ss_amp })),
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
    effect_overrides: Option<EffectOverrides>,
    custom_params: Option<CustomParams>,
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

    let overrides = effect_overrides.unwrap_or_else(|| default_effects_for_style(style, full_fx));
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
    let (default_a0, default_omega, default_k, default_zoom_max) = match style.to_uppercase().as_str() {
        "SMOOTH" => (3.0, 8.0, 2.0, 1.05),
        "HYBRID" => (5.0, 12.0, 2.5, 1.10),
        _ => (8.0, 15.0, 3.0, 1.15), // HARD default
    };

    let a0 = custom_params.as_ref().map_or(default_a0, |c| c.shake_a0);
    let omega = custom_params.as_ref().map_or(default_omega, |c| c.shake_omega);
    let k = custom_params.as_ref().map_or(default_k, |c| c.shake_k);
    let zoom_start = custom_params.as_ref().map_or(1.0, |c| c.zoom_scale_start);
    let zoom_max = custom_params.as_ref().map_or(default_zoom_max, |c| c.zoom_scale_end);

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

            // Zoom continuity: alternating between zoom_start and zoom_max
            let (mut scale_start, mut scale_end) = if seg_index % 2 == 0 {
                (zoom_start, zoom_max)
            } else {
                (zoom_max, zoom_start)
            };
            if !overrides.zoom {
                scale_start = 1.0;
                scale_end = 1.0;
            }

            let seed = ((seg_index as u32).wrapping_mul(1664525).wrapping_add(1013904223)) ^ 0x5bf03635;

            // ── T14 engine generation ─────────────────────────────────────────
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
                default_bouncy_prob, default_bouncy_amp,
                default_dissolve_prob, default_dissolve_pct_val,
                default_skew_prob, default_skew_s0,
                default_squish_prob,
                default_optics_prob,
                default_chain_prob,
                default_stretch_prob,
                default_stretch_scale,
            ) = match style_up.as_str() {
                "SMOOTH" => (
                    if overrides.bouncy_shake { 30u32 } else { 0u32 }, 25.0f64,
                    if overrides.dissolve_shake { 25u32 } else { 0u32 }, 15.0f64,
                    if overrides.skew_shake { 20u32 } else { 0u32 }, 7.0f64,
                    if overrides.squish_pop { 40u32 } else { 0u32 },
                    if overrides.optics_bounce { 25u32 } else { 0u32 },
                    if overrides.buildup_chain { 30u32 } else { 0u32 },
                    if overrides.warp_stretch { 20u32 } else { 0u32 },
                    1.20f64,
                ),
                "HYBRID" => (
                    if overrides.bouncy_shake { 15u32 } else { 0u32 }, 25.0,
                    if overrides.dissolve_shake { 10u32 } else { 0u32 }, 15.0,
                    if overrides.skew_shake { 10u32 } else { 0u32 }, 7.0,
                    if overrides.squish_pop { 20u32 } else { 0u32 },
                    if overrides.optics_bounce { 10u32 } else { 0u32 },
                    if overrides.buildup_chain { 15u32 } else { 0u32 },
                    if overrides.warp_stretch { 10u32 } else { 0u32 },
                    1.30f64,
                ),
                _ => (
                    if overrides.bouncy_shake { 30u32 } else { 0u32 }, 40.0,
                    if overrides.dissolve_shake { 25u32 } else { 0u32 }, 30.0,
                    if overrides.skew_shake { 20u32 } else { 0u32 }, 10.0,
                    if overrides.squish_pop { 40u32 } else { 0u32 },
                    if overrides.optics_bounce { 25u32 } else { 0u32 },
                    if overrides.buildup_chain { 30u32 } else { 0u32 },
                    if overrides.warp_stretch { 20u32 } else { 0u32 },
                    1.40f64,
                ),
            };

            let bouncy_amp = custom_params.as_ref().map_or(default_bouncy_amp, |c| c.bouncy_amplitude);
            let dissolve_pct_val = custom_params.as_ref().map_or(default_dissolve_pct_val, |c| c.dissolve_pct);
            let skew_s0 = custom_params.as_ref().map_or(default_skew_s0, |c| c.skew_s0);
            let optics_k0_val = custom_params.as_ref().map_or(0.08, |c| c.optics_k0);
            let stretch_scale_val = custom_params.as_ref().map_or(default_stretch_scale, |c| c.stretch_scale);

            let bouncy_shake = if default_bouncy_prob > 0 && pct(s1) < default_bouncy_prob {
                Some(BouncyShake { axis: (s1 % 2) as u8, amplitude: bouncy_amp })
            } else {
                None
            };

            let mut effective_a0 = match custom_params.as_ref() {
                Some(cp) => cp.shake_a0,
                None => if bouncy_shake.is_some() { 0.0 } else { a0 },
            };
            if !overrides.shakes {
                effective_a0 = 0.0;
            }

            let dissolve_shake = if default_dissolve_prob > 0 && pct(s2) < default_dissolve_prob {
                Some(DissolveShake { pct: dissolve_pct_val })
            } else {
                None
            };

            let skew_shake = if default_skew_prob > 0 && pct(s3) < default_skew_prob {
                Some(SkewShake { s0_deg: skew_s0 })
            } else {
                None
            };

            let squish_pop = if default_squish_prob > 0 && pct(s4) < default_squish_prob {
                Some(SquishPop { _pad: 0 })
            } else {
                None
            };

            let optics_bounce = if default_optics_prob > 0 && pct(s5) < default_optics_prob {
                Some(OpticsBounce { k0: optics_k0_val })
            } else {
                None
            };

            let buildup_chain = if default_chain_prob > 0 && pct(s6) < default_chain_prob {
                Some(BuildupChain { chain_next: true, chain_from_prev: false })
            } else {
                None
            };

            let warp_stretch = if default_stretch_prob > 0 && pct(s7) < default_stretch_prob {
                Some(WarpStretch { axis: (s7 % 2) as u8, scale_start: stretch_scale_val })
            } else {
                None
            };

            let zoom_beat_offset = if overrides.zoom_beat_offset {
                custom_params.as_ref().map_or(s8 % 3, |c| c.zoom_beat_offset_frames)
            } else {
                0
            };

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
                    t0: seg_t0,
                    t1: seg_t1,
                    s0: (s0 * 1000.0).round() / 1000.0,
                    s1: (s1 * 1000.0).round() / 1000.0,
                    curve: curve_name.clone(),
                    effects,
                    transition: None,
                });
                seg_index += 1;
                seg_t0 = seg_t1;
            } else {
                let available = video_duration - s_cursor;
                if available > 1e-6 {
                    let dt_sub = available / r;
                    let mut s0 = s_cursor;
                    let mut s1 = video_duration;
                    if reverse_this_segment {
                        std::mem::swap(&mut s0, &mut s1);
                    }

                    segments.push(PlanSegment {
                        t0: seg_t0,
                        t1: seg_t0 + dt_sub,
                        s0: (s0 * 1000.0).round() / 1000.0,
                        s1: (s1 * 1000.0).round() / 1000.0,
                        curve: curve_name.clone(),
                        effects: effects.clone(),
                        transition: None,
                    });
                    seg_index += 1;
                    seg_t0 += dt_sub;
                }
                s_cursor = 0.0;
                loops += 1;
                wrap_indices.push(segments.len());
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

    let one_framers = if overrides.one_framers {
        generate_one_framers(style, &segments, downbeats, fps, target_dur)
    } else {
        vec![]
    };
    let transitions = if overrides.transitions {
        generate_transitions(style, &mut segments, &wrap_indices, fps, custom_params.as_ref())
    } else {
        vec![]
    };

    let has_any_ambiance = overrides.flicker
        || overrides.exposure_flash
        || overrides.echo_trail
        || overrides.tint
        || overrides.vignette
        || overrides.scanlines;

    let ambiance = if has_any_ambiance {
        let mut a = default_ambiance(style, downbeats);
        if overrides.tint {
            let default_tint = {
                let seed = 0x9e3779b9u32;
                let r = ((seed.wrapping_mul(1664525).wrapping_add(1013904223)) % 21) as i16 - 10;
                let g = ((seed.wrapping_mul(22695477).wrapping_add(1)) % 11) as i16 - 5;
                let b = ((seed.wrapping_mul(6364136223846793005u64 as u32).wrapping_add(1442695040)) % 17) as i16 - 8;
                [r, g, b]
            };
            a.tint.offset_rgb = custom_params.as_ref().map_or(default_tint, |c| [c.tint_r_offset, c.tint_g_offset, c.tint_b_offset]);
        } else {
            a.tint.offset_rgb = [0, 0, 0];
        }
        if overrides.flicker {
            if let Some(c) = custom_params.as_ref() {
                a.flicker.amplitude = c.flicker_amplitude;
                a.flicker.f = c.flicker_frequency_hz;
            }
        } else {
            a.flicker.amplitude = 0.0;
        }
        if overrides.exposure_flash {
            if let Some(c) = custom_params.as_ref() {
                a.exposure_flash.peak = c.exposure_flash_peak;
            }
        } else {
            a.exposure_flash.times.clear();
            a.exposure_flash.peak = 0.0;
        }
        if overrides.echo_trail {
            if let Some(c) = custom_params.as_ref() {
                a.echo_trail.enabled = true;
                a.echo_trail.alpha = c.echo_alpha;
                a.echo_trail.k = c.echo_k_depth;
            }
        } else {
            a.echo_trail.enabled = false;
        }
        if overrides.vignette {
            if let Some(c) = custom_params.as_ref() {
                a.vignette.strength = c.vignette_strength;
            }
        } else {
            a.vignette.strength = 0.0;
        }
        if overrides.scanlines {
            if let Some(c) = custom_params.as_ref() {
                a.scanlines.opacity = c.scanlines_opacity;
            }
        } else {
            a.scanlines.opacity = 0.0;
        }
        Some(a)
    } else {
        None
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
        custom_params,
        one_framers,
        transitions,
        ambiance,
        segments,
        export: ExportConfig::default(),
    })
}

#[tauri::command]
pub fn generate_plan(
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
    effect_overrides: Option<EffectOverrides>,
    custom_params: Option<CustomParams>,
    export_config: Option<ExportConfig>,
) -> Result<String, String> {
    let mut plan = create_plan_internal(
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
        effect_overrides,
        custom_params,
    )?;
    if let Some(ec) = export_config {
        plan.export = ec;
    }
    serde_json::to_string_pretty(&plan).map_err(|e| format!("Failed to serialize plan: {e}"))
}

#[tauri::command]
pub fn cmd_get_style_defaults(style: String) -> Result<CustomParams, String> {
    Ok(get_style_defaults(&style))
}

#[tauri::command]
pub fn save_plan(app: tauri::AppHandle, plan_json: String) -> Result<String, String> {
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
