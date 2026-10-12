//! Deterministic shot selection with motion, pacing and recurrence costs.
use crate::{
    plan::{PlanSegment, ProjectPlan, RemapEnvelope},
    scene_analysis::PreparedScene,
    sources::SourceRange,
};
use serde::{Deserialize, Serialize};

const RECURRENCE_TARGET: f64 = 8.0;
const MIN_RATE: f64 = 0.67;
const MAX_RATE: f64 = 1.5;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ShotDiagnostic {
    pub scene_id: usize,
    pub source_id: usize,
    pub t0: f64,
    pub t1: f64,
    pub ranges: Vec<SourceRange>,
    pub reused_fraction: f64,
    pub activity: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SourceStamp {
    pub source_id: usize,
    pub fingerprint: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlanningMetadata {
    pub version: u32,
    pub mode: String,
    pub no_repetitive_scene: bool,
    pub motion_guided: bool,
    pub source_fingerprints: Vec<SourceStamp>,
    pub shots: Vec<ShotDiagnostic>,
}

#[derive(Clone, Debug)]
struct Shot {
    scene: usize,
    start: f64,
    span: f64,
    t0: f64,
    t1: f64,
    reused: f64,
    activity: f64,
    score: f64,
}
#[derive(Clone, Default)]
struct Usage {
    covered: Vec<SourceRange>,
    recent: Vec<ShotDiagnostic>,
}

struct SceneView<'a> {
    scene: &'a PreparedScene,
    base: f64,
    duration: f64,
}
impl SceneView<'_> {
    fn source_time(&self, time: f64) -> f64 {
        let mut offset = 0.0;
        for range in &self.scene.fragments {
            if time <= offset + range.duration + 1e-9 {
                return range.start + (time - offset).clamp(0.0, range.duration);
            }
            offset += range.duration;
        }
        self.scene.source_end
    }
    fn local_time(&self, source_time: f64) -> Option<f64> {
        let mut offset = 0.0;
        for range in &self.scene.fragments {
            if source_time >= range.start && source_time <= range.start + range.duration {
                return Some(offset + source_time - range.start);
            }
            offset += range.duration;
        }
        None
    }
    fn pieces(&self, start: f64, span: f64) -> Vec<SourceRange> {
        let mut offset = 0.0;
        let mut pieces = Vec::new();
        for range in &self.scene.fragments {
            let from = start.max(offset);
            let to = (start + span).min(offset + range.duration);
            if to > from + 1e-9 {
                pieces.push(SourceRange {
                    source_id: range.source_id,
                    start: range.start + from - offset,
                    duration: to - from,
                });
            }
            offset += range.duration;
        }
        pieces
    }
    fn activity(&self, start: f64, span: f64) -> (f64, f64) {
        let mut sum = 0.0;
        let mut active = 0.0;
        let mut count = 0.0;
        for piece in self.pieces(start, span) {
            let first = self
                .scene
                .motion
                .partition_point(|sample| sample.time < piece.start);
            let last = self
                .scene
                .motion
                .partition_point(|sample| sample.time <= piece.start + piece.duration);
            for sample in &self.scene.motion[first..last] {
                let value = sample.activity * sample.confidence;
                sum += (value / 0.18).min(1.0);
                if value > 0.015 {
                    active += 1.0;
                }
                count += 1.0;
            }
        }
        if count > 0.0 {
            (sum / count, active / count)
        } else {
            (0.0, 0.0)
        }
    }
}

fn overlap(pieces: &[SourceRange], covered: &[SourceRange]) -> f64 {
    pieces
        .iter()
        .map(|piece| {
            covered
                .iter()
                .filter(|used| used.source_id == piece.source_id)
                .map(|used| {
                    (piece.start + piece.duration).min(used.start + used.duration)
                        - piece.start.max(used.start)
                })
                .map(|duration| duration.max(0.0))
                .sum::<f64>()
                .min(piece.duration)
        })
        .sum()
}
fn add_coverage(covered: &mut Vec<SourceRange>, pieces: &[SourceRange]) {
    covered.extend_from_slice(pieces);
    covered.sort_by(|a, b| {
        a.source_id
            .cmp(&b.source_id)
            .then(a.start.total_cmp(&b.start))
    });
    let mut merged: Vec<SourceRange> = Vec::new();
    for range in covered.drain(..) {
        if let Some(last) = merged.last_mut() {
            if last.source_id == range.source_id && range.start <= last.start + last.duration + 1e-7
            {
                last.duration =
                    (last.start + last.duration).max(range.start + range.duration) - last.start;
                continue;
            }
        }
        merged.push(range);
    }
    *covered = merged;
}
fn diagnostic(shot: &Shot, views: &[SceneView<'_>]) -> ShotDiagnostic {
    let view = &views[shot.scene];
    ShotDiagnostic {
        scene_id: view.scene.scene_id,
        source_id: view.scene.source_id,
        t0: shot.t0,
        t1: shot.t1,
        ranges: view.pieces(shot.start, shot.span),
        reused_fraction: shot.reused,
        activity: shot.activity,
    }
}
fn commit(usage: &mut Usage, shot: &Shot, views: &[SceneView<'_>]) {
    let entry = diagnostic(shot, views);
    add_coverage(&mut usage.covered, &entry.ranges);
    usage
        .recent
        .retain(|recent| shot.t1 - recent.t1 < RECURRENCE_TARGET * 2.0);
    usage.recent.push(entry);
}
fn first_unused(view: &SceneView<'_>, coverage: &[SourceRange]) -> Option<f64> {
    let mut offset = 0.0;
    for fragment in &view.scene.fragments {
        let mut cursor = fragment.start;
        for covered in coverage
            .iter()
            .filter(|range| range.source_id == fragment.source_id)
        {
            if covered.start > cursor + 0.04 {
                break;
            }
            if covered.start + covered.duration > cursor {
                cursor = cursor.max(covered.start + covered.duration);
            }
        }
        if cursor < fragment.start + fragment.duration - 0.04 {
            return Some(offset + cursor - fragment.start);
        }
        offset += fragment.duration;
    }
    None
}
fn nearest_boundary(time: f64, boundaries: &[f64], target: f64) -> f64 {
    let index = boundaries.partition_point(|&boundary| boundary < time);
    let before = index
        .checked_sub(1)
        .and_then(|index| boundaries.get(index))
        .copied();
    let after = boundaries.get(index).copied();
    before
        .into_iter()
        .chain(after)
        .min_by(|a, b| (a - time).abs().total_cmp(&(b - time).abs()))
        .unwrap_or(time)
        .min(target)
}

struct Context<'a> {
    views: Vec<SceneView<'a>>,
    beats: &'a [f64],
    downbeats: &'a [f64],
    target: f64,
    preferred: f64,
    base_rate: f64,
    long_clip: bool,
}
fn candidates(
    context: &Context<'_>,
    usage: &Usage,
    time: f64,
    previous: Option<&Shot>,
) -> Vec<Shot> {
    let remaining = context.target - time;
    let mut scene_order: Vec<_> = (0..context.views.len()).collect();
    scene_order.sort_by(|&a, &b| {
        let value = |index: usize| {
            let view = &context.views[index];
            let unseen = 1.0 - overlap(&view.scene.fragments, &usage.covered) / view.duration;
            let recent = usage.recent.iter().rev().find(|shot| {
                shot.source_id == view.scene.source_id && shot.scene_id == view.scene.scene_id
            });
            let novelty = if recent.is_some() { 0.0 } else { 0.5 };
            unseen
                + novelty
                + view
                    .scene
                    .actions
                    .iter()
                    .map(|action| action.activity)
                    .fold(0.0, f64::max)
        };
        value(b).total_cmp(&value(a)).then(a.cmp(&b))
    });
    scene_order.truncate(48);
    if let Some(previous) = previous {
        if !scene_order.contains(&previous.scene) {
            scene_order.push(previous.scene);
        }
    }
    let mut result = Vec::new();
    for index in scene_order {
        let view = &context.views[index];
        let mut starts = vec![0.0];
        if let Some(start) = first_unused(view, &usage.covered) {
            starts.push(start);
        }
        if let Some(previous) = previous.filter(|previous| previous.scene == index) {
            if previous.start + previous.span < view.duration - 0.03 {
                starts.push(previous.start + previous.span);
            }
        }
        let mut actions: Vec<_> = view.scene.actions.iter().collect();
        actions.sort_by(|a, b| {
            b.activity
                .total_cmp(&a.activity)
                .then(a.start.total_cmp(&b.start))
        });
        for action in actions.into_iter().take(4) {
            if let Some(start) = view.local_time(action.start) {
                starts.push(start);
            }
        }
        if context.long_clip {
            // Search each part of a long scene, including sustained actions with only one broad window.
            for bin in 0..8 {
                let from = view.duration * bin as f64 / 8.0;
                let to = view.duration * (bin + 1) as f64 / 8.0;
                let first = view
                    .scene
                    .motion
                    .partition_point(|sample| sample.time < view.source_time(from));
                let last = view
                    .scene
                    .motion
                    .partition_point(|sample| sample.time < view.source_time(to));
                let peak = view.scene.motion[first..last].iter().max_by(|a, b| {
                    (a.activity * a.confidence).total_cmp(&(b.activity * b.confidence))
                });
                let start = peak
                    .and_then(|sample| view.local_time(sample.time))
                    .unwrap_or(from);
                starts.push((start - 0.35).max(from));
            }
        }
        starts.sort_by(f64::total_cmp);
        starts.dedup_by(|a, b| (*a - *b).abs() < 0.03);
        for start in starts {
            let available = view.duration - start;
            if available < 0.001 {
                continue;
            }
            let mut lengths = vec![
                context.preferred * 0.65,
                context.preferred,
                context.preferred * 1.5,
                available / context.base_rate,
            ];
            for action in view
                .scene
                .actions
                .iter()
                .filter(|action| action.end > view.source_time(start))
                .take(4)
            {
                if let Some(end) = view.local_time(action.end) {
                    lengths.push((end - start) / context.base_rate);
                }
            }
            lengths.push(remaining);
            let mut ends: Vec<_> = lengths
                .into_iter()
                .filter(|length| *length > 0.04)
                .map(|length| {
                    let wanted = time + length.min(available / MIN_RATE).min(remaining);
                    nearest_boundary(wanted, context.beats, context.target)
                })
                .filter(|end| *end > time + 0.025 && *end - time <= available / MIN_RATE + 0.001)
                .collect();
            if remaining <= available / MIN_RATE {
                ends.push(context.target);
            }
            if ends.is_empty() {
                ends.push((time + available / context.base_rate).min(context.target));
            }
            ends.sort_by(f64::total_cmp);
            ends.dedup_by(|a, b| (*a - *b).abs() < 0.01);
            for end in ends {
                let duration = end - time;
                let mut spans = vec![
                    (duration * context.base_rate).min(available),
                    duration.min(available),
                ];
                if available / duration <= MAX_RATE {
                    spans.push(available);
                }
                for action in view.scene.actions.iter().take(8) {
                    if let Some(action_end) = view.local_time(action.end) {
                        if action_end > start {
                            spans.push(action_end - start);
                        }
                    }
                }
                spans.sort_by(f64::total_cmp);
                spans.dedup_by(|a, b| (*a - *b).abs() < 0.02);
                for span in spans {
                    let rate = span / duration;
                    if span <= 1e-6
                        || span > available + 1e-6
                        || !(MIN_RATE - 0.001..=MAX_RATE).contains(&rate)
                    {
                        continue;
                    }
                    let pieces = view.pieces(start, span);
                    let reused = (overlap(&pieces, &usage.covered) / span).clamp(0.0, 1.0);
                    let (activity, active_fraction) = view.activity(start, span);
                    let continuation = previous.is_some_and(|previous| {
                        previous.scene == index
                            && (previous.start + previous.span - start).abs() < 0.03
                    });
                    let recent = usage.recent.iter().rev().find(|shot| {
                        shot.source_id == view.scene.source_id
                            && shot.scene_id == view.scene.scene_id
                    });
                    let recurrence = 8.0 / (1.0 + ((time - RECURRENCE_TARGET) / 1.5).exp());
                    let replay_recency = usage
                        .recent
                        .iter()
                        .filter(|shot| overlap(&pieces, &shot.ranges) > 0.001)
                        .map(|shot| 4.0 * (-(time - shot.t1).max(0.0) / RECURRENCE_TARGET).exp())
                        .fold(0.0, f64::max);
                    let scene_recency = recent
                        .map(|shot| {
                            (if context.long_clip { 0.3 } else { 1.3 })
                                * (-(time - shot.t1).max(0.0) / 4.0).exp()
                        })
                        .unwrap_or(0.0);
                    let pacing = ((duration / context.preferred).ln()).abs();
                    let quiet = (1.0 - active_fraction) * (duration / context.preferred).min(2.0);
                    let musical = if context
                        .downbeats
                        .iter()
                        .any(|beat| (beat - end).abs() < 0.03)
                    {
                        0.3
                    } else {
                        0.1
                    };
                    let action_cut = view.scene.actions.iter().any(|action| {
                        let source_end = view.source_time(start + span);
                        source_end > action.start
                            && source_end < action.peak + 0.15
                            && source_end < action.end - 0.05
                    });
                    let spread_cost = if context.long_clip {
                        let total = context
                            .views
                            .last()
                            .map(|view| view.base + view.duration)
                            .unwrap_or(1.0);
                        let source_progress = (view.base + start + span * 0.5) / total;
                        5.0 * (source_progress - (time + duration * 0.5) / context.target).abs()
                    } else {
                        0.0
                    };
                    let score = 2.5 * activity
                        + 0.9 * active_fraction
                        + 1.0 * (1.0 - reused) * (span / context.preferred).min(2.0)
                        + if recent.is_none() { 0.65 } else { 0.0 }
                        + if continuation {
                            if context.long_clip {
                                0.3
                            } else {
                                1.1
                            }
                        } else {
                            0.0
                        }
                        + musical
                        - reused * (4.0 + recurrence + replay_recency)
                        - if continuation { 0.0 } else { scene_recency }
                        - 0.6 * pacing
                        - 1.5 * quiet
                        - 1.8 * (rate / context.base_rate).ln().abs()
                        - if action_cut { 0.9 } else { 0.0 }
                        - spread_cost;
                    result.push(Shot {
                        scene: index,
                        start,
                        span,
                        t0: time,
                        t1: end,
                        reused,
                        activity,
                        score,
                    });
                }
            }
        }
    }
    result.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then(a.scene.cmp(&b.scene))
            .then(a.start.total_cmp(&b.start))
            .then(a.t1.total_cmp(&b.t1))
    });
    result.truncate(6);
    result
}

fn adaptive_shots(context: &Context<'_>) -> Result<Vec<Shot>, String> {
    let mut usage = Usage::default();
    let mut shots = Vec::new();
    let mut time = 0.0;
    while time < context.target - 1e-7 {
        let choices = candidates(context, &usage, time, shots.last());
        let mut best: Option<(f64, Shot)> = None;
        for candidate in choices {
            let mut preview = usage.clone();
            commit(&mut preview, &candidate, &context.views);
            let future = if candidate.t1 < context.target - 0.025 {
                candidates(context, &preview, candidate.t1, Some(&candidate))
                    .first()
                    .map(|shot| shot.score)
                    .unwrap_or(-8.0)
            } else {
                0.0
            };
            let score = candidate.score + future * 0.35;
            if best.as_ref().is_none_or(|(previous, _)| score > *previous) {
                best = Some((score, candidate));
            }
        }
        let shot = best
            .map(|(_, shot)| shot)
            .ok_or("Cannot build a valid scene schedule")?;
        time = shot.t1;
        commit(&mut usage, &shot, &context.views);
        shots.push(shot);
        if shots.len() > 100_000 {
            return Err("Scene schedule exceeds the supported segment count".into());
        }
    }
    Ok(shots)
}

fn legacy_shots(plan: &ProjectPlan, views: &[SceneView<'_>]) -> Vec<Shot> {
    let mut shots: Vec<Shot> = Vec::new();
    for segment in &plan.segments {
        let start = segment.s0.min(segment.s1);
        let end = segment.s0.max(segment.s1);
        let index = views
            .partition_point(|view| view.base + view.duration <= start + 1e-7)
            .min(views.len() - 1);
        let view = &views[index];
        if let Some(previous) = shots.last_mut() {
            if previous.scene == index
                && (previous.start + previous.span - (start - view.base)).abs() < 0.003
            {
                previous.span = end - view.base - previous.start;
                previous.t1 = segment.t1;
                continue;
            }
        }
        shots.push(Shot {
            scene: index,
            start: start - view.base,
            span: end - start,
            t0: segment.t0,
            t1: segment.t1,
            reused: 0.0,
            activity: 0.0,
            score: 0.0,
        });
    }
    let mut usage = Usage::default();
    for shot in &mut shots {
        let view = &views[shot.scene];
        if let Some(action) = view
            .scene
            .actions
            .iter()
            .max_by(|a, b| a.activity.total_cmp(&b.activity))
        {
            if let Some(start) = view.local_time(action.start) {
                shot.start = start.min((view.duration - shot.span).max(0.0));
            }
        }
        shot.span = shot.span.min(view.duration - shot.start);
        shot.activity = view.activity(shot.start, shot.span).0;
        shot.reused = (overlap(&view.pieces(shot.start, shot.span), &usage.covered) / shot.span)
            .clamp(0.0, 1.0);
        commit(&mut usage, shot, views);
    }
    shots
}

fn inverse_curve(curve: &str, strength: f64, value: f64) -> f64 {
    let mut low = 0.0;
    let mut high = 1.0;
    for _ in 0..40 {
        let middle = (low + high) * 0.5;
        let mapped =
            middle * (1.0 - strength) + strength * crate::effects::evaluate_curve(curve, middle);
        if mapped < value {
            low = middle;
        } else {
            high = middle;
        }
    }
    (low + high) * 0.5
}

fn materialize(plan: &ProjectPlan, shots: &[Shot], context: &Context<'_>) -> Vec<PlanSegment> {
    let mut result = Vec::new();
    let mut template_index = 0;
    for shot in shots {
        let view = &context.views[shot.scene];
        let mut times = vec![shot.t0];
        times.extend(plan.segments.iter().filter_map(|segment| {
            (segment.t0 > shot.t0 + 1e-7 && segment.t0 < shot.t1 - 1e-7).then_some(segment.t0)
        }));
        times.push(shot.t1);
        let mut positions = vec![shot.start];
        for index in 1..times.len() - 1 {
            let expected = shot.start + shot.span * (times[index] - shot.t0) / (shot.t1 - shot.t0);
            let step = shot.span * (times[index] - times[index - 1]) / (shot.t1 - shot.t0);
            let min = positions[index - 1] + step * 0.25;
            let max = shot.start + shot.span
                - (shot.t1 - times[index]) / (shot.t1 - shot.t0) * shot.span * 0.35;
            let radius = ((times[index] - times[index - 1]) * 0.35).min(0.45);
            let source_expected = view.source_time(expected);
            let first = view
                .scene
                .motion
                .partition_point(|sample| sample.time < source_expected - radius);
            let last = view
                .scene
                .motion
                .partition_point(|sample| sample.time <= source_expected + radius);
            let anchor = view.scene.motion[first..last]
                .iter()
                .filter_map(|sample| {
                    let local = view.local_time(sample.time)?;
                    (local >= min && local <= max && sample.activity * sample.confidence > 0.015)
                        .then_some((
                            sample.activity * sample.confidence - (local - expected).abs() * 0.03,
                            local,
                        ))
                })
                .max_by(|a, b| a.0.total_cmp(&b.0));
            positions.push(
                anchor
                    .map(|(_, local)| local)
                    .unwrap_or(expected)
                    .clamp(min, max),
            );
        }
        positions.push(shot.start + shot.span);
        for index in 0..times.len() - 1 {
            let t0 = times[index];
            let t1 = times[index + 1];
            while template_index + 1 < plan.segments.len()
                && plan.segments[template_index].t1 <= t0 + 1e-7
            {
                template_index += 1;
            }
            let template = &plan.segments[template_index];
            let mut s0 = view.base + positions[index];
            let mut s1 = view.base + positions[index + 1];
            if template.effects.reverse {
                std::mem::swap(&mut s0, &mut s1);
            }
            let end_activity =
                view.scene
                    .activity_at(view.source_time(if template.effects.reverse {
                        positions[index]
                    } else {
                        positions[index + 1]
                    }));
            let strength = if end_activity < 0.015 {
                0.0
            } else {
                (end_activity / 0.18).min(0.85)
            };
            let mut phases = vec![0.0];
            let mut boundary = view.base;
            for fragment in &view.scene.fragments {
                boundary += fragment.duration;
                if boundary > s0.min(s1) + 1e-7 && boundary < s0.max(s1) - 1e-7 {
                    phases.push(inverse_curve(
                        &template.curve,
                        strength,
                        (boundary - s0) / (s1 - s0),
                    ));
                }
            }
            phases.push(1.0);
            phases.sort_by(f64::total_cmp);
            for phase in phases.windows(2) {
                let map = |x| {
                    x * (1.0 - strength)
                        + strength * crate::effects::evaluate_curve(&template.curve, x)
                };
                let mut segment = template.clone();
                segment.t0 = t0 + (t1 - t0) * phase[0];
                segment.t1 = t0 + (t1 - t0) * phase[1];
                segment.s0 = s0 + (s1 - s0) * map(phase[0]);
                segment.s1 = s0 + (s1 - s0) * map(phase[1]);
                segment.transition = None;
                segment.remap = Some(RemapEnvelope {
                    strength,
                    phase_start: phase[0],
                    phase_end: phase[1],
                });
                result.push(segment);
            }
        }
    }
    result
}

pub fn validate_prepared(scenes: &[PreparedScene]) -> Result<(), String> {
    if scenes.is_empty() || scenes.len() > 100_000 {
        return Err("No usable scenes selected".into());
    }
    for scene in scenes {
        if !scene.source_start.is_finite()
            || !scene.source_end.is_finite()
            || scene.source_start < 0.0
            || scene.source_end <= scene.source_start
            || scene.fragments.is_empty()
            || !scene.fps.is_finite()
            || scene.fps <= 0.0
        {
            return Err("Invalid prepared scene".into());
        }
        let mut last_end = scene.source_start;
        for range in &scene.fragments {
            if range.source_id != scene.source_id
                || !range.start.is_finite()
                || !range.duration.is_finite()
                || range.start < last_end - 1e-7
                || range.duration <= 0.0
                || range.start + range.duration > scene.source_end + 0.051
            {
                return Err("Invalid prepared source fragment".into());
            }
            last_end = range.start + range.duration;
        }
        let mut previous_time = -1.0;
        for sample in &scene.motion {
            if !sample.time.is_finite()
                || sample.time < previous_time
                || sample.time < scene.source_start
                || sample.time > scene.source_end
                || !sample.activity.is_finite()
                || !(0.0..=1.0).contains(&sample.activity)
                || !sample.confidence.is_finite()
                || !(0.0..=1.0).contains(&sample.confidence)
            {
                return Err("Invalid motion profile".into());
            }
            previous_time = sample.time;
        }
        for action in &scene.actions {
            if [action.start, action.end, action.peak, action.activity]
                .iter()
                .any(|value| !value.is_finite())
                || action.start < scene.source_start - 1e-4
                || action.end > scene.source_end + 1e-4
                || action.end <= action.start
                || action.peak < action.start - 1e-4
                || action.peak > action.end + 1e-4
                || action.activity < -1e-4
                || action.activity > 1.0 + 1e-4
            {
                return Err("Invalid action window".into());
            }
        }
    }
    Ok(())
}

pub fn apply_schedule(
    plan: &mut ProjectPlan,
    scenes: &[PreparedScene],
    beats: &[f64],
    downbeats: &[f64],
    rhythm: &str,
    adaptive: bool,
    mode: &str,
) -> Result<(), String> {
    validate_prepared(scenes)?;
    let mut base = 0.0;
    let views: Vec<_> = scenes
        .iter()
        .map(|scene| {
            let duration = scene.duration();
            let view = SceneView {
                scene,
                base,
                duration,
            };
            base += duration;
            view
        })
        .collect();
    let mut boundaries: Vec<_> = beats
        .iter()
        .copied()
        .chain(downbeats.iter().copied())
        .filter(|time| time.is_finite() && *time > 0.0 && *time < plan.target_duration)
        .collect();
    boundaries.push(plan.target_duration);
    boundaries.sort_by(f64::total_cmp);
    boundaries.dedup_by(|a, b| (*a - *b).abs() < 0.01);
    let beat_period = if beats.len() > 1 {
        (beats[1] - beats[0]).clamp(0.2, 1.5)
    } else {
        (60.0 / plan.bpm.max(30.0)).clamp(0.2, 1.5)
    };
    let cadence = match rhythm {
        "fast" => beat_period,
        "slow" => (beat_period * 8.0).max(3.0),
        _ => beat_period * 4.0,
    };
    let scarcity_floor = (RECURRENCE_TARGET / scenes.len() as f64).min(5.5);
    let preferred = cadence
        .max(scarcity_floor)
        .min(6.0)
        .min(plan.target_duration);
    let context = Context {
        views,
        beats: &boundaries,
        downbeats,
        target: plan.target_duration,
        preferred,
        base_rate: if plan.style == "SMOOTH" { 0.9 } else { 1.0 },
        long_clip: mode == "long_clip",
    };
    let shots = if adaptive {
        adaptive_shots(&context)?
    } else {
        legacy_shots(plan, &context.views)
    };
    let segments = materialize(plan, &shots, &context);
    let source_fingerprints: Vec<_> =
        scenes
            .iter()
            .fold(Vec::new(), |mut stamps: Vec<SourceStamp>, scene| {
                if !stamps
                    .iter()
                    .any(|stamp| stamp.source_id == scene.source_id)
                {
                    stamps.push(SourceStamp {
                        source_id: scene.source_id,
                        fingerprint: scene.fingerprint.clone(),
                    });
                }
                stamps
            });
    plan.planning = Some(PlanningMetadata {
        version: 1,
        mode: mode.into(),
        no_repetitive_scene: mode == "scenepack" && adaptive,
        motion_guided: true,
        source_fingerprints,
        shots: shots
            .iter()
            .map(|shot| diagnostic(shot, &context.views))
            .collect(),
    });
    plan.loops = shots.iter().filter(|shot| shot.reused > 0.5).count() as u32;
    plan.segments = segments;
    plan.source_ranges = Some(
        scenes
            .iter()
            .flat_map(|scene| scene.fragments.clone())
            .collect(),
    );
    plan.source_intervals = None;
    plan.video_duration = base;
    plan.schema_version = 4;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        plan::*,
        scene_analysis::{ActionWindow, MotionSample},
    };
    fn scene(id: usize, duration: f64, moving: bool) -> PreparedScene {
        PreparedScene {
            scene_id: id,
            source_id: id,
            source_start: 0.0,
            source_end: duration,
            fingerprint: String::new(),
            fps: 30.0,
            fragments: vec![SourceRange {
                source_id: id,
                start: 0.0,
                duration,
            }],
            motion: (0..(duration * 12.0) as usize)
                .map(|index| MotionSample {
                    time: index as f64 / 12.0,
                    activity: if moving { 0.3 } else { 0.0 },
                    confidence: if moving { 0.9 } else { 0.0 },
                })
                .collect(),
            actions: if moving {
                vec![ActionWindow {
                    start: 0.0,
                    end: duration,
                    peak: duration * 0.6,
                    activity: 0.27,
                }]
            } else {
                Vec::new()
            },
        }
    }
    #[test]
    fn action_windows_accept_floating_point_boundary_rounding() {
        let mut clip = scene(0, 4.3, true);
        clip.actions[0].start = -1e-15;
        clip.actions[0].end = clip.source_end + 1e-15;
        clip.actions[0].peak = clip.actions[0].end;
        validate_prepared(&[clip]).expect("Boundary rounding must not stop rendering");
    }
    #[test]
    fn action_windows_reject_real_overruns_and_nonfinite_values() {
        for (end, peak, activity) in [(4.301, 2.0, 0.5), (4.3, f64::NAN, 0.5), (4.3, 2.0, 1.01)] {
            let mut clip = scene(0, 4.3, true);
            clip.actions[0].end = end;
            clip.actions[0].peak = peak;
            clip.actions[0].activity = activity;
            assert_eq!(
                validate_prepared(&[clip]).unwrap_err(),
                "Invalid action window"
            );
        }
    }
    fn make_plan(scenes: &[PreparedScene], duration: f64) -> ProjectPlan {
        make_plan_mode(scenes, duration, "scenepack")
    }
    fn make_plan_mode(scenes: &[PreparedScene], duration: f64, mode: &str) -> ProjectPlan {
        let beats: Vec<_> = (1..(duration * 2.0) as usize)
            .map(|index| index as f64 / 2.0)
            .collect();
        let downbeats: Vec<_> = beats.iter().step_by(4).copied().collect();
        let mut plan = create_plan_internal(
            "HARD",
            24,
            &beats,
            &downbeats,
            duration + 2.0,
            duration,
            320,
            180,
            120.0,
            false,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        apply_schedule(&mut plan, scenes, &beats, &downbeats, "fast", true, mode).unwrap();
        validate_plan(&plan).unwrap();
        plan
    }
    #[test]
    fn scarce_unequal_scenes_use_fresh_motion_before_replay() {
        let scenes = vec![
            scene(0, 4.3, true),
            scene(1, 5.7, true),
            scene(2, 3.6, true),
        ];
        let plan = make_plan(&scenes, 19.3);
        let shots = &plan.planning.as_ref().unwrap().shots;
        assert!(shots.iter().take(3).all(|shot| shot.reused_fraction < 0.01));
        assert!(shots
            .iter()
            .find(|shot| shot.reused_fraction > 0.5)
            .is_none_or(|shot| shot.t0 >= 7.0));
        assert!(shots.iter().any(|shot| shot.t1 - shot.t0 > 2.0));
        assert_eq!(
            serde_json::to_string(&plan).unwrap(),
            serde_json::to_string(&make_plan(&scenes, 19.3)).unwrap()
        );
    }
    #[test]
    fn short_source_keeps_bounded_speed_and_static_remap_is_linear() {
        let plan = make_plan(&[scene(0, 1.4, false)], 13.7);
        assert!(plan.planning.as_ref().unwrap().shots.iter().all(|shot| {
            let span: f64 = shot.ranges.iter().map(|range| range.duration).sum();
            span / (shot.t1 - shot.t0) >= MIN_RATE - 0.001
        }));
        assert!(plan
            .segments
            .iter()
            .all(|segment| segment.remap.as_ref().unwrap().strength == 0.0));
    }
    #[test]
    fn long_clip_uses_motion_across_the_source() {
        let mut clip = scene(0, 90.0, true);
        for sample in &mut clip.motion {
            sample.activity = if (sample.time % 15.0) > 9.0 {
                0.6
            } else {
                0.02
            };
        }
        clip.actions.clear();
        let plan = make_plan_mode(&[clip], 20.0, "long_clip");
        let shots = &plan.planning.as_ref().unwrap().shots;
        assert!(shots
            .iter()
            .flat_map(|shot| &shot.ranges)
            .any(|range| range.start > 45.0));
        assert!(shots.iter().all(|shot| shot.activity > 0.1));
        assert!(shots.iter().all(|shot| shot.reused_fraction < 0.01));
    }
    #[test]
    fn motion_anchors_keep_positive_source_advance() {
        let mut clip = scene(0, 5.7, true);
        for sample in &mut clip.motion {
            sample.activity = if (sample.time % 0.5) > 0.3 { 0.7 } else { 0.03 };
        }
        let plan = make_plan(&[clip], 11.3);
        for segment in &plan.segments {
            assert!((segment.s1 - segment.s0).abs() > 1e-7);
            let mut previous = segment.sample_curve(0.0).0;
            for index in 1..=100 {
                let (value, derivative) = segment.sample_curve(index as f64 / 100.0);
                assert!(value > previous);
                assert!(derivative > 0.0);
                previous = value;
            }
        }
    }
    #[test]
    fn overlaps_across_parent_scenes_are_still_replays() {
        let pieces = vec![SourceRange {
            source_id: 0,
            start: 3.0,
            duration: 4.0,
        }];
        let mut covered = vec![SourceRange {
            source_id: 0,
            start: 1.0,
            duration: 4.0,
        }];
        assert_eq!(overlap(&pieces, &covered), 2.0);
        add_coverage(&mut covered, &pieces);
        assert_eq!(covered[0].duration, 6.0);
    }
    #[test]
    fn blank_gaps_keep_parent_identity_and_segment_source_bounds() {
        let mut scene = scene(0, 6.0, true);
        scene.fragments = vec![
            SourceRange {
                source_id: 0,
                start: 0.0,
                duration: 2.0,
            },
            SourceRange {
                source_id: 0,
                start: 3.0,
                duration: 3.0,
            },
        ];
        scene
            .motion
            .retain(|sample| sample.time < 2.0 || sample.time >= 3.0);
        let plan = make_plan(&[scene], 10.3);
        assert!(plan
            .planning
            .as_ref()
            .unwrap()
            .shots
            .iter()
            .all(|shot| shot.scene_id == 0));
        assert!(plan
            .segments
            .iter()
            .all(|segment| segment.s0.max(segment.s1) <= 2.0 + 1e-6
                || segment.s0.min(segment.s1) >= 2.0 - 1e-6));
    }
}
