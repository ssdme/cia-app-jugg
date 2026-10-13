//! Spatial accents for trial use. Each pass reads clean RGB without image allocations.

const UNIT: i32 = 65_536;

#[allow(clippy::too_many_arguments)]
pub fn focus_pull(
    frame: &[u8],
    out: &mut [u8],
    width: usize,
    height: usize,
    phase: f64,
    seed: u32,
    amount: i32,
) {
    let amount = prepare(frame, out, width, height, phase, amount);
    if amount == 0 {
        return;
    }
    let radius = (width.min(height) / 70).clamp(1, 18);
    let diagonal = radius.div_ceil(2);
    let axis = seed % 4;
    let progress = if seed & 8 == 0 { phase } else { 1.0 - phase };
    let plane = (18_000.0 + progress * 29_536.0).round() as i32;

    for y in 0..height {
        let ny = normalized(y, height);
        for x in 0..width {
            let nx = normalized(x, width);
            let position = match axis {
                0 => nx,
                1 => ny,
                2 => (3 * nx + ny) / 4,
                _ => (nx + 3 * ny) / 4,
            };
            let distance = (position - plane).abs();
            let defocus = ((distance - 5_400).max(0) * 256 / 10_000).min(256);
            let weight = defocus * amount * 7 / 2_048;
            if weight == 0 {
                continue;
            }
            let index = (y * width + x) * 3;
            let left = pixel(width, x.saturating_sub(radius), y);
            let right = pixel(width, (x + radius).min(width - 1), y);
            let top = pixel(width, x, y.saturating_sub(radius));
            let bottom = pixel(width, x, (y + radius).min(height - 1));
            let corners = [
                pixel(
                    width,
                    x.saturating_sub(diagonal),
                    y.saturating_sub(diagonal),
                ),
                pixel(
                    width,
                    (x + diagonal).min(width - 1),
                    y.saturating_sub(diagonal),
                ),
                pixel(
                    width,
                    x.saturating_sub(diagonal),
                    (y + diagonal).min(height - 1),
                ),
                pixel(
                    width,
                    (x + diagonal).min(width - 1),
                    (y + diagonal).min(height - 1),
                ),
            ];
            for channel in 0..3 {
                let mut sum = 4 * i32::from(frame[index + channel]);
                for neighbor in [left, right, top, bottom] {
                    sum += 2 * i32::from(frame[neighbor + channel]);
                }
                for corner in corners {
                    sum += i32::from(frame[corner + channel]);
                }
                let blurred = (sum + 8) / 16;
                out[index + channel] = blend(frame[index + channel], blurred, weight);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn relief_light(
    frame: &[u8],
    out: &mut [u8],
    width: usize,
    height: usize,
    phase: f64,
    seed: u32,
    amount: i32,
) {
    let amount = prepare(frame, out, width, height, phase, amount);
    if amount == 0 {
        return;
    }
    let radius = (width.min(height) / 180).clamp(3, 8);
    let angle = (seed % 628) as f64 / 100.0 + (phase - 0.5) * 1.8;
    let light_x = (angle.cos() * 256.0).round() as i32;
    let light_y = (angle.sin() * 256.0).round() as i32;
    let beam_center = (phase * f64::from(UNIT)).round() as i32;
    let reverse = seed & 1 != 0;

    for y in 0..height {
        for x in 0..width {
            let left = pixel(width, x.saturating_sub(radius), y);
            let right = pixel(width, (x + radius).min(width - 1), y);
            let top = pixel(width, x, y.saturating_sub(radius));
            let bottom = pixel(width, x, (y + radius).min(height - 1));
            let dx = luminance(frame, right) - luminance(frame, left);
            let dy = luminance(frame, bottom) - luminance(frame, top);
            let facing = (dx * light_x + dy * light_y) / 512;
            if facing == 0 {
                continue;
            }
            let nx = normalized(x, width);
            let beam_position = if reverse { UNIT - nx } else { nx };
            let beam = (256 - (beam_position - beam_center).abs() * 256 / 24_000).max(0);
            let shade = (facing.clamp(-160, 160) * amount * (192 + beam) / 131_072).clamp(-64, 64);
            let index = (y * width + x) * 3;
            for channel in 0..3 {
                let original = i32::from(frame[index + channel]);
                // Warm crests and cool recesses model colored light, without shifting channels.
                let tint = if shade > 0 {
                    [100, 86, 65]
                } else {
                    [64, 83, 100]
                };
                let headroom = if shade > 0 { 255 - original } else { original };
                let change = shade * tint[channel] * headroom / 25_600;
                out[index + channel] = (original + change).clamp(0, 255) as u8;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn paper_gate(
    frame: &[u8],
    out: &mut [u8],
    width: usize,
    height: usize,
    phase: f64,
    seed: u32,
    amount: i32,
) {
    let amount = prepare(frame, out, width, height, phase, amount);
    if amount == 0 {
        return;
    }
    let travel = (phase * 3_200.0).round() as i32;
    let reach = 2_300;
    let paper = if seed & 1 == 0 {
        [228, 224, 211]
    } else {
        [214, 222, 230]
    };
    let backing = if seed & 2 == 0 {
        [46, 54, 69]
    } else {
        [82, 54, 49]
    };

    for y in 0..height {
        let ny = normalized(y, height);
        let left = paper_depth(ny, travel, seed, 0, amount);
        let right = paper_depth(ny, travel, seed, 1, amount);
        for x in 0..width {
            let nx = normalized(x, width);
            if nx > reach && nx < UNIT - reach && ny > reach && ny < UNIT - reach {
                continue;
            }
            let mut gap = (left - nx).max(right - (UNIT - nx));
            if ny <= reach {
                gap = gap.max(paper_depth(nx, travel, seed, 2, amount) - ny);
            }
            if ny >= UNIT - reach {
                gap = gap.max(paper_depth(nx, travel, seed, 3, amount) - (UNIT - ny));
            }
            let backing_width = 140 * amount / 256;
            let shadow_width = 600 * amount / 256;
            if gap < -shadow_width {
                continue;
            }
            let index = (y * width + x) * 3;
            if gap >= 0 {
                let grain =
                    (hash(seed ^ (x as u32).wrapping_mul(17) ^ (y as u32).wrapping_mul(7919)) % 7)
                        as i32
                        - 3;
                let fiber = if gap < 90 * amount / 256 { 7 } else { 0 };
                for channel in 0..3 {
                    out[index + channel] = blend(
                        frame[index + channel],
                        paper[channel] + grain + fiber,
                        amount,
                    );
                }
            } else if gap >= -backing_width {
                for channel in 0..3 {
                    out[index + channel] =
                        blend(frame[index + channel], backing[channel], amount * 3 / 4);
                }
            } else {
                let shadow = (shadow_width + gap) * amount / shadow_width.max(1) / 3;
                for channel in 0..3 {
                    let original = i32::from(frame[index + channel]);
                    out[index + channel] = (original * (256 - shadow) / 256) as u8;
                }
            }
        }
    }
}

fn prepare(
    frame: &[u8],
    out: &mut [u8],
    width: usize,
    height: usize,
    phase: f64,
    amount: i32,
) -> i32 {
    let len = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(3))
        .expect("Creative frame dimensions overflow");
    assert_eq!(frame.len(), len, "Invalid creative input buffer");
    assert_eq!(out.len(), len, "Invalid creative output buffer");
    out.copy_from_slice(frame);
    if width == 0 || height == 0 || !phase.is_finite() {
        0
    } else {
        amount.clamp(0, 256)
    }
}

#[inline]
fn pixel(width: usize, x: usize, y: usize) -> usize {
    (y * width + x) * 3
}

#[inline]
fn normalized(coordinate: usize, dimension: usize) -> i32 {
    if dimension <= 1 {
        UNIT / 2
    } else {
        (coordinate as i64 * i64::from(UNIT) / (dimension - 1) as i64) as i32
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
fn blend(original: u8, target: i32, weight: i32) -> u8 {
    ((i32::from(original) * (256 - weight) + target * weight + 128) / 256).clamp(0, 255) as u8
}

#[inline]
fn hash(mut value: u32) -> u32 {
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb_352d);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846c_a68b);
    value ^ (value >> 16)
}

fn paper_depth(position: i32, travel: i32, seed: u32, edge: u32, amount: i32) -> i32 {
    let edge_seed = hash(seed ^ edge.wrapping_mul(0x9e37_79b9));
    let direction = if edge & 1 == 0 { 1 } else { -1 };
    let position = position + travel * direction;
    let coarse = torn_noise(position, 28, edge_seed);
    let fine = torn_noise(position, 117, edge_seed ^ 0xa341_316c);
    let fibers = torn_noise(position, 431, edge_seed ^ 0xc801_3ea4);
    let base = 950 + (edge_seed % 350) as i32;
    let depth =
        (base + coarse * 390 / 256 + fine * 170 / 256 + fibers * 75 / 256).clamp(200, 1_700);
    depth * amount / 256
}

#[inline]
fn torn_noise(position: i32, frequency: i32, seed: u32) -> i32 {
    let position = position * frequency;
    let cell = position.div_euclid(UNIT);
    let fraction = position.rem_euclid(UNIT);
    let first = (hash(seed ^ cell as u32) & 511) as i32 - 256;
    let second = (hash(seed ^ (cell + 1) as u32) & 511) as i32 - 256;
    first + (second - first) * fraction / UNIT
}

#[cfg(test)]
mod tests {
    use super::*;

    type Effect = fn(&[u8], &mut [u8], usize, usize, f64, u32, i32);
    const EFFECTS: [Effect; 3] = [focus_pull, relief_light, paper_gate];

    fn drawing(width: usize, height: usize) -> Vec<u8> {
        (0..width * height)
            .flat_map(|pixel| {
                let x = pixel % width;
                let y = pixel / width;
                let value = if (x / 3 + y / 5).is_multiple_of(2) {
                    30
                } else {
                    220
                };
                [value, (x * 191 / width) as u8, (y * 173 / height) as u8]
            })
            .collect()
    }

    #[test]
    fn trial_accents_are_visible_distinct_and_deterministic() {
        let (width, height) = (137, 91);
        let frame = drawing(width, height);
        let outputs: Vec<_> = EFFECTS
            .iter()
            .map(|effect| {
                let mut first = vec![0; frame.len()];
                let mut second = first.clone();
                effect(&frame, &mut first, width, height, 0.5, 31, 256);
                effect(&frame, &mut second, width, height, 0.5, 31, 256);
                assert_eq!(first, second);
                assert!(first.iter().zip(&frame).filter(|(a, b)| a != b).count() > 200);
                first
            })
            .collect();
        assert_ne!(outputs[0], outputs[1]);
        assert_ne!(outputs[1], outputs[2]);
        assert_ne!(outputs[0], outputs[2]);
    }

    #[test]
    fn zero_amount_and_nonfinite_phase_are_exact_passthrough() {
        let frame = drawing(17, 11);
        for effect in EFFECTS {
            for (phase, amount) in [(0.5, 0), (0.5, -1), (f64::NAN, 256)] {
                let mut out = vec![255; frame.len()];
                effect(&frame, &mut out, 17, 11, phase, 3, amount);
                assert_eq!(out, frame);
            }
        }
    }

    #[test]
    fn focus_and_relief_preserve_flat_fields_including_white_and_black() {
        for color in [[0, 0, 0], [255, 255, 255], [61, 114, 189]] {
            let frame = color.repeat(37 * 29);
            for effect in [focus_pull as Effect, relief_light] {
                let mut out = vec![0; frame.len()];
                effect(&frame, &mut out, 37, 29, 0.62, 117, 256);
                assert_eq!(out, frame);
            }
        }
    }

    #[test]
    fn relief_bevel_has_spatial_support_without_changing_flat_regions() {
        let (width, height) = (320, 180);
        let mut frame = vec![0; width * height * 3];
        for y in 0..height {
            for x in 0..width {
                let value = if x < width / 2 { 65 } else { 190 };
                let index = pixel(width, x, y);
                frame[index..index + 3].fill(value);
            }
        }
        let mut out = vec![0; frame.len()];
        relief_light(&frame, &mut out, width, height, 0.5, 914_189_097, 256);
        let changed = out
            .chunks_exact(3)
            .zip(frame.chunks_exact(3))
            .filter(|(a, b)| a != b)
            .count();
        assert_eq!(changed, 6 * height);
        let index = pixel(width, width / 2 - 2, height / 2);
        assert!(out[index].abs_diff(frame[index]) >= 10);
        for y in 0..height {
            let left_end = pixel(width, width / 2 - 3, y);
            let right_start = pixel(width, width / 2 + 3, y);
            let row_end = pixel(width, width, y);
            let row_start = pixel(width, 0, y);
            assert_eq!(out[row_start..left_end], frame[row_start..left_end]);
            assert_eq!(out[right_start..row_end], frame[right_start..row_end]);
        }
    }

    #[test]
    fn seeds_and_progress_change_each_trial_accent() {
        let frame = drawing(157, 103);
        for effect in EFFECTS {
            let mut first = vec![0; frame.len()];
            let mut reseeded = first.clone();
            let mut advanced = first.clone();
            effect(&frame, &mut first, 157, 103, 0.35, 22, 256);
            effect(&frame, &mut reseeded, 157, 103, 0.35, 35, 256);
            effect(&frame, &mut advanced, 157, 103, 0.65, 22, 256);
            assert_ne!(first, reseeded);
            assert_ne!(first, advanced);
        }
    }

    #[test]
    fn paper_protects_center_and_limits_coverage() {
        let (width, height) = (400, 300);
        let frame = [103, 129, 157].repeat(width * height);
        for seed in [0, 7, 51, u32::MAX] {
            let mut out = vec![0; frame.len()];
            paper_gate(&frame, &mut out, width, height, 0.5, seed, 256);
            for y in height / 10..height * 9 / 10 {
                let start = pixel(width, width / 10, y);
                let end = pixel(width, width * 9 / 10, y);
                assert_eq!(out[start..end], frame[start..end]);
            }
            let changed = out
                .chunks_exact(3)
                .zip(frame.chunks_exact(3))
                .filter(|(a, b)| a != b)
                .count();
            assert!(changed > width * height / 50);
            assert!(changed < width * height * 15 / 100);
        }
    }

    #[test]
    fn arbitrary_rectangles_and_tiny_frames_stay_in_bounds() {
        for (width, height) in [
            (0, 0),
            (0, 4),
            (4, 0),
            (1, 1),
            (1, 19),
            (19, 1),
            (2, 3),
            (3, 2),
            (301, 7),
            (7, 301),
        ] {
            let frame = drawing(width, height);
            for effect in EFFECTS {
                let mut out = vec![0; frame.len()];
                effect(&frame, &mut out, width, height, 0.7, u32::MAX, 256);
                assert_eq!(out.len(), frame.len());
            }
        }
    }
}
