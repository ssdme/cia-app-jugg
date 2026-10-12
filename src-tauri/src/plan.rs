use crate::effects::{
    default_ambiance, default_segment_effects, deterministic_hash_pos, AmbianceConfig, AspectRatio,
    BouncyShake, BuildupChain, DissolveShake, OneFramer, OpticsBounce, SegmentEffects,
    SegmentTransition, ShakeEffect, SkewShake, SquishPop, TransitionItem, WarpStretch, ZoomEffect,
    ANTI_FLASH_ONE_FRAMER_TYPES, ONE_FRAMER_TYPES,
};
use tauri::Manager;

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
    #[serde(default = "default_false")]
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
    #[serde(default = "default_false")]
    pub cc_deep_dark: bool,
    #[serde(default = "default_false", alias = "antiFlash")]
    pub anti_flash: bool,
}

impl Default for EffectOverrides {
    fn default() -> Self {
        Self {
            shakes: true,
            zoom: true,
            flicker: true,
            one_framers: true,
            transitions: false,
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
            cc_deep_dark: false,
            anti_flash: false,
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
        one_framers: full_fx,
        transitions: false,
        tint: full_fx,
        vignette: full_fx,
        scanlines: false,
        echo_trail: false,
        exposure_flash: full_fx && (is_hard || is_hybrid),
        bouncy_shake: is_hard || is_hybrid,
        dissolve_shake: is_hard || is_hybrid,
        skew_shake: is_hard || is_hybrid,
        squish_pop: is_hard || is_hybrid,
        optics_bounce: is_hard || is_hybrid,
        buildup_chain: true,
        warp_stretch: is_hard || is_hybrid,
        zoom_beat_offset: true,
        cc_deep_dark: false,
        anti_flash: false,
    }
}

// ─── T18 Custom Style Parameters Engine ─────────────────────────────────────

fn default_hard_shake_a0() -> f64 {
    8.0
}
fn default_hard_shake_omega() -> f64 {
    15.0
}
fn default_hard_shake_k() -> f64 {
    3.0
}
fn default_hard_bouncy_amp() -> f64 {
    40.0
}
fn default_hard_dissolve_pct() -> f64 {
    30.0
}
fn default_hard_skew_s0() -> f64 {
    10.0
}
fn default_squish_scale_y_min() -> f64 {
    0.85
}
fn default_squish_scale_x_max() -> f64 {
    1.18
}
fn default_optics_k0() -> f64 {
    0.08
}
fn default_stretch_scale() -> f64 {
    1.40
}
fn default_zoom_start() -> f64 {
    1.0
}
fn default_zoom_end() -> f64 {
    1.15
}
fn default_zoom_beat_offset() -> u32 {
    1
}
fn default_flicker_amplitude() -> f64 {
    0.15
}
fn default_flicker_frequency() -> f64 {
    12.0
}
fn default_exposure_flash_peak() -> f64 {
    0.5
}
fn default_echo_alpha() -> f64 {
    0.3
}
fn default_echo_k_depth() -> u32 {
    3
}
fn default_vignette_strength() -> f64 {
    0.3
}
fn default_scanlines_opacity() -> f64 {
    0.15
}
fn default_warp_bubble_amplitude() -> f64 {
    0.5
}
fn default_warp_bubble_frequency() -> f64 {
    1.2
}
fn default_wave_warp_height() -> f64 {
    280.0
}
fn default_wave_warp_speed() -> f64 {
    20.0
}
fn default_slide_shake_pixels() -> f64 {
    100.0
}

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

fn default_export_codec() -> String {
    "H264".to_string()
}
fn default_export_bitrate() -> u32 {
    12
}
fn default_export_format() -> String {
    "MP4".to_string()
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExportConfig {
    #[serde(default = "default_export_codec")]
    pub codec: String, // "H264" | "H265" | "VP9"
    #[serde(default = "default_export_bitrate")]
    pub bitrate_mbps: u32, // 5..=50
    #[serde(default = "default_export_format")]
    pub format: String, // "MP4" | "MKV" | "WEBM"
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
pub struct RemapEnvelope {
    pub strength: f64,
    pub phase_start: f64,
    pub phase_end: f64,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct PlanSegment {
    pub t0: f64,
    pub t1: f64,
    pub s0: f64,
    pub s1: f64,
    pub curve: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remap: Option<RemapEnvelope>,
    #[serde(default = "default_segment_effects")]
    pub effects: SegmentEffects,
    #[serde(default)]
    pub transition: Option<SegmentTransition>,
}
impl PlanSegment {
    pub fn sample_curve(&self, x: f64) -> (f64, f64) {
        let x = x.clamp(0.0, 1.0);
        let Some(remap) = &self.remap else {
            return (
                crate::effects::evaluate_curve(&self.curve, x),
                crate::effects::evaluate_curve_derivative(&self.curve, x),
            );
        };
        let map = |phase| {
            phase * (1.0 - remap.strength)
                + remap.strength * crate::effects::evaluate_curve(&self.curve, phase)
        };
        let phase_span = remap.phase_end - remap.phase_start;
        let phase = remap.phase_start + phase_span * x;
        let first = map(remap.phase_start);
        let span = (map(remap.phase_end) - first).max(1e-12);
        let derivative = 1.0 - remap.strength
            + remap.strength * crate::effects::evaluate_curve_derivative(&self.curve, phase);
        (
            ((map(phase) - first) / span).clamp(0.0, 1.0),
            derivative * phase_span / span,
        )
    }
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_intervals: Option<Vec<(f64, f64)>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ranges: Option<Vec<crate::sources::SourceRange>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub planning: Option<crate::scene_schedule::PlanningMetadata>,
}

pub fn generate_one_framers(
    style: &str,
    segments: &[PlanSegment],
    downbeats: &[f64],
    fps: u32,
    target_duration: f64,
    anti_flash: bool,
) -> Vec<OneFramer> {
    if fps == 0 || segments.is_empty() {
        return Vec::new();
    }
    let dt = 1.0 / (fps as f64);
    let mut raw_candidates: Vec<OneFramer> = Vec::new();
    let style_upper = style.to_uppercase();

    let framer_pool: &[&str] = if anti_flash {
        &ANTI_FLASH_ONE_FRAMER_TYPES
    } else {
        &ONE_FRAMER_TYPES
    };
    let pool_len = framer_pool.len() as u64;

    // 1. Cuts: boundaries between segments
    for (seg_idx, seg) in segments.iter().enumerate() {
        let t_cut = seg.t0;

        let place_cut = match style_upper.as_str() {
            "HARD" => true,
            "SMOOTH" => deterministic_hash_pos(t_cut, 100).is_multiple_of(2),
            "HYBRID" => true,
            _ => true,
        };

        if place_cut {
            let offsets = [-2.0 * dt, -dt, 0.0, 1.0 * dt];
            for (idx, &off) in offsets.iter().enumerate() {
                let t_pos = t_cut + off;
                let t_rounded = (t_pos * (fps as f64)).round() / (fps as f64);
                let seed =
                    deterministic_hash_pos(t_rounded, (seg_idx as u64) * 10 + (idx as u64) + 1);
                let framer_type = framer_pool[(seed % pool_len) as usize].to_string();
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
            "HYBRID" => deterministic_hash_pos(db, 200).is_multiple_of(2),
            _ => true,
        };

        if place_downbeat {
            let t_rounded = (db * (fps as f64)).round() / (fps as f64);
            let seed = deterministic_hash_pos(t_rounded, (db_idx as u64) * 100 + 777);
            let framer_type = framer_pool[(seed % pool_len) as usize].to_string();
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
                "WARP_BUBBLE" => (
                    4,
                    serde_json::json!({ "amplitude": wb_amp, "frequency": wb_freq }),
                ),
                "WAVE_WARP" => (
                    6,
                    serde_json::json!({ "height": ww_height, "speed": ww_speed }),
                ),
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

pub fn parse_scene_ranges(scenes: &[f64], video_duration: f64) -> Vec<(f64, f64)> {
    if scenes.is_empty() {
        return vec![(0.0, video_duration)];
    }

    let mut ranges = Vec::new();
    for win in scenes.windows(2) {
        let s = win[0].clamp(0.0, video_duration);
        let e = win[1].clamp(0.0, video_duration);
        if e > s {
            ranges.push((s, e));
        }
    }

    if ranges.is_empty() {
        vec![(0.0, video_duration)]
    } else {
        ranges
    }
}

// Positional image/IPC inputs preserve the existing public contract.
#[allow(clippy::too_many_arguments)]
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
    long_video_mode: Option<&str>,
    detected_scenes: Option<&[f64]>,
    scenepack_rhythm: Option<&str>,
) -> Result<ProjectPlan, String> {
    validate_dimensions(aspect_w, aspect_h)?;
    if !matches!(style.to_uppercase().as_str(), "HARD" | "SMOOTH" | "HYBRID") {
        return Err("Unknown remap style".into());
    }
    if !(1..=120).contains(&fps) {
        return Err("FPS must be between 1 and 120".into());
    }
    if !video_duration.is_finite() || video_duration <= 0.0 {
        return Err("Video duration must be greater than 0".to_string());
    }
    if !audio_duration.is_finite() || audio_duration <= 0.0 || audio_duration > 86400.0 {
        return Err("Audio duration must be greater than 0".to_string());
    }

    if beats
        .iter()
        .chain(downbeats)
        .any(|v| !v.is_finite() || *v < 0.0)
        || !bpm.is_finite()
    {
        return Err("Beat metadata must contain finite nonnegative values".into());
    }
    if let Some(params) = &custom_params {
        validate_custom_params(params)?;
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
    let (default_a0, default_omega, default_k, default_zoom_max) =
        match style.to_uppercase().as_str() {
            "SMOOTH" => (3.0, 8.0, 2.0, 1.05),
            "HYBRID" => (5.0, 12.0, 2.5, 1.10),
            _ => (8.0, 15.0, 3.0, 1.15), // HARD default
        };

    let a0 = custom_params.as_ref().map_or(default_a0, |c| c.shake_a0);
    let omega = custom_params
        .as_ref()
        .map_or(default_omega, |c| c.shake_omega);
    let k = custom_params.as_ref().map_or(default_k, |c| c.shake_k);
    let zoom_start = custom_params.as_ref().map_or(1.0, |c| c.zoom_scale_start);
    let zoom_max = custom_params
        .as_ref()
        .map_or(default_zoom_max, |c| c.zoom_scale_end);

    // 4. Build segments and handle video loops
    let mode_str = long_video_mode.unwrap_or("no_change");

    // Scenepack structure & Virtual Trimmed Source Pipeline
    let (raw_intervals, scene_ranges, effective_video_dur) = if mode_str == "scenepack" {
        let raw = if let Some(scenes) = detected_scenes {
            parse_scene_ranges(scenes, video_duration)
        } else {
            vec![(0.0, video_duration)]
        };
        let mut compact_ranges = Vec::new();
        let mut cur_offset = 0.0f64;
        let mut extract_intervals = Vec::new();
        for (s, e) in raw {
            let dur = e - s;
            compact_ranges.push((cur_offset, cur_offset + dur));
            extract_intervals.push((s, dur));
            cur_offset += dur;
        }
        (Some(extract_intervals), compact_ranges, cur_offset)
    } else if mode_str == "long_clip" && video_duration > audio_duration {
        let m = ((audio_duration / 3.25).round() as usize).clamp(2, 8);
        let span_step = (video_duration - 3.5).max(0.0) / ((m - 1) as f64).max(1.0);
        let mut extract_intervals = Vec::new();
        let mut compact_ranges = Vec::new();
        let anchor_span = 4.0f64;
        let mut compact_offset = 0.0;
        for i in 0..m {
            let src_start = (i as f64) * span_step;
            let dur = anchor_span.min(video_duration - src_start);
            let comp_start = compact_offset;
            compact_offset += dur;
            extract_intervals.push((src_start, dur));
            compact_ranges.push((comp_start, comp_start + dur));
        }
        let total_comp = compact_offset;
        (Some(extract_intervals), compact_ranges, total_comp)
    } else {
        let needed_dur = video_duration.min(audio_duration + 2.0);
        (
            Some(vec![(0.0, needed_dur)]),
            vec![(0.0, needed_dur)],
            needed_dur,
        )
    };

    let num_scenes = scene_ranges.len().max(1);
    let mut current_scene_idx = 0usize;
    let mut scene_target_start = 0.0f64;

    let long_clip_anchors: Vec<f64> = if mode_str == "long_clip" {
        scene_ranges.iter().map(|r| r.0).collect()
    } else {
        vec![0.0]
    };
    let mut current_anchor_idx = 0usize;
    let mut anchor_target_start = 0.0f64;

    let mut segments = Vec::new();
    let mut wrap_indices = Vec::new();
    let mut s_cursor = if mode_str == "scenepack" {
        scene_ranges[0].0
    } else if mode_str == "long_clip" {
        long_clip_anchors[0]
    } else {
        0.0
    };
    let mut loops = 0u32;
    let mut downbeat_count = 0usize;
    let mut seg_index = 0usize;

    for win in filtered_bounds.windows(2) {
        let t0 = win[0];
        let t1 = win[1];

        // Check if segment starts on a downbeat or regular beat
        let is_on_downbeat = downbeats
            .iter()
            .any(|&d| (d - t0).abs() < 0.03 || (d >= t0 - 1e-4 && d < t1 - 1e-4));
        let is_on_beat = beats
            .iter()
            .any(|&b| (b - t0).abs() < 0.03 || (b >= t0 - 1e-4 && b < t1 - 1e-4));
        let rhythm_mode = scenepack_rhythm.unwrap_or("mid").to_lowercase();

        // Mode-specific scene or anchor switching check
        if mode_str == "scenepack" && num_scenes > 1 {
            let elapsed_target = t0 - scene_target_start;
            let current_scene_end = scene_ranges[current_scene_idx].1;
            let reached_scene_end = s_cursor >= current_scene_end - 0.05;

            // Strict beat-following rule:
            // NEVER switch unless on a musical beat/downbeat (the ONLY exception being clip exhaustion).
            let should_switch = match rhythm_mode.as_str() {
                "fast" => {
                    // FAST: switch on EVERY beat
                    is_on_beat && t0 > 0.0
                }
                "slow" => {
                    // SLOW: switch on downbeats after at least 3.0s of continuous footage
                    is_on_downbeat && elapsed_target >= 3.0 && t0 > 0.0
                }
                _ => {
                    // MID (default): switch on every downbeat (measure/bar)
                    is_on_downbeat && t0 > 0.0
                }
            };

            if (should_switch || reached_scene_end) && t0 > 0.0 {
                current_scene_idx = (current_scene_idx + 1) % num_scenes;
                scene_target_start = t0;
                s_cursor = scene_ranges[current_scene_idx].0;
                wrap_indices.push(segments.len());
            }
        } else if mode_str == "long_clip" && long_clip_anchors.len() > 1 {
            let elapsed_target = t0 - anchor_target_start;
            if elapsed_target >= 3.0
                && (is_on_downbeat || elapsed_target >= 4.0)
                && current_anchor_idx + 1 < long_clip_anchors.len()
            {
                current_anchor_idx += 1;
                anchor_target_start = t0;
                s_cursor = long_clip_anchors[current_anchor_idx];
                wrap_indices.push(segments.len());
            }
        }

        let (curve_name, r) = match style.to_uppercase().as_str() {
            "SMOOTH" => ("saddle".to_string(), 0.75),
            "HYBRID" => {
                if seg_index.is_multiple_of(2) {
                    ("snap".to_string(), 1.0)
                } else {
                    ("saddle".to_string(), 0.75)
                }
            }
            _ => ("snap".to_string(), 1.0), // HARD default
        };

        let mut reverse_this_segment = false;
        if is_on_downbeat {
            downbeat_count += 1;
            match style.to_uppercase().as_str() {
                "HARD" => {
                    if downbeat_count % 5 == 1 {
                        reverse_this_segment = true;
                    }
                }
                "HYBRID" if downbeat_count % 10 == 1 => {
                    reverse_this_segment = true;
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
            let (mut scale_start, mut scale_end) = if seg_index.is_multiple_of(2) {
                (zoom_start, zoom_max)
            } else {
                (zoom_max, zoom_start)
            };
            if !overrides.zoom {
                scale_start = 1.0;
                scale_end = 1.0;
            }

            let seed = ((seg_index as u32)
                .wrapping_mul(1664525)
                .wrapping_add(1013904223))
                ^ 0x5bf03635;

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
                default_bouncy_prob,
                default_bouncy_amp,
                default_dissolve_prob,
                default_dissolve_pct_val,
                default_skew_prob,
                default_skew_s0,
                default_squish_prob,
                default_optics_prob,
                default_chain_prob,
                default_stretch_prob,
                default_stretch_scale,
            ) = match style_up.as_str() {
                "SMOOTH" => (
                    if overrides.bouncy_shake { 30u32 } else { 0u32 },
                    25.0f64,
                    if overrides.dissolve_shake {
                        25u32
                    } else {
                        0u32
                    },
                    15.0f64,
                    if overrides.skew_shake { 20u32 } else { 0u32 },
                    7.0f64,
                    if overrides.squish_pop { 40u32 } else { 0u32 },
                    if overrides.optics_bounce { 25u32 } else { 0u32 },
                    if overrides.buildup_chain { 30u32 } else { 0u32 },
                    if overrides.warp_stretch { 20u32 } else { 0u32 },
                    1.20f64,
                ),
                "HYBRID" => (
                    if overrides.bouncy_shake { 15u32 } else { 0u32 },
                    25.0,
                    if overrides.dissolve_shake {
                        10u32
                    } else {
                        0u32
                    },
                    15.0,
                    if overrides.skew_shake { 10u32 } else { 0u32 },
                    7.0,
                    if overrides.squish_pop { 20u32 } else { 0u32 },
                    if overrides.optics_bounce { 10u32 } else { 0u32 },
                    if overrides.buildup_chain { 15u32 } else { 0u32 },
                    if overrides.warp_stretch { 10u32 } else { 0u32 },
                    1.30f64,
                ),
                _ => (
                    if overrides.bouncy_shake { 30u32 } else { 0u32 },
                    40.0,
                    if overrides.dissolve_shake {
                        25u32
                    } else {
                        0u32
                    },
                    30.0,
                    if overrides.skew_shake { 20u32 } else { 0u32 },
                    10.0,
                    if overrides.squish_pop { 40u32 } else { 0u32 },
                    if overrides.optics_bounce { 25u32 } else { 0u32 },
                    if overrides.buildup_chain { 30u32 } else { 0u32 },
                    if overrides.warp_stretch { 20u32 } else { 0u32 },
                    1.40f64,
                ),
            };

            let bouncy_amp = custom_params
                .as_ref()
                .map_or(default_bouncy_amp, |c| c.bouncy_amplitude);
            let dissolve_pct_val = custom_params
                .as_ref()
                .map_or(default_dissolve_pct_val, |c| c.dissolve_pct);
            let skew_s0 = custom_params
                .as_ref()
                .map_or(default_skew_s0, |c| c.skew_s0);
            let optics_k0_val = custom_params.as_ref().map_or(0.08, |c| c.optics_k0);
            let stretch_scale_val = custom_params
                .as_ref()
                .map_or(default_stretch_scale, |c| c.stretch_scale);

            let bouncy_shake = if default_bouncy_prob > 0 && pct(s1) < default_bouncy_prob {
                Some(BouncyShake {
                    axis: (s1 % 2) as u8,
                    amplitude: bouncy_amp,
                })
            } else {
                None
            };

            let mut effective_a0 = match custom_params.as_ref() {
                Some(cp) => cp.shake_a0,
                None => {
                    if bouncy_shake.is_some() {
                        0.0
                    } else {
                        a0
                    }
                }
            };
            if !overrides.shakes {
                effective_a0 = 0.0;
            }

            let dissolve_shake = if default_dissolve_prob > 0 && pct(s2) < default_dissolve_prob {
                Some(DissolveShake {
                    pct: dissolve_pct_val,
                })
            } else {
                None
            };

            let skew_shake = if default_skew_prob > 0 && pct(s3) < default_skew_prob {
                Some(SkewShake { s0_deg: skew_s0 })
            } else {
                None
            };

            let squish_pop = if default_squish_prob > 0 && pct(s4) < default_squish_prob {
                Some(SquishPop {
                    _pad: 0,
                    scale_y_min: custom_params
                        .as_ref()
                        .map_or(0.88, |c| c.squish_scale_y_min),
                    scale_x_max: custom_params
                        .as_ref()
                        .map_or(1.10, |c| c.squish_scale_x_max),
                })
            } else {
                None
            };

            let optics_bounce = if default_optics_prob > 0 && pct(s5) < default_optics_prob {
                Some(OpticsBounce { k0: optics_k0_val })
            } else {
                None
            };

            let buildup_chain = if default_chain_prob > 0 && pct(s6) < default_chain_prob {
                Some(BuildupChain {
                    chain_next: true,
                    chain_from_prev: false,
                })
            } else {
                None
            };

            let warp_stretch = if default_stretch_prob > 0 && pct(s7) < default_stretch_prob {
                Some(WarpStretch {
                    axis: (s7 % 2) as u8,
                    scale_start: stretch_scale_val,
                })
            } else {
                None
            };

            let zoom_beat_offset = if overrides.zoom_beat_offset {
                custom_params
                    .as_ref()
                    .map_or(s8 % 3, |c| c.zoom_beat_offset_frames)
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

            let current_max_s = if mode_str == "scenepack" {
                scene_ranges[current_scene_idx].1
            } else {
                effective_video_dur
            };

            if s_cursor + span <= current_max_s + 1e-9 {
                let mut s0 = s_cursor;
                let mut s1 = s_cursor + span;
                s_cursor += span;

                let is_exact_wrap = (s_cursor - current_max_s).abs() < 1e-9;
                if is_exact_wrap {
                    if mode_str == "scenepack" {
                        current_scene_idx = (current_scene_idx + 1) % num_scenes;
                        s_cursor = scene_ranges[current_scene_idx].0;
                    } else {
                        s_cursor = 0.0;
                    }
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
                    remap: None,
                    effects,
                    transition: None,
                });
                seg_index += 1;
                seg_t0 = seg_t1;
            } else {
                let available = current_max_s - s_cursor;
                if available > 1e-6 {
                    let dt_sub = available / r;
                    let mut s0 = s_cursor;
                    let mut s1 = current_max_s;
                    if reverse_this_segment {
                        std::mem::swap(&mut s0, &mut s1);
                    }

                    segments.push(PlanSegment {
                        t0: seg_t0,
                        t1: seg_t0 + dt_sub,
                        s0: (s0 * 1000.0).round() / 1000.0,
                        s1: (s1 * 1000.0).round() / 1000.0,
                        curve: curve_name.clone(),
                        remap: None,
                        effects: effects.clone(),
                        transition: None,
                    });
                    seg_index += 1;
                    seg_t0 += dt_sub;
                }
                if mode_str == "scenepack" {
                    current_scene_idx = (current_scene_idx + 1) % num_scenes;
                    s_cursor = scene_ranges[current_scene_idx].0;
                } else {
                    s_cursor = 0.0;
                }
                loops += 1;
                wrap_indices.push(segments.len());
            }
        }
    }

    let target_dur = (target * 1000.0).round() / 1000.0;

    // ── T14 post-pass: propagate buildup chain_next → chain_from_prev on next segment ──
    for i in 0..segments.len().saturating_sub(1) {
        let chain_next = segments[i]
            .effects
            .buildup_chain
            .as_ref()
            .is_some_and(|bc| bc.chain_next);
        if chain_next {
            if let Some(bc) = segments[i + 1].effects.buildup_chain.as_mut() {
                bc.chain_from_prev = true;
            } else {
                segments[i + 1].effects.buildup_chain = Some(BuildupChain {
                    chain_next: false,
                    chain_from_prev: true,
                });
            }
        }
    }

    let one_framers = if overrides.one_framers {
        let mut framers = generate_one_framers(
            style,
            &segments,
            downbeats,
            fps,
            target_dur,
            overrides.anti_flash,
        );
        if overrides.anti_flash {
            framers.retain(|f| {
                f.framer_type != "FLASH_WHITE"
                    && f.framer_type != "FLASH_BLACK"
                    && f.framer_type != "INVERT"
            });
        }
        framers
    } else {
        vec![]
    };
    let transitions = if overrides.transitions {
        generate_transitions(
            style,
            &mut segments,
            &wrap_indices,
            fps,
            custom_params.as_ref(),
        )
    } else {
        vec![]
    };

    let has_any_ambiance = overrides.flicker
        || overrides.exposure_flash
        || overrides.echo_trail
        || overrides.tint
        || overrides.vignette
        || overrides.scanlines
        || overrides.cc_deep_dark;

    let ambiance = if has_any_ambiance {
        let mut a = default_ambiance(style, downbeats);
        a.cc_deep_dark = overrides.cc_deep_dark;
        if overrides.tint && !overrides.anti_flash {
            a.tint.invert_bw = true;
            a.tint.downbeat_times = downbeats.to_vec();
            a.tint.offset_rgb = custom_params.as_ref().map_or([0, 0, 0], |c| {
                [c.tint_r_offset, c.tint_g_offset, c.tint_b_offset]
            });
        } else if overrides.tint && overrides.anti_flash {
            a.tint.invert_bw = false;
            a.tint.downbeat_times.clear();
            a.tint.offset_rgb = custom_params.as_ref().map_or([0, 0, 0], |c| {
                [c.tint_r_offset, c.tint_g_offset, c.tint_b_offset]
            });
        } else {
            a.tint.invert_bw = false;
            a.tint.downbeat_times.clear();
            a.tint.offset_rgb = [0, 0, 0];
        }
        if overrides.flicker && !overrides.anti_flash {
            if let Some(c) = custom_params.as_ref() {
                a.flicker.amplitude = c.flicker_amplitude;
                a.flicker.f = c.flicker_frequency_hz;
            }
        } else {
            a.flicker.amplitude = 0.0;
        }
        if overrides.exposure_flash && !overrides.anti_flash {
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
        video_duration: (effective_video_dur * 1000.0).round() / 1000.0,
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
        source_intervals: raw_intervals,
        source_ranges: None,
        planning: None,
    })
}

#[tauri::command(async)]
// Positional image/IPC inputs preserve the existing public contract.
#[allow(clippy::too_many_arguments)]
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
    long_video_mode: Option<String>,
    detected_scenes: Option<Vec<f64>>,
    scenepack_rhythm: Option<String>,
    scenes_format: Option<String>,
    source_ranges: Option<Vec<crate::sources::SourceRange>>,
    prepared_scenes: Option<Vec<crate::scene_analysis::PreparedScene>>,
    no_repetitive_scene: Option<bool>,
) -> Result<String, String> {
    if let Some(scenes) = prepared_scenes {
        let mode = long_video_mode.as_deref().unwrap_or("no_change");
        if !matches!(mode, "scenepack" | "long_clip") {
            return Err("Motion planning requires ScenePack or Long Clip".into());
        }
        crate::scene_schedule::validate_prepared(&scenes)?;
        let adaptive = mode == "long_clip" || no_repetitive_scene.unwrap_or(true);
        let mut bounds = vec![0.0];
        for scene in &scenes {
            bounds.push(bounds.last().unwrap() + scene.duration());
        }
        let overrides =
            effect_overrides.unwrap_or_else(|| default_effects_for_style(&style, full_fx));
        let mut plan = create_plan_internal(
            &style,
            fps,
            &beats,
            &downbeats,
            if adaptive {
                audio_duration + 2.0
            } else {
                *bounds.last().unwrap()
            },
            audio_duration,
            aspect_w,
            aspect_h,
            bpm,
            full_fx,
            Some(overrides.clone()),
            custom_params,
            if adaptive { None } else { Some("scenepack") },
            if adaptive {
                None
            } else {
                Some(bounds.as_slice())
            },
            scenepack_rhythm.as_deref(),
        )?;
        crate::scene_schedule::apply_schedule(
            &mut plan,
            &scenes,
            &beats,
            &downbeats,
            scenepack_rhythm.as_deref().unwrap_or("mid"),
            adaptive,
            mode,
        )?;
        let cuts: Vec<_> = plan
            .planning
            .as_ref()
            .unwrap()
            .shots
            .windows(2)
            .filter_map(|shots| {
                let last = shots[0].ranges.last()?;
                let first = shots[1].ranges.first()?;
                let continuous = last.source_id == first.source_id
                    && (last.start + last.duration - first.start).abs() < 0.003;
                (!continuous).then_some(shots[1].t0)
            })
            .collect();
        let cut_indices: Vec<_> = cuts
            .iter()
            .filter_map(|time| {
                plan.segments
                    .iter()
                    .position(|segment| (segment.t0 - time).abs() < 1e-6)
            })
            .collect();
        plan.transitions = if overrides.transitions {
            generate_transitions(
                &style,
                &mut plan.segments,
                &cut_indices,
                fps,
                plan.custom_params.as_ref(),
            )
        } else {
            Vec::new()
        };
        if let Some(config) = export_config {
            plan.export = config;
        }
        validate_plan(&plan)?;
        return serde_json::to_string_pretty(&plan)
            .map_err(|error| format!("Failed to serialize plan: {error}"));
    }
    let (video_duration, detected_scenes) = if let Some(ranges) = &source_ranges {
        let mut offset = 0.0;
        let mut bounds = vec![0.0];
        for range in ranges {
            if !range.start.is_finite()
                || range.start < 0.0
                || !range.duration.is_finite()
                || range.duration <= 0.0
            {
                return Err("Invalid selected source range".into());
            }
            offset += range.duration;
            bounds.push(offset);
        }
        (offset, Some(bounds))
    } else if scenes_format.as_deref() == Some("pairs") {
        let scenes = detected_scenes.unwrap_or_default();
        if !scenes.len().is_multiple_of(2) {
            return Err("Scene pairs must have an even number of values".into());
        }
        // Pairs may be discontinuous. Preserve extraction coordinates separately below.
        (video_duration, Some(scenes))
    } else {
        if let Some(format) = &scenes_format {
            if format != "boundaries" {
                return Err("Unknown scene range format".into());
            }
        }
        (video_duration, detected_scenes)
    };
    let explicit_pairs = source_ranges.is_none()
        && scenes_format.as_deref() == Some("pairs")
        && long_video_mode.as_deref() == Some("scenepack");
    let mut compact_bounds = vec![0.0];
    let mut pair_intervals = Vec::new();
    if explicit_pairs {
        for pair in detected_scenes
            .as_deref()
            .unwrap_or_default()
            .chunks_exact(2)
        {
            if !pair[0].is_finite()
                || !pair[1].is_finite()
                || pair[0] < 0.0
                || pair[1] <= pair[0]
                || pair[1] > video_duration + 0.051
            {
                return Err("Invalid scene pair".into());
            }
            let duration = pair[1] - pair[0];
            pair_intervals.push((pair[0], duration));
            compact_bounds.push(compact_bounds.last().unwrap() + duration);
        }
    }
    let mut plan = create_plan_internal(
        &style,
        fps,
        &beats,
        &downbeats,
        if explicit_pairs {
            *compact_bounds.last().unwrap()
        } else {
            video_duration
        },
        audio_duration,
        aspect_w,
        aspect_h,
        bpm,
        full_fx,
        effect_overrides,
        custom_params,
        long_video_mode.as_deref(),
        if explicit_pairs {
            Some(compact_bounds.as_slice())
        } else {
            detected_scenes.as_deref()
        },
        scenepack_rhythm.as_deref(),
    )?;
    if explicit_pairs && !pair_intervals.is_empty() {
        plan.source_intervals = Some(pair_intervals);
    }
    if let Some(ranges) = source_ranges {
        plan.schema_version = 3;
        let available_durations: Vec<f64> = ranges.iter().map(|range| range.duration).collect();
        let selected = crate::sources::migrate_intervals(
            plan.source_intervals
                .as_deref()
                .unwrap_or(&[(0.0, plan.video_duration)]),
            &available_durations,
        )?;
        plan.source_ranges = Some(
            selected
                .into_iter()
                .map(|range| {
                    let available = &ranges[range.source_id];
                    crate::sources::SourceRange {
                        source_id: available.source_id,
                        start: available.start + range.start,
                        duration: range.duration,
                    }
                })
                .collect(),
        );
    }
    if let Some(ec) = export_config {
        plan.export = ec;
    }
    validate_plan(&plan)?;
    serde_json::to_string_pretty(&plan).map_err(|e| format!("Failed to serialize plan: {e}"))
}

#[tauri::command]
pub fn cmd_get_style_defaults(style: String) -> Result<CustomParams, String> {
    Ok(get_style_defaults(&style))
}

#[derive(serde::Serialize)]
pub struct StylePreset {
    pub effects: EffectOverrides,
    pub params: CustomParams,
}

#[tauri::command]
pub fn cmd_get_style_preset(style: String, full_fx: bool) -> Result<StylePreset, String> {
    if !matches!(style.as_str(), "HARD" | "SMOOTH" | "HYBRID") {
        return Err("Unknown remap style".into());
    }
    Ok(StylePreset {
        effects: default_effects_for_style(&style, full_fx),
        params: get_style_defaults(&style),
    })
}

pub fn validate_dimensions(width: u32, height: u32) -> Result<(), String> {
    // Small values are legacy aspect ratios. Actual output dimensions are checked too.
    if width == 0 || height == 0 || width > 7680 || height > 7680 {
        return Err("Dimensions must be nonzero and no larger than 7680 pixels".into());
    }
    let crop = crate::probe::compute_crop_to_fill(1920, 1080, width, height);
    if crop.out_w > 7680 || crop.out_h > 7680 || crop.out_w * crop.out_h > 33_177_600 {
        return Err("Output dimensions exceed the supported 8K canvas".into());
    }
    if width >= 100 && height >= 100 && (!width.is_multiple_of(2) || !height.is_multiple_of(2)) {
        return Err("Output width and height must be even for YUV420 video".into());
    }
    Ok(())
}

pub fn validate_custom_params(params: &CustomParams) -> Result<(), String> {
    let value = serde_json::to_value(params).map_err(|e| e.to_string())?;
    for (key, value) in value.as_object().ok_or("Invalid custom parameters")? {
        let number = value
            .as_f64()
            .ok_or_else(|| format!("{key} must be finite"))?;
        let (min, max) = match key.as_str() {
            "shakeA0" | "shakeOmega" | "skewS0" => (0.0, 30.0),
            "shakeK" => (0.0, 10.0),
            "bouncyAmplitude" => (0.0, 60.0),
            "dissolvePct" => (0.0, 100.0),
            "squishScaleYMin" => (0.5, 1.0),
            "squishScaleXMax" => (1.0, 1.5),
            "opticsK0" => (0.0, 0.3),
            "stretchScale" => (1.0, 2.0),
            "zoomScaleStart" | "zoomScaleEnd" => (0.8, 1.5),
            "zoomBeatOffsetFrames" => (0.0, 8.0),
            "echoKDepth" => (1.0, 4.0),
            "flickerFrequencyHz" => (0.0, 30.0),
            "warpBubbleFrequency" => (0.0, 5.0),
            "waveWarpHeight" => (0.0, 600.0),
            "waveWarpSpeed" => (0.0, 40.0),
            "slideShakePixels" => (0.0, 200.0),
            "tintROffset" | "tintGOffset" | "tintBOffset" => (-255.0, 255.0),
            _ => (0.0, 1.0),
        };
        if !number.is_finite() || number < min || number > max {
            return Err(format!("{key} must be between {min} and {max}"));
        }
    }
    Ok(())
}

pub fn validate_plan(plan: &ProjectPlan) -> Result<(), String> {
    let bounded = |name: &str, number: f64, min: f64, max: f64| -> Result<(), String> {
        if number.is_finite() && (min..=max).contains(&number) {
            Ok(())
        } else {
            Err(format!("Invalid {name}: expected {min}..{max}"))
        }
    };
    if !(1..=4).contains(&plan.schema_version) {
        return Err("Unsupported plan schema version".into());
    }
    if !matches!(
        plan.style.to_uppercase().as_str(),
        "HARD" | "SMOOTH" | "HYBRID"
    ) {
        return Err("Unknown remap style".into());
    }
    validate_dimensions(plan.aspect.w, plan.aspect.h)?;
    if !(1..=120).contains(&plan.fps)
        || !plan.target_duration.is_finite()
        || plan.target_duration <= 0.0
        || plan.target_duration * (plan.fps as f64) < 0.5
        || plan.target_duration > 86400.0
        || !plan.video_duration.is_finite()
        || plan.video_duration <= 0.0
        || plan.segments.is_empty()
    {
        return Err("Invalid render duration, FPS or empty segments".into());
    }
    let codec = plan.export.codec.to_uppercase().replace('.', "");
    let format = plan.export.format.to_uppercase();
    if !matches!(codec.as_str(), "H264" | "H265" | "HEVC" | "VP9")
        || !matches!(format.as_str(), "MP4" | "MKV" | "WEBM")
        || (format == "WEBM" && codec != "VP9")
        || !(1..=100).contains(&plan.export.bitrate_mbps)
    {
        return Err(
            "Unsupported export settings: WEBM requires VP9; use MP4 or MKV for H.264/H.265".into(),
        );
    }
    if let Some(params) = &plan.custom_params {
        validate_custom_params(params)?;
    }
    if let Some(ambiance) = &plan.ambiance {
        bounded("flicker amplitude", ambiance.flicker.amplitude, 0.0, 1.0)?;
        bounded("flicker frequency", ambiance.flicker.f, 0.0, 30.0)?;
        bounded("flicker phase", ambiance.flicker.phase, -1e6, 1e6)?;
        bounded("exposure flash", ambiance.exposure_flash.peak, 0.0, 1.0)?;
        bounded("echo alpha", ambiance.echo_trail.alpha, 0.0, 1.0)?;
        if !(1..=4).contains(&ambiance.echo_trail.k) {
            return Err("Echo depth must be between 1 and 4".into());
        }
        bounded("vignette", ambiance.vignette.strength, 0.0, 1.0)?;
        bounded("scanlines", ambiance.scanlines.opacity, 0.0, 1.0)?;
        for time in ambiance
            .exposure_flash
            .times
            .iter()
            .chain(&ambiance.tint.downbeat_times)
        {
            bounded("ambiance event time", *time, 0.0, 86400.0)?;
        }
        if ambiance
            .tint
            .offset_rgb
            .iter()
            .any(|value| !(-255..=255).contains(value))
        {
            return Err("Tint offsets must be between -255 and 255".into());
        }
    }
    for framer in &plan.one_framers {
        bounded("one-framer time", framer.t, 0.0, plan.target_duration)?;
        if !crate::effects::ONE_FRAMER_TYPES.contains(&framer.framer_type.as_str()) {
            return Err("Unknown one-framer type".into());
        }
    }
    for transition in &plan.transitions {
        if !transition.t.is_finite()
            || transition.t < 0.0
            || transition.t > plan.target_duration
            || transition.duration_frames > 120
        {
            return Err("Invalid transition timing".into());
        }
        let limits: &[(&str, f64, f64)] = match transition.transition_type.as_str() {
            "WARP_BUBBLE" => &[("amplitude", 0.0, 1.0), ("frequency", 0.0, 5.0)],
            "WAVE_WARP" => &[("height", 0.0, 600.0), ("speed", 0.0, 40.0)],
            "SLIDE_SHAKE" => &[("amplitude", 0.0, 200.0)],
            _ => return Err("Unknown transition type".into()),
        };
        for &(key, min, max) in limits {
            if let Some(value) = transition.params.get(key) {
                let number = value
                    .as_f64()
                    .ok_or("Transition parameter must be finite")?;
                if !number.is_finite() || number < min || number > max {
                    return Err(format!("Invalid transition parameter: {key}"));
                }
            }
        }
    }
    let mut end = 0.0;
    for segment in &plan.segments {
        let effects = &segment.effects;
        bounded("shake amplitude", effects.shake.a0, 0.0, 60.0)?;
        bounded("shake frequency", effects.shake.omega, 0.0, 30.0)?;
        bounded("shake decay", effects.shake.k, 0.0, 10.0)?;
        bounded("zoom start", effects.zoom.scale_start, 0.5, 2.0)?;
        bounded("zoom end", effects.zoom.scale_end, 0.5, 2.0)?;
        if let Some(effect) = &effects.bouncy_shake {
            bounded("bouncy shake", effect.amplitude, 0.0, 60.0)?;
            if effect.axis > 1 {
                return Err("Invalid bouncy shake axis".into());
            }
        }
        if let Some(effect) = &effects.dissolve_shake {
            bounded("dissolve", effect.pct, 0.0, 100.0)?;
        }
        if let Some(effect) = &effects.skew_shake {
            bounded("skew", effect.s0_deg, -30.0, 30.0)?;
        }
        if let Some(effect) = &effects.optics_bounce {
            bounded("optics", effect.k0, -0.3, 0.3)?;
        }
        if let Some(effect) = &effects.squish_pop {
            bounded("squish Y", effect.scale_y_min, 0.5, 1.0)?;
            bounded("squish X", effect.scale_x_max, 1.0, 1.5)?;
        }
        if let Some(effect) = &effects.warp_stretch {
            bounded("stretch", effect.scale_start, 1.0, 2.0)?;
            if effect.axis > 1 {
                return Err("Invalid stretch axis".into());
            }
        }
        if effects.zoom_beat_offset > 8 {
            return Err("Invalid zoom beat offset".into());
        }
        if let Some(remap) = &segment.remap {
            if [remap.strength, remap.phase_start, remap.phase_end]
                .iter()
                .any(|value| !value.is_finite())
                || !(0.0..=1.0).contains(&remap.strength)
                || remap.phase_start < 0.0
                || remap.phase_end > 1.0
                || remap.phase_end <= remap.phase_start
            {
                return Err("Invalid motion remap envelope".into());
            }
        }
        if [segment.t0, segment.t1, segment.s0, segment.s1]
            .iter()
            .any(|n| !n.is_finite())
            || (segment.t0 - end).abs() > 0.002
            || segment.t1 <= segment.t0
            || segment.s0 < 0.0
            || segment.s1 < 0.0
            || segment.s0 > plan.video_duration + 0.051
            || segment.s1 > plan.video_duration + 0.051
            || !matches!(segment.curve.as_str(), "snap" | "saddle" | "linear")
        {
            return Err("Invalid or discontinuous render segment".into());
        }
        end = segment.t1;
    }
    if (end - plan.target_duration).abs() > 0.002 {
        return Err("Segments do not cover the output duration".into());
    }
    Ok(())
}

#[tauri::command]
pub fn save_plan(app: tauri::AppHandle, plan_json: String) -> Result<String, String> {
    let plan: ProjectPlan =
        serde_json::from_str(&plan_json).map_err(|e| format!("Invalid plan JSON: {e}"))?;
    validate_plan(&plan)?;

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
