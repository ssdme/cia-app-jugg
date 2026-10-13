//! Sparse musical accents, independent of the existing effect presets.
use crate::{creative::CreativeKind, effects::deterministic_hash_pos, plan::ProjectPlan};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CreativeOptions {
    pub time_slice: bool,
    pub ink_pulse: bool,
    pub prism_drift: bool,
    pub focus_pull: bool,
    pub relief_light: bool,
    pub paper_gate: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreativeEvent {
    pub t0: f64,
    pub t1: f64,
    pub kind: CreativeKind,
    pub seed: u32,
    pub strength: f64,
}

impl CreativeEvent {
    pub fn phase(&self, time: f64) -> f64 {
        ((time - self.t0) / (self.t1 - self.t0)).clamp(0.0, 1.0)
    }
}

fn anchors(times: &[f64], target: f64) -> Vec<f64> {
    let mut times: Vec<_> = times
        .iter()
        .copied()
        .filter(|time| time.is_finite() && *time >= 0.0 && *time < target)
        .collect();
    times.sort_by(f64::total_cmp);
    times.dedup_by(|a, b| (*a - *b).abs() < 0.02);
    times
}

pub fn apply_to_plan(
    plan: &mut ProjectPlan,
    options: CreativeOptions,
    beats: &[f64],
    downbeats: &[f64],
) -> Result<(), String> {
    plan.creative_events.clear();
    if options == CreativeOptions::default() {
        return Ok(());
    }
    let mut musical = anchors(beats, plan.target_duration);
    let period = (60.0 / plan.bpm.max(30.0)).clamp(0.2, 1.5);
    if musical.is_empty() {
        musical = anchors(
            &plan
                .segments
                .iter()
                .map(|segment| segment.t0)
                .collect::<Vec<_>>(),
            plan.target_duration,
        );
    }
    let mut strong = anchors(downbeats, plan.target_duration);
    if strong.is_empty() {
        strong = musical.iter().step_by(4).copied().collect();
    }
    let frame = 1.0 / plan.fps as f64;
    for (kind, enabled, salt) in [
        (CreativeKind::TimeSlice, options.time_slice, 0x19ab),
        (CreativeKind::InkPulse, options.ink_pulse, 0x72d1),
        (CreativeKind::PrismDrift, options.prism_drift, 0xc3e7),
        (CreativeKind::FocusPull, options.focus_pull, 0x3f4a),
        (CreativeKind::ReliefLight, options.relief_light, 0x59c2),
        (CreativeKind::PaperGate, options.paper_gate, 0xe13b),
    ] {
        if !enabled {
            continue;
        }
        let times = if kind == CreativeKind::PrismDrift {
            &musical
        } else {
            &strong
        };
        let mut last_end = -1.0;
        for (index, &anchor) in times.iter().enumerate() {
            let selected = match kind {
                CreativeKind::TimeSlice => index % 2 == 0,
                CreativeKind::InkPulse => index % 2 == 1 || times.len() == 1,
                CreativeKind::PrismDrift => index % 3 == 1 || times.len() == 1,
                CreativeKind::FocusPull => index % 3 == 0,
                CreativeKind::ReliefLight => index % 3 == 1 || times.len() == 1,
                CreativeKind::PaperGate => {
                    index % 3 == 2 || (times.len() < 3 && index == times.len() - 1)
                }
            };
            if !selected {
                continue;
            }
            let offset = if times.len() == 1 {
                period
                    * match kind {
                        CreativeKind::InkPulse => 0.4,
                        CreativeKind::ReliefLight => 0.35,
                        CreativeKind::PaperGate => 0.7,
                        _ => 0.0,
                    }
            } else {
                0.0
            };
            let latest = ((plan.target_duration / frame).floor() - 2.0).max(0.0) * frame;
            let t0 = (((anchor + offset) / frame).round() * frame).min(latest);
            let duration = match kind {
                CreativeKind::TimeSlice => period * 0.75,
                CreativeKind::InkPulse => period * 1.1,
                CreativeKind::PrismDrift => period * 0.9,
                CreativeKind::FocusPull => period,
                CreativeKind::ReliefLight => period * 1.2,
                CreativeKind::PaperGate => period * 0.9,
            }
            .max(frame * 2.0)
            .min(1.2);
            let t1 = (t0 + duration).min(plan.target_duration);
            let clean_gap = period
                * match kind {
                    CreativeKind::PrismDrift => 1.5,
                    CreativeKind::FocusPull
                    | CreativeKind::ReliefLight
                    | CreativeKind::PaperGate => 3.0,
                    _ => 2.0,
                };
            if (last_end >= 0.0 && t0 < last_end + clean_gap) || t1 - t0 <= frame {
                continue;
            }
            let segment = plan
                .segments
                .iter()
                .find(|segment| t0 >= segment.t0 && t0 < segment.t1)
                .unwrap_or(&plan.segments[0]);
            let source = segment.s0
                + (segment.s1 - segment.s0)
                    * segment
                        .sample_curve((t0 - segment.t0) / (segment.t1 - segment.t0))
                        .0;
            let seed =
                deterministic_hash_pos(anchor + source + plan.video_duration * 0.17, salt) as u32;
            let strength = (if plan.style == "SMOOTH" { 0.72 } else { 0.9 })
                * (0.9 + (seed % 11) as f64 * 0.01);
            plan.creative_events.push(CreativeEvent {
                t0,
                t1,
                kind,
                seed,
                strength,
            });
            last_end = t1;
        }
    }
    plan.creative_events.sort_by(|a, b| a.t0.total_cmp(&b.t0));
    validate(&plan.creative_events, plan.target_duration)
}

pub fn validate(events: &[CreativeEvent], target: f64) -> Result<(), String> {
    if events.len() > 100_000 {
        return Err("Too many creative accents".into());
    }
    let mut last_start = -1.0;
    let mut ends = [0.0; CreativeKind::COUNT];
    for event in events {
        let index = event.kind.slot();
        if !event.t0.is_finite()
            || !event.t1.is_finite()
            || !event.strength.is_finite()
            || event.t0 < 0.0
            || event.t1 <= event.t0
            || event.t1 > target + 1e-7
            || event.t1 - event.t0 > 1.5
            || event.t0 < last_start
            || event.t0 < ends[index] - 1e-7
            || !(0.0..=1.0).contains(&event.strength)
        {
            return Err("Invalid creative accent".into());
        }
        last_start = event.t0;
        ends[index] = event.t1;
    }
    Ok(())
}
