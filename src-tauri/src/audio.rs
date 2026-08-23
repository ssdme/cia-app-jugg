use crate::plan::ProjectPlan;
use crate::render::compute_source_time_for_target_time;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AudioMixConfig {
    #[serde(default = "default_true")]
    pub sidechain_ducking: bool,
    #[serde(default = "default_ducking_amount")]
    pub ducking_amount_db: f32, // -12.0 dB
    #[serde(default = "default_attack_ms")]
    pub attack_ms: f32, // 5.0 ms
    #[serde(default = "default_release_ms")]
    pub release_ms: f32, // 150.0 ms
    #[serde(default = "default_true")]
    pub varispeed_audio: bool,
    #[serde(default = "default_true")]
    pub staccato_cuts: bool,
    #[serde(default)]
    pub mix_source_audio: bool,
    #[serde(default)]
    pub source_volume_db: f32,
    #[serde(default)]
    pub target_volume_db: f32,
}

fn default_true() -> bool { true }
fn default_ducking_amount() -> f32 { -12.0 }
fn default_attack_ms() -> f32 { 5.0 }
fn default_release_ms() -> f32 { 150.0 }

impl Default for AudioMixConfig {
    fn default() -> Self {
        Self {
            sidechain_ducking: true,
            ducking_amount_db: -12.0,
            attack_ms: 5.0,
            release_ms: 150.0,
            varispeed_audio: true,
            staccato_cuts: true,
            mix_source_audio: false,
            source_volume_db: 0.0,
            target_volume_db: 0.0,
        }
    }
}

/// Compute gain multiplier for a given timestamp t with sidechain ducking on downbeats
pub fn compute_sidechain_ducking_gain(
    t: f64,
    downbeats: &[f64],
    ducking_amount_db: f32,
    attack_ms: f32,
    release_ms: f32,
) -> f32 {
    let min_gain = 10.0f32.powf(ducking_amount_db / 20.0);
    let attack_s = (attack_ms as f64) / 1000.0;
    let release_s = (release_ms as f64) / 1000.0;

    let mut lowest_gain = 1.0f32;

    for &db in downbeats {
        if t >= db - 1e-5 && t < db + attack_s + release_s + 1e-4 {
            let dt = t - db;
            let gain = if dt < 0.0 {
                1.0
            } else if dt <= attack_s {
                let prog = (dt / attack_s.max(1e-6)) as f32;
                1.0 - (1.0 - min_gain) * prog
            } else if dt <= attack_s + release_s {
                let rel_dt = (dt - attack_s) / release_s.max(1e-6);
                let recovery = (1.0 - (-3.0 * rel_dt).exp()) / (1.0 - (-3.0f64).exp());
                min_gain + (1.0 - min_gain) * (recovery as f32)
            } else {
                1.0
            };

            if gain < lowest_gain {
                lowest_gain = gain;
            }
        }
    }

    lowest_gain
}

/// Apply sidechain ducking in-place on audio samples (mono or interleaved stereo)
pub fn apply_sidechain_ducking(
    samples: &mut [f32],
    channels: usize,
    sample_rate: u32,
    downbeats: &[f64],
    config: &AudioMixConfig,
) {
    if !config.sidechain_ducking || downbeats.is_empty() || channels == 0 || sample_rate == 0 {
        return;
    }

    let total_frames = samples.len() / channels;
    let sr_f64 = sample_rate as f64;

    for f_idx in 0..total_frames {
        let t = (f_idx as f64) / sr_f64;
        let gain = compute_sidechain_ducking_gain(
            t,
            downbeats,
            config.ducking_amount_db,
            config.attack_ms,
            config.release_ms,
        );

        for ch in 0..channels {
            samples[f_idx * channels + ch] *= gain;
        }
    }
}

/// Resample and time-stretch source audio according to ProjectPlan time-curve.
/// Includes varispeed pitch shift, reverse/freeze gating (mute), and staccato cut fades.
pub fn resample_varispeed_audio(
    src_samples: &[f32],
    src_sample_rate: u32,
    plan: &ProjectPlan,
    target_duration: f64,
    target_sample_rate: u32,
    channels: usize,
    config: &AudioMixConfig,
) -> Vec<f32> {
    let total_target_frames = (target_duration * (target_sample_rate as f64)).round() as usize;
    let mut out_samples = vec![0.0f32; total_target_frames * channels];

    if src_samples.is_empty() || channels == 0 {
        return out_samples;
    }

    let src_total_frames = src_samples.len() / channels;

    for f_idx in 0..total_target_frames {
        let target_t = (f_idx as f64) / (target_sample_rate as f64);
        let (s_time, seg_idx) = compute_source_time_for_target_time(plan, target_t);

        // Check velocity / slope
        let is_muted = if seg_idx < plan.segments.len() {
            let seg = &plan.segments[seg_idx];
            // Freeze (s0 == s1) or Reverse (s1 < s0) -> Mute source audio
            (seg.s1 - seg.s0).abs() < 1e-6 || seg.s1 < seg.s0
        } else {
            false
        };

        if is_muted {
            continue; // Muted (samples remain 0.0)
        }

        // Varispeed linear interpolation
        let src_pos = s_time * (src_sample_rate as f64);
        let k = src_pos.floor() as usize;
        let frac = (src_pos - k as f64) as f32;

        let mut cut_gain = 1.0f32;
        if config.staccato_cuts && seg_idx < plan.segments.len() {
            let seg = &plan.segments[seg_idx];
            let dt_cut = (target_t - seg.t0).abs();
            if dt_cut < 0.002 {
                cut_gain = (dt_cut / 0.002) as f32;
            }
        }

        if k + 1 < src_total_frames {
            for ch in 0..channels {
                let s0 = src_samples[k * channels + ch];
                let s1 = src_samples[(k + 1) * channels + ch];
                let interpolated = s0 * (1.0 - frac) + s1 * frac;
                out_samples[f_idx * channels + ch] = interpolated * cut_gain;
            }
        } else if k < src_total_frames {
            for ch in 0..channels {
                out_samples[f_idx * channels + ch] = src_samples[k * channels + ch] * cut_gain;
            }
        }
    }

    out_samples
}
