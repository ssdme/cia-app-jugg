use std::path::{Path, PathBuf};
use tauri::{Emitter, Manager};
use rayon::prelude::*;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub fn encode_base64(data: &[u8]) -> String {
    const B64_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0];
        let b1 = if chunk.len() > 1 { chunk[1] } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] } else { 0 };

        let n = ((b0 as u32) << 16) | ((b1 as u32) << 8) | (b2 as u32);

        result.push(B64_CHARS[((n >> 18) & 63) as usize] as char);
        result.push(B64_CHARS[((n >> 12) & 63) as usize] as char);

        if chunk.len() > 1 {
            result.push(B64_CHARS[((n >> 6) & 63) as usize] as char);
        } else {
            result.push('=');
        }

        if chunk.len() > 2 {
            result.push(B64_CHARS[(n & 63) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LayerItem {
    pub name: String,
    pub file: String,
    pub z_order: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_content: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail_base64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub z_depth: Option<f32>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CompositionResult {
    pub status: String,
    pub character_path: String,
    pub background_path: Option<String>,
    pub output_dir: String,
    pub layers_count: usize,
    pub layers_json_path: String,
    pub layers: Vec<LayerItem>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CompProject {
    pub schema_version: String, // "comp_project_v1"
    pub character_path: String,
    pub background_path: Option<String>,
    #[serde(default)]
    pub audio_path: Option<String>,
    pub layers: Vec<LayerItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallax_strength: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub beat_punch_intensity: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub light_wrap_intensity: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chromatic_aberration: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub impact_blur_strength: Option<f32>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum BlendMode {
    Normal,
    Multiply,
    Add,
    Screen,
    Lighten,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CompositionOp {
    pub id: String,
    pub name: String,
    pub op_type: String, // "drop_shadow" | "light_wrap" | "tint" | "rim_light" | "gradient" | "blur"
    pub blend_mode: BlendMode,
    pub opacity: f32, // 0.0 .. 1.0
    pub mask_by_alpha: bool,
    pub enabled: bool,
    #[serde(default)]
    pub params: serde_json::Value,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CompositionProgress {
    pub phase: String, // "COMPOSITING" | "ENCODING" | "DONE"
    pub percent: u32,
    pub current_frame: u32,
    pub total_frames: u32,
    pub message: String,
}

pub fn get_default_composition_ops() -> Vec<CompositionOp> {
    vec![
        CompositionOp {
            id: "drop_shadow".to_string(),
            name: "Drop Shadow".to_string(),
            op_type: "drop_shadow".to_string(),
            blend_mode: BlendMode::Multiply,
            opacity: 0.60,
            mask_by_alpha: false,
            enabled: true,
            params: serde_json::json!({
                "offsetX": 12.0,
                "offsetY": 16.0,
                "blurRadius": 14.0,
                "color": [0, 0, 0]
            }),
        },
        CompositionOp {
            id: "light_wrap".to_string(),
            name: "Light Wrap".to_string(),
            op_type: "light_wrap".to_string(),
            blend_mode: BlendMode::Screen,
            opacity: 0.55,
            mask_by_alpha: true,
            enabled: true,
            params: serde_json::json!({
                "blurRadius": 22.0,
                "edgeWidth": 10.0
            }),
        },
        CompositionOp {
            id: "tint_raccord".to_string(),
            name: "Tint de raccord".to_string(),
            op_type: "tint".to_string(),
            blend_mode: BlendMode::Multiply,
            opacity: 0.07,
            mask_by_alpha: true,
            enabled: true,
            params: serde_json::json!({
                "mode": "auto_background"
            }),
        },
        CompositionOp {
            id: "rim_light".to_string(),
            name: "Rim Light".to_string(),
            op_type: "rim_light".to_string(),
            blend_mode: BlendMode::Add,
            opacity: 0.65,
            mask_by_alpha: true,
            enabled: true,
            params: serde_json::json!({
                "edgeWidth": 4.0,
                "color": [220, 240, 255]
            }),
        },
    ]
}

#[inline(always)]
pub fn apply_blend_mode(b: f32, s: f32, mode: &BlendMode) -> f32 {
    match mode {
        BlendMode::Normal => s,
        BlendMode::Multiply => b * s,
        BlendMode::Add => (b + s).min(1.0),
        BlendMode::Screen => 1.0 - (1.0 - b) * (1.0 - s),
        BlendMode::Lighten => b.max(s),
    }
}

#[inline(always)]
pub fn alpha_over_pixel(bg: [u8; 4], fg: [u8; 4]) -> [u8; 4] {
    let a_fg = fg[3] as f32 / 255.0;
    if a_fg <= 0.0 {
        return bg;
    }
    let a_bg = bg[3] as f32 / 255.0;
    let a_out = a_fg + a_bg * (1.0 - a_fg);
    if a_out <= 0.0 {
        return [0, 0, 0, 0];
    }
    let r = (fg[0] as f32 * a_fg + bg[0] as f32 * a_bg * (1.0 - a_fg)) / a_out;
    let g = (fg[1] as f32 * a_fg + bg[1] as f32 * a_bg * (1.0 - a_fg)) / a_out;
    let b = (fg[2] as f32 * a_fg + bg[2] as f32 * a_bg * (1.0 - a_fg)) / a_out;

    [
        r.round().clamp(0.0, 255.0) as u8,
        g.round().clamp(0.0, 255.0) as u8,
        b.round().clamp(0.0, 255.0) as u8,
        (a_out * 255.0).round().clamp(0.0, 255.0) as u8,
    ]
}

#[inline(always)]
pub fn box_blur_1d_h(src: &[f32], out: &mut [f32], w: usize, h: usize, r: usize) {
    if w == 0 || h == 0 { return; }
    let r_clamped = r.min(w - 1);
    let iarr = 1.0 / ((r_clamped + r_clamped + 1) as f32);

    let rows_per_chunk = (h / rayon::current_num_threads().max(1)).max(8);
    out.par_chunks_mut(w * rows_per_chunk)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let start_y = chunk_idx * rows_per_chunk;
            let chunk_rows = chunk.len() / w;
            for cy in 0..chunk_rows {
                let y = start_y + cy;
                let row_offset = y * w;
                let row_out = &mut chunk[cy * w..(cy + 1) * w];

                let fv = src[row_offset];
                let lv = src[row_offset + w - 1];
                let mut val = (r_clamped + 1) as f32 * fv;
                for i in 0..r_clamped {
                    val += src[row_offset + i];
                }
                for i in 0..=r_clamped {
                    val += src[row_offset + (i + r_clamped).min(w - 1)] - fv;
                    row_out[i] = val * iarr;
                }
                for i in (r_clamped + 1)..(w.saturating_sub(r_clamped)) {
                    val += src[row_offset + i + r_clamped] - src[row_offset + i - r_clamped - 1];
                    row_out[i] = val * iarr;
                }
                for i in (w.saturating_sub(r_clamped))..w {
                    val += lv - src[row_offset + i.saturating_sub(r_clamped + 1)];
                    row_out[i] = val * iarr;
                }
            }
        });
}

#[inline(always)]
pub fn box_blur_1d_v(src: &[f32], out: &mut [f32], w: usize, h: usize, r: usize) {
    if h == 0 || w == 0 { return; }
    let r_clamped = r.min(h - 1);
    let iarr = 1.0 / ((r_clamped + r_clamped + 1) as f32);

    let num_threads = rayon::current_num_threads().max(1);
    let chunk_w = ((w + num_threads - 1) / num_threads).max(16);

    (0..w).into_par_iter().step_by(chunk_w).for_each(|start_x| {
        let end_x = (start_x + chunk_w).min(w);
        let out_ptr = out.as_ptr() as *mut f32;
        unsafe {
            for x in start_x..end_x {
                let fv = src[x];
                let lv = src[(h - 1) * w + x];
                let mut val = (r_clamped + 1) as f32 * fv;
                for j in 0..r_clamped {
                    val += src[j * w + x];
                }
                for j in 0..=r_clamped {
                    val += src[(j + r_clamped).min(h - 1) * w + x] - fv;
                    *out_ptr.add(j * w + x) = val * iarr;
                }
                for j in (r_clamped + 1)..(h.saturating_sub(r_clamped)) {
                    val += src[(j + r_clamped) * w + x] - src[(j - r_clamped - 1) * w + x];
                    *out_ptr.add(j * w + x) = val * iarr;
                }
                for j in (h.saturating_sub(r_clamped))..h {
                    val += lv - src[j.saturating_sub(r_clamped + 1) * w + x];
                    *out_ptr.add(j * w + x) = val * iarr;
                }
            }
        }
    });
}

#[inline(always)]
pub fn downscale_2x_f32(src: &[f32], w: usize, h: usize) -> (Vec<f32>, usize, usize) {
    let dw = (w + 1) / 2;
    let dh = (h + 1) / 2;
    let mut out = vec![0.0f32; dw * dh];

    out.par_chunks_mut(dw * 8)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let start_dy = chunk_idx * 8;
            let num_rows = chunk.len() / dw;
            for c_dy in 0..num_rows {
                let dy = start_dy + c_dy;
                let sy0 = (dy * 2).min(h - 1);
                let sy1 = (dy * 2 + 1).min(h - 1);
                let r0 = sy0 * w;
                let r1 = sy1 * w;
                let row_offset = c_dy * dw;
                for dx in 0..dw {
                    let sx0 = (dx * 2).min(w - 1);
                    let sx1 = (dx * 2 + 1).min(w - 1);
                    let avg = (src[r0 + sx0] + src[r0 + sx1] + src[r1 + sx0] + src[r1 + sx1]) * 0.25;
                    chunk[row_offset + dx] = avg;
                }
            }
        });

    (out, dw, dh)
}

#[inline(always)]
pub fn downscale_4x_f32(src: &[f32], w: usize, h: usize) -> (Vec<f32>, usize, usize) {
    let dw = (w + 3) / 4;
    let dh = (h + 3) / 4;
    let mut out = vec![0.0f32; dw * dh];

    out.par_chunks_mut(dw * 8)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let start_dy = chunk_idx * 8;
            let num_rows = chunk.len() / dw;
            for c_dy in 0..num_rows {
                let dy = start_dy + c_dy;
                let sy_base = dy * 4;
                let sy0 = sy_base.min(h - 1) * w;
                let sy1 = (sy_base + 1).min(h - 1) * w;
                let sy2 = (sy_base + 2).min(h - 1) * w;
                let sy3 = (sy_base + 3).min(h - 1) * w;
                let row_offset = c_dy * dw;

                for dx in 0..dw {
                    let sx_base = dx * 4;
                    let sx0 = sx_base.min(w - 1);
                    let sx1 = (sx_base + 1).min(w - 1);
                    let sx2 = (sx_base + 2).min(w - 1);
                    let sx3 = (sx_base + 3).min(w - 1);

                    let sum0 = src[sy0 + sx0] + src[sy0 + sx1] + src[sy0 + sx2] + src[sy0 + sx3];
                    let sum1 = src[sy1 + sx0] + src[sy1 + sx1] + src[sy1 + sx2] + src[sy1 + sx3];
                    let sum2 = src[sy2 + sx0] + src[sy2 + sx1] + src[sy2 + sx2] + src[sy2 + sx3];
                    let sum3 = src[sy3 + sx0] + src[sy3 + sx1] + src[sy3 + sx2] + src[sy3 + sx3];

                    chunk[row_offset + dx] = (sum0 + sum1 + sum2 + sum3) * 0.0625;
                }
            }
        });

    (out, dw, dh)
}

#[inline(always)]
pub fn upscale_bilinear_2x_f32(src: &[f32], dw: usize, dh: usize, w: usize, h: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; w * h];
    let x_ratio = if w > 1 { (dw - 1) as f32 / (w - 1) as f32 } else { 0.0 };
    let y_ratio = if h > 1 { (dh - 1) as f32 / (h - 1) as f32 } else { 0.0 };

    let chunk_rows = 16;
    out.par_chunks_mut(w * chunk_rows)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let start_y = chunk_idx * chunk_rows;
            let num_rows = chunk.len() / w;
            for cy in 0..num_rows {
                let y = start_y + cy;
                let sy = y as f32 * y_ratio;
                let y_low = sy.floor() as usize;
                let y_high = (y_low + 1).min(dh - 1);
                let wy = sy - y_low as f32;

                let r_low = y_low * dw;
                let r_high = y_high * dw;
                let row_offset = cy * w;

                for x in 0..w {
                    let sx = x as f32 * x_ratio;
                    let x_low = sx.floor() as usize;
                    let x_high = (x_low + 1).min(dw - 1);
                    let wx = sx - x_low as f32;

                    let top = src[r_low + x_low] * (1.0 - wx) + src[r_low + x_high] * wx;
                    let bot = src[r_high + x_low] * (1.0 - wx) + src[r_high + x_high] * wx;
                    chunk[row_offset + x] = top * (1.0 - wy) + bot * wy;
                }
            }
        });

    out
}

#[inline(always)]
pub fn upscale_bilinear_4x_f32(src: &[f32], dw: usize, dh: usize, w: usize, h: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; w * h];
    let x_ratio = if w > 1 { (dw - 1) as f32 / (w - 1) as f32 } else { 0.0 };
    let y_ratio = if h > 1 { (dh - 1) as f32 / (h - 1) as f32 } else { 0.0 };

    let chunk_rows = 16;
    out.par_chunks_mut(w * chunk_rows)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let start_y = chunk_idx * chunk_rows;
            let num_rows = chunk.len() / w;
            for cy in 0..num_rows {
                let y = start_y + cy;
                let sy = y as f32 * y_ratio;
                let y_low = sy.floor() as usize;
                let y_high = (y_low + 1).min(dh - 1);
                let wy = sy - y_low as f32;

                let r_low = y_low * dw;
                let r_high = y_high * dw;
                let row_offset = cy * w;

                for x in 0..w {
                    let sx = x as f32 * x_ratio;
                    let x_low = sx.floor() as usize;
                    let x_high = (x_low + 1).min(dw - 1);
                    let wx = sx - x_low as f32;

                    let top = src[r_low + x_low] * (1.0 - wx) + src[r_low + x_high] * wx;
                    let bot = src[r_high + x_low] * (1.0 - wx) + src[r_high + x_high] * wx;
                    chunk[row_offset + x] = top * (1.0 - wy) + bot * wy;
                }
            }
        });

    out
}

pub fn gaussian_blur_channel(src: &[f32], w: usize, h: usize, radius: f32) -> Vec<f32> {
    if radius <= 0.5 || w == 0 || h == 0 {
        return src.to_vec();
    }
    if radius >= 4.0 && w >= 1280 && h >= 720 {
        let (down_4x, dw, dh) = downscale_4x_f32(src, w, h);
        let r = (radius * 0.35).round().max(1.0) as usize;
        let mut buf_a = down_4x;
        let mut buf_b = vec![0.0f32; dw * dh];
        for _ in 0..2 {
            box_blur_1d_h(&buf_a, &mut buf_b, dw, dh, r);
            box_blur_1d_v(&buf_b, &mut buf_a, dw, dh, r);
        }
        upscale_bilinear_4x_f32(&buf_a, dw, dh, w, h)
    } else if radius >= 2.0 && w >= 640 && h >= 480 {
        let (down_src, dw, dh) = downscale_2x_f32(src, w, h);
        let r = (radius * 0.65).round().max(1.0) as usize;
        let mut buf_a = down_src;
        let mut buf_b = vec![0.0f32; dw * dh];
        for _ in 0..2 {
            box_blur_1d_h(&buf_a, &mut buf_b, dw, dh, r);
            box_blur_1d_v(&buf_b, &mut buf_a, dw, dh, r);
        }
        upscale_bilinear_2x_f32(&buf_a, dw, dh, w, h)
    } else {
        let r = (radius * 0.70).round().max(1.0) as usize;
        let mut buf_a = src.to_vec();
        let mut buf_b = vec![0.0f32; w * h];
        for _ in 0..2 {
            box_blur_1d_h(&buf_a, &mut buf_b, w, h, r);
            box_blur_1d_v(&buf_b, &mut buf_a, w, h, r);
        }
        buf_a
    }
}

pub fn apply_light_wrap_post_fx(
    composite_buf: &mut [u8],
    bg_buf: &[u8],
    char_alpha: &[f32],
    w: usize,
    h: usize,
    intensity: f32,
) {
    if intensity <= 1e-4 || w == 0 || h == 0 {
        return;
    }

    if w >= 640 && h >= 480 {
        let dw = (w + 3) / 4;
        let dh = (h + 3) / 4;
        let mut down_inv_alpha = vec![0.0f32; dw * dh];

        // 1. Direct 4x downscale of (1.0 - char_alpha)
        down_inv_alpha.par_chunks_mut(dw * 8)
            .enumerate()
            .for_each(|(chunk_idx, chunk)| {
                let start_dy = chunk_idx * 8;
                let num_rows = chunk.len() / dw;
                for c_dy in 0..num_rows {
                    let dy = start_dy + c_dy;
                    let sy_base = dy * 4;
                    let sy0 = sy_base.min(h - 1) * w;
                    let sy1 = (sy_base + 1).min(h - 1) * w;
                    let sy2 = (sy_base + 2).min(h - 1) * w;
                    let sy3 = (sy_base + 3).min(h - 1) * w;
                    let row_offset = c_dy * dw;

                    for dx in 0..dw {
                        let sx_base = dx * 4;
                        let sx0 = sx_base.min(w - 1);
                        let sx1 = (sx_base + 1).min(w - 1);
                        let sx2 = (sx_base + 2).min(w - 1);
                        let sx3 = (sx_base + 3).min(w - 1);

                        let a0 = char_alpha[sy0 + sx0] + char_alpha[sy0 + sx1] + char_alpha[sy0 + sx2] + char_alpha[sy0 + sx3];
                        let a1 = char_alpha[sy1 + sx0] + char_alpha[sy1 + sx1] + char_alpha[sy1 + sx2] + char_alpha[sy1 + sx3];
                        let a2 = char_alpha[sy2 + sx0] + char_alpha[sy2 + sx1] + char_alpha[sy2 + sx2] + char_alpha[sy2 + sx3];
                        let a3 = char_alpha[sy3 + sx0] + char_alpha[sy3 + sx1] + char_alpha[sy3 + sx2] + char_alpha[sy3 + sx3];

                        let avg_alpha = (a0 + a1 + a2 + a3) * 0.0625;
                        chunk[row_offset + dx] = (1.0 - avg_alpha).clamp(0.0, 1.0);
                    }
                }
            });

        // 2. Fast 2-pass blur on 480x270 buffer
        let r = (6.0f32 * 0.35f32).round().max(1.0) as usize;
        let mut temp = vec![0.0f32; dw * dh];
        for _ in 0..2 {
            box_blur_1d_h(&down_inv_alpha, &mut temp, dw, dh, r);
            box_blur_1d_v(&temp, &mut down_inv_alpha, dw, dh, r);
        }

        // 3. Bleed ambient background light onto character contour with bilinear sampling
        let x_ratio = if w > 1 { (dw - 1) as f32 / (w - 1) as f32 } else { 0.0 };
        let y_ratio = if h > 1 { (dh - 1) as f32 / (h - 1) as f32 } else { 0.0 };

        let chunk_rows = 16;
        composite_buf.par_chunks_mut(w * 4 * chunk_rows)
            .enumerate()
            .for_each(|(chunk_idx, chunk)| {
                let start_y = chunk_idx * chunk_rows;
                let num_rows = chunk.len() / (w * 4);
                for cy in 0..num_rows {
                    let y = start_y + cy;
                    let sy = y as f32 * y_ratio;
                    let y_low = sy.floor() as usize;
                    let y_high = (y_low + 1).min(dh - 1);
                    let wy = sy - y_low as f32;
                    let r_low = y_low * dw;
                    let r_high = y_high * dw;

                    let row_offset = y * w;
                    let chunk_row_offset = cy * w * 4;

                    for x in 0..w {
                        let idx = row_offset + x;
                        let ca = char_alpha[idx];
                        if ca > 0.001 {
                            let sx = x as f32 * x_ratio;
                            let x_low = sx.floor() as usize;
                            let x_high = (x_low + 1).min(dw - 1);
                            let wx = sx - x_low as f32;

                            let top = down_inv_alpha[r_low + x_low] * (1.0 - wx) + down_inv_alpha[r_low + x_high] * wx;
                            let bot = down_inv_alpha[r_high + x_low] * (1.0 - wx) + down_inv_alpha[r_high + x_high] * wx;
                            let bleed_mask = top * (1.0 - wy) + bot * wy;

                            let bleed = (ca * bleed_mask * intensity).clamp(0.0, 1.0);
                            if bleed > 0.001 {
                                let bg_r = bg_buf[idx * 4] as f32;
                                let bg_g = bg_buf[idx * 4 + 1] as f32;
                                let bg_b = bg_buf[idx * 4 + 2] as f32;

                                let px_idx = chunk_row_offset + x * 4;
                                let px_r = chunk[px_idx] as f32;
                                let px_g = chunk[px_idx + 1] as f32;
                                let px_b = chunk[px_idx + 2] as f32;

                                let out_r = (px_r + bg_r * bleed).min(255.0);
                                let out_g = (px_g + bg_g * bleed).min(255.0);
                                let out_b = (px_b + bg_b * bleed).min(255.0);

                                chunk[px_idx] = out_r.round() as u8;
                                chunk[px_idx + 1] = out_g.round() as u8;
                                chunk[px_idx + 2] = out_b.round() as u8;
                            }
                        }
                    }
                }
            });
    } else {
        let mut inv_alpha = vec![0.0f32; w * h];
        for i in 0..(w * h) {
            inv_alpha[i] = (1.0 - char_alpha[i]).clamp(0.0, 1.0);
        }
        let blurred_bg_bleed = gaussian_blur_channel(&inv_alpha, w, h, 6.0);
        for y in 0..h {
            let row_offset = y * w;
            for x in 0..w {
                let idx = row_offset + x;
                let ca = char_alpha[idx];
                if ca > 0.001 {
                    let bleed = (ca * blurred_bg_bleed[idx] * intensity).clamp(0.0, 1.0);
                    if bleed > 0.001 {
                        let bg_r = bg_buf[idx * 4] as f32;
                        let bg_g = bg_buf[idx * 4 + 1] as f32;
                        let bg_b = bg_buf[idx * 4 + 2] as f32;

                        let px_idx = idx * 4;
                        let px_r = composite_buf[px_idx] as f32;
                        let px_g = composite_buf[px_idx + 1] as f32;
                        let px_b = composite_buf[px_idx + 2] as f32;

                        let out_r = (px_r + bg_r * bleed).min(255.0);
                        let out_g = (px_g + bg_g * bleed).min(255.0);
                        let out_b = (px_b + bg_b * bleed).min(255.0);

                        composite_buf[px_idx] = out_r.round() as u8;
                        composite_buf[px_idx + 1] = out_g.round() as u8;
                        composite_buf[px_idx + 2] = out_b.round() as u8;
                    }
                }
            }
        }
    }
}

pub fn apply_chromatic_aberration_post_fx(
    buf: &mut [u8],
    w: usize,
    h: usize,
    intensity: f32,
) {
    if intensity <= 1e-4 || w == 0 || h == 0 {
        return;
    }

    let src = buf.to_vec();
    let cx = w as f32 * 0.5;
    let cy = h as f32 * 0.5;
    let max_r = (cx * cx + cy * cy).sqrt().max(1.0);
    let inv_max_r = (18.0 * intensity) / max_r;
    let max_x_idx = (w - 1) as f32;
    let max_y_idx = (h - 1) as f32;

    buf.par_chunks_mut(w * 4)
        .enumerate()
        .for_each(|(y, row_chunk)| {
            let yf = y as f32;
            let dy_norm = (yf - cy) * inv_max_r;

            for x in 0..w {
                let xf = x as f32;
                let dx_norm = (xf - cx) * inv_max_r;

                let rx = ((xf + dx_norm).max(0.0).min(max_x_idx)) as usize;
                let ry = ((yf + dy_norm).max(0.0).min(max_y_idx)) as usize;
                let r_val = src[(ry * w + rx) * 4];

                let g_val = src[(y * w + x) * 4 + 1];

                let bx = ((xf - dx_norm).max(0.0).min(max_x_idx)) as usize;
                let by = ((yf - dy_norm).max(0.0).min(max_y_idx)) as usize;
                let b_val = src[(by * w + bx) * 4 + 2];

                row_chunk[x * 4] = r_val;
                row_chunk[x * 4 + 1] = g_val;
                row_chunk[x * 4 + 2] = b_val;
            }
        });
}

pub fn apply_impact_motion_blur_post_fx(
    buf: &mut [u8],
    w: usize,
    h: usize,
    blur_amount: f32,
) {
    if blur_amount <= 1e-4 || w == 0 || h == 0 {
        return;
    }

    let src = buf.to_vec();
    let cx = w as f32 * 0.5;
    let cy = h as f32 * 0.5;

    // 5-tap radial smear from center
    const TAPS: [f32; 5] = [-0.020, -0.010, 0.0, 0.010, 0.020];

    buf.par_chunks_mut(w * 4)
        .enumerate()
        .for_each(|(y, row_chunk)| {
            let yf = y as f32;
            let dy = yf - cy;

            for x in 0..w {
                let xf = x as f32;
                let dx = xf - cx;

                let mut r_acc = 0.0f32;
                let mut g_acc = 0.0f32;
                let mut b_acc = 0.0f32;
                let mut a_acc = 0.0f32;

                for &t_scale in &TAPS {
                    let factor = 1.0 + t_scale * blur_amount;
                    let sx = (cx + dx * factor).round().clamp(0.0, (w - 1) as f32) as usize;
                    let sy = (cy + dy * factor).round().clamp(0.0, (h - 1) as f32) as usize;
                    let idx = (sy * w + sx) * 4;

                    r_acc += src[idx] as f32;
                    g_acc += src[idx + 1] as f32;
                    b_acc += src[idx + 2] as f32;
                    a_acc += src[idx + 3] as f32;
                }

                row_chunk[x * 4] = (r_acc * 0.20).round() as u8;
                row_chunk[x * 4 + 1] = (g_acc * 0.20).round() as u8;
                row_chunk[x * 4 + 2] = (b_acc * 0.20).round() as u8;
                row_chunk[x * 4 + 3] = (a_acc * 0.20).round() as u8;
            }
        });
}

pub fn extract_inner_edge_mask(alpha: &[f32], w: usize, h: usize, edge_radius: f32) -> Vec<f32> {
    let blurred = gaussian_blur_channel(alpha, w, h, edge_radius.max(2.0));
    let mut edge = vec![0.0f32; w * h];
    edge.par_chunks_mut(w)
        .enumerate()
        .for_each(|(y, row_chunk)| {
            let offset = y * w;
            for x in 0..w {
                let a = alpha[offset + x];
                if a > 0.05 {
                    let diff = (a - blurred[offset + x]).max(0.0);
                    row_chunk[x] = (diff * 2.8 * a).clamp(0.0, 1.0);
                }
            }
        });
    edge
}

pub fn extract_contour_rim_mask(alpha: &[f32], w: usize, h: usize) -> Vec<f32> {
    let mut rim = vec![0.0f32; w * h];
    rim.par_chunks_mut(w)
        .enumerate()
        .for_each(|(y, row_chunk)| {
            let y_prev = y.saturating_sub(1);
            let y_next = (y + 1).min(h - 1);
            for x in 0..w {
                let x_prev = x.saturating_sub(1);
                let x_next = (x + 1).min(w - 1);

                let a = alpha[y * w + x];
                if a > 0.05 {
                    let dx = (alpha[y * w + x_next] - alpha[y * w + x_prev]) * 0.5;
                    let dy = (alpha[y_next * w + x] - alpha[y_prev * w + x]) * 0.5;
                    let mag = (dx * dx + dy * dy).sqrt();
                    row_chunk[x] = (mag * 3.5 * a).clamp(0.0, 1.0);
                }
            }
        });
    rim
}

#[derive(Debug, Clone, PartialEq)]
pub struct RawImage {
    pub width: usize,
    pub height: usize,
    pub data: Vec<u8>, // RGBA (width * height * 4)
}

pub fn resize_bilinear_rgba(src: &RawImage, target_w: usize, target_h: usize) -> RawImage {
    if src.width == target_w && src.height == target_h {
        return src.clone();
    }
    let mut out_data = vec![0u8; target_w * target_h * 4];
    let x_ratio = if target_w > 1 { (src.width - 1) as f32 / (target_w - 1) as f32 } else { 0.0 };
    let y_ratio = if target_h > 1 { (src.height - 1) as f32 / (target_h - 1) as f32 } else { 0.0 };

    for y in 0..target_h {
        let sy = y as f32 * y_ratio;
        let y_low = sy.floor() as usize;
        let y_high = (y_low + 1).min(src.height - 1);
        let y_weight = sy - y_low as f32;

        let row_out = y * target_w * 4;
        let row_low = y_low * src.width * 4;
        let row_high = y_high * src.width * 4;

        for x in 0..target_w {
            let sx = x as f32 * x_ratio;
            let x_low = sx.floor() as usize;
            let x_high = (x_low + 1).min(src.width - 1);
            let x_weight = sx - x_low as f32;

            let idx_out = row_out + x * 4;
            let idx_00 = row_low + x_low * 4;
            let idx_10 = row_low + x_high * 4;
            let idx_01 = row_high + x_low * 4;
            let idx_11 = row_high + x_high * 4;

            for c in 0..4 {
                let top = src.data[idx_00 + c] as f32 * (1.0 - x_weight) + src.data[idx_10 + c] as f32 * x_weight;
                let bottom = src.data[idx_01 + c] as f32 * (1.0 - x_weight) + src.data[idx_11 + c] as f32 * x_weight;
                let val = top * (1.0 - y_weight) + bottom * y_weight;
                out_data[idx_out + c] = val.round().clamp(0.0, 255.0) as u8;
            }
        }
    }

    RawImage {
        width: target_w,
        height: target_h,
        data: out_data,
    }
}

pub fn probe_image_dimensions(path: &Path, ffmpeg_bin: Option<&Path>) -> Result<(usize, usize), String> {
    // 1. Try ffprobe
    let ffprobe_bin = if let Some(bin) = ffmpeg_bin {
        if let Some(parent) = bin.parent() {
            let direct = parent.join("ffprobe.exe");
            if direct.exists() {
                direct
            } else {
                PathBuf::from("ffprobe")
            }
        } else {
            PathBuf::from("ffprobe")
        }
    } else {
        PathBuf::from("ffprobe")
    };

    let mut probe_cmd = std::process::Command::new(&ffprobe_bin);
    probe_cmd.args([
        "-v", "error",
        "-select_streams", "v:0",
        "-show_entries", "stream=width,height",
        "-of", "csv=p=0:s=x",
        path.to_str().unwrap(),
    ]);
    #[cfg(target_os = "windows")]
    probe_cmd.creation_flags(CREATE_NO_WINDOW);

    if let Ok(output) = probe_cmd.output() {
        if output.status.success() {
            let out_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let parts: Vec<&str> = out_str.split('x').collect();
            if parts.len() == 2 {
                if let (Ok(w), Ok(h)) = (parts[0].trim().parse::<usize>(), parts[1].trim().parse::<usize>()) {
                    if w > 0 && h > 0 {
                        return Ok((w, h));
                    }
                }
            }
        }
    }

    // 2. Fallback: Parse ffmpeg -i stderr
    let bin = ffmpeg_bin.map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("ffmpeg"));
    let mut cmd = std::process::Command::new(&bin);
    cmd.args(["-i", path.to_str().unwrap()]);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = cmd.output().map_err(|e| format!("Failed to run ffmpeg: {e}"))?;
    let stderr_str = String::from_utf8_lossy(&output.stderr);
    for line in stderr_str.lines() {
        if line.contains("Video:") {
            for token in line.split(',') {
                let tok = token.trim();
                let parts: Vec<&str> = tok.split('x').collect();
                if parts.len() == 2 {
                    let w_str = parts[0].trim().chars().take_while(|c| c.is_ascii_digit()).collect::<String>();
                    let h_str = parts[1].trim().chars().take_while(|c| c.is_ascii_digit()).collect::<String>();
                    if let (Ok(w), Ok(h)) = (w_str.parse::<usize>(), h_str.parse::<usize>()) {
                        if w > 0 && h > 0 {
                            return Ok((w, h));
                        }
                    }
                }
            }
        }
    }

    Err(format!("Could not determine dimensions of image {}", path.display()))
}

pub fn load_image_rgba(path: &Path, ffmpeg_bin: Option<&Path>) -> Result<RawImage, String> {
    if !path.exists() {
        return Err(format!("File not found: {}", path.display()));
    }

    let (w, h) = probe_image_dimensions(path, ffmpeg_bin)?;

    let bin = ffmpeg_bin.map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("ffmpeg"));
    let mut cmd = std::process::Command::new(&bin);
    cmd.args([
        "-i", path.to_str().unwrap(),
        "-f", "rawvideo",
        "-pix_fmt", "rgba",
        "-",
    ]);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::null());
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = cmd.output().map_err(|e| format!("Failed to decode image via ffmpeg: {e}"))?;
    if !output.status.success() || output.stdout.len() < w * h * 4 {
        return Err(format!("Failed to read raw RGBA data from {}", path.display()));
    }

    Ok(RawImage {
        width: w,
        height: h,
        data: output.stdout[..w * h * 4].to_vec(),
    })
}

pub fn save_image_rgba(img: &RawImage, path: &Path, ffmpeg_bin: Option<&Path>) -> Result<(), String> {
    let bin = ffmpeg_bin.map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("ffmpeg"));
    let mut cmd = std::process::Command::new(&bin);
    cmd.args([
        "-y",
        "-f", "rawvideo",
        "-pix_fmt", "rgba",
        "-s", &format!("{}x{}", img.width, img.height),
        "-i", "-",
        "-frames:v", "1",
        path.to_str().unwrap(),
    ]);
    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn ffmpeg for image save: {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        let _ = stdin.write_all(&img.data);
    }
    let status = child.wait().map_err(|e| format!("Failed to wait for ffmpeg save: {e}"))?;
    if !status.success() {
        return Err("Failed to save image via ffmpeg".to_string());
    }
    Ok(())
}

pub fn validate_and_load_character_png(path: &Path, ffmpeg_bin: Option<&Path>) -> Result<RawImage, String> {
    let raw = load_image_rgba(path, ffmpeg_bin)?;

    // Check if there is actual alpha transparency
    let mut has_transparency = false;
    for i in 0..(raw.width * raw.height) {
        if raw.data[i * 4 + 3] < 250 {
            has_transparency = true;
            break;
        }
    }

    if !has_transparency {
        return Err("PNG sans canal alpha — détourage requis".to_string());
    }

    Ok(raw)
}

pub fn check_nvidia_gpu_internal() -> Result<String, String> {
    let mut cmd = std::process::Command::new("nvidia-smi");
    cmd.args(["--query-gpu=name,driver_version,memory.total", "--format=csv,noheader"]);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    match cmd.output() {
        Ok(output) => {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !stdout.is_empty() {
                    Ok(stdout)
                } else {
                    Ok("NVIDIA GPU Detected".to_string())
                }
            } else {
                Err("NVIDIA GPU not detected. See-through layer decomposition requires an NVIDIA GPU with CUDA.".to_string())
            }
        }
        Err(e) => {
            Err(format!("NVIDIA GPU check failed: {e}. See-through requires an NVIDIA GPU with CUDA."))
        }
    }
}

fn find_see_through_cli_script() -> Option<PathBuf> {
    let direct = PathBuf::from("vendor").join("see_through").join("see_through_cli.py");
    if direct.exists() {
        return Some(direct);
    }
    let cwd = std::env::current_dir().unwrap_or_default();
    let in_cwd = cwd.join("vendor").join("see_through").join("see_through_cli.py");
    if in_cwd.exists() {
        return Some(in_cwd);
    }
    if let Some(parent) = cwd.parent() {
        let in_parent = parent.join("vendor").join("see_through").join("see_through_cli.py");
        if in_parent.exists() {
            return Some(in_parent);
        }
    }
    if let Ok(exe_p) = std::env::current_exe() {
        let mut cur = exe_p.parent();
        while let Some(p) = cur {
            let in_p = p.join("vendor").join("see_through").join("see_through_cli.py");
            if in_p.exists() {
                return Some(in_p);
            }
            cur = p.parent();
        }
    }
    None
}

pub fn resolve_see_through_python(app: Option<&tauri::AppHandle>) -> Result<(PathBuf, PathBuf), String> {
    let cli_script = find_see_through_cli_script().ok_or_else(|| {
        "vendor/see_through/see_through_cli.py not found in project or vendor directory.".to_string()
    })?;

    // 1. Check venv in %LOCALAPPDATA%/cia_app/sidecars/see_through/venv
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let venv_py = PathBuf::from(&local_app_data)
            .join("cia_app")
            .join("sidecars")
            .join("see_through")
            .join("venv")
            .join("Scripts")
            .join("python.exe");

        if venv_py.exists() {
            return Ok((venv_py, cli_script));
        }
    }

    // 2. Check app data dir if tauri app is available
    if let Some(app_handle) = app {
        if let Ok(app_dir) = app_handle.path().app_data_dir() {
            let venv_py = app_dir
                .join("sidecars")
                .join("see_through")
                .join("venv")
                .join("Scripts")
                .join("python.exe");

            if venv_py.exists() {
                return Ok((venv_py, cli_script));
            }
        }
    }

    // 3. Fallback: check system python
    let candidates = ["py", "python", "python3"];
    for cand in candidates {
        let mut check_cmd = std::process::Command::new(cand);
        if cand == "py" {
            check_cmd.arg("-3.11");
        }
        check_cmd.arg("--version");
        #[cfg(target_os = "windows")]
        check_cmd.creation_flags(CREATE_NO_WINDOW);

        if let Ok(out) = check_cmd.output() {
            if out.status.success() {
                return Ok((PathBuf::from(cand), cli_script));
            }
        }
    }

    Err(
        "See-through sidecar is not installed or configured.\n\
        To install, run 'python vendor/see_through/bootstrap_see_through.py' from PowerShell.".to_string()
    )
}

pub fn segment_character_internal(
    app: Option<&tauri::AppHandle>,
    character_path: &str,
    output_dir_opt: Option<&str>,
) -> Result<CompositionResult, String> {
    let char_p = Path::new(character_path);
    if !char_p.exists() {
        return Err(format!("Character image not found at: {character_path}"));
    }

    // GPU Check
    check_nvidia_gpu_internal()?;

    // Resolve sidecar runner
    let (py_exe, cli_script) = resolve_see_through_python(app)?;

    let out_dir = match output_dir_opt {
        Some(d) => PathBuf::from(d),
        None => {
            let mut base = std::env::temp_dir().join("cia_composition");
            if let Some(app_handle) = app {
                if let Ok(app_dir) = app_handle.path().app_data_dir() {
                    base = app_dir.join("composition");
                }
            }
            std::fs::create_dir_all(&base).map_err(|e| format!("Failed to create composition dir: {e}"))?;
            let id = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(12345);
            base.join(format!("comp_{id}"))
        }
    };

    std::fs::create_dir_all(&out_dir).map_err(|e| format!("Failed to create output dir: {e}"))?;

    let mut cmd = std::process::Command::new(&py_exe);
    if py_exe.to_string_lossy() == "py" {
        cmd.arg("-3.11");
    }
    cmd.arg(&cli_script);
    cmd.args([
        "--input",
        character_path,
        "--output-dir",
        out_dir.to_str().unwrap(),
    ]);

    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = cmd.output().map_err(|e| format!("Failed to execute See-through CLI: {e}"))?;

    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        return Err(format!("See-through layer decomposition failed: {err_msg}"));
    }

    let _stdout_str = String::from_utf8_lossy(&output.stdout);
    let layers_json_path = out_dir.join("layers.json");
    if !layers_json_path.exists() {
        return Err(format!("Expected layers.json not found in output directory: {}", out_dir.display()));
    }

    let layers_content = std::fs::read_to_string(&layers_json_path)
        .map_err(|e| format!("Failed to read layers.json: {e}"))?;

    let mut raw_layers: Vec<LayerItem> = serde_json::from_str(&layers_content)
        .map_err(|e| format!("Failed to parse layers.json: {e}"))?;

    // Populate full paths and base64 thumbnails
    for layer in &mut raw_layers {
        let layer_path = out_dir.join(&layer.file);
        if layer_path.exists() {
            layer.full_path = Some(layer_path.to_string_lossy().to_string());
            if let Ok(bytes) = std::fs::read(&layer_path) {
                let b64 = encode_base64(&bytes);
                layer.thumbnail_base64 = Some(format!("data:image/png;base64,{b64}"));
            }
        }
    }

    Ok(CompositionResult {
        status: "success".to_string(),
        character_path: character_path.to_string(),
        background_path: None,
        output_dir: out_dir.to_string_lossy().to_string(),
        layers_count: raw_layers.len(),
        layers_json_path: layers_json_path.to_string_lossy().to_string(),
        layers: raw_layers,
    })
}

// --- Tauri Commands ---

pub fn gaussian_blur_channel_downscaled(src: &[f32], w: usize, h: usize, radius: f32) -> Vec<f32> {
    if radius <= 1.0 || w < 8 || h < 8 {
        return gaussian_blur_channel(src, w, h, radius);
    }
    // Downscale by factor of 4
    let dw = (w / 4).max(2);
    let dh = (h / 4).max(2);
    let mut down = vec![0.0f32; dw * dh];
    let x_scale = w as f32 / dw as f32;
    let y_scale = h as f32 / dh as f32;

    for dy in 0..dh {
        let sy = ((dy as f32 + 0.5) * y_scale) as usize;
        let sy_c = sy.min(h - 1);
        for dx in 0..dw {
            let sx = ((dx as f32 + 0.5) * x_scale) as usize;
            let sx_c = sx.min(w - 1);
            down[dy * dw + dx] = src[sy_c * w + sx_c];
        }
    }

    let down_radius = (radius / 4.0).max(1.5);
    let blurred_down = gaussian_blur_channel(&down, dw, dh, down_radius);

    // Upscale bilinearly back to w x h
    let mut out = vec![0.0f32; w * h];
    for y in 0..h {
        let ty = (y as f32 / y_scale).min((dh - 1) as f32);
        let y0 = ty.floor() as usize;
        let y1 = (y0 + 1).min(dh - 1);
        let wy = ty - y0 as f32;

        let r_out = y * w;
        let r0 = y0 * dw;
        let r1 = y1 * dw;

        for x in 0..w {
            let tx = (x as f32 / x_scale).min((dw - 1) as f32);
            let x0 = tx.floor() as usize;
            let x1 = (x0 + 1).min(dw - 1);
            let wx = tx - x0 as f32;

            let top = blurred_down[r0 + x0] * (1.0 - wx) + blurred_down[r0 + x1] * wx;
            let bot = blurred_down[r1 + x0] * (1.0 - wx) + blurred_down[r1 + x1] * wx;
            out[r_out + x] = top * (1.0 - wy) + bot * wy;
        }
    }
    out
}

#[derive(Debug, Clone)]
pub struct PrecomputedCompMasks {
    pub shadow_mask: Option<Vec<f32>>,
    pub edge_mask: Option<Vec<f32>>,
    pub rim_mask: Option<Vec<f32>>,
    pub alpha_channel: Vec<f32>,
}

pub fn precompute_composition_masks(
    char_raw: &RawImage,
    ops: &[CompositionOp],
    w: usize,
    h: usize,
) -> PrecomputedCompMasks {
    let mut alpha_channel = vec![0.0f32; w * h];
    alpha_channel.par_chunks_mut(w)
        .enumerate()
        .for_each(|(y, row_chunk)| {
            let row_offset = y * w;
            for x in 0..w {
                row_chunk[x] = char_raw.data[(row_offset + x) * 4 + 3] as f32 / 255.0;
            }
        });

    let mut shadow_mask = None;
    let mut edge_mask = None;
    let mut rim_mask = None;

    for op in ops {
        if !op.enabled {
            continue;
        }
        match op.op_type.as_str() {
            "drop_shadow" => {
                let offset_x = op.params.get("offsetX").and_then(|v| v.as_f64()).unwrap_or(12.0) as f32;
                let offset_y = op.params.get("offsetY").and_then(|v| v.as_f64()).unwrap_or(16.0) as f32;
                let blur_radius = op.params.get("blurRadius").and_then(|v| v.as_f64()).unwrap_or(14.0) as f32;

                let mut shifted_alpha = vec![0.0f32; w * h];
                let dx_i = offset_x.round() as i32;
                let dy_i = offset_y.round() as i32;
                shifted_alpha.par_chunks_mut(w)
                    .enumerate()
                    .for_each(|(y, row_chunk)| {
                        let sy = y as i32 - dy_i;
                        if sy >= 0 && sy < h as i32 {
                            let sy_u = sy as usize;
                            for x in 0..w {
                                let sx = x as i32 - dx_i;
                                if sx >= 0 && sx < w as i32 {
                                    row_chunk[x] = alpha_channel[sy_u * w + sx as usize];
                                }
                            }
                        }
                    });
                shadow_mask = Some(gaussian_blur_channel(&shifted_alpha, w, h, blur_radius));
            }
            "light_wrap" => {
                let edge_width = op.params.get("edgeWidth").and_then(|v| v.as_f64()).unwrap_or(10.0) as f32;
                edge_mask = Some(extract_inner_edge_mask(&alpha_channel, w, h, edge_width));
            }
            "rim_light" => {
                rim_mask = Some(extract_contour_rim_mask(&alpha_channel, w, h));
            }
            _ => {}
        }
    }

    PrecomputedCompMasks {
        shadow_mask,
        edge_mask,
        rim_mask,
        alpha_channel,
    }
}

pub fn composite_frame_fast(
    bg_rgba: &mut [u8], // W x H x 4
    char_raw: &RawImage, // W x H x 4
    ops: &[CompositionOp],
    masks: &PrecomputedCompMasks,
    w: usize,
    h: usize,
) {
    use rayon::prelude::*;
    let has_shadow = ops.iter().any(|op| op.enabled && op.op_type == "drop_shadow");
    let shadow_opacity = ops.iter().find(|op| op.enabled && op.op_type == "drop_shadow").map(|op| op.opacity as f32).unwrap_or(0.0);

    let has_tint = ops.iter().any(|op| op.enabled && op.op_type == "tint");
    let tint_opacity = ops.iter().find(|op| op.enabled && op.op_type == "tint").map(|op| op.opacity as f32).unwrap_or(0.0);

    let has_edge = ops.iter().any(|op| op.enabled && op.op_type == "light_wrap");
    let edge_opacity = ops.iter().find(|op| op.enabled && op.op_type == "light_wrap").map(|op| op.opacity as f32).unwrap_or(0.0);

    let has_rim = ops.iter().any(|op| op.enabled && op.op_type == "rim_light");
    let rim_opacity = ops.iter().find(|op| op.enabled && op.op_type == "rim_light").map(|op| op.opacity as f32).unwrap_or(0.0);

    // Compute background ambient color for tint in sub-sampled pass
    let mut bg_mean = [128.0f32, 128.0f32, 128.0f32];
    if has_tint {
        let (mut sum_r, mut sum_g, mut sum_b, mut count) = (0.0f64, 0.0f64, 0.0f64, 0usize);
        for i in (0..(w * h)).step_by(16) {
            if masks.alpha_channel[i] < 0.2 {
                let idx = i * 4;
                sum_r += bg_rgba[idx] as f64;
                sum_g += bg_rgba[idx + 1] as f64;
                sum_b += bg_rgba[idx + 2] as f64;
                count += 1;
            }
        }
        if count > 0 {
            bg_mean = [
                (sum_r / count as f64) as f32,
                (sum_g / count as f64) as f32,
                (sum_b / count as f64) as f32,
            ];
        }
    }

    let shadow_ref = masks.shadow_mask.as_deref();
    let edge_ref = masks.edge_mask.as_deref();
    let rim_ref = masks.rim_mask.as_deref();
    let char_data = &char_raw.data;
    let alpha_chan = &masks.alpha_channel;

    // Unified single-pass Rayon scanline parallel compositor
    bg_rgba.par_chunks_mut(w * 4)
        .enumerate()
        .for_each(|(y, row_chunk)| {
            let row_offset = y * w;
            for x in 0..w {
                let idx = row_offset + x;
                let px_idx = x * 4;

                let mut bg_r = row_chunk[px_idx] as f32;
                let mut bg_g = row_chunk[px_idx + 1] as f32;
                let mut bg_b = row_chunk[px_idx + 2] as f32;
                let bg_a = row_chunk[px_idx + 3] as f32 / 255.0;

                // 1. Drop shadow on background
                if has_shadow {
                    if let Some(shadow) = shadow_ref {
                        let s_val = shadow[idx] * shadow_opacity;
                        if s_val > 0.001 {
                            let factor = 1.0 - s_val;
                            bg_r *= factor;
                            bg_g *= factor;
                            bg_b *= factor;
                        }
                    }
                }

                // 2. Alpha Over character
                let char_idx = idx * 4;
                let fg_a = char_data[char_idx + 3] as f32 / 255.0;
                let (mut out_r, mut out_g, mut out_b, out_a) = if fg_a <= 0.001 {
                    (bg_r, bg_g, bg_b, bg_a)
                } else if fg_a >= 0.999 {
                    (
                        char_data[char_idx] as f32,
                        char_data[char_idx + 1] as f32,
                        char_data[char_idx + 2] as f32,
                        1.0,
                    )
                } else {
                    let fg_r = char_data[char_idx] as f32;
                    let fg_g = char_data[char_idx + 1] as f32;
                    let fg_b = char_data[char_idx + 2] as f32;
                    let a_out = fg_a + bg_a * (1.0 - fg_a);
                    if a_out > 0.001 {
                        let r = (fg_r * fg_a + bg_r * bg_a * (1.0 - fg_a)) / a_out;
                        let g = (fg_g * fg_a + bg_g * bg_a * (1.0 - fg_a)) / a_out;
                        let b = (fg_b * fg_a + bg_b * bg_a * (1.0 - fg_a)) / a_out;
                        (r, g, b, a_out)
                    } else {
                        (0.0, 0.0, 0.0, 0.0)
                    }
                };

                let char_a = alpha_chan[idx];

                // 3. Tint de raccord
                if has_tint && char_a > 0.01 {
                    let k = tint_opacity * char_a;
                    let tr = (out_r * (bg_mean[0] / 128.0)).min(255.0);
                    let tg = (out_g * (bg_mean[1] / 128.0)).min(255.0);
                    let tb = (out_b * (bg_mean[2] / 128.0)).min(255.0);
                    out_r = out_r * (1.0 - k) + tr * k;
                    out_g = out_g * (1.0 - k) + tg * k;
                    out_b = out_b * (1.0 - k) + tb * k;
                }

                // 4. Layer op light wrap (screen blend on inner edge)
                if has_edge && char_a > 0.01 {
                    if let Some(edge) = edge_ref {
                        let e_val = edge[idx] * edge_opacity;
                        if e_val > 0.001 {
                            let cb_r = out_r / 255.0;
                            let cb_g = out_g / 255.0;
                            let cb_b = out_b / 255.0;
                            let bg_norm_r = bg_r / 255.0;
                            let bg_norm_g = bg_g / 255.0;
                            let bg_norm_b = bg_b / 255.0;

                            let cs_r = 1.0 - (1.0 - cb_r) * (1.0 - bg_norm_r);
                            let cs_g = 1.0 - (1.0 - cb_g) * (1.0 - bg_norm_g);
                            let cs_b = 1.0 - (1.0 - cb_b) * (1.0 - bg_norm_b);

                            out_r = ((cb_r * (1.0 - e_val) + cs_r * e_val) * 255.0).min(255.0);
                            out_g = ((cb_g * (1.0 - e_val) + cs_g * e_val) * 255.0).min(255.0);
                            out_b = ((cb_b * (1.0 - e_val) + cs_b * e_val) * 255.0).min(255.0);
                        }
                    }
                }

                // 5. Rim light op (additive)
                if has_rim && char_a > 0.01 {
                    if let Some(rim) = rim_ref {
                        let r_val = rim[idx] * rim_opacity;
                        if r_val > 0.001 {
                            out_r = (out_r + 240.0 * r_val).min(255.0);
                            out_g = (out_g + 245.0 * r_val).min(255.0);
                            out_b = (out_b + 255.0 * r_val).min(255.0);
                        }
                    }
                }

                row_chunk[px_idx] = out_r.round().clamp(0.0, 255.0) as u8;
                row_chunk[px_idx + 1] = out_g.round().clamp(0.0, 255.0) as u8;
                row_chunk[px_idx + 2] = out_b.round().clamp(0.0, 255.0) as u8;
                row_chunk[px_idx + 3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
            }
        });
}

pub fn composite_frame_with_ops(
    bg_rgba: &mut [u8], // W x H x 4
    char_raw: &RawImage, // W x H x 4
    ops: &[CompositionOp],
    w: usize,
    h: usize,
) {
    let masks = precompute_composition_masks(char_raw, ops, w, h);
    composite_frame_fast(bg_rgba, char_raw, ops, &masks, w, h);
}

pub fn render_composition_internal(
    app: Option<&tauri::AppHandle>,
    character_path: &str,
    background_path: &str,
    ops_opt: Option<Vec<CompositionOp>>,
    output_dir_opt: Option<&str>,
) -> Result<String, String> {
    use std::io::{Read, Write};
    use tauri::Emitter;

    let char_p = Path::new(character_path);
    let bg_p = Path::new(background_path);

    if !char_p.exists() {
        return Err(format!("Character image not found at: {character_path}"));
    }
    if !bg_p.exists() {
        return Err(format!("Background file not found at: {background_path}"));
    }

    let ffmpeg_bin_opt = app.and_then(|a| crate::probe::get_ffmpeg_binary(a).ok());

    // 1. Validate Character PNG (strictly checks alpha channel)
    let char_raw = validate_and_load_character_png(char_p, ffmpeg_bin_opt.as_deref())?;

    let ops = ops_opt.unwrap_or_else(get_default_composition_ops);

    let base_out = match output_dir_opt {
        Some(d) => PathBuf::from(d),
        None => {
            let mut base = std::env::temp_dir().join("cia_composition");
            if let Some(app_handle) = app {
                if let Ok(app_dir) = app_handle.path().app_data_dir() {
                    base = app_dir.join("composition_renders");
                }
            }
            std::fs::create_dir_all(&base).map_err(|e| format!("Failed to create render dir: {e}"))?;
            base
        }
    };
    std::fs::create_dir_all(&base_out).map_err(|e| format!("Failed to create output dir: {e}"))?;

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(123456);

    let bg_ext = bg_p.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let is_video = ["mp4", "mkv", "webm", "mov", "avi"].contains(&bg_ext.as_str());

    if !is_video {
        // STATIC IMAGE COMPOSITION
        let bg_raw = load_image_rgba(bg_p, ffmpeg_bin_opt.as_deref())?;
        let (w, h) = (bg_raw.width, bg_raw.height);

        // Scale character to fit canvas
        let char_scaled = resize_bilinear_rgba(&char_raw, w, h);
        let mut canvas_data = bg_raw.data.clone();

        composite_frame_with_ops(&mut canvas_data, &char_scaled, &ops, w, h);

        let out_path = base_out.join(format!("composition_{timestamp}.png"));
        let out_img = RawImage {
            width: w,
            height: h,
            data: canvas_data,
        };

        save_image_rgba(&out_img, &out_path, ffmpeg_bin_opt.as_deref())?;

        if let Some(app_handle) = app {
            let _ = app_handle.emit("comp-progress", CompositionProgress {
                phase: "DONE".to_string(),
                percent: 100,
                current_frame: 1,
                total_frames: 1,
                message: "Image composition complete".to_string(),
            });
        }

        Ok(out_path.to_string_lossy().to_string())
    } else {
        // VIDEO BACKGROUND COMPOSITION
        let ffmpeg_bin = ffmpeg_bin_opt.unwrap_or_else(|| PathBuf::from("ffmpeg"));

        let probe = crate::probe::probe_media_internal(background_path, None)
            .map_err(|e| format!("Background video could not be read: {e}"))?;

        let fps = if probe.fps > 0.0 { probe.fps } else { 30.0 };
        let total_frames = ((probe.duration * fps).ceil() as u32).max(1);
        let (w, h) = (probe.width.max(2) as usize, probe.height.max(2) as usize);

        // Scale character to match video canvas
        let char_scaled = resize_bilinear_rgba(&char_raw, w, h);
        let precomputed_masks = precompute_composition_masks(&char_scaled, &ops, w, h);

        let out_path = base_out.join(format!("composition_{timestamp}.mp4"));
        let cache_file = base_out.join(format!("comp_raw_{timestamp}.raw"));

        // 1. Decode background frames to raw cache file
        let mut decode_cmd = std::process::Command::new(&ffmpeg_bin);
        decode_cmd.args([
            "-y",
            "-i", background_path,
            "-f", "rawvideo",
            "-pix_fmt", "rgba",
            "-an",
            cache_file.to_str().unwrap(),
        ]);
        decode_cmd.stdout(std::process::Stdio::null());
        decode_cmd.stderr(std::process::Stdio::null());
        #[cfg(target_os = "windows")]
        decode_cmd.creation_flags(CREATE_NO_WINDOW);

        let mut decode_child = decode_cmd.spawn()
            .map_err(|e| format!("Failed to spawn ffmpeg decoder: {e}"))?;

        let decode_status = decode_child.wait()
            .map_err(|e| format!("Decoder wait failed: {e}"))?;

        if !decode_status.success() {
            let _ = std::fs::remove_file(&cache_file);
            return Err("FFmpeg video decoding failed".to_string());
        }

        // 2. Launch ffmpeg encoder process
        let mut encode_cmd = std::process::Command::new(&ffmpeg_bin);
        encode_cmd.args([
            "-y",
            "-f", "rawvideo",
            "-pix_fmt", "rgba",
            "-s", &format!("{w}x{h}"),
            "-r", &format!("{fps}"),
            "-i", "-",
            "-i", background_path,
            "-map", "0:v:0",
            "-map", "1:a:0?",
            "-c:v", "libx264",
            "-preset", "veryfast",
            "-pix_fmt", "yuv420p",
            "-c:a", "aac",
            "-shortest",
            out_path.to_str().unwrap(),
        ]);
        encode_cmd.stdin(std::process::Stdio::piped());
        encode_cmd.stdout(std::process::Stdio::null());
        encode_cmd.stderr(std::process::Stdio::null());
        #[cfg(target_os = "windows")]
        encode_cmd.creation_flags(CREATE_NO_WINDOW);

        let mut encode_child = encode_cmd.spawn()
            .map_err(|e| format!("Failed to spawn ffmpeg encoder: {e}"))?;

        let mut encode_stdin = encode_child.stdin.take()
            .ok_or_else(|| "Failed to open ffmpeg encode stdin".to_string())?;

        let frame_bytes = w * h * 4;
        let mut frame_buf = vec![0u8; frame_bytes];
        let mut raw_file = std::io::BufReader::new(
            std::fs::File::open(&cache_file).map_err(|e| format!("Failed to open cache file: {e}"))?
        );
        let mut current_frame = 0u32;

        while raw_file.read_exact(&mut frame_buf).is_ok() {
            current_frame += 1;

            composite_frame_fast(&mut frame_buf, &char_scaled, &ops, &precomputed_masks, w, h);

            if encode_stdin.write_all(&frame_buf).is_err() {
                break;
            }

            if current_frame % 10 == 0 || current_frame == total_frames {
                let pct = ((current_frame as f64 / total_frames as f64) * 100.0).clamp(0.0, 100.0) as u32;
                if let Some(app_handle) = app {
                    let _ = app_handle.emit("comp-progress", CompositionProgress {
                        phase: "COMPOSITING".to_string(),
                        percent: pct,
                        current_frame,
                        total_frames,
                        message: format!("Compositing frame {current_frame}/{total_frames}"),
                    });
                }
            }
        }

        drop(encode_stdin);
        let encode_status = encode_child.wait()
            .map_err(|e| format!("Encoder wait failed: {e}"))?;

        let _ = std::fs::remove_file(&cache_file);

        if !encode_status.success() {
            return Err("FFmpeg video encoding failed".to_string());
        }

        if let Some(app_handle) = app {
            let _ = app_handle.emit("comp-progress", CompositionProgress {
                phase: "DONE".to_string(),
                percent: 100,
                current_frame: total_frames,
                total_frames,
                message: "Video composition complete".to_string(),
            });
        }

        Ok(out_path.to_string_lossy().to_string())
    }
}

// ─── T28 Mesh Deformation & Procedural Animation ─────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct MeshVertex {
    pub orig_x: f32,
    pub orig_y: f32,
    pub u: f32, // [0, 1] relative to layer bbox
    pub v: f32, // [0, 1] relative to layer bbox (v=0 at top root, v=1 at bottom tips)
    pub root_weight: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MeshTriangle {
    pub v_indices: [usize; 3],
}

#[derive(Debug, Clone, PartialEq)]
pub struct LayerMesh {
    pub layer_name: String,
    pub grid_w: usize,
    pub grid_h: usize,
    pub bbox: [f32; 4], // [min_x, min_y, max_x, max_y]
    pub vertices: Vec<MeshVertex>,
    pub triangles: Vec<MeshTriangle>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnimationConfig {
    pub entrance_enabled: bool,
    pub entrance_downbeat: f64,
    pub breathing_amplitude: f32,
    pub hair_sway_amplitude: f32,
    pub blink_interval_sec: f32,
    pub pump_decay_rate: f32,
    pub parallax_strength: f32,       // 0.0 to 1.0, default 0.5
    pub beat_punch_intensity: f32,    // 0.0 to 1.0, default 0.6
    pub light_wrap_intensity: f32,    // 0.0 to 1.0, default 0.5
    pub chromatic_aberration: f32,    // 0.0 to 1.0, default 0.3
    pub impact_blur_strength: f32,    // 0.0 to 1.0, default 0.5
}

impl Default for AnimationConfig {
    fn default() -> Self {
        Self {
            entrance_enabled: true,
            entrance_downbeat: 0.0,
            breathing_amplitude: 0.005,
            hair_sway_amplitude: 1.0,
            blink_interval_sec: 3.2,
            pump_decay_rate: 6.0,
            parallax_strength: 0.5,
            beat_punch_intensity: 0.6,
            light_wrap_intensity: 0.5,
            chromatic_aberration: 0.3,
            impact_blur_strength: 0.5,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraState {
    pub pan_x: f32,
    pub pan_y: f32,
    pub zoom: f32,
    pub roll: f32,
}

impl Default for CameraState {
    fn default() -> Self {
        Self {
            pan_x: 0.0,
            pan_y: 0.0,
            zoom: 1.0,
            roll: 0.0,
        }
    }
}

pub fn compute_camera_state(
    t: f64,
    beats: &[f64],
    downbeats: &[f64],
    config: &AnimationConfig,
) -> CameraState {
    // 1. Procedural Ambient Drift (sinusoidal, max amplitude 1% of screen on 4 seconds)
    let drift_phase = (2.0 * std::f64::consts::PI * t / 4.0) as f32;
    let mut pan_x = 0.01 * drift_phase.sin();
    let mut pan_y = 0.006 * (drift_phase + 0.5 * std::f32::consts::PI).cos();
    let mut zoom = 1.0f32;
    let roll = 0.003 * (drift_phase * 0.5).sin();

    // 2. Downbeat Beat Punch & Decay (returns to ±0.001 of 1.0 within 200 ms)
    for &db in downbeats {
        if t >= db {
            let dt = (t - db) as f32;
            if dt < 0.25 {
                let punch = config.beat_punch_intensity * 0.05 * (-35.0 * dt).exp();
                zoom += punch;
            }
        }
    }

    // 3. Beat Shake Micro-offset (seeded by beat number, ±2px normalized over 100ms)
    for (beat_idx, &b) in beats.iter().enumerate() {
        if t >= b {
            let dt = (t - b) as f32;
            if dt < 0.15 {
                let seed = (beat_idx as u32).wrapping_mul(2654435761);
                let angle = (seed % 360) as f32 * std::f32::consts::PI / 180.0;
                let shake_decay = (-35.0 * dt).exp();
                let amp = (2.0 / 1080.0) * shake_decay;
                pan_x += amp * angle.cos();
                pan_y += amp * angle.sin();
            }
        }
    }

    CameraState {
        pan_x,
        pan_y,
        zoom,
        roll,
    }
}

pub fn compute_layer_bbox(img: &RawImage) -> [f32; 4] {
    let mut min_x = img.width as f32;
    let mut max_x = 0.0f32;
    let mut min_y = img.height as f32;
    let mut max_y = 0.0f32;
    let mut has_pixel = false;

    for y in 0..img.height {
        for x in 0..img.width {
            let a = img.data[(y * img.width + x) * 4 + 3];
            if a > 10 {
                has_pixel = true;
                let xf = x as f32;
                let yf = y as f32;
                if xf < min_x { min_x = xf; }
                if xf > max_x { max_x = xf; }
                if yf < min_y { min_y = yf; }
                if yf > max_y { max_y = yf; }
            }
        }
    }

    if !has_pixel {
        [0.0, 0.0, img.width as f32, img.height as f32]
    } else {
        [
            min_x.max(0.0),
            min_y.max(0.0),
            (max_x + 1.0).min(img.width as f32),
            (max_y + 1.0).min(img.height as f32),
        ]
    }
}

pub fn build_layer_mesh(layer_name: &str, img: &RawImage) -> LayerMesh {
    let is_hair = layer_name.contains("hair");
    let (grid_w, grid_h) = if is_hair {
        (12, 14) // 12x14 for hair
    } else {
        (10, 10) // 10x10 default
    };

    let bbox = compute_layer_bbox(img);
    let min_x = bbox[0];
    let min_y = bbox[1];
    let max_x = bbox[2];
    let max_y = bbox[3];

    let bw = (max_x - min_x).max(1.0);
    let bh = (max_y - min_y).max(1.0);

    let mut vertices = Vec::with_capacity(grid_w * grid_h);

    for j in 0..grid_h {
        let v = j as f32 / (grid_h - 1) as f32;
        let y = min_y + v * bh;
        
        let root_weight = if is_hair {
            // Root at top (v=0) has weight 0.0, tip (v=1) has weight 1.0
            v.powf(1.2)
        } else {
            1.0
        };

        for i in 0..grid_w {
            let u = i as f32 / (grid_w - 1) as f32;
            let x = min_x + u * bw;

            vertices.push(MeshVertex {
                orig_x: x,
                orig_y: y,
                u,
                v,
                root_weight,
            });
        }
    }

    let mut triangles = Vec::with_capacity((grid_w - 1) * (grid_h - 1) * 2);
    for j in 0..(grid_h - 1) {
        for i in 0..(grid_w - 1) {
            let v00 = j * grid_w + i;
            let v10 = j * grid_w + (i + 1);
            let v01 = (j + 1) * grid_w + i;
            let v11 = (j + 1) * grid_w + (i + 1);

            triangles.push(MeshTriangle {
                v_indices: [v00, v10, v01],
            });
            triangles.push(MeshTriangle {
                v_indices: [v10, v11, v01],
            });
        }
    }

    LayerMesh {
        layer_name: layer_name.to_string(),
        grid_w,
        grid_h,
        bbox,
        vertices,
        triangles,
    }
}

pub fn compute_deformed_vertices(
    mesh: &LayerMesh,
    t: f64,
    _frame_idx: u32,
    fps: f64,
    beats: &[f64],
    downbeats: &[f64],
    config: &AnimationConfig,
    layer_z: usize,
    layer_z_depth: f32,
    camera: &CameraState,
    canvas_w: usize,
    canvas_h: usize,
) -> Vec<(f32, f32)> {
    let is_hair = mesh.layer_name.contains("hair");
    let is_eyes = mesh.layer_name.contains("eye");
    let is_body = mesh.layer_name.contains("body") || mesh.layer_name.contains("clothes") || mesh.layer_name.contains("skin");
    let is_acc = mesh.layer_name.contains("acc");
    let is_face = mesh.layer_name.contains("face") || mesh.layer_name.contains("mouth");

    let bbox = mesh.bbox;
    let cx = (bbox[0] + bbox[2]) * 0.5;
    let cy = (bbox[1] + bbox[3]) * 0.5;
    let by_bottom = bbox[3];

    // 1. Entrance Stagger & Easing Bounce
    let stagger_sec = (layer_z as f64) * (2.5 / fps);
    let entrance_time = t - config.entrance_downbeat - stagger_sec;
    let entrance_bounce = if config.entrance_enabled && entrance_time < 0.6 {
        if entrance_time < 0.0 {
            0.0f32
        } else {
            let tau = (entrance_time / 0.6) as f32;
            (1.0 - (-5.0 * tau).exp() * (2.5 * std::f32::consts::PI * tau).cos() * 0.35).clamp(0.0, 1.05)
        }
    } else {
        1.0f32
    };

    // 2. Downbeat Pump Decay
    let mut pump_scale = 0.0f32;
    for &db in downbeats {
        if t >= db {
            let dt = (t - db) as f32;
            if dt < 0.6 {
                pump_scale += 0.02 * (-config.pump_decay_rate * dt).exp();
            }
        }
    }

    // 3. Beat Accents for Sway
    let mut beat_accent = 0.0f32;
    for &b in beats {
        if t >= b {
            let dt = (t - b) as f32;
            if dt < 0.35 {
                beat_accent += 4.0 * (8.0 * std::f32::consts::PI * dt).sin() * (-8.0 * dt).exp();
            }
        }
    }

    // 4. Eyes Blink Controller (scaleY -> 0 over 2-4 frames every 2.5-4s)
    let blink_scale_y = if is_eyes {
        let period = config.blink_interval_sec.max(1.0);
        let cycle_t = (t as f32) % period;
        let blink_duration = 3.0 / fps as f32; // 3 frames
        if cycle_t < blink_duration {
            let progress = cycle_t / blink_duration;
            // Cosine dip: 1.0 -> 0.0 -> 1.0
            (1.0 - (std::f32::consts::PI * progress).sin()).clamp(0.0, 1.0)
        } else {
            1.0f32
        }
    } else {
        1.0f32
    };

    // 2.5D Orthographic Parallax Math & Perspective Scale Factor
    let z_centered = layer_z_depth - 0.5; // [-0.5, 0.5]
    let parallax_strength = config.parallax_strength;

    // offset_x = (layer.z - 0.5) * camera.pan_x * parallax_strength * viewport_width
    // offset_y = (layer.z - 0.5) * camera.pan_y * parallax_strength * viewport_height
    let offset_x = z_centered * camera.pan_x * parallax_strength * (canvas_w as f32);
    let offset_y = z_centered * camera.pan_y * parallax_strength * (canvas_h as f32);

    // layer_scale_factor = 1.0 + ((layer.z - 0.5) * camera.zoom_effect)
    let zoom_effect = (camera.zoom - 1.0) * parallax_strength;
    let layer_scale_factor = 1.0 + z_centered * zoom_effect;
    let total_zoom = camera.zoom * layer_scale_factor;

    let cam_cx = canvas_w as f32 * 0.5;
    let cam_cy = canvas_h as f32 * 0.5;

    let cos_r = camera.roll.cos();
    let sin_r = camera.roll.sin();

    mesh.vertices.iter().map(|v| {
        let mut x = v.orig_x;
        let mut y = v.orig_y;

        if is_hair {
            // Sway = sum of 2 sines + phase lag proportional to distance from root
            let phase_lag = 1.6 * v.v;
            let tf = t as f32;
            let s1 = 14.0 * (2.2 * std::f32::consts::PI * tf - phase_lag + 0.4).sin();
            let s2 = 7.0 * (4.4 * std::f32::consts::PI * tf - 1.5 * phase_lag + 1.1).sin();
            let idle = 0.8 * (1.0 * std::f32::consts::PI * tf).sin();
            let sway = (s1 + s2 + beat_accent) * config.hair_sway_amplitude;

            let dx = v.root_weight * sway + idle;
            let dy = v.root_weight * 1.5 * (3.0 * std::f32::consts::PI * tf + 0.5 * phase_lag).sin();

            x += dx;
            y += dy;
        } else if is_eyes {
            // Blink around center of eyes bbox
            y = cy + (y - cy) * blink_scale_y;
            // Slight idle micro-motion
            y += 0.5 * (1.2 * std::f32::consts::PI * t as f32).sin();
        } else if is_body {
            // Breathing scale + downbeat pump relative to bottom anchor
            let breath = 1.0 + config.breathing_amplitude * (1.2 * std::f32::consts::PI * t as f32).sin();
            let total_scale = breath + pump_scale;

            x = cx + (x - cx) * total_scale;
            y = by_bottom + (y - by_bottom) * total_scale;
        } else if is_acc {
            // Micro-rotation around bbox center
            let theta = 0.035 * (1.8 * std::f32::consts::PI * t as f32 + 0.8).sin();
            let rx = x - cx;
            let ry = y - cy;
            x = cx + rx * theta.cos() - ry * theta.sin();
            y = cy + rx * theta.sin() + ry * theta.cos();
        } else if is_face {
            // Gentle breath follow
            y += 1.0 * (1.2 * std::f32::consts::PI * t as f32).sin();
        }

        // Apply entrance bounce
        if config.entrance_enabled {
            x = cx + (x - cx) * entrance_bounce;
            y = cy + (y - cy) * entrance_bounce;
        }

        // Fused Camera Transformation & 2.5D Parallax in a single pass (Rayon-accelerated)
        let rx = x - cam_cx;
        let ry = y - cam_cy;

        let final_x = cam_cx + (rx * cos_r - ry * sin_r) * total_zoom + offset_x;
        let final_y = cam_cy + (rx * sin_r + ry * cos_r) * total_zoom + offset_y;

        (final_x, final_y)
    }).collect()
}

#[inline(always)]
pub fn sample_bilinear_pixel(img: &RawImage, sx: f32, sy: f32) -> [u8; 4] {
    let w = img.width;
    let h = img.height;

    if sx < 0.0 || sx >= (w - 1) as f32 || sy < 0.0 || sy >= (h - 1) as f32 {
        let ix = (sx.round() as usize).clamp(0, w.saturating_sub(1));
        let iy = (sy.round() as usize).clamp(0, h.saturating_sub(1));
        let idx = (iy * w + ix) * 4;
        return [img.data[idx], img.data[idx + 1], img.data[idx + 2], img.data[idx + 3]];
    }

    let x0 = sx.floor() as usize;
    let x1 = (x0 + 1).min(w - 1);
    let y0 = sy.floor() as usize;
    let y1 = (y0 + 1).min(h - 1);

    let fx = sx - x0 as f32;
    let fy = sy - y0 as f32;

    let idx00 = (y0 * w + x0) * 4;
    let idx10 = (y0 * w + x1) * 4;
    let idx01 = (y1 * w + x0) * 4;
    let idx11 = (y1 * w + x1) * 4;

    let mut out = [0u8; 4];
    for c in 0..4 {
        let p00 = img.data[idx00 + c] as f32;
        let p10 = img.data[idx10 + c] as f32;
        let p01 = img.data[idx01 + c] as f32;
        let p11 = img.data[idx11 + c] as f32;

        let top = p00 * (1.0 - fx) + p10 * fx;
        let bot = p01 * (1.0 - fx) + p11 * fx;
        let val = top * (1.0 - fy) + bot * fy;

        out[c] = val.round().clamp(0.0, 255.0) as u8;
    }
    out
}

pub fn composite_deformed_mesh_direct(
    target_canvas: &mut [u8],
    canvas_w: usize,
    canvas_h: usize,
    src_layer: &RawImage,
    mesh: &LayerMesh,
    deformed_verts: &[(f32, f32)],
) {
    let w = canvas_w;
    let h = canvas_h;
    let src_w = src_layer.width;
    let src_h = src_layer.height;

    struct TriData {
        v0_dst: (f32, f32),
        v1_dst: (f32, f32),
        v2_dst: (f32, f32),
        v0_src: (f32, f32),
        v1_src: (f32, f32),
        v2_src: (f32, f32),
        min_y: usize,
        max_y: usize,
        min_x: usize,
        max_x: usize,
        inv_det: f32,
    }

    let mut tri_list = Vec::with_capacity(mesh.triangles.len());

    for tri in &mesh.triangles {
        let i0 = tri.v_indices[0];
        let i1 = tri.v_indices[1];
        let i2 = tri.v_indices[2];

        let p0 = deformed_verts[i0];
        let p1 = deformed_verts[i1];
        let p2 = deformed_verts[i2];

        let det = (p1.0 - p0.0) * (p2.1 - p0.1) - (p2.0 - p0.0) * (p1.1 - p0.1);
        if det.abs() < 1e-6 {
            continue;
        }

        let s0 = (mesh.vertices[i0].orig_x, mesh.vertices[i0].orig_y);
        let s1 = (mesh.vertices[i1].orig_x, mesh.vertices[i1].orig_y);
        let s2 = (mesh.vertices[i2].orig_x, mesh.vertices[i2].orig_y);

        let src_min_x = (s0.0.min(s1.0).min(s2.0).floor() as i32).clamp(0, src_w as i32 - 1) as usize;
        let src_max_x = (s0.0.max(s1.0).max(s2.0).ceil() as i32).clamp(0, src_w as i32 - 1) as usize;
        let src_min_y = (s0.1.min(s1.1).min(s2.1).floor() as i32).clamp(0, src_h as i32 - 1) as usize;
        let src_max_y = (s0.1.max(s1.1).max(s2.1).ceil() as i32).clamp(0, src_h as i32 - 1) as usize;

        let mut has_opaque = false;
        'check_opaque: for sy in src_min_y..=src_max_y {
            let row = sy * src_w * 4;
            for sx in src_min_x..=src_max_x {
                if src_layer.data[row + sx * 4 + 3] > 0 {
                    has_opaque = true;
                    break 'check_opaque;
                }
            }
        }

        if !has_opaque {
            continue;
        }

        let min_x = (p0.0.min(p1.0).min(p2.0).floor() as i32).clamp(0, w as i32 - 1) as usize;
        let max_x = (p0.0.max(p1.0).max(p2.0).ceil() as i32).clamp(0, w as i32 - 1) as usize;
        let min_y = (p0.1.min(p1.1).min(p2.1).floor() as i32).clamp(0, h as i32 - 1) as usize;
        let max_y = (p0.1.max(p1.1).max(p2.1).ceil() as i32).clamp(0, h as i32 - 1) as usize;

        tri_list.push(TriData {
            v0_dst: p0,
            v1_dst: p1,
            v2_dst: p2,
            v0_src: s0,
            v1_src: s1,
            v2_src: s2,
            min_y,
            max_y,
            min_x,
            max_x,
            inv_det: 1.0 / det,
        });
    }

    let band_height = 32;

    target_canvas.par_chunks_mut(band_height * w * 4)
        .enumerate()
        .for_each(|(band_idx, band_chunk)| {
            let y_start = band_idx * band_height;
            let y_end = (y_start + band_height).min(h);

            for tri in &tri_list {
                if tri.max_y < y_start || tri.min_y >= y_end {
                    continue;
                }

                let sub_min_y = tri.min_y.max(y_start);
                let sub_max_y = tri.max_y.min(y_end.saturating_sub(1));

                let p0 = tri.v0_dst;
                let p1 = tri.v1_dst;
                let p2 = tri.v2_dst;
                let s0 = tri.v0_src;
                let s1 = tri.v1_src;
                let s2 = tri.v2_src;
                let inv_det = tri.inv_det;

                for py in sub_min_y..=sub_max_y {
                    let local_y = py - y_start;
                    let row_offset = local_y * w * 4;
                    let pyf = py as f32;

                    for px in tri.min_x..=tri.max_x {
                        let pxf = px as f32;

                        let w1 = ((pxf - p0.0) * (p2.1 - p0.1) - (p2.0 - p0.0) * (pyf - p0.1)) * inv_det;
                        let w2 = ((p1.0 - p0.0) * (pyf - p0.1) - (pxf - p0.0) * (p1.1 - p0.1)) * inv_det;
                        let w0 = 1.0 - w1 - w2;

                        if w0 >= -1e-4 && w1 >= -1e-4 && w2 >= -1e-4 {
                            let norm = w0 + w1 + w2;
                            let nw0 = w0 / norm;
                            let nw1 = w1 / norm;
                            let nw2 = w2 / norm;

                            let sx = nw0 * s0.0 + nw1 * s1.0 + nw2 * s2.0;
                            let sy = nw0 * s0.1 + nw1 * s1.1 + nw2 * s2.1;

                            let pixel = sample_bilinear_pixel(src_layer, sx, sy);
                            if pixel[3] > 0 {
                                let idx = row_offset + px * 4;
                                let existing_a = band_chunk[idx + 3];
                                if existing_a == 0 {
                                    band_chunk[idx] = pixel[0];
                                    band_chunk[idx + 1] = pixel[1];
                                    band_chunk[idx + 2] = pixel[2];
                                    band_chunk[idx + 3] = pixel[3];
                                } else {
                                    let fg_a = pixel[3] as f32 / 255.0;
                                    let bg_a = existing_a as f32 / 255.0;
                                    let out_a = fg_a + bg_a * (1.0 - fg_a);
                                    if out_a > 0.0 {
                                        band_chunk[idx] = ((pixel[0] as f32 * fg_a + band_chunk[idx] as f32 * bg_a * (1.0 - fg_a)) / out_a).round() as u8;
                                        band_chunk[idx + 1] = ((pixel[1] as f32 * fg_a + band_chunk[idx + 1] as f32 * bg_a * (1.0 - fg_a)) / out_a).round() as u8;
                                        band_chunk[idx + 2] = ((pixel[2] as f32 * fg_a + band_chunk[idx + 2] as f32 * bg_a * (1.0 - fg_a)) / out_a).round() as u8;
                                        band_chunk[idx + 3] = (out_a * 255.0).round() as u8;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });
}

pub fn render_deformed_mesh(
    src_layer: &RawImage,
    mesh: &LayerMesh,
    deformed_verts: &[(f32, f32)],
) -> RawImage {
    let w = src_layer.width;
    let h = src_layer.height;
    let mut out_data = vec![0u8; w * h * 4];

    composite_deformed_mesh_direct(
        &mut out_data,
        w,
        h,
        src_layer,
        mesh,
        deformed_verts,
    );

    RawImage {
        width: w,
        height: h,
        data: out_data,
    }
}

pub fn render_animated_character_frame(
    layers: &[(String, RawImage, LayerMesh, usize, f32)],
    t: f64,
    frame_idx: u32,
    fps: f64,
    beats: &[f64],
    downbeats: &[f64],
    config: &AnimationConfig,
    camera: &CameraState,
    canvas_w: usize,
    canvas_h: usize,
) -> RawImage {
    let mut composite_data = vec![0u8; canvas_w * canvas_h * 4];

    // Layers are ordered by z_order ascending
    for (_layer_name, raw_img, mesh, z_idx, z_depth) in layers {
        let deformed_verts = compute_deformed_vertices(
            mesh,
            t,
            frame_idx,
            fps,
            beats,
            downbeats,
            config,
            *z_idx,
            *z_depth,
            camera,
            canvas_w,
            canvas_h,
        );

        composite_deformed_mesh_direct(
            &mut composite_data,
            canvas_w,
            canvas_h,
            raw_img,
            mesh,
            &deformed_verts,
        );
    }

    RawImage {
        width: canvas_w,
        height: canvas_h,
        data: composite_data,
    }
}

pub fn render_mesh_preview_internal(
    app: Option<&tauri::AppHandle>,
    character_path: &str,
    background_path: Option<&str>,
    audio_path: Option<&str>,
    ops: Option<Vec<CompositionOp>>,
    parallax_strength: Option<f32>,
    beat_punch_intensity: Option<f32>,
    light_wrap_intensity: Option<f32>,
    chromatic_aberration: Option<f32>,
    impact_blur_strength: Option<f32>,
    duration_sec: Option<f64>,
    ffmpeg_bin_opt: Option<&Path>,
) -> Result<String, String> {
    let char_path = Path::new(character_path);
    if !char_path.exists() {
        return Err(format!("Character image not found at: {character_path}"));
    }

    let ffmpeg_bin = ffmpeg_bin_opt.map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("ffmpeg"));

    // 1. Ensure semantic layer segmentation is available
    let seg_res = segment_character_internal(app, character_path, None)
        .map_err(|e| format!("See-through layer segmentation unavailable. Please run bootstrap_see_through.py to initialize sidecar environment: {e}"))?;

    let output_dir = Path::new(&seg_res.output_dir);
    let mut loaded_layers = Vec::new();

    // Sort layers by z_order ascending
    let mut sorted_layers = seg_res.layers.clone();
    sorted_layers.sort_by_key(|l| l.z_order);

    let total_layers = sorted_layers.len().max(1);
    for (z_idx, layer) in sorted_layers.iter().enumerate() {
        let layer_file = output_dir.join(&layer.file);
        if layer_file.exists() && layer.has_content != Some(false) {
            if let Ok(img) = load_image_rgba(&layer_file, Some(&ffmpeg_bin)) {
                let mesh = build_layer_mesh(&layer.name, &img);
                let z_depth = layer.z_depth.unwrap_or_else(|| {
                    if total_layers > 1 {
                        z_idx as f32 / (total_layers - 1) as f32
                    } else {
                        0.5
                    }
                });
                loaded_layers.push((layer.name.clone(), img, mesh, z_idx, z_depth));
            }
        }
    }

    if loaded_layers.is_empty() {
        // Fallback to full character image as single body layer
        let full_img = load_image_rgba(char_path, Some(&ffmpeg_bin))?;
        let mesh = build_layer_mesh("body", &full_img);
        loaded_layers.push(("body".to_string(), full_img, mesh, 0, 0.5));
    }

    let (w, h) = (loaded_layers[0].1.width, loaded_layers[0].1.height);

    // 2. Audio beat detection
    let mut beats = Vec::new();
    let mut downbeats = Vec::new();

    if let Some(audio_file) = audio_path {
        if Path::new(audio_file).exists() {
            if let Ok(beat_res) = crate::beat::detect_beats_internal(app, audio_file) {
                beats = beat_res.beats;
                downbeats = beat_res.downbeats;
            }
        }
    }

    let mut anim_config = AnimationConfig::default();
    if let Some(ps) = parallax_strength {
        anim_config.parallax_strength = ps.clamp(0.0, 1.0);
    }
    if let Some(bpi) = beat_punch_intensity {
        anim_config.beat_punch_intensity = bpi.clamp(0.0, 1.0);
    }
    if let Some(lwi) = light_wrap_intensity {
        anim_config.light_wrap_intensity = lwi.clamp(0.0, 1.0);
    }
    if let Some(ca) = chromatic_aberration {
        anim_config.chromatic_aberration = ca.clamp(0.0, 1.0);
    }
    if let Some(ibs) = impact_blur_strength {
        anim_config.impact_blur_strength = ibs.clamp(0.0, 1.0);
    }

    let fps = 30.0f64;
    let duration = duration_sec.unwrap_or(3.0);
    let total_frames = ((duration * fps).ceil() as u32).max(1);

    let active_ops = ops.unwrap_or_else(get_default_composition_ops);

    let base_out = app
        .and_then(|a| a.path().app_data_dir().ok())
        .unwrap_or_else(|| std::env::temp_dir().join("cia_composition"));
    std::fs::create_dir_all(&base_out).map_err(|e| format!("Failed to create output dir: {e}"))?;

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(12345);

    let out_mp4 = base_out.join(format!("preview_mesh_{timestamp}.mp4"));

    // 3. Launch FFmpeg encoder
    let mut encode_cmd = std::process::Command::new(&ffmpeg_bin);
    let mut args = vec![
        "-y".to_string(),
        "-f".to_string(), "rawvideo".to_string(),
        "-pix_fmt".to_string(), "rgba".to_string(),
        "-s".to_string(), format!("{w}x{h}"),
        "-r".to_string(), format!("{fps}"),
        "-i".to_string(), "-".to_string(),
    ];

    if let Some(aud) = audio_path {
        if Path::new(aud).exists() {
            args.extend(["-ss".to_string(), "0".to_string(), "-t".to_string(), format!("{duration}"), "-i".to_string(), aud.to_string(), "-c:a".to_string(), "aac".to_string()]);
        }
    }

    args.extend([
        "-c:v".to_string(), "libx264".to_string(),
        "-preset".to_string(), "veryfast".to_string(),
        "-pix_fmt".to_string(), "yuv420p".to_string(),
        "-shortest".to_string(),
        out_mp4.to_string_lossy().to_string(),
    ]);

    encode_cmd.args(&args);
    encode_cmd.stdin(std::process::Stdio::piped());
    encode_cmd.stdout(std::process::Stdio::null());
    encode_cmd.stderr(std::process::Stdio::null());
    #[cfg(target_os = "windows")]
    encode_cmd.creation_flags(CREATE_NO_WINDOW);

    let mut encode_child = encode_cmd.spawn()
        .map_err(|e| format!("Failed to spawn ffmpeg encoder for mesh preview: {e}"))?;

    let mut encode_stdin = encode_child.stdin.take()
        .ok_or_else(|| "Failed to open ffmpeg encode stdin".to_string())?;

    // Precompute solid background or load background image
    let bg_frame = if let Some(bg_p) = background_path {
        if Path::new(bg_p).exists() {
            load_image_rgba(Path::new(bg_p), Some(&ffmpeg_bin))
                .map(|img| resize_bilinear_rgba(&img, w, h))
                .unwrap_or_else(|_| RawImage { width: w, height: h, data: vec![20u8; w * h * 4] })
        } else {
            // Dark solid background
            let mut bg_data = vec![0u8; w * h * 4];
            for i in 0..(w * h) {
                bg_data[i * 4] = 18;
                bg_data[i * 4 + 1] = 18;
                bg_data[i * 4 + 2] = 22;
                bg_data[i * 4 + 3] = 255;
            }
            RawImage { width: w, height: h, data: bg_data }
        }
    } else {
        let mut bg_data = vec![0u8; w * h * 4];
        for i in 0..(w * h) {
            bg_data[i * 4] = 18;
            bg_data[i * 4 + 1] = 18;
            bg_data[i * 4 + 2] = 22;
            bg_data[i * 4 + 3] = 255;
        }
        RawImage { width: w, height: h, data: bg_data }
    };

    use std::io::Write;

    for frame_idx in 0..total_frames {
        let t = (frame_idx as f64) / fps;
        let camera = compute_camera_state(t, &beats, &downbeats, &anim_config);

        let char_frame = render_animated_character_frame(
            &loaded_layers,
            t,
            frame_idx,
            fps,
            &beats,
            &downbeats,
            &anim_config,
            &camera,
            w,
            h,
        );

        let precomputed_masks = precompute_composition_masks(&char_frame, &active_ops, w, h);
        let mut frame_buf = bg_frame.data.clone();

        composite_frame_fast(&mut frame_buf, &char_frame, &active_ops, &precomputed_masks, w, h);

        // --- POST-FX STACK ---
        // 1. Light Wrap
        if anim_config.light_wrap_intensity > 0.001 {
            apply_light_wrap_post_fx(
                &mut frame_buf,
                &bg_frame.data,
                &precomputed_masks.alpha_channel,
                w,
                h,
                anim_config.light_wrap_intensity,
            );
        }

        // 2. Impact Downbeat Pulse for Chromatic & Motion Blur
        let mut chromatic_spike = 0.0f32;
        let mut impact_blur_spike = 0.0f32;
        for &db in &downbeats {
            if t >= db {
                let dt = (t - db) as f32;
                if dt < 0.10 { // 2 to 3 frames max (<0.10s)
                    let decay = (-35.0 * dt).exp();
                    chromatic_spike += 0.35 * decay;
                    impact_blur_spike += decay;
                }
            }
        }

        // 3. Impact Motion Blur
        let total_blur = anim_config.impact_blur_strength * impact_blur_spike;
        if total_blur > 0.001 {
            apply_impact_motion_blur_post_fx(&mut frame_buf, w, h, total_blur);
        }

        // 4. Chromatic Aberration
        let total_chroma = (anim_config.chromatic_aberration + chromatic_spike).clamp(0.0, 1.0);
        if total_chroma > 0.001 {
            apply_chromatic_aberration_post_fx(&mut frame_buf, w, h, total_chroma);
        }

        if encode_stdin.write_all(&frame_buf).is_err() {
            break;
        }

        if frame_idx % 5 == 0 || frame_idx == total_frames - 1 {
            let pct = (((frame_idx + 1) as f64 / total_frames as f64) * 100.0).clamp(0.0, 100.0) as u32;
            if let Some(app_handle) = app {
                let _ = app_handle.emit("comp-progress", CompositionProgress {
                    phase: "MESH_ANIM".to_string(),
                    percent: pct,
                    current_frame: frame_idx + 1,
                    total_frames,
                    message: format!("Deforming & compositing frame {}/{}", frame_idx + 1, total_frames),
                });
            }
        }
    }

    drop(encode_stdin);
    let status = encode_child.wait()
        .map_err(|e| format!("FFmpeg encoder failed: {e}"))?;

    if !status.success() {
        return Err("FFmpeg preview encoding failed".to_string());
    }

    if let Some(app_handle) = app {
        let _ = app_handle.emit("comp-progress", CompositionProgress {
            phase: "DONE".to_string(),
            percent: 100,
            current_frame: total_frames,
            total_frames,
            message: "Mesh animation preview complete".to_string(),
        });
    }

    Ok(out_mp4.to_string_lossy().to_string())
}

// --- Tauri Commands ---

#[tauri::command]
pub fn check_gpu_status() -> Result<String, String> {
    check_nvidia_gpu_internal()
}

#[tauri::command]
pub fn segment_character(
    app: tauri::AppHandle,
    character_path: String,
) -> Result<CompositionResult, String> {
    segment_character_internal(Some(&app), &character_path, None)
}

#[tauri::command]
pub fn save_composition_project(
    project: CompProject,
    target_path: Option<String>,
) -> Result<String, String> {
    let path_str = match target_path {
        Some(p) => p,
        None => {
            let base = std::env::temp_dir().join("cia_composition");
            std::fs::create_dir_all(&base).map_err(|e| format!("Failed to create dir: {e}"))?;
            let id = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(12345);
            base.join(format!("comp_project_{id}.json")).to_string_lossy().to_string()
        }
    };

    let json_content = serde_json::to_string_pretty(&project)
        .map_err(|e| format!("Failed to serialize comp project: {e}"))?;

    std::fs::write(&path_str, json_content)
        .map_err(|e| format!("Failed to write comp project to {path_str}: {e}"))?;

    Ok(path_str)
}

#[tauri::command]
pub fn render_composition(
    app: tauri::AppHandle,
    character_path: String,
    background_path: String,
    ops: Option<Vec<CompositionOp>>,
) -> Result<String, String> {
    render_composition_internal(Some(&app), &character_path, &background_path, ops, None)
}

#[tauri::command]
pub fn render_mesh_preview(
    app: tauri::AppHandle,
    character_path: String,
    background_path: Option<String>,
    audio_path: Option<String>,
    ops: Option<Vec<CompositionOp>>,
    parallax_strength: Option<f32>,
    beat_punch_intensity: Option<f32>,
    light_wrap_intensity: Option<f32>,
    chromatic_aberration: Option<f32>,
    impact_blur_strength: Option<f32>,
) -> Result<String, String> {
    render_mesh_preview_internal(
        Some(&app),
        &character_path,
        background_path.as_deref(),
        audio_path.as_deref(),
        ops,
        parallax_strength,
        beat_punch_intensity,
        light_wrap_intensity,
        chromatic_aberration,
        impact_blur_strength,
        Some(3.0),
        None,
    )
}

#[tauri::command]
pub fn get_default_composition_ops_cmd() -> Vec<CompositionOp> {
    get_default_composition_ops()
}
