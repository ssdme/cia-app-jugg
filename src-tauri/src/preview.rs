use crate::effects::{
    apply_transform_stack, apply_warp_bubble, TransformParams,
};

pub fn generate_generic_preview_frame(width: usize, height: usize) -> Vec<u8> {
    let mut frame = vec![0u8; width * height * 3];
    let max_sum = ((width + height).saturating_sub(2)).max(1) as f64;
    for y in 0..height {
        for x in 0..width {
            let val = (((x + y) as f64 / max_sum) * 255.0).round().clamp(0.0, 255.0) as u8;
            let idx = (y * width + x) * 3;
            frame[idx] = val;
            frame[idx + 1] = val;
            frame[idx + 2] = val;
        }
    }
    frame
}

pub fn render_effect_preview(effect_id: &str, width: usize, height: usize) -> Vec<u8> {
    let base_frame = generate_generic_preview_frame(width, height);
    let mut out_frame = vec![0u8; width * height * 3];
    let cx = width as f64 / 2.0;
    let cy = height as f64 / 2.0;

    match effect_id {
        "shakes" => {
            let params = TransformParams {
                dx: 24.0,
                dy: 16.0,
                scale: 1.05,
                tilt_rad: 0.08,
                skew_x: 0.0,
                scale_y: 1.0,
                scale_x: 1.0,
                barrel_k: 0.0,
            };
            apply_transform_stack(&base_frame, &mut out_frame, width, height, params);
        }
        "zoom" => {
            let params = TransformParams {
                dx: 0.0,
                dy: 0.0,
                scale: 1.35,
                tilt_rad: 0.0,
                skew_x: 0.0,
                scale_y: 1.0,
                scale_x: 1.0,
                barrel_k: 0.0,
            };
            apply_transform_stack(&base_frame, &mut out_frame, width, height, params);
        }
        "flicker" => {
            for (i, p) in base_frame.iter().enumerate() {
                out_frame[i] = ((*p as f64 * 1.45).clamp(0.0, 255.0)) as u8;
            }
        }
        "one_framers" => {
            crate::effects::apply_one_framer("DIRECTIONAL_MINIMAX", &base_frame, &mut out_frame, width, height);
        }
        "transitions" => {
            apply_warp_bubble(&base_frame, &mut out_frame, width, height, 0.8, 1.25);
        }
        "tint" => {
            for (i, chunk) in base_frame.chunks(3).enumerate() {
                let out_idx = i * 3;
                let gray = ((chunk[0] as u32 * 77 + chunk[1] as u32 * 150 + chunk[2] as u32 * 29) >> 8) as u8;
                let inv = 255 - gray;
                out_frame[out_idx] = inv;
                out_frame[out_idx + 1] = inv;
                out_frame[out_idx + 2] = inv;
            }
        }
        "vignette" => {
            let r_max = (cx * cx + cy * cy).sqrt();
            for y in 0..height {
                let dy = (y as f64) - cy;
                for x in 0..width {
                    let dx = (x as f64) - cx;
                    let r = (dx * dx + dy * dy).sqrt();
                    let factor = 1.0 - 0.75 * (r / r_max).powi(2);
                    let idx = (y * width + x) * 3;
                    for c in 0..3 {
                        out_frame[idx + c] = (base_frame[idx + c] as f64 * factor.clamp(0.0, 1.0)) as u8;
                    }
                }
            }
        }
        "scanlines" => {
            for y in 0..height {
                let factor = if y % 3 == 0 { 0.45 } else { 1.0 };
                for x in 0..width {
                    let idx = (y * width + x) * 3;
                    for c in 0..3 {
                        out_frame[idx + c] = (base_frame[idx + c] as f64 * factor) as u8;
                    }
                }
            }
        }
        "echo_trail" => {
            let mut ghost = vec![0u8; width * height * 3];
            let params = TransformParams {
                dx: -20.0,
                dy: -14.0,
                scale: 1.08,
                tilt_rad: -0.06,
                skew_x: 0.0,
                scale_y: 1.0,
                scale_x: 1.0,
                barrel_k: 0.0,
            };
            apply_transform_stack(&base_frame, &mut ghost, width, height, params);
            for i in 0..out_frame.len() {
                out_frame[i] = ((base_frame[i] as u32 * 170 + ghost[i] as u32 * 86) >> 8) as u8;
            }
        }
        "exposure_flash" => {
            for (i, p) in base_frame.iter().enumerate() {
                out_frame[i] = (*p as u16 + 115).min(255) as u8;
            }
        }
        "bouncy_shake" => {
            let params = TransformParams {
                dx: -38.0,
                dy: 0.0,
                scale: 1.0,
                tilt_rad: 0.0,
                skew_x: 0.0,
                scale_y: 1.0,
                scale_x: 1.0,
                barrel_k: 0.0,
            };
            apply_transform_stack(&base_frame, &mut out_frame, width, height, params);
        }
        "dissolve_shake" => {
            let mut ghost = vec![0u8; width * height * 3];
            let params = TransformParams {
                dx: 30.0,
                dy: 0.0,
                scale: 1.0,
                tilt_rad: 0.0,
                skew_x: 0.0,
                scale_y: 1.0,
                scale_x: 1.0,
                barrel_k: 0.0,
            };
            apply_transform_stack(&base_frame, &mut ghost, width, height, params);
            for i in 0..out_frame.len() {
                out_frame[i] = ((base_frame[i] as u32 * 170 + ghost[i] as u32 * 86) >> 8) as u8;
            }
        }
        "skew_shake" => {
            let params = TransformParams {
                dx: 0.0,
                dy: 0.0,
                scale: 1.0,
                tilt_rad: 0.0,
                skew_x: 0.25,
                scale_y: 1.0,
                scale_x: 1.0,
                barrel_k: 0.0,
            };
            apply_transform_stack(&base_frame, &mut out_frame, width, height, params);
        }
        "squish_pop" => {
            let params = TransformParams {
                dx: 0.0,
                dy: 0.0,
                scale: 1.0,
                tilt_rad: 0.0,
                skew_x: 0.0,
                scale_y: 0.85,
                scale_x: 1.18,
                barrel_k: 0.0,
            };
            apply_transform_stack(&base_frame, &mut out_frame, width, height, params);
        }
        "optics_bounce" => {
            let params = TransformParams {
                dx: 0.0,
                dy: 0.0,
                scale: 1.0,
                tilt_rad: 0.0,
                skew_x: 0.0,
                scale_y: 1.0,
                scale_x: 1.0,
                barrel_k: 0.28,
            };
            apply_transform_stack(&base_frame, &mut out_frame, width, height, params);
        }
        "buildup_chain" => {
            let params = TransformParams {
                dx: 20.0,
                dy: 20.0,
                scale: 1.06,
                tilt_rad: 0.05,
                skew_x: 0.0,
                scale_y: 1.0,
                scale_x: 1.0,
                barrel_k: 0.0,
            };
            apply_transform_stack(&base_frame, &mut out_frame, width, height, params);
        }
        "warp_stretch" => {
            let params = TransformParams {
                dx: 0.0,
                dy: 0.0,
                scale: 1.0,
                tilt_rad: 0.0,
                skew_x: 0.0,
                scale_y: 1.48,
                scale_x: 1.0,
                barrel_k: 0.0,
            };
            apply_transform_stack(&base_frame, &mut out_frame, width, height, params);
        }
        "zoom_beat_offset" => {
            let params = TransformParams {
                dx: -14.0,
                dy: 0.0,
                scale: 1.26,
                tilt_rad: 0.04,
                skew_x: 0.0,
                scale_y: 1.0,
                scale_x: 1.0,
                barrel_k: 0.0,
            };
            apply_transform_stack(&base_frame, &mut out_frame, width, height, params);
        }
        "cc_deep_dark" => {
            crate::effects::apply_cc_deep_dark(&base_frame, &mut out_frame, width, height, 42);
        }
        _ => {
            out_frame.copy_from_slice(&base_frame);
        }
    }
    out_frame
}

pub fn rgb_to_bmp_data_url(rgb: &[u8], width: u32, height: u32) -> String {
    let row_bytes = (width * 3 + 3) & !3;
    let image_size = row_bytes * height;
    let file_size = 54 + image_size;
    let mut bmp = Vec::with_capacity(file_size as usize);

    // BITMAPFILEHEADER (14 bytes)
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&file_size.to_le_bytes());
    bmp.extend_from_slice(&[0, 0, 0, 0]);
    bmp.extend_from_slice(&54u32.to_le_bytes());

    // BITMAPINFOHEADER (40 bytes)
    bmp.extend_from_slice(&40u32.to_le_bytes());
    bmp.extend_from_slice(&(width as i32).to_le_bytes());
    bmp.extend_from_slice(&(-(height as i32)).to_le_bytes()); // top-down
    bmp.extend_from_slice(&1u16.to_le_bytes()); // planes
    bmp.extend_from_slice(&24u16.to_le_bytes()); // 24-bit RGB
    bmp.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB
    bmp.extend_from_slice(&image_size.to_le_bytes());
    bmp.extend_from_slice(&2835u32.to_le_bytes()); // 72 DPI
    bmp.extend_from_slice(&2835u32.to_le_bytes());
    bmp.extend_from_slice(&0u32.to_le_bytes());
    bmp.extend_from_slice(&0u32.to_le_bytes());

    let pad = (row_bytes - width * 3) as usize;
    for y in 0..height as usize {
        for x in 0..width as usize {
            let idx = (y * width as usize + x) * 3;
            // BMP pixel order is BGR
            bmp.push(rgb[idx + 2]);
            bmp.push(rgb[idx + 1]);
            bmp.push(rgb[idx]);
        }
        for _ in 0..pad {
            bmp.push(0);
        }
    }

    let base64_str = to_base64(&bmp);
    format!("data:image/bmp;base64,{base64_str}")
}

pub fn to_base64(bytes: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut res = String::with_capacity(bytes.len() * 4 / 3 + 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = if chunk.len() > 1 { chunk[1] as usize } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as usize } else { 0 };
        let n = (b0 << 16) | (b1 << 8) | b2;
        res.push(CHARS[(n >> 18) & 63] as char);
        res.push(CHARS[(n >> 12) & 63] as char);
        if chunk.len() > 1 {
            res.push(CHARS[(n >> 6) & 63] as char);
        } else {
            res.push('=');
        }
        if chunk.len() > 2 {
            res.push(CHARS[n & 63] as char);
        } else {
            res.push('=');
        }
    }
    res
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EffectItemInfo {
    pub id: String,
    pub name: String,
    pub category: String,
    pub description: String,
    pub preview_data_url: String,
}

#[tauri::command]
pub fn get_effect_previews() -> Result<Vec<EffectItemInfo>, String> {
    const EFFECTS_METADATA: &[(&str, &str, &str, &str)] = &[
        ("shakes", "Harmonic Shakes (X/Y/Z/Tilt)", "SHAKES", "Smooth exponential damped oscillations on 4 axes"),
        ("bouncy_shake", "Bouncy Shake", "SHAKES", "BlurMoCurves-style piecewise keyframe bounce"),
        ("dissolve_shake", "Dissolve Ghost Shake", "SHAKES", "Ghost frame blend ±2 frames with envelope"),
        ("skew_shake", "Skew Shake", "SHAKES", "Damped cisaillement / horizontal perspective skew"),
        ("squish_pop", "Squish Pop", "WARP", "Scale Y compression into spring overshoot"),
        ("optics_bounce", "Optics Bounce", "WARP", "Dynamic parabolic barrel distortion at beat onsets"),
        ("warp_stretch", "Warp Transform Stretch", "WARP", "Vertical / horizontal scale stretch with saddle curve"),
        ("zoom", "Beat Zoom", "ZOOM", "Continuous rhythmic scale zooms matching beat cadence"),
        ("zoom_beat_offset", "Zoom Past-The-Beat", "ZOOM", "Micro-delayed zoom peak offset +1..+2 frames"),
        ("buildup_chain", "Buildup Chaining", "MOTION", "Continuous shake envelope bleed into next segment"),
        ("transitions", "Geometric Transitions", "TRANSITIONS", "Warp Bubble, Wave Warp, and Slide Shake cuts"),
        ("one_framers", "One-Framers Library (10 Styles)", "CUTS", "Multi-style library: Minimax Beams, Fisheye, Bokeh, Offset Blur, Radial Blur, Scene Tint & Soft Flash"),
        ("flicker", "Flicker Oscillation", "AMBIANCE", "Sinusoidal luminosity micro-oscillations"),
        ("exposure_flash", "Exposure Flash", "AMBIANCE", "Sharp white flashes at musical impact points"),
        ("echo_trail", "Echo / Trail", "AMBIANCE", "Motion time blend with trailing ghost frames"),
        ("tint", "Invert B&W (Negative)", "AMBIANCE", "Inverts colors to black and white with zero saturation"),
        ("vignette", "Vignette Darkening", "AMBIANCE", "Radial corner darkening focusing visual center"),
        ("scanlines", "CRT Scanlines", "AMBIANCE", "Retro television horizontal scanline rasterization"),
        ("cc_deep_dark", "Color Correction: Deep Dark", "AMBIANCE", "Grayscale luma crush, 10px bloom glow, 19% film grain"),
    ];

    let mut list = Vec::with_capacity(EFFECTS_METADATA.len());
    for &(id, name, cat, desc) in EFFECTS_METADATA {
        let frame_rgb = render_effect_preview(id, 256, 256);
        let preview_data_url = rgb_to_bmp_data_url(&frame_rgb, 256, 256);
        list.push(EffectItemInfo {
            id: id.to_string(),
            name: name.to_string(),
            category: cat.to_string(),
            description: desc.to_string(),
            preview_data_url,
        });
    }
    Ok(list)
}
