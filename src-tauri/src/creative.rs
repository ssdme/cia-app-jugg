//! Short, resolving accents. All transforms read clean RGB frames and allocate no image buffers.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CreativeKind {
    TimeSlice,
    InkPulse,
    PrismDrift,
    FocusPull,
    ReliefLight,
    PaperGate,
}

impl CreativeKind {
    pub const COUNT: usize = 6;

    pub fn slot(self) -> usize {
        match self {
            Self::TimeSlice => 0,
            Self::InkPulse => 1,
            Self::PrismDrift => 2,
            Self::FocusPull => 3,
            Self::ReliefLight => 4,
            Self::PaperGate => 5,
        }
    }
}

/// `phase` covers one accent, including clean endpoints. `alternate` shares the RGB layout.
#[allow(clippy::too_many_arguments)]
pub fn apply(
    kind: CreativeKind,
    frame: &[u8],
    alternate: Option<&[u8]>,
    out: &mut [u8],
    width: usize,
    height: usize,
    phase: f64,
    seed: u32,
    strength: f64,
) {
    let len = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(3))
        .expect("Creative frame dimensions overflow");
    assert_eq!(frame.len(), len, "Invalid creative input buffer");
    assert_eq!(out.len(), len, "Invalid creative output buffer");
    if let Some(alternate) = alternate {
        assert_eq!(alternate.len(), len, "Invalid creative alternate buffer");
    }
    out.copy_from_slice(frame);
    if len == 0
        || !phase.is_finite()
        || phase <= 0.0
        || phase >= 1.0
        || !strength.is_finite()
        || strength <= 0.0
    {
        return;
    }

    let envelope = (std::f64::consts::PI * phase).sin().powi(2);
    let amount = (envelope * strength.min(1.0) * 256.0).round() as i32;
    if amount == 0 {
        return;
    }
    let seed = mix_seed(seed);
    match kind {
        CreativeKind::TimeSlice => {
            if let Some(alternate) = alternate {
                time_slice(alternate, out, width, height, phase, seed, amount);
            }
        }
        CreativeKind::InkPulse => ink_pulse(frame, out, width, height, seed, amount),
        CreativeKind::PrismDrift => prism_drift(frame, out, width, height, seed, amount),
        CreativeKind::FocusPull => {
            crate::creative_beta::focus_pull(frame, out, width, height, phase, seed, amount)
        }
        CreativeKind::ReliefLight => {
            crate::creative_beta::relief_light(frame, out, width, height, phase, seed, amount)
        }
        CreativeKind::PaperGate => {
            crate::creative_beta::paper_gate(frame, out, width, height, phase, seed, amount)
        }
    }
}

fn mix_seed(mut seed: u32) -> u32 {
    seed ^= seed >> 16;
    seed = seed.wrapping_mul(0x7feb_352d);
    seed ^= seed >> 15;
    seed = seed.wrapping_mul(0x846c_a68b);
    seed ^ (seed >> 16)
}

fn time_slice(
    alternate: &[u8],
    out: &mut [u8],
    width: usize,
    height: usize,
    phase: f64,
    seed: u32,
    amount: i32,
) {
    let panels = 2 + (seed % 3) as i64;
    let axis = (seed >> 3) % 3;
    let direction = if seed & 64 == 0 { 1 } else { -1 };
    let travel = (phase * 65_536.0 * direction as f64).round() as i64;
    let coverage = i64::from(amount) * 145;
    let step_x = 65_536 / width.max(1) as i64;
    let step_y = 65_536 / height.max(1) as i64;
    let displacement = (width.min(height) as i64 * i64::from(amount) / 10_240).min(18);

    for y in 0..height {
        for x in 0..width {
            let projection = match axis {
                0 => x as i64 * step_x,
                1 => y as i64 * step_y,
                _ => (x as i64 * step_x + y as i64 * step_y) / 2,
            };
            let position = projection * panels + travel;
            if position.rem_euclid(65_536) >= coverage {
                continue;
            }
            let panel = position.div_euclid(65_536);
            let shift = displacement * if panel & 1 == 0 { 1 } else { -1 };
            let sx = if axis == 1 {
                x
            } else {
                mirrored(x as i64 + shift, width)
            };
            let sy = if axis == 0 {
                y
            } else {
                mirrored(y as i64 - shift, height)
            };
            let input = (sy * width + sx) * 3;
            let output = (y * width + x) * 3;
            out[output..output + 3].copy_from_slice(&alternate[input..input + 3]);
        }
    }
}

fn ink_pulse(frame: &[u8], out: &mut [u8], width: usize, height: usize, seed: u32, amount: i32) {
    let radius = (width.min(height) / 240).clamp(1, 5);
    let cell = (width.min(height) / 130).clamp(3, 9);
    let offset_x = seed as usize % cell;
    let offset_y = (seed >> 8) as usize % cell;
    let quantization = if seed & 1 == 0 { 32 } else { 24 };

    for y in 0..height {
        for x in 0..width {
            let index = (y * width + x) * 3;
            let edge = edge_strength(frame, width, height, x, y, radius);
            if edge <= 6 {
                continue;
            }
            let texture = (edge - 6).min(128) * 2;
            let ink = (edge - 18).clamp(0, 160) * amount / 256;
            let quant_mix = texture * amount / 1_024;
            let luma = luminance(frame, index);
            let dx = 2 * ((x + offset_x) % cell) as i32 - cell as i32 + 1;
            let dy = 2 * ((y + offset_y) % cell) as i32 - cell as i32 + 1;
            let dot_radius = (255 - luma) * (cell * cell) as i32 / 384;
            let dot = if dx * dx + dy * dy < dot_radius {
                texture * amount / 2_048
            } else {
                0
            };
            for channel in 0..3 {
                let original = i32::from(frame[index + channel]);
                let posterized =
                    ((original + quantization / 2) / quantization * quantization).min(255);
                let printed = original + (posterized - original) * quant_mix / 256;
                out[index + channel] = (printed * (256 - ink - dot).max(0) / 256) as u8;
            }
        }
    }
}

fn prism_drift(frame: &[u8], out: &mut [u8], width: usize, height: usize, seed: u32, amount: i32) {
    let vertical = seed & 1 != 0;
    let direction = if seed & 2 == 0 { 1 } else { -1 };
    let maximum = (width.min(height) / 36).clamp(1, 18);
    let radius = maximum.div_ceil(2);
    let displacement = maximum as i64 * i64::from(amount) * direction;
    let cross_dimension = if vertical { width } else { height };

    for y in 0..height {
        for x in 0..width {
            let index = (y * width + x) * 3;
            let edge = edge_strength(frame, width, height, x, y, radius);
            let weight = (edge - 10).clamp(0, 112) * amount / 112;
            if weight == 0 {
                continue;
            }
            let cross_position = if vertical { x } else { y };
            // The affine shear varies across the frame, rather than translating whole channels.
            let shear = 144 + cross_position as i64 * 112 / cross_dimension.max(1) as i64;
            let shift = displacement * shear / 256;
            let red = sample_channel(frame, width, height, x, y, shift, vertical, 0);
            let blue = sample_channel(frame, width, height, x, y, -shift, vertical, 2);
            let dr = (red - i32::from(frame[index])).clamp(-96, 96) * weight / 256;
            let db = (blue - i32::from(frame[index + 2])).clamp(-96, 96) * weight / 256;
            let dg = -(77 * dr + 29 * db) / 150;
            out[index] = (i32::from(frame[index]) + dr).clamp(0, 255) as u8;
            out[index + 1] = (i32::from(frame[index + 1]) + dg).clamp(0, 255) as u8;
            out[index + 2] = (i32::from(frame[index + 2]) + db).clamp(0, 255) as u8;
        }
    }
}

#[inline]
fn luminance(frame: &[u8], index: usize) -> i32 {
    (77 * i32::from(frame[index])
        + 150 * i32::from(frame[index + 1])
        + 29 * i32::from(frame[index + 2]))
        / 256
}

#[inline]
fn edge_strength(
    frame: &[u8],
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    radius: usize,
) -> i32 {
    let left = (y * width + x.saturating_sub(radius)) * 3;
    let right = (y * width + (x + radius).min(width - 1)) * 3;
    let top = (y.saturating_sub(radius) * width + x) * 3;
    let bottom = ((y + radius).min(height - 1) * width + x) * 3;
    let center = (y * width + x) * 3;
    let mut gradient = 0;
    for neighbor in [left, right, top, bottom] {
        for channel in 0..3 {
            gradient = gradient.max(
                (i32::from(frame[center + channel]) - i32::from(frame[neighbor + channel])).abs(),
            );
        }
    }
    gradient
}

#[allow(clippy::too_many_arguments)]
#[inline]
fn sample_channel(
    frame: &[u8],
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    shift: i64,
    vertical: bool,
    channel: usize,
) -> i32 {
    let position = if vertical { y } else { x } as i64 * 256 + shift;
    let low = position.div_euclid(256);
    let fraction = position.rem_euclid(256) as i32;
    let dimension = if vertical { height } else { width };
    let first = mirrored(low, dimension);
    let second = mirrored(low + 1, dimension);
    let first = if vertical {
        first * width + x
    } else {
        y * width + first
    } * 3
        + channel;
    let second = if vertical {
        second * width + x
    } else {
        y * width + second
    } * 3
        + channel;
    (i32::from(frame[first]) * (256 - fraction) + i32::from(frame[second]) * fraction + 128) / 256
}

#[inline]
fn mirrored(coordinate: i64, dimension: usize) -> usize {
    if dimension <= 1 {
        return 0;
    }
    let last = dimension as i64 - 1;
    let position = coordinate.rem_euclid(last * 2);
    if position > last {
        (last * 2 - position) as usize
    } else {
        position as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KINDS: [CreativeKind; 3] = [
        CreativeKind::TimeSlice,
        CreativeKind::InkPulse,
        CreativeKind::PrismDrift,
    ];

    fn pattern(width: usize, height: usize, offset: usize) -> Vec<u8> {
        (0..width * height * 3)
            .map(|index| {
                let pixel = index / 3;
                let x = pixel % width;
                let y = pixel / width;
                ((x * 17 + y * 31 + index % 3 * 67 + offset) % 256) as u8
            })
            .collect()
    }

    fn rendered(kind: CreativeKind, width: usize, height: usize, seed: u32) -> Vec<u8> {
        let frame = pattern(width, height, 0);
        let alternate = pattern(width, height, 103);
        let mut out = vec![0; frame.len()];
        apply(
            kind,
            &frame,
            Some(&alternate),
            &mut out,
            width,
            height,
            0.5,
            seed,
            0.9,
        );
        out
    }

    #[test]
    fn bypass_and_endpoints_preserve_every_byte() {
        let frame = pattern(13, 7, 0);
        let alternate = pattern(13, 7, 49);
        for kind in KINDS.into_iter().chain([
            CreativeKind::FocusPull,
            CreativeKind::ReliefLight,
            CreativeKind::PaperGate,
        ]) {
            for (phase, strength) in [
                (0.5, 0.0),
                (0.5, -1.0),
                (0.5, f64::NAN),
                (-0.1, 1.0),
                (0.0, 1.0),
                (1.0, 1.0),
                (1.1, 1.0),
                (f64::NAN, 1.0),
            ] {
                let mut out = vec![255; frame.len()];
                apply(
                    kind,
                    &frame,
                    Some(&alternate),
                    &mut out,
                    13,
                    7,
                    phase,
                    7,
                    strength,
                );
                assert_eq!(out, frame, "{kind:?}: {phase}, {strength}");
            }
        }
    }

    #[test]
    fn tiny_and_rectangular_frames_are_supported() {
        for (width, height) in [(0, 0), (1, 1), (1, 11), (13, 1), (3, 7), (17, 5)] {
            for kind in KINDS {
                for seed in 0..8 {
                    assert_eq!(
                        rendered(kind, width, height, seed).len(),
                        width * height * 3
                    );
                }
            }
        }
    }

    #[test]
    fn treatments_are_deterministic_distinct_and_seeded() {
        let frame = pattern(61, 43, 0);
        let renders: Vec<_> = KINDS
            .iter()
            .map(|&kind| rendered(kind, 61, 43, 12))
            .collect();
        for (index, kind) in KINDS.iter().enumerate() {
            assert_eq!(renders[index], rendered(*kind, 61, 43, 12));
            assert_ne!(
                renders[index], frame,
                "{kind:?} must produce a visible treatment"
            );
            assert_ne!(
                renders[index],
                rendered(*kind, 61, 43, 23),
                "{kind:?} seed variation"
            );
            for other in &renders[..index] {
                assert_ne!(&renders[index], other);
            }
        }
    }

    #[test]
    fn flat_fields_including_black_and_white_remain_stable() {
        for color in [[0, 0, 0], [255, 255, 255], [32, 109, 202], [117, 117, 117]] {
            let frame = color.repeat(17 * 13);
            for kind in KINDS {
                let mut out = vec![0; frame.len()];
                apply(kind, &frame, Some(&frame), &mut out, 17, 13, 0.5, 4, 1.0);
                assert_eq!(out, frame, "{kind:?}: {color:?}");
            }
        }
    }

    #[test]
    fn temporal_panels_use_alternate_without_blending() {
        let frame = [21, 35, 89].repeat(39 * 23);
        let alternate = [202, 150, 54].repeat(39 * 23);
        let mut out = vec![0; frame.len()];
        apply(
            CreativeKind::TimeSlice,
            &frame,
            Some(&alternate),
            &mut out,
            39,
            23,
            0.5,
            8,
            1.0,
        );
        assert!(out.chunks_exact(3).any(|pixel| pixel == [202, 150, 54]));
        assert!(out.chunks_exact(3).any(|pixel| pixel == [21, 35, 89]));
        assert!(out
            .chunks_exact(3)
            .all(|pixel| pixel == [202, 150, 54] || pixel == [21, 35, 89]));
        apply(
            CreativeKind::TimeSlice,
            &frame,
            None,
            &mut out,
            39,
            23,
            0.5,
            8,
            1.0,
        );
        assert_eq!(out, frame);
    }

    #[test]
    #[should_panic(expected = "Invalid creative input buffer")]
    fn malformed_buffers_fail_before_pixel_access() {
        apply(
            CreativeKind::InkPulse,
            &[1, 2],
            None,
            &mut [0; 3],
            1,
            1,
            0.5,
            0,
            1.0,
        );
    }

    #[test]
    #[ignore = "Writes a visual proof sheet to tests/generated"]
    fn write_creative_proof_sheet() {
        use std::io::Write;

        let width = 320;
        let height = 240;
        let artwork = |offset: i32| {
            let mut frame = vec![0; width * height * 3];
            for y in 0..height {
                for x in 0..width {
                    let px = x as i32 - 152 - offset;
                    let py = y as i32 - 117;
                    let ellipse = px * px * 64 + py * py * 49;
                    let pixel = if ellipse < 64 * 49 * 68 {
                        if py < -25 {
                            [232, 138, 39]
                        } else {
                            [244, 192, 151]
                        }
                    } else if ellipse < 64 * 49 * 76 {
                        [34, 29, 57]
                    } else if y > 154 && (x as i32 - 156 - offset).abs() < 83 {
                        [71, 96, 170]
                    } else {
                        [
                            (33 + x * 73 / width) as u8,
                            (91 + y * 61 / height) as u8,
                            (161 + x * 56 / width) as u8,
                        ]
                    };
                    let index = (y * width + x) * 3;
                    frame[index..index + 3].copy_from_slice(&pixel);
                }
            }
            frame
        };
        let frame = artwork(0);
        let alternate = artwork(-27);
        let mut tiles = vec![frame.clone(), alternate.clone()];
        for kind in KINDS.into_iter().chain([
            CreativeKind::FocusPull,
            CreativeKind::ReliefLight,
            CreativeKind::PaperGate,
        ]) {
            let mut out = vec![0; frame.len()];
            apply(
                kind,
                &frame,
                Some(&alternate),
                &mut out,
                width,
                height,
                0.5,
                12,
                1.0,
            );
            tiles.push(out);
        }
        let directory =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/generated/creative");
        std::fs::create_dir_all(&directory).unwrap();
        let mut sheet = std::fs::File::create(directory.join("proof.ppm")).unwrap();
        write!(
            sheet,
            "P6\n# Current | Alternate | Time Slice | Ink Pulse | Prism Drift | Focus Pull | Relief Light | Paper Gate\n{} {height}\n255\n",
            width * tiles.len()
        )
        .unwrap();
        for y in 0..height {
            for tile in &tiles {
                sheet
                    .write_all(&tile[y * width * 3..(y + 1) * width * 3])
                    .unwrap();
            }
        }
    }
}
