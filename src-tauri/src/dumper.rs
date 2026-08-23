use std::io::Read;
use tauri::{Emitter, Manager};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

use crate::beat::{detect_beats_internal, get_binary_path_opt, BeatResult};
use crate::effects::{
    default_ambiance, AspectRatio, BouncyShake, BuildupChain, DissolveShake, OneFramer,
    OpticsBounce, SegmentEffects, ShakeEffect, SkewShake, SquishPop, WarpStretch, ZoomEffect,
    ONE_FRAMER_TYPES,
};
use crate::plan::{
    generate_one_framers, generate_transitions, get_style_defaults, ColorHints, ExportConfig,
    PlanSegment, ProjectPlan, SourceFxKeyframe, SourceFxType,
};
use crate::presets::RemapParams;
use crate::probe::probe_media_internal;

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
pub struct FrameMotion {
    pub t: f64,
    pub tx: f64,
    pub ty: f64,
    pub divergence: f64,
    pub curl: f64,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SegmentMotion {
    pub shake_energy: f64,
    pub zoom_presence: bool,
    pub mean_divergence: f64,
    pub mean_curl: f64,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DumpSegment {
    pub start: f64,
    pub end: f64,
    pub lab: LabStats,
    pub mad_mean: f64,
    pub mad_peak: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub motion: Option<SegmentMotion>,
    pub one_framer_count: usize,
    pub speed_hint: String, // "slow" | "normal" | "fast" | "snap"
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Archetype {
    #[serde(rename = "jugg")]
    JUGG,
    #[serde(rename = "flow")]
    FLOW,
    #[serde(rename = "vibe")]
    VIBE,
    #[serde(rename = "glitch")]
    GLITCH,
    #[serde(rename = "clean")]
    CLEAN,
    #[serde(rename = "hybrid")]
    HYBRID,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StyleDecision {
    pub style_name: String, // "jugg" | "jugg (strict)" | "glitch-leaning" | "velocity/flow" | "vibe (groove)" | "basic/clean" | "hybrid/unclassified"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub_style: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archetype: Option<Archetype>,
    pub confidence: f64,   // 0.0 .. 1.0
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sync_tolerance_ms: Option<f64>,
    pub justifications: Vec<String>,
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
    pub sync_na: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sync_tolerance_ms: Option<f64>,
    pub detected_style: StyleDecision,
    pub one_framers: Vec<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub one_framers_v2: Option<Vec<f64>>,
    pub segments: Vec<DumpSegment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub motion_warning: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reusable_project_path: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReusableSegment {
    pub start: f64,
    pub end: f64,
    pub lab_mean: [f64; 3],
    pub lab_std: [f64; 3],
    pub speed_hint: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReusableProject {
    pub schema_version: String, // "dumper_project_v1"
    pub source: String,
    pub beats: BeatResult,
    pub cuts: Vec<f64>,
    pub segments: Vec<ReusableSegment>,
    pub suggested_style: String,
    pub fps_suggestion: f64,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DumpProgressPayload {
    pub phase: String, // "SCENES" | "BEATS" | "MOTION" | "PROFILES" | "REPORT"
    pub percent: u32,
    pub message: String,
}

// --- Math & Color Functions ---

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

    let x = 0.4124564 * r + 0.3575761 * g + 0.1804375 * b;
    let y = 0.2126729 * r + 0.7151522 * g + 0.0721750 * b;
    let z = 0.0193339 * r + 0.1191920 * g + 0.9503041 * b;

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

// --- Fix T23 & T33: Cut-Beat Sync & Adaptive Tolerance Window ---

pub fn compute_sync_tolerance_ms(bpm: f64) -> f64 {
    if bpm <= 0.0 {
        return 60.0;
    }
    (12000.0 / bpm).clamp(40.0, 120.0)
}

pub fn compute_cut_beat_sync_adaptive(cuts: &[f64], beats: &[f64], bpm: f64) -> (f64, bool, f64) {
    let tolerance_ms = compute_sync_tolerance_ms(bpm);
    let tolerance_sec = tolerance_ms / 1000.0;

    if cuts.is_empty() {
        return (0.0, true, tolerance_ms);
    }
    if beats.is_empty() {
        return (0.0, false, tolerance_ms);
    }

    let mut synced_count = 0usize;
    for &cut in cuts {
        let mut min_diff = f64::MAX;
        for &beat in beats {
            let diff = (cut - beat).abs();
            if diff < min_diff {
                min_diff = diff;
            }
        }
        if min_diff <= tolerance_sec + 1e-4 {
            synced_count += 1;
        }
    }

    let sync = ((synced_count as f64 / cuts.len() as f64) * 10000.0).round() / 10000.0;
    (sync, false, tolerance_ms)
}

pub fn compute_cut_beat_sync(cuts: &[f64], beats: &[f64]) -> (f64, bool) {
    let (sync, na, _) = compute_cut_beat_sync_adaptive(cuts, beats, 200.0); // 60ms default
    (sync, na)
}

pub fn check_downbeats_sync_profile(cuts: &[f64], beats: &[f64], downbeats: &[f64], tol_sec: f64) -> bool {
    if cuts.is_empty() || downbeats.is_empty() {
        return false;
    }
    let mut total_synced_beats = 0usize;
    let mut synced_downbeats = 0usize;

    for &cut in cuts {
        let is_near_beat = beats.iter().any(|&b| (cut - b).abs() <= tol_sec + 1e-4);
        let is_near_downbeat = downbeats.iter().any(|&db| (cut - db).abs() <= tol_sec + 1e-4);
        if is_near_beat {
            total_synced_beats += 1;
            if is_near_downbeat {
                synced_downbeats += 1;
            }
        }
    }

    total_synced_beats >= 2 && (synced_downbeats as f64 / total_synced_beats as f64) >= 0.75
}

// --- One-Framer & Slowdown Detection ---

pub fn detect_one_framers_v2(mad_series: &[f64], timestamps: &[f64]) -> Vec<f64> {
    if mad_series.len() < 3 || mad_series.len() != timestamps.len() {
        return Vec::new();
    }
    let mut one_framers = Vec::new();
    let mean_mad: f64 = mad_series.iter().sum::<f64>() / mad_series.len() as f64;
    let threshold_spike = (mean_mad * 1.25).max(8.0);

    for i in 1..(mad_series.len() - 1) {
        let prev = mad_series[i - 1];
        let curr = mad_series[i];
        let next = mad_series[i + 1];

        // 3-frame local variance of luminance/MAD
        let mean_local = (prev + curr + next) / 3.0;
        let var_local = ((prev - mean_local).powi(2) + (curr - mean_local).powi(2) + (next - mean_local).powi(2)) / 3.0;

        // Peak condition: curr is higher than both neighbors and exceeds dynamic threshold or high local variance
        // Duration <= 1.5 frames: returns immediately to baseline on the next frame
        if curr > prev && curr > next && (curr >= threshold_spike || var_local >= 16.0) {
            let left_rise = curr - prev;
            let right_drop = curr - next;
            if left_rise >= 4.0 && right_drop >= 4.0 {
                one_framers.push(timestamps[i]);
            }
        }
    }
    one_framers
}

pub fn detect_one_framers(mad_series: &[f64], timestamps: &[f64]) -> Vec<f64> {
    if mad_series.len() < 3 || mad_series.len() != timestamps.len() {
        return Vec::new();
    }
    let mut one_framers = Vec::new();
    let mean_mad: f64 = mad_series.iter().sum::<f64>() / mad_series.len() as f64;
    let threshold_spike = (mean_mad * 1.5).max(12.0);

    for i in 1..(mad_series.len() - 1) {
        let prev = mad_series[i - 1];
        let curr = mad_series[i];
        let next = mad_series[i + 1];

        if curr >= threshold_spike && curr >= prev + 6.0 && curr >= next + 6.0 {
            one_framers.push(timestamps[i]);
        }
    }
    one_framers
}

pub fn detect_slowdown_presence(mad_series: &[f64]) -> bool {
    if mad_series.len() < 4 {
        return false;
    }
    let mean_mad: f64 = mad_series.iter().sum::<f64>() / mad_series.len() as f64;
    let low_threshold = (mean_mad * 0.4).min(4.0);

    let mut consecutive_low = 0usize;
    for &val in mad_series {
        if val <= low_threshold {
            consecutive_low += 1;
            if consecutive_low >= 4 {
                return true;
            }
        } else {
            consecutive_low = 0;
        }
    }
    false
}

// --- Motion Extraction & Parsing ---

#[derive(Debug, Clone, Default)]
pub struct RawMotionBlock {
    pub dx: f64,
    pub dy: f64,
    pub x: f64,
    pub y: f64,
}

pub fn parse_trf_content(content: &str, width: f64, height: f64, fps: f64) -> Vec<FrameMotion> {
    let mut frames = Vec::new();
    let cx = width / 2.0;
    let cy = height / 2.0;
    let dt = if fps > 0.0 { 1.0 / fps } else { 1.0 / 30.0 };
    let mut frame_idx = 0usize;

    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("Frame ") {
            continue;
        }

        let mut blocks = Vec::new();
        if let Some(start_idx) = trimmed.find("(LM ") {
            let list_str = &trimmed[start_idx..];
            for item in list_str.split("(LM ") {
                let item = item.trim_matches(|c| c == ')' || c == ',' || c == ']' || c == ' ' || c == '(');
                if item.is_empty() {
                    continue;
                }
                let mut parts = item.split_whitespace();
                if let (Some(dx_s), Some(dy_s), Some(x_s), Some(y_s)) =
                    (parts.next(), parts.next(), parts.next(), parts.next())
                {
                    if let (Ok(dx), Ok(dy), Ok(x), Ok(y)) = (
                        dx_s.parse::<f64>(),
                        dy_s.parse::<f64>(),
                        x_s.parse::<f64>(),
                        y_s.parse::<f64>(),
                    ) {
                        blocks.push(RawMotionBlock { dx, dy, x, y });
                    }
                }
            }
        }

        let t = frame_idx as f64 * dt;
        frame_idx += 1;

        if blocks.is_empty() {
            frames.push(FrameMotion {
                t,
                tx: 0.0,
                ty: 0.0,
                divergence: 0.0,
                curl: 0.0,
            });
            continue;
        }

        let mut dxs: Vec<f64> = blocks.iter().map(|b| b.dx).collect();
        let mut dys: Vec<f64> = blocks.iter().map(|b| b.dy).collect();
        dxs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        dys.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let med_dx = dxs[dxs.len() / 2];
        let med_dy = dys[dys.len() / 2];

        let mut divs = Vec::new();
        let mut curls = Vec::new();

        for b in &blocks {
            let rx = b.x - cx;
            let ry = b.y - cy;
            let r2 = rx * rx + ry * ry;
            if r2 > 100.0 {
                let vx = b.dx - med_dx;
                let vy = b.dy - med_dy;
                let div = (vx * rx + vy * ry) / r2;
                let curl = (vy * rx - vx * ry) / r2;
                divs.push(div);
                curls.push(curl);
            }
        }

        divs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        curls.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let med_div = if divs.is_empty() { 0.0 } else { divs[divs.len() / 2] };
        let med_curl = if curls.is_empty() { 0.0 } else { curls[curls.len() / 2] };

        frames.push(FrameMotion {
            t,
            tx: if width > 0.0 { med_dx / width } else { 0.0 },
            ty: if height > 0.0 { med_dy / height } else { 0.0 },
            divergence: med_div,
            curl: med_curl,
        });
    }

    frames
}

pub fn parse_lavfi_mv_content(content: &str, width: f64, height: f64) -> Vec<FrameMotion> {
    let mut frames = Vec::new();
    let mut curr_t = 0.0;
    let mut curr_tx = 0.0;
    let mut curr_ty = 0.0;
    let mut has_data = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("pkt_pts_time=") || trimmed.starts_with("pts_time=") {
            if let Some(val) = trimmed.split('=').nth(1) {
                if let Ok(t) = val.parse::<f64>() {
                    if has_data {
                        frames.push(FrameMotion {
                            t: curr_t,
                            tx: if width > 0.0 { curr_tx / width } else { 0.0 },
                            ty: if height > 0.0 { curr_ty / height } else { 0.0 },
                            divergence: 0.0,
                            curl: 0.0,
                        });
                    }
                    curr_t = t;
                    curr_tx = 0.0;
                    curr_ty = 0.0;
                    has_data = false;
                }
            }
        } else if trimmed.starts_with("lavfi.mv.dx=") || trimmed.starts_with("lavfi.mv.x=") {
            if let Some(val) = trimmed.split('=').nth(1) {
                if let Ok(v) = val.parse::<f64>() {
                    curr_tx = v;
                    has_data = true;
                }
            }
        } else if trimmed.starts_with("lavfi.mv.dy=") || trimmed.starts_with("lavfi.mv.y=") {
            if let Some(val) = trimmed.split('=').nth(1) {
                if let Ok(v) = val.parse::<f64>() {
                    curr_ty = v;
                    has_data = true;
                }
            }
        }
    }
    if has_data {
        frames.push(FrameMotion {
            t: curr_t,
            tx: if width > 0.0 { curr_tx / width } else { 0.0 },
            ty: if height > 0.0 { curr_ty / height } else { 0.0 },
            divergence: 0.0,
            curl: 0.0,
        });
    }
    frames
}

pub fn compute_segment_motion(motion_frames: &[FrameMotion]) -> Option<SegmentMotion> {
    if motion_frames.len() < 2 {
        return None;
    }

    let mut dxs = Vec::with_capacity(motion_frames.len() - 1);
    let mut dys = Vec::with_capacity(motion_frames.len() - 1);
    let mut divs = Vec::with_capacity(motion_frames.len());
    let mut curls = Vec::with_capacity(motion_frames.len());

    for i in 1..motion_frames.len() {
        dxs.push(motion_frames[i].tx - motion_frames[i - 1].tx);
        dys.push(motion_frames[i].ty - motion_frames[i - 1].ty);
    }
    for f in motion_frames {
        divs.push(f.divergence);
        curls.push(f.curl);
    }

    let mean_dx = dxs.iter().sum::<f64>() / dxs.len() as f64;
    let mean_dy = dys.iter().sum::<f64>() / dys.len() as f64;

    let var_dx = dxs.iter().map(|&d| (d - mean_dx).powi(2)).sum::<f64>() / dxs.len() as f64;
    let var_dy = dys.iter().map(|&d| (d - mean_dy).powi(2)).sum::<f64>() / dys.len() as f64;

    let std_dx = var_dx.sqrt();
    let std_dy = var_dy.sqrt();
    let shake_energy = (std_dx + std_dy) * 10.0;

    let mean_div = divs.iter().sum::<f64>() / divs.len() as f64;
    let mean_curl = curls.iter().sum::<f64>() / curls.len() as f64;
    let zoom_presence = divs.iter().map(|&d| d.abs()).sum::<f64>() / divs.len() as f64 > 0.008;

    Some(SegmentMotion {
        shake_energy: (shake_energy * 10000.0).round() / 10000.0,
        zoom_presence,
        mean_divergence: (mean_div * 10000.0).round() / 10000.0,
        mean_curl: (mean_curl * 10000.0).round() / 10000.0,
    })
}

// --- Style Classifier v2 ---

#[derive(Debug, Clone, Default)]
pub struct ClassifierFeatures {
    pub cuts_count: usize,
    pub cut_density: f64,
    pub shake_energy: f64,
    pub one_framer_density: f64,
    pub one_framer_density_v2: f64,
    pub sync: f64,
    pub sync_downbeats_only: bool,
    pub zoom_presence: bool,
    pub slowdown_presence: bool,
    pub motion_available: bool,
    pub bpm: f64,
    pub sync_tolerance_ms: f64,
}

pub fn classify_style(features: &ClassifierFeatures) -> StyleDecision {
    let mut justifications = Vec::new();
    let norm_shake = if features.shake_energy > 0.1 {
        features.shake_energy
    } else {
        (features.shake_energy * 35.0).clamp(0.0, 1.0)
    };
    let effective_one_framer_density = features.one_framer_density_v2.max(features.one_framer_density);

    // 1. Basic / Clean: cuts < 2 and low shake
    if features.cuts_count < 2 && (norm_shake < 0.20 || features.shake_energy < 0.015) {
        justifications.push(format!("Very low cut count ({})", features.cuts_count));
        justifications.push(format!("Low camera shake energy ({:.4})", features.shake_energy));
        let conf = if features.cuts_count <= 1 { 0.90 } else { 0.80 };
        return StyleDecision {
            style_name: "basic/clean".to_string(),
            sub_style: Some("CLEAN (Minimal)".to_string()),
            archetype: Some(Archetype::CLEAN),
            confidence: conf,
            sync_tolerance_ms: if features.sync_tolerance_ms > 0.0 { Some(features.sync_tolerance_ms) } else { None },
            justifications,
        };
    }

    // 2. Jugg (Strict & Standard):
    // Strict: shake_intensity > 0.6 AND one_framer_density_v2 > 0.3 AND sync_score > 0.7
    // Standard: shake >= 0.40 AND one_framers >= 0.30 AND sync >= 0.40
    let is_jugg_strict = norm_shake > 0.60 && effective_one_framer_density > 0.30 && features.sync > 0.70;
    let is_jugg_std = (norm_shake >= 0.40 || features.shake_energy >= 0.012)
        && effective_one_framer_density >= 0.30
        && features.sync >= 0.40;

    if is_jugg_strict || is_jugg_std {
        if is_jugg_strict {
            justifications.push("Heavy sustained camera shake (intensity > 0.6)".to_string());
            justifications.push(format!("Frequent 1-frame micro-cuts/flashes ({:.2}/s)", effective_one_framer_density));
            justifications.push(format!("Tight beat synchronization ({:.1}%)", features.sync * 100.0));
            return StyleDecision {
                style_name: "jugg (strict)".to_string(),
                sub_style: Some("JUGG (Strict)".to_string()),
                archetype: Some(Archetype::JUGG),
                confidence: (0.85 + (features.sync * 0.12)).min(0.98),
                sync_tolerance_ms: if features.sync_tolerance_ms > 0.0 { Some(features.sync_tolerance_ms) } else { None },
                justifications,
            };
        } else {
            justifications.push(format!("High shake energy ({:.4})", features.shake_energy));
            justifications.push(format!("Frequent one-framers ({:.2} per sec)", effective_one_framer_density));
            justifications.push(format!("Rhythmic beat synchronization ({:.1}%)", features.sync * 100.0));
            let conf = (0.80 + (features.sync * 0.15)).min(0.95);
            return StyleDecision {
                style_name: "jugg".to_string(),
                sub_style: Some("JUGG (Standard)".to_string()),
                archetype: Some(Archetype::JUGG),
                confidence: (conf * 100.0).round() / 100.0,
                sync_tolerance_ms: if features.sync_tolerance_ms > 0.0 { Some(features.sync_tolerance_ms) } else { None },
                justifications,
            };
        }
    }

    // 3. Glitch-leaning: extreme cut density (>1.5/s) or extreme one-framers (>0.8/s)
    if features.cut_density >= 1.5 || (effective_one_framer_density >= 0.8 && (norm_shake >= 0.30 || features.shake_energy >= 0.010)) {
        if features.cut_density >= 1.5 {
            justifications.push(format!("High cut density ({:.2} cuts/sec)", features.cut_density));
        }
        if effective_one_framer_density >= 0.8 {
            justifications.push(format!("Heavy 1-frame flashes ({:.2} per sec)", effective_one_framer_density));
        }
        return StyleDecision {
            style_name: "glitch-leaning".to_string(),
            sub_style: Some("GLITCH (Flash)".to_string()),
            archetype: Some(Archetype::GLITCH),
            confidence: 0.85,
            sync_tolerance_ms: if features.sync_tolerance_ms > 0.0 { Some(features.sync_tolerance_ms) } else { None },
            justifications,
        };
    }

    // 4. Velocity / Flow (Liquid & Standard):
    // Liquid: sync_score > 0.5 AND shake_intensity < 0.4 AND slowdown_presence (low optical flow variance > 1s)
    let is_flow_liquid = features.sync > 0.50 && norm_shake < 0.40 && features.slowdown_presence;
    let is_flow_std = features.sync >= 0.45 && (norm_shake < 0.55 || features.shake_energy < 0.020) && features.slowdown_presence;

    if is_flow_liquid || is_flow_std {
        justifications.push(format!("Solid beat synchronization ({:.1}%)", features.sync * 100.0));
        justifications.push("Speed ramping / slowdowns detected".to_string());
        justifications.push(format!("Controlled motion flow (shake: {:.4})", features.shake_energy));
        let sub = if is_flow_liquid { "FLOW (Liquid)" } else { "FLOW (Standard)" };
        return StyleDecision {
            style_name: "velocity/flow".to_string(),
            sub_style: Some(sub.to_string()),
            archetype: Some(Archetype::FLOW),
            confidence: 0.85,
            sync_tolerance_ms: if features.sync_tolerance_ms > 0.0 { Some(features.sync_tolerance_ms) } else { None },
            justifications,
        };
    }

    // 5. Vibe (Groove):
    // Cuts aligned on downbeats only AND moderate shake (0.3-0.5) AND one_framer_density < 0.1
    let is_vibe = (features.sync_downbeats_only || (features.cuts_count >= 2 && features.cuts_count <= 6 && features.sync >= 0.40))
        && (norm_shake >= 0.25 && norm_shake <= 0.55)
        && effective_one_framer_density < 0.10;

    if is_vibe {
        justifications.push("Cuts locked to musical downbeat groove".to_string());
        justifications.push("Moderate smooth camera shake without flash interrupts".to_string());
        justifications.push(format!("Minimal one-framers ({:.2}/s)", effective_one_framer_density));
        return StyleDecision {
            style_name: "vibe (groove)".to_string(),
            sub_style: Some("VIBE (Groove)".to_string()),
            archetype: Some(Archetype::VIBE),
            confidence: 0.80,
            sync_tolerance_ms: if features.sync_tolerance_ms > 0.0 { Some(features.sync_tolerance_ms) } else { None },
            justifications,
        };
    }

    // 6. Hybrid / Unclassified
    justifications.push("Mixed characteristics across multiple style archetypes".to_string());
    if features.cut_density > 0.0 {
        justifications.push(format!(
            "Cut density: {:.2}/s, Sync: {:.1}%",
            features.cut_density,
            features.sync * 100.0
        ));
    }
    if !features.motion_available {
        justifications.push("Motion metadata not available; classification operating in degraded mode".to_string());
    }

    StyleDecision {
        style_name: "hybrid/unclassified".to_string(),
        sub_style: Some("HYBRID (Mixed)".to_string()),
        archetype: Some(Archetype::HYBRID),
        confidence: if features.motion_available { 0.65 } else { 0.50 },
        sync_tolerance_ms: if features.sync_tolerance_ms > 0.0 { Some(features.sync_tolerance_ms) } else { None },
        justifications,
    }
}

// --- Markdown Report Generator ---

pub fn generate_markdown_report(analysis: &DumpAnalysis, project: &ReusableProject) -> String {
    let source_name = std::path::Path::new(&analysis.source)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| analysis.source.clone());

    let mut md = String::new();
    md.push_str(&format!("# Dump Report: {}\n\n", source_name));

    // ## Detected style
    md.push_str("## Detected style\n");
    if let Some(ref sub) = analysis.detected_style.sub_style {
        md.push_str(&format!("- **Style:** `{}` ({})\n", analysis.detected_style.style_name, sub));
    } else {
        md.push_str(&format!("- **Style:** `{}`\n", analysis.detected_style.style_name));
    }
    md.push_str(&format!("- **Confidence:** {:.0}%\n", analysis.detected_style.confidence * 100.0));
    md.push_str("- **Justifications:**\n");
    for just in &analysis.detected_style.justifications {
        md.push_str(&format!("  - {}\n", just));
    }
    md.push('\n');

    // ## Cuts & sync
    md.push_str("## Cuts & sync\n");
    let cut_density = analysis.cuts.len() as f64 / analysis.duration.max(0.1);
    md.push_str(&format!("- **Total cuts:** {} (Density: {:.2} cuts/s)\n", analysis.cuts.len(), cut_density));
    if !analysis.cuts.is_empty() {
        md.push_str(&format!("- **Cut timestamps (s):** `{:?}`\n", analysis.cuts));
    }
    let tol_ms = analysis.sync_tolerance_ms.or(analysis.detected_style.sync_tolerance_ms).unwrap_or(60.0);
    if analysis.sync_na {
        md.push_str(&format!("- **Cut-Beat Sync (±{:.0}ms):** N/A (0 cuts detected)\n", tol_ms));
    } else {
        md.push_str(&format!("- **Cut-Beat Sync (±{:.0}ms):** {:.1}%\n", tol_ms, analysis.cut_beat_sync * 100.0));
    }
    md.push('\n');

    // ## Beats
    md.push_str("## Beats\n");
    md.push_str(&format!("- **Detected BPM:** {:.1}\n", analysis.beats.bpm));
    md.push_str(&format!("- **Total beats:** {}\n", analysis.beats.beats.len()));
    md.push_str(&format!("- **Total downbeats:** {}\n", analysis.beats.downbeats.len()));
    md.push('\n');

    // ## Segments (signatures)
    md.push_str("## Segments (signatures)\n");
    md.push_str("| # | Time Range | LAB Mean [L, a, b] | LAB Std | MAD Mean | MAD Peak | Shake Energy | One-Framers | Speed Hint |\n");
    md.push_str("|---|---|---|---|---|---|---|---|---|\n");
    for (i, seg) in analysis.segments.iter().enumerate() {
        let shake_str = seg
            .motion
            .as_ref()
            .map(|m| format!("{:.3}", m.shake_energy))
            .unwrap_or_else(|| "N/A".to_string());
        md.push_str(&format!(
            "| {} | {:.2}s - {:.2}s | [{:.1}, {:.1}, {:.1}] | [{:.1}, {:.1}, {:.1}] | {:.1} | {:.1} | {} | {} | `{}` |\n",
            i + 1,
            seg.start,
            seg.end,
            seg.lab.mean[0], seg.lab.mean[1], seg.lab.mean[2],
            seg.lab.std[0], seg.lab.std[1], seg.lab.std[2],
            seg.mad_mean,
            seg.mad_peak,
            shake_str,
            seg.one_framer_count,
            seg.speed_hint
        ));
    }
    md.push('\n');

    // ## Color signatures
    md.push_str("## Color signatures\n");
    let avg_l: f64 = if !analysis.segments.is_empty() {
        analysis.segments.iter().map(|s| s.lab.mean[0]).sum::<f64>() / analysis.segments.len() as f64
    } else {
        0.0
    };
    let avg_a: f64 = if !analysis.segments.is_empty() {
        analysis.segments.iter().map(|s| s.lab.mean[1]).sum::<f64>() / analysis.segments.len() as f64
    } else {
        0.0
    };
    let avg_b: f64 = if !analysis.segments.is_empty() {
        analysis.segments.iter().map(|s| s.lab.mean[2]).sum::<f64>() / analysis.segments.len() as f64
    } else {
        0.0
    };
    md.push_str(&format!("- **Average Global Luminance (L*):** {:.1}\n", avg_l));
    md.push_str(&format!("- **Average Chromaticity (a*, b*):** [{:.1}, {:.1}]\n", avg_a, avg_b));
    md.push('\n');

    // ## One-framers
    md.push_str("## One-framers\n");
    let one_framer_density = analysis.one_framers.len() as f64 / analysis.duration.max(0.1);
    md.push_str(&format!("- **Total detected:** {} ({:.2}/s)\n", analysis.one_framers.len(), one_framer_density));
    if !analysis.one_framers.is_empty() {
        md.push_str(&format!("- **Timestamps:** `{:?}`\n", analysis.one_framers));
    }
    md.push('\n');

    // ## Motion
    md.push_str("## Motion\n");
    if let Some(ref warn) = analysis.motion_warning {
        md.push_str(&format!("- **Status:** Warning — {}\n", warn));
    } else {
        let avg_shake: f64 = analysis
            .segments
            .iter()
            .filter_map(|s| s.motion.as_ref().map(|m| m.shake_energy))
            .sum::<f64>()
            / analysis.segments.len().max(1) as f64;
        let zoom_count = analysis
            .segments
            .iter()
            .filter(|s| s.motion.as_ref().map(|m| m.zoom_presence).unwrap_or(false))
            .count();
        md.push_str(&format!("- **Average Shake Energy:** {:.4}\n", avg_shake));
        md.push_str(&format!("- **Zoom Segments:** {} / {}\n", zoom_count, analysis.segments.len()));
        md.push_str("- **Reverse remap status:** Non mesurable depuis la sortie seule (non deviné, laissé aux métadonnées de projet originales).\n");
    }
    md.push('\n');

    // ## Reusable vs descriptive
    md.push_str("## Reusable vs descriptive\n");
    md.push_str("- **Measurable & Reusable (Project schema):**\n");
    md.push_str("  - Audio BPM, beat and downbeat grid\n");
    md.push_str("  - Cut timestamps & segment boundaries\n");
    md.push_str("  - Color palettes (LAB mean/std per segment)\n");
    md.push_str(&format!("  - Suggested style preset (`{}`) & target FPS ({:.0} fps)\n", project.suggested_style, project.fps_suggestion));
    md.push_str("- **Descriptive Only (Non-reconstructible from output):**\n");
    md.push_str("  - Clip source reverse remap (cannot be mathematically derived from single rendered clip without original sources)\n");
    md.push_str("  - Layer composition and raw footage pre-FX state\n");

    md
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
    let analysis_fps = original_fps.min(30.0);

    // Phase 1: SCENES (0..20%)
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
                percent: 20,
                message: format!("Detected {} cuts ({} scenes)", cuts.len(), scenes.len()),
            },
        );
    }

    // Phase 2: BEATS (20..40%)
    if let Some(app_handle) = app {
        let _ = app_handle.emit(
            "dump-progress",
            DumpProgressPayload {
                phase: "BEATS".to_string(),
                percent: 25,
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

    let (cut_beat_sync, sync_na, sync_tolerance_ms) = compute_cut_beat_sync_adaptive(&cuts, &beats.beats, beats.bpm);

    if let Some(app_handle) = app {
        let _ = app_handle.emit(
            "dump-progress",
            DumpProgressPayload {
                phase: "BEATS".to_string(),
                percent: 40,
                message: format!(
                    "BPM: {:.1} · Cut-Beat Sync: {:.0}% (±{:.0}ms)",
                    beats.bpm,
                    cut_beat_sync * 100.0,
                    sync_tolerance_ms
                ),
            },
        );
    }

    // Phase 3: MOTION (40..60%)
    if let Some(app_handle) = app {
        let _ = app_handle.emit(
            "dump-progress",
            DumpProgressPayload {
                phase: "MOTION".to_string(),
                percent: 45,
                message: "Extracting motion vectors and camera dynamics...".to_string(),
            },
        );
    }

    let decode_w: u32 = 640;
    let decode_h: u32 = ((media_info.height as f64 * (640.0 / media_info.width as f64)) as u32) & !1;

    let temp_trf_path = cache_dir.join(format!("dump_motion_{}.trf", std::process::id()));
    let temp_trf_str = temp_trf_path.to_string_lossy().replace('\\', "/").replace(':', "\\:");

    // vidstabdetect writes multi-block motion vectors directly to ASCII format on disk at >100 fps.
    // mestimate only stores motion vectors in internal AVFrame side-data, which metadata=print
    // does not serialize to stdout/dictionary, making vidstabdetect far more robust and efficient.
    let mut motion_cmd = std::process::Command::new(ffmpeg_bin);
    motion_cmd.args([
        "-y",
        "-i",
        video_path,
        "-vf",
        &format!("scale={decode_w}:{decode_h},fps={analysis_fps:.3},vidstabdetect=result='{temp_trf_str}':fileformat=ascii:shakiness=5:accuracy=9:stepsize=12"),
        "-an",
        "-f",
        "null",
        "-",
    ]);
    motion_cmd.stdout(std::process::Stdio::null());
    motion_cmd.stderr(std::process::Stdio::null());
    #[cfg(target_os = "windows")]
    motion_cmd.creation_flags(CREATE_NO_WINDOW);

    let motion_status = motion_cmd.status();
    let (motion_frames, motion_warning) = if motion_status.map(|s| s.success()).unwrap_or(false)
        && temp_trf_path.exists()
    {
        if let Ok(content) = std::fs::read_to_string(&temp_trf_path) {
            let _ = std::fs::remove_file(&temp_trf_path);
            let frames = parse_trf_content(
                &content,
                decode_w as f64,
                decode_h as f64,
                analysis_fps,
            );
            if frames.is_empty() {
                (Vec::new(), Some("Motion vector metadata was empty".to_string()))
            } else {
                (frames, None)
            }
        } else {
            let _ = std::fs::remove_file(&temp_trf_path);
            (Vec::new(), Some("Failed to read motion transforms file".to_string()))
        }
    } else {
        (Vec::new(), Some("Motion estimation filter unavailable or execution failed".to_string()))
    };

    if let Some(app_handle) = app {
        let _ = app_handle.emit(
            "dump-progress",
            DumpProgressPayload {
                phase: "MOTION".to_string(),
                percent: 60,
                message: format!("Motion vectors extracted ({} frames)", motion_frames.len()),
            },
        );
    }

    // Phase 4: PROFILES & COLOR & MAD (60..85%)
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
            let pct = 65 + (((frame_idx as f64 / total_est_frames as f64) * 20.0).min(20.0) as u32);
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

    let all_mads: Vec<f64> = frame_metrics.iter().map(|f| f.mad).collect();
    let all_timestamps: Vec<f64> = frame_metrics.iter().map(|f| f.t).collect();
    let global_one_framers = detect_one_framers(&all_mads, &all_timestamps);
    let global_slowdown_presence = detect_slowdown_presence(&all_mads);

    // Aggregate metrics per segment
    let mut segments: Vec<DumpSegment> = Vec::new();
    let mut segment_shake_energies = Vec::new();

    for win in boundaries.windows(2) {
        let seg_start = win[0];
        let seg_end = win[1];

        let seg_frames: Vec<&FrameMetric> = frame_metrics
            .iter()
            .filter(|f| f.t >= seg_start - 1e-4 && f.t < seg_end + 1e-4)
            .collect();

        let seg_motion_frames: Vec<FrameMotion> = motion_frames
            .iter()
            .filter(|f| f.t >= seg_start - 1e-4 && f.t < seg_end + 1e-4)
            .cloned()
            .collect();

        let seg_motion = compute_segment_motion(&seg_motion_frames);
        if let Some(ref m) = seg_motion {
            segment_shake_energies.push(m.shake_energy);
        }

        let seg_mads: Vec<f64> = seg_frames.iter().map(|f| f.mad).collect();
        let seg_ts: Vec<f64> = seg_frames.iter().map(|f| f.t).collect();
        let seg_one_framers = detect_one_framers(&seg_mads, &seg_ts);

        if seg_frames.is_empty() {
            segments.push(DumpSegment {
                start: (seg_start * 1000.0).round() / 1000.0,
                end: (seg_end * 1000.0).round() / 1000.0,
                lab: LabStats {
                    mean: [0.0, 0.0, 0.0],
                    std: [0.0, 0.0, 0.0],
                },
                mad_mean: 0.0,
                mad_peak: 0.0,
                motion: seg_motion,
                one_framer_count: 0,
                speed_hint: "normal".to_string(),
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

        let mad_mean = ((sum_mad / n) * 100.0).round() / 100.0;
        let mad_peak = (peak_mad * 100.0).round() / 100.0;

        let speed_hint = if mad_mean < 4.0 && (seg_end - seg_start) > 0.8 {
            "slow".to_string()
        } else if mad_mean > 18.0 {
            "fast".to_string()
        } else if mad_peak > 35.0 && !seg_one_framers.is_empty() {
            "snap".to_string()
        } else {
            "normal".to_string()
        };

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
            mad_mean,
            mad_peak,
            motion: seg_motion,
            one_framer_count: seg_one_framers.len(),
            speed_hint,
        });
    }

    // Phase 5: CLASSIFICATION, REPORT & REUSABLE PROJECT (85..100%)
    if let Some(app_handle) = app {
        let _ = app_handle.emit(
            "dump-progress",
            DumpProgressPayload {
                phase: "REPORT".to_string(),
                percent: 90,
                message: "Running style classifier and generating reports...".to_string(),
            },
        );
    }

    let avg_shake = if !segment_shake_energies.is_empty() {
        segment_shake_energies.iter().sum::<f64>() / segment_shake_energies.len() as f64
    } else {
        0.0
    };

    let zoom_presence = segments
        .iter()
        .any(|s| s.motion.as_ref().map(|m| m.zoom_presence).unwrap_or(false));

    let global_one_framers_v2 = detect_one_framers_v2(&all_mads, &all_timestamps);
    let sync_downbeats_only = check_downbeats_sync_profile(&cuts, &beats.beats, &beats.downbeats, sync_tolerance_ms / 1000.0);

    let features = ClassifierFeatures {
        cuts_count: cuts.len(),
        cut_density: cuts.len() as f64 / duration.max(0.1),
        shake_energy: avg_shake,
        one_framer_density: global_one_framers.len() as f64 / duration.max(0.1),
        one_framer_density_v2: global_one_framers_v2.len() as f64 / duration.max(0.1),
        sync: cut_beat_sync,
        sync_downbeats_only,
        zoom_presence,
        slowdown_presence: global_slowdown_presence,
        motion_available: motion_warning.is_none() && !motion_frames.is_empty(),
        bpm: beats.bpm,
        sync_tolerance_ms,
    };

    let detected_style = classify_style(&features);

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

    let video_stem = std::path::Path::new(video_path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "video".to_string());

    let json_file_path = dump_dir.join(format!("analysis_{timestamp}.json"));
    let report_file_path = dump_dir.join(format!("{video_stem}_report.md"));
    let reusable_project_path = dump_dir.join("reusable_project.json");

    // Build Reusable Project
    let reusable_segments: Vec<ReusableSegment> = segments
        .iter()
        .map(|s| ReusableSegment {
            start: s.start,
            end: s.end,
            lab_mean: s.lab.mean,
            lab_std: s.lab.std,
            speed_hint: s.speed_hint.clone(),
        })
        .collect();

    let fps_suggestion = if original_fps >= 55.0 {
        60.0
    } else if original_fps >= 28.0 {
        30.0
    } else {
        16.0
    };

    let reusable_project = ReusableProject {
        schema_version: "dumper_project_v1".to_string(),
        source: video_path.to_string(),
        beats: beats.clone(),
        cuts: cuts.clone(),
        segments: reusable_segments,
        suggested_style: detected_style.style_name.clone(),
        fps_suggestion,
    };

    let analysis = DumpAnalysis {
        schema_version: 1,
        source: video_path.to_string(),
        duration: (duration * 1000.0).round() / 1000.0,
        fps: (original_fps * 100.0).round() / 100.0,
        cuts,
        scenes,
        beats,
        cut_beat_sync,
        sync_na,
        sync_tolerance_ms: Some(sync_tolerance_ms),
        detected_style,
        one_framers: global_one_framers,
        one_framers_v2: Some(global_one_framers_v2),
        segments,
        motion_warning,
        json_path: Some(json_file_path.to_string_lossy().to_string()),
        report_path: Some(report_file_path.to_string_lossy().to_string()),
        reusable_project_path: Some(reusable_project_path.to_string_lossy().to_string()),
    };

    // Serialize analysis.json
    let json_content = serde_json::to_string_pretty(&analysis)
        .map_err(|e| format!("Failed to serialize analysis JSON: {e}"))?;
    std::fs::write(&json_file_path, json_content)
        .map_err(|e| format!("Failed to write analysis JSON file: {e}"))?;

    // Serialize reusable_project.json
    let project_content = serde_json::to_string_pretty(&reusable_project)
        .map_err(|e| format!("Failed to serialize reusable project JSON: {e}"))?;
    std::fs::write(&reusable_project_path, project_content)
        .map_err(|e| format!("Failed to write reusable project file: {e}"))?;

    // Generate markdown report
    let report_content = generate_markdown_report(&analysis, &reusable_project);
    std::fs::write(&report_file_path, report_content)
        .map_err(|e| format!("Failed to write markdown report file: {e}"))?;

    if let Some(app_handle) = app {
        let _ = app_handle.emit(
            "dump-progress",
            DumpProgressPayload {
                phase: "REPORT".to_string(),
                percent: 100,
                message: "Analysis and reports completed successfully".to_string(),
            },
        );
    }

    Ok(analysis)
}

// --- Style & FPS Mapping ---

pub fn map_dumper_style_to_jugg_style(suggested_style: &str) -> &'static str {
    match suggested_style.to_lowercase().as_str() {
        "jugg" => "HARD",
        "glitch-leaning" => "HARD",
        "velocity/flow" => "SMOOTH",
        "basic/clean" => "SMOOTH",
        "hybrid/unclassified" => "HYBRID",
        _ => "HYBRID",
    }
}

pub fn clamp_dumper_fps(fps_suggestion: f64) -> u32 {
    fps_suggestion.round().clamp(12.0, 60.0) as u32
}

pub fn convert_dumper_project_to_plan(project: &ReusableProject) -> Result<ProjectPlan, String> {
    let mapped_style = map_dumper_style_to_jugg_style(&project.suggested_style);
    let fps = clamp_dumper_fps(project.fps_suggestion);
    let bpm = project.beats.bpm;
    let beats = &project.beats.beats;
    let downbeats = &project.beats.downbeats;

    // Determine target duration
    let target_duration = if let Some(last_seg) = project.segments.last() {
        last_seg.end
    } else if let Some(&last_cut) = project.cuts.last() {
        last_cut
    } else if let Some(&last_beat) = beats.last() {
        last_beat
    } else {
        10.0
    };

    if target_duration <= 0.0 {
        return Err("Target duration must be greater than 0".to_string());
    }

    let min_seg_dur = 3.0 / (fps as f64);
    let (default_a0, default_omega, default_k, default_zoom_max) = match mapped_style {
        "SMOOTH" => (3.0, 8.0, 2.0, 1.05),
        "HYBRID" => (5.0, 12.0, 2.5, 1.10),
        _ => (8.0, 15.0, 3.0, 1.15), // HARD default
    };

    let mut plan_segments = Vec::new();

    if !project.segments.is_empty() {
        let mut curr_t0 = 0.0;
        for (idx, seg) in project.segments.iter().enumerate() {
            let mut t1 = seg.end;
            if idx == project.segments.len() - 1 {
                t1 = target_duration;
            }
            if t1 <= curr_t0 {
                t1 = curr_t0 + min_seg_dur;
            }

            let curve_name = match mapped_style {
                "SMOOTH" => "saddle".to_string(),
                "HYBRID" => {
                    if idx % 2 == 0 {
                        "snap".to_string()
                    } else {
                        "saddle".to_string()
                    }
                }
                _ => "snap".to_string(),
            };

            let (scale_start, scale_end) = if idx % 2 == 0 {
                (1.0, default_zoom_max)
            } else {
                (default_zoom_max, 1.0)
            };

            let seed = ((idx as u32).wrapping_mul(1664525).wrapping_add(1013904223)) ^ 0x5bf03635;
            let s1 = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let s2 = s1.wrapping_mul(1664525).wrapping_add(1013904223);
            let s3 = s2.wrapping_mul(1664525).wrapping_add(1013904223);
            let s4 = s3.wrapping_mul(1664525).wrapping_add(1013904223);
            let s5 = s4.wrapping_mul(1664525).wrapping_add(1013904223);
            let s6 = s5.wrapping_mul(1664525).wrapping_add(1013904223);
            let s7 = s6.wrapping_mul(1664525).wrapping_add(1013904223);

            let pct = |sn: u32| -> u32 { sn % 100 };
            let is_hard = mapped_style == "HARD";
            let is_hybrid = mapped_style == "HYBRID";

            let bouncy_shake = if (is_hard || is_hybrid) && pct(s1) < 30 {
                Some(BouncyShake { axis: (s1 % 2) as u8, amplitude: if is_hard { 40.0 } else { 25.0 } })
            } else {
                None
            };

            let dissolve_shake = if (is_hard || is_hybrid) && pct(s2) < 25 {
                Some(DissolveShake { pct: if is_hard { 30.0 } else { 15.0 } })
            } else {
                None
            };

            let skew_shake = if (is_hard || is_hybrid) && pct(s3) < 20 {
                Some(SkewShake { s0_deg: if is_hard { 10.0 } else { 7.0 } })
            } else {
                None
            };

            let squish_pop = if (is_hard || is_hybrid) && pct(s4) < 40 {
                Some(SquishPop { _pad: 0 })
            } else {
                None
            };

            let optics_bounce = if (is_hard || is_hybrid) && pct(s5) < 25 {
                Some(OpticsBounce { k0: if is_hard { 0.08 } else { 0.06 } })
            } else {
                None
            };

            let buildup_chain = if pct(s6) < 30 {
                Some(BuildupChain { chain_next: true, chain_from_prev: false })
            } else {
                None
            };

            let warp_stretch = if (is_hard || is_hybrid) && pct(s7) < 20 {
                Some(WarpStretch { axis: (s7 % 2) as u8, scale_start: if is_hard { 1.40 } else { 1.30 } })
            } else {
                None
            };

            let s_dur = t1 - curr_t0;

            let effects = SegmentEffects {
                shake: ShakeEffect {
                    a0: if bouncy_shake.is_some() { 0.0 } else { default_a0 },
                    omega: default_omega,
                    k: default_k,
                    seed,
                },
                zoom: ZoomEffect {
                    scale_start,
                    scale_end,
                },
                reverse: false,
                bouncy_shake,
                dissolve_shake,
                skew_shake,
                squish_pop,
                optics_bounce,
                buildup_chain,
                warp_stretch,
                zoom_beat_offset: if mapped_style == "SMOOTH" { 0 } else { 1 },
            };

            plan_segments.push(PlanSegment {
                t0: curr_t0,
                t1,
                s0: 0.0,
                s1: s_dur,
                curve: curve_name,
                effects,
                transition: None,
                color_hints: Some(ColorHints {
                    lab_mean: seg.lab_mean,
                    lab_std: seg.lab_std,
                }),
            });

            curr_t0 = t1;
        }
    } else {
        let mut bounds = vec![0.0];
        for &c in &project.cuts {
            if c > 0.0 && c < target_duration && c > *bounds.last().unwrap() + min_seg_dur {
                bounds.push(c);
            }
        }
        bounds.push(target_duration);

        for (idx, win) in bounds.windows(2).enumerate() {
            let t0 = win[0];
            let t1 = win[1];
            let s_dur = t1 - t0;
            let (scale_start, scale_end) = if idx % 2 == 0 {
                (1.0, default_zoom_max)
            } else {
                (default_zoom_max, 1.0)
            };
            let seed = ((idx as u32).wrapping_mul(1664525).wrapping_add(1013904223)) ^ 0x5bf03635;

            plan_segments.push(PlanSegment {
                t0,
                t1,
                s0: 0.0,
                s1: s_dur,
                curve: if mapped_style == "SMOOTH" { "saddle".to_string() } else { "snap".to_string() },
                effects: SegmentEffects {
                    shake: ShakeEffect { a0: default_a0, omega: default_omega, k: default_k, seed },
                    zoom: ZoomEffect { scale_start, scale_end },
                    reverse: false,
                    ..crate::default_segment_effects()
                },
                transition: None,
                color_hints: None,
            });
        }
    }

    let one_framers = generate_one_framers(mapped_style, &plan_segments, downbeats, fps, target_duration);
    let transitions = generate_transitions(mapped_style, &mut plan_segments, &[], fps, None);
    let ambiance = Some(default_ambiance(mapped_style, downbeats));

    Ok(ProjectPlan {
        schema_version: 2,
        style: mapped_style.to_string(),
        fps,
        aspect: AspectRatio { w: 1080, h: 1080 },
        borderless: true,
        bpm,
        target_duration,
        video_duration: target_duration,
        audio_duration: target_duration,
        loops: 1,
        motion_blur: false,
        full_fx: true,
        custom_params: Some(get_style_defaults(mapped_style)),
        one_framers,
        transitions,
        ambiance,
        source_fx: vec![],
        audio_mix: Some(crate::audio::AudioMixConfig::default()),
        remap_params: Some(crate::presets::get_preset_params(mapped_style)),
        segments: plan_segments,
        export: ExportConfig::default(),
    })
}

pub type EditPlan = ProjectPlan;

pub fn generate_remap_plan_from_analysis(analysis: &DumpAnalysis) -> Result<ProjectPlan, String> {
    generate_remap_plan_from_analysis_with_params(analysis, None)
}

pub fn generate_remap_plan_from_analysis_with_params(
    analysis: &DumpAnalysis,
    user_params: Option<&RemapParams>,
) -> Result<ProjectPlan, String> {
    let mapped_style = match analysis.detected_style.archetype {
        Some(Archetype::JUGG) | Some(Archetype::GLITCH) => "HARD",
        Some(Archetype::FLOW) | Some(Archetype::CLEAN) => "SMOOTH",
        Some(Archetype::VIBE) | Some(Archetype::HYBRID) => "HYBRID",
        None => map_dumper_style_to_jugg_style(&analysis.detected_style.style_name),
    };

    let params = user_params.cloned().unwrap_or_else(|| crate::presets::get_preset_params(mapped_style));

    let fps = clamp_dumper_fps(analysis.fps);
    let bpm = if analysis.beats.bpm > 0.0 { analysis.beats.bpm } else { 120.0 };
    let _beats = &analysis.beats.beats;
    let downbeats = &analysis.beats.downbeats;
    let target_duration = if analysis.duration > 0.0 { analysis.duration } else { 10.0 };
    let dt_frame = 1.0 / (fps as f64);
    let min_seg_dur = 3.0 * dt_frame;

    // 1. Build initial bounds: start at 0.0, include cuts, downbeats, end at target_duration
    let mut initial_bounds = vec![0.0];
    for &c in &analysis.cuts {
        if c > 0.0 && c < target_duration {
            initial_bounds.push(c);
        }
    }
    for &db in downbeats {
        if db > 0.0 && db < target_duration && !initial_bounds.iter().any(|&b| (b - db).abs() < min_seg_dur) {
            initial_bounds.push(db);
        }
    }
    initial_bounds.push(target_duration);
    initial_bounds.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    initial_bounds.dedup_by(|a, b| (*b - *a).abs() < 1e-4);

    let mut filtered_bounds = vec![0.0];
    for &b in &initial_bounds[1..] {
        let last = *filtered_bounds.last().unwrap();
        if b - last >= min_seg_dur {
            filtered_bounds.push(b);
        }
    }
    if *filtered_bounds.last().unwrap() < target_duration {
        if target_duration - *filtered_bounds.last().unwrap() < min_seg_dur && filtered_bounds.len() > 1 {
            filtered_bounds.pop();
        }
        filtered_bounds.push(target_duration);
    }

    // 2. Punch-in amplitude based on sync_score or user_params
    let sync_score = if analysis.sync_na { 0.50 } else { analysis.cut_beat_sync.clamp(0.0, 1.0) };
    let punch_amp = if user_params.is_some() {
        params.punch_in_scale as f64
    } else {
        match mapped_style {
            "HARD" => 1.10 + 0.15 * sync_score,
            "SMOOTH" => 1.04 + 0.04 * sync_score,
            _ => 1.07 + 0.08 * sync_score,
        }
    };

    let (default_a0, default_omega, default_k) = if user_params.is_some() {
        ((params.shake_intensity * 15.0) as f64, params.shake_freq_hz as f64, (1000.0 / params.shake_decay_ms.max(1.0)) as f64)
    } else {
        match mapped_style {
            "HARD" => {
                let conf_factor = (analysis.detected_style.confidence - 0.5).max(0.0);
                (8.0 * (1.0 + conf_factor), 20.0, 3.0)
            }
            "SMOOTH" => (2.5, 7.0, 1.8),
            _ => (5.0, 12.0, 2.2), // HYBRID
        }
    };

    let mut plan_segments = Vec::new();
    let mut downbeat_count = 0usize;

    for (idx, win) in filtered_bounds.windows(2).enumerate() {
        let t0 = win[0];
        let t1 = win[1];
        let s_dur = t1 - t0;

        let is_on_downbeat = downbeats.iter().any(|&db| (t0 - db).abs() <= dt_frame + 1e-4);
        let mut reverse_this_segment = false;
        let mut scale_start = 1.0;
        let mut scale_end = 1.0;

        if is_on_downbeat {
            downbeat_count += 1;
            scale_start = punch_amp;
            scale_end = 1.0;
            if user_params.is_some() {
                if params.reverse_cut_probability > 0.0 {
                    let seed = ((idx as u32).wrapping_mul(1664525).wrapping_add(1013904223)) ^ 0x5bf03635;
                    if ((seed % 100) as f32 / 100.0) < params.reverse_cut_probability {
                        reverse_this_segment = true;
                    }
                }
            } else {
                match mapped_style {
                    "HARD" => {
                        if downbeat_count % 3 == 1 || downbeat_count == 1 {
                            reverse_this_segment = true;
                        }
                    }
                    "SMOOTH" => {
                        reverse_this_segment = false;
                    }
                    _ => {
                        if downbeat_count % 5 == 1 {
                            reverse_this_segment = true;
                        }
                    }
                }
            }
        }

        let curve_name = match mapped_style {
            "HARD" => "snap".to_string(),
            "SMOOTH" => "saddle".to_string(),
            _ => {
                if idx % 2 == 0 {
                    "snap".to_string()
                } else {
                    "saddle".to_string()
                }
            }
        };

        let (s0, s1) = if reverse_this_segment {
            (s_dur, 0.0)
        } else {
            (0.0, s_dur)
        };

        let seed = ((idx as u32).wrapping_mul(1664525).wrapping_add(1013904223)) ^ 0x5bf03635;

        plan_segments.push(PlanSegment {
            t0,
            t1,
            s0,
            s1,
            curve: curve_name,
            effects: SegmentEffects {
                shake: ShakeEffect {
                    a0: default_a0,
                    omega: default_omega,
                    k: default_k,
                    seed,
                },
                zoom: ZoomEffect {
                    scale_start,
                    scale_end,
                },
                reverse: reverse_this_segment,
                ..crate::default_segment_effects()
            },
            transition: None,
            color_hints: None,
        });
    }

    let one_framers = if let Some(ref v2) = analysis.one_framers_v2 {
        if !v2.is_empty() {
            v2.iter()
                .enumerate()
                .map(|(i, &t)| OneFramer {
                    t,
                    framer_type: ONE_FRAMER_TYPES[i % ONE_FRAMER_TYPES.len()].to_string(),
                })
                .collect()
        } else {
            generate_one_framers(mapped_style, &plan_segments, downbeats, fps, target_duration)
        }
    } else {
        generate_one_framers(mapped_style, &plan_segments, downbeats, fps, target_duration)
    };

    let transitions = generate_transitions(mapped_style, &mut plan_segments, &[], fps, None);
    let ambiance = Some(default_ambiance(mapped_style, downbeats));

    // Procedural Source FX Generation (T36 & T38)
    let shake_intensity = if user_params.is_some() {
        params.shake_intensity
    } else if let Some(Archetype::JUGG) | Some(Archetype::GLITCH) = analysis.detected_style.archetype {
        (analysis.detected_style.confidence as f32).max(0.8)
    } else if let Some(Archetype::HYBRID) = analysis.detected_style.archetype {
        0.6f32
    } else if analysis.detected_style.style_name.to_uppercase().contains("JUGG") {
        0.8f32
    } else {
        0.0f32
    };

    let one_framer_density = if let Some(ref v2) = analysis.one_framers_v2 {
        v2.len() as f64 / target_duration.max(1.0)
    } else {
        analysis.one_framers.len() as f64 / target_duration.max(1.0)
    };

    let mut source_fx: Vec<SourceFxKeyframe> = Vec::new();

    // 1. If shake_intensity > 0.5 -> Inject RGB Split
    if shake_intensity > 0.5 {
        let rgb_intensity = if user_params.is_some() { params.rgb_split_intensity } else { shake_intensity };
        for &b in &analysis.beats.beats {
            if b >= 0.0 && b < target_duration {
                source_fx.push(SourceFxKeyframe {
                    timestamp: b,
                    duration: (3.0 * dt_frame).min(0.12),
                    fx_type: SourceFxType::RgbSplit,
                    intensity: rgb_intensity,
                });
            }
        }
    }

    // 2. If one_framer_density > 0.2 -> Inject Flashes
    let flash_intensity = if user_params.is_some() { params.flash_intensity } else { 0.9 };
    if one_framer_density > 0.2 || analysis.one_framers_v2.as_ref().map_or(false, |v| !v.is_empty()) || !analysis.one_framers.is_empty() {
        for &db in downbeats {
            if db >= 0.0 && db < target_duration {
                source_fx.push(SourceFxKeyframe {
                    timestamp: db,
                    duration: dt_frame,
                    fx_type: SourceFxType::Flash,
                    intensity: flash_intensity,
                });
            }
        }
    }

    // 3. If style is GLITCH -> Add micro-cuts / block glitch on segments
    let is_glitch = matches!(analysis.detected_style.archetype, Some(Archetype::GLITCH))
        || analysis.detected_style.style_name.to_uppercase().contains("GLITCH");

    if is_glitch {
        for (i, seg) in plan_segments.iter().enumerate() {
            if i % 2 == 0 {
                source_fx.push(SourceFxKeyframe {
                    timestamp: seg.t0,
                    duration: (seg.t1 - seg.t0) * 0.5,
                    fx_type: SourceFxType::BlockGlitch,
                    intensity: 0.75,
                });
            }
        }
    }

    Ok(ProjectPlan {
        schema_version: 2,
        style: mapped_style.to_string(),
        fps,
        aspect: AspectRatio { w: 1080, h: 1080 },
        borderless: true,
        bpm,
        target_duration,
        video_duration: target_duration,
        audio_duration: target_duration,
        loops: 1,
        motion_blur: false,
        full_fx: true,
        custom_params: Some(get_style_defaults(mapped_style)),
        one_framers,
        transitions,
        ambiance,
        source_fx,
        audio_mix: Some(crate::audio::AudioMixConfig::default()),
        remap_params: Some(params),
        segments: plan_segments,
        export: ExportConfig::default(),
    })
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

#[tauri::command]
pub fn apply_dumper_project(
    project_path: Option<String>,
    project: Option<ReusableProject>,
) -> Result<ProjectPlan, String> {
    if let Some(proj) = project {
        convert_dumper_project_to_plan(&proj)
    } else if let Some(path) = project_path {
        let content = std::fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read reusable project at {path}: {e}"))?;
        let proj: ReusableProject = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse reusable project JSON: {e}"))?;
        convert_dumper_project_to_plan(&proj)
    } else {
        Err("Either project or project_path must be provided".to_string())
    }
}

#[tauri::command]
pub fn generate_remap_plan(
    analysis_path: Option<String>,
    analysis: Option<DumpAnalysis>,
) -> Result<ProjectPlan, String> {
    if let Some(a) = analysis {
        generate_remap_plan_from_analysis(&a)
    } else if let Some(path) = analysis_path {
        let content = std::fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read analysis JSON at {path}: {e}"))?;
        let parsed: DumpAnalysis = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse analysis JSON: {e}"))?;
        generate_remap_plan_from_analysis(&parsed)
    } else {
        Err("Either analysis or analysis_path must be provided".to_string())
    }
}

pub fn run_one_click_jugg_internal(
    app: Option<&tauri::AppHandle>,
    source_video: &str,
    target_audio: &str,
    output_path: Option<&str>,
) -> Result<crate::render::RenderStats, String> {
    if let Some(a) = app {
        let _ = a.emit("render-progress", crate::render::RenderProgressPayload {
            phase: "DECODING".to_string(),
            percent: 5,
            current_frame: 0,
            total_frames: 100,
            message: "Phase 1/3: Analyzing source video with Dumper v2...".to_string(),
        });
    }

    // Step 1: Run Dumper analysis (Fail-fast: if analysis fails, abort immediately)
    let analysis = run_dump_pipeline_internal(app, source_video)
        .map_err(|e| format!("One-Click Jugg aborted at Phase 1 (Analysis): {e}"))?;

    if let Some(a) = app {
        let _ = a.emit("render-progress", crate::render::RenderProgressPayload {
            phase: "SAMPLING".to_string(),
            percent: 25,
            current_frame: 25,
            total_frames: 100,
            message: "Phase 2/3: Generating rhythmic remap plan & Source FX...".to_string(),
        });
    }

    // Step 2: Generate Remap Plan & Source FX
    let plan = generate_remap_plan_from_analysis(&analysis)
        .map_err(|e| format!("One-Click Jugg aborted at Phase 2 (Plan Generation): {e}"))?;

    let plan_json = serde_json::to_string(&plan)
        .map_err(|e| format!("Failed to serialize plan JSON: {e}"))?;

    if let Some(a) = app {
        let _ = a.emit("render-progress", crate::render::RenderProgressPayload {
            phase: "ENCODING".to_string(),
            percent: 30,
            current_frame: 30,
            total_frames: 100,
            message: "Phase 3/3: Rendering Final Assembly...".to_string(),
        });
    }

    // Step 3: Render Final Jugg Assembly
    let stats = crate::render::render_final_jugg_internal(
        app,
        &plan_json,
        source_video,
        target_audio,
        None,
        output_path,
    )?;

    Ok(stats)
}

#[tauri::command]
pub async fn run_one_click_jugg(
    app: tauri::AppHandle,
    source_video: String,
    target_audio: String,
    output_path: Option<String>,
) -> Result<crate::render::RenderStats, String> {
    run_one_click_jugg_internal(
        Some(&app),
        &source_video,
        &target_audio,
        output_path.as_deref(),
    )
}
