//! Cached source-local frame filtering and visual activity profiles.
use crate::{
    analysis::{check_cancelled, fingerprint, AnalysisJob, AnalysisPermit},
    probe::{get_ffmpeg_binary, get_ffprobe_binary, probe_media_internal, MediaInfo},
    process::ManagedChild,
    sources::SourceRange,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    io::Read,
    path::Path,
    process::{Command, Stdio},
    sync::{atomic::AtomicBool, mpsc, Arc, Mutex, OnceLock},
    time::Duration,
};
use tauri::Emitter;

const SAMPLE_RATE: f64 = 12.0;
const CACHE_BYTES: usize = 32 * 1024 * 1024;
const MAX_SAMPLES: usize = 1_100_000;
const VERSION: u32 = 3;

struct FrameLayout {
    format: String,
    bytes: usize,
    sample_bytes: usize,
    plane_bytes: usize,
    black: u16,
    white: u16,
    chroma: Option<u16>,
    depth: u32,
    rgb: bool,
    classify: bool,
}
impl FrameLayout {
    fn rgb(width: usize, height: usize, depth: u32) -> Self {
        let sample_bytes = if depth > 8 { 2 } else { 1 };
        let plane_bytes = width * height * sample_bytes;
        Self {
            format: if depth == 8 {
                "gbrp".into()
            } else {
                format!("gbrp{depth}le")
            },
            bytes: plane_bytes * 3,
            sample_bytes,
            plane_bytes,
            black: 0,
            white: ((1u32 << depth) - 1) as u16,
            chroma: None,
            depth,
            rgb: true,
            classify: true,
        }
    }
    fn native(format: &str, range: &str, width: usize, height: usize) -> Self {
        let depth = |prefix: &str| {
            format
                .strip_prefix(prefix)
                .and_then(|tail| {
                    tail.chars()
                        .take_while(char::is_ascii_digit)
                        .collect::<String>()
                        .parse::<u32>()
                        .ok()
                })
                .filter(|depth| (8..=16).contains(depth))
                .unwrap_or(8)
        };
        for (prefix, horizontal, vertical) in [
            ("yuv420p", 2, 2),
            ("yuv422p", 2, 1),
            ("yuv444p", 1, 1),
            ("yuv440p", 1, 2),
            ("yuv411p", 4, 1),
            ("yuv410p", 4, 4),
            ("yuvj420p", 2, 2),
            ("yuvj422p", 2, 1),
            ("yuvj444p", 1, 1),
        ] {
            if format.starts_with(prefix) {
                let depth = depth(prefix);
                let sample_bytes = if depth > 8 { 2 } else { 1 };
                let plane_bytes = width * height * sample_bytes;
                let full = range == "pc" || format.starts_with("yuvj");
                return Self {
                    format: if depth == 8 {
                        prefix.into()
                    } else {
                        format!("{prefix}{depth}le")
                    },
                    bytes: plane_bytes
                        + 2 * width.div_ceil(horizontal) * height.div_ceil(vertical) * sample_bytes,
                    sample_bytes,
                    plane_bytes,
                    black: if full { 0 } else { 16 << (depth - 8) },
                    white: if full {
                        ((1u32 << depth) - 1) as u16
                    } else {
                        235 << (depth - 8)
                    },
                    chroma: Some(128 << (depth - 8)),
                    depth,
                    rgb: false,
                    classify: true,
                };
            }
        }
        if format.starts_with("gbrp") && !format.starts_with("gbrpf") {
            return Self::rgb(width, height, depth("gbrp"));
        }
        if format.starts_with("gbrap") && !format.starts_with("gbrapf") {
            return Self::rgb(width, height, depth("gbrap"));
        }
        if format.starts_with("rgb")
            || format.starts_with("bgr")
            || ["rgba", "bgra", "argb", "abgr", "0rgb", "0bgr"].contains(&format)
        {
            return Self::rgb(
                width,
                height,
                if format.contains("48") || format.contains("64") {
                    16
                } else {
                    8
                },
            );
        }
        if format.starts_with("gray") && !format.starts_with("grayf") {
            let depth = depth("gray");
            let sample_bytes = if depth > 8 { 2 } else { 1 };
            let plane_bytes = width * height * sample_bytes;
            return Self {
                format: if depth == 8 {
                    "gray".into()
                } else {
                    format!("gray{depth}le")
                },
                bytes: plane_bytes,
                sample_bytes,
                plane_bytes,
                black: if range == "tv" { 16 << (depth - 8) } else { 0 },
                white: if range == "tv" {
                    235 << (depth - 8)
                } else {
                    ((1u32 << depth) - 1) as u16
                },
                chroma: None,
                depth,
                rgb: false,
                classify: true,
            };
        }
        // Unknown precision/range: retain frames rather than misclassifying near-white detail.
        let mut layout = Self::rgb(width, height, 16);
        layout.classify = false;
        layout
    }
}

fn source_layout(
    ffprobe: &Path,
    path: &Path,
    info: &MediaInfo,
    cancelled: Arc<AtomicBool>,
) -> Result<FrameLayout, String> {
    let mut command = Command::new(ffprobe);
    command
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=pix_fmt,color_range",
            "-of",
            "json",
        ])
        .arg(path)
        .stdout(Stdio::piped());
    let mut child = ManagedChild::spawn(&mut command, cancelled.clone())?;
    let mut bytes = Vec::new();
    child
        .stdout
        .take()
        .ok_or("Missing pixel format metadata")?
        .take(65536)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    check_cancelled(&cancelled)?;
    child.wait_success("Pixel format probe")?;
    let json: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    let stream = json["streams"].get(0).ok_or("Missing video pixel format")?;
    Ok(FrameLayout::native(
        stream["pix_fmt"].as_str().unwrap_or(""),
        stream["color_range"].as_str().unwrap_or(""),
        info.width as usize,
        info.height as usize,
    ))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MotionSample {
    pub time: f64,
    pub activity: f64,
    pub confidence: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ActionWindow {
    pub start: f64,
    pub end: f64,
    pub peak: f64,
    pub activity: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneSelection {
    pub scene_id: usize,
    pub source_id: usize,
    pub start: f64,
    pub end: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedScene {
    pub scene_id: usize,
    pub source_id: usize,
    pub source_start: f64,
    pub source_end: f64,
    pub fingerprint: String,
    pub fps: f64,
    pub fragments: Vec<SourceRange>,
    pub motion: Vec<MotionSample>,
    pub actions: Vec<ActionWindow>,
}
impl PreparedScene {
    pub fn duration(&self) -> f64 {
        self.fragments.iter().map(|range| range.duration).sum()
    }
    pub fn activity_at(&self, time: f64) -> f64 {
        let index = self.motion.partition_point(|sample| sample.time < time);
        let first = index.saturating_sub(2);
        let last = (index + 2).min(self.motion.len());
        self.motion[first..last]
            .iter()
            .map(|sample| sample.activity * sample.confidence)
            .fold(0.0, f64::max)
    }
}

#[derive(Debug)]
pub struct SourceAnalysis {
    pub fingerprint: String,
    pub info: MediaInfo,
    pub fragments: Vec<(f64, f64)>,
    pub motion: Vec<MotionSample>,
    pub cuts: Vec<f64>,
    pub decoded_frames: u64,
    pub removed_frames: u64,
}
impl SourceAnalysis {
    fn bytes(&self) -> usize {
        self.motion.len() * std::mem::size_of::<MotionSample>()
            + self.fragments.len() * 16
            + self.cuts.len() * 8
    }
    pub fn prepare(&self, request: &SceneSelection) -> Option<PreparedScene> {
        let fragments: Vec<_> = self
            .fragments
            .iter()
            .filter_map(|&(start, end)| {
                let start = start.max(request.start);
                let end = end.min(request.end);
                (end > start + 1e-7).then_some(SourceRange {
                    source_id: request.source_id,
                    start,
                    duration: end - start,
                })
            })
            .collect();
        if fragments.is_empty() {
            return None;
        }
        let motion: Vec<_> = self
            .motion
            .iter()
            .filter(|sample| {
                fragments.iter().any(|range| {
                    sample.time >= range.start && sample.time < range.start + range.duration
                })
            })
            .cloned()
            .collect();
        let mut actions = action_windows(&motion, &fragments);
        actions.retain_mut(|action| {
            action.start = action.start.max(request.start);
            action.end = action.end.min(request.end);
            action.peak = action.peak.clamp(action.start, action.end);
            action.activity = action.activity.clamp(0.0, 1.0);
            action.end > action.start + 1e-7
        });
        Some(PreparedScene {
            scene_id: request.scene_id,
            source_id: request.source_id,
            source_start: request.start,
            source_end: request.end,
            fingerprint: self.fingerprint.clone(),
            fps: self.info.fps,
            fragments,
            motion,
            actions,
        })
    }
}

struct CacheEntry {
    key: String,
    analysis: Arc<SourceAnalysis>,
}
fn cache() -> &'static Mutex<VecDeque<CacheEntry>> {
    static CACHE: OnceLock<Mutex<VecDeque<CacheEntry>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(VecDeque::new()))
}

pub fn analyze_source(
    ffmpeg: &Path,
    ffprobe: &Path,
    path: &Path,
    remove_blank: bool,
    cancelled: Arc<AtomicBool>,
    progress: &mut dyn FnMut(f64),
) -> Result<Arc<SourceAnalysis>, String> {
    check_cancelled(&cancelled)?;
    let before = fingerprint(path)?;
    let key = format!("{VERSION}:{remove_blank}:{before}");
    {
        let mut entries = cache().lock().map_err(|_| "Scene cache unavailable")?;
        if let Some(index) = entries.iter().position(|entry| entry.key == key) {
            let entry = entries.remove(index).unwrap();
            let result = entry.analysis.clone();
            entries.push_back(entry);
            if fingerprint(path)? != before {
                return Err("Source changed during analysis".into());
            }
            progress(1.0);
            check_cancelled(&cancelled)?;
            return Ok(result);
        }
    }
    let info = probe_media_internal(&path.to_string_lossy(), Some(ffprobe))?;
    if info.width == 0
        || info.height == 0
        || info.width as u64 * info.height as u64 > 33_177_600
        || !info.duration.is_finite()
        || info.duration <= 0.0
        || info.duration > 86400.0
    {
        return Err("Invalid or unsupported scene dimensions/duration".into());
    }
    let layout = if remove_blank {
        source_layout(ffprobe, path, &info, cancelled.clone())?
    } else {
        FrameLayout::rgb(160, 90, 16)
    };
    let result = Arc::new(scan_source(
        ffmpeg,
        path,
        info,
        remove_blank,
        layout,
        cancelled.clone(),
        progress,
    )?);
    check_cancelled(&cancelled)?;
    if fingerprint(path)? != before {
        return Err("Source changed during analysis; analyze it again".into());
    }
    if result.bytes() <= CACHE_BYTES {
        let mut entries = cache().lock().map_err(|_| "Scene cache unavailable")?;
        entries.retain(|entry| entry.key != key);
        entries.push_back(CacheEntry {
            key,
            analysis: result.clone(),
        });
        while entries.len() > 16
            || entries
                .iter()
                .map(|entry| entry.analysis.bytes())
                .sum::<usize>()
                > CACHE_BYTES
        {
            entries.pop_front();
        }
    }
    Ok(result)
}

fn metadata_value(line: &str, key: &str) -> Option<f64> {
    let rest = line.split_once(key)?.1;
    rest.split_whitespace()
        .next()?
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

fn scan_source(
    ffmpeg: &Path,
    path: &Path,
    info: MediaInfo,
    remove_blank: bool,
    layout: FrameLayout,
    cancelled: Arc<AtomicBool>,
    progress: &mut dyn FnMut(f64),
) -> Result<SourceAnalysis, String> {
    let fingerprint = fingerprint(path)?;
    let (width, height) = if remove_blank {
        (info.width as usize, info.height as usize)
    } else {
        (160, 90)
    };
    let sample_w = width.min(160);
    let sample_h = height.min(90);
    let pixels: Vec<_> = (0..sample_h)
        .flat_map(|y| {
            (0..sample_w).map(move |x| {
                (((2 * y + 1) * height / (2 * sample_h)) * width
                    + (2 * x + 1) * width / (2 * sample_w))
                    * layout.sample_bytes
            })
        })
        .collect();
    let mut offsets = Vec::new();
    for plane in 0..if layout.rgb { 3 } else { 1 } {
        offsets.extend(
            pixels
                .iter()
                .map(|offset| offset + plane * layout.plane_bytes),
        );
    }
    let (sender, receiver) = mpsc::channel();
    let mut pending_time = None;
    let observer = Box::new(move |line: &str| {
        if line.contains("Parsed_metadata") {
            if let Some(time) = metadata_value(line, "pts_time:") {
                pending_time = Some(time);
            }
            if let Some(score) = metadata_value(line, "lavfi.scene_score=") {
                if let Some(time) = pending_time.take() {
                    let _ = sender.send((time, score));
                }
            }
        }
    });
    let filter = if remove_blank {
        format!("split=2[clean][score];[clean]format={}[frames];[score]scale=160:90,format=yuv420p,select=gte(scene\\,0),metadata=mode=print:key=lavfi.scene_score[analysis]", layout.format)
    } else {
        format!("select=gte(scene\\,0)*(isnan(prev_selected_t)+gte(t-prev_selected_t\\,{:.9})+gt(scene\\,0.30)),scale=160:90,metadata=mode=print:key=lavfi.scene_score", 1.0 / SAMPLE_RATE)
    };
    let mut command = Command::new(ffmpeg);
    command
        .args([
            "-nostdin",
            "-hide_banner",
            "-nostats",
            "-loglevel",
            "info",
            "-threads",
            "2",
            "-noautorotate",
            "-i",
        ])
        .arg(path);
    if remove_blank {
        command.args([
            "-filter_complex_threads",
            "1",
            "-filter_complex",
            &filter,
            "-map",
            "[frames]",
        ]);
    } else {
        command.args(["-map", "0:v:0", "-filter_threads", "1", "-vf", &filter]);
    }
    command
        .args([
            "-an",
            "-sn",
            "-dn",
            "-fps_mode",
            "passthrough",
            "-pix_fmt",
            &layout.format,
            "-c:v",
            "rawvideo",
            "-threads",
            "1",
            "-f",
            "rawvideo",
            "pipe:1",
        ])
        .stdout(Stdio::piped());
    if remove_blank {
        command.args([
            "-map",
            "[analysis]",
            "-an",
            "-fps_mode",
            "passthrough",
            "-f",
            "null",
            "-",
        ]);
    }
    let mut child = ManagedChild::spawn_observed(&mut command, cancelled.clone(), Some(observer))?;
    let mut stdout = child.stdout.take().ok_or("Missing analysis stream")?;
    let mut buffer = vec![0u8; 65_532];
    let mut motion = Vec::new();
    let mut cuts = vec![0.0];
    let mut motion_resets = vec![0.0];
    let mut fragments = Vec::new();
    let mut run_start = None;
    let mut previous_frame: Option<(f64, bool)> = None;
    let mut previous_motion: Option<Vec<f32>> = None;
    let mut last_motion_time = -1.0;
    let mut last_progress = -1.0;
    let mut decoded_frames = 0;
    let mut removed_frames = 0;
    loop {
        check_cancelled(&cancelled)?;
        let Some((blank, sampled)) =
            read_frame(&mut stdout, &layout, &offsets, &mut buffer, &cancelled)?
        else {
            break;
        };
        let (time, cut_score) = loop {
            check_cancelled(&cancelled)?;
            match receiver.recv_timeout(Duration::from_millis(100)) {
                Ok(metadata) => break metadata,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(_) => return Err("Frame timestamps are missing from analysis".into()),
            }
        };
        if time < -0.001
            || time > info.duration + 0.1
            || previous_frame.is_some_and(|(previous, _)| time <= previous)
        {
            return Err("Invalid or nonmonotonic source frame timestamps".into());
        }
        let time = time.max(0.0);
        if let Some((previous_time, valid)) = previous_frame {
            if valid {
                run_start.get_or_insert(previous_time);
            } else if let Some(start) = run_start.take() {
                fragments.push((start, previous_time));
            }
        }
        let valid = !remove_blank || !blank;
        decoded_frames += 1;
        if !valid {
            removed_frames += 1;
            previous_motion = None;
            if previous_frame.is_none_or(|(_, previous_valid)| previous_valid) {
                motion_resets.push(time);
            }
        }
        if valid && previous_frame.is_some_and(|(_, previous_valid)| !previous_valid) {
            motion_resets.push(time);
        }
        let is_cut = cut_score > 0.30;
        if is_cut {
            if time - cuts.last().copied().unwrap_or(0.0) >= if remove_blank { 1.0 } else { 0.15 }
                && time < info.duration - 0.1
            {
                cuts.push(time);
            }
            motion_resets.push(time);
            previous_motion = None;
        }
        if valid && (time - last_motion_time >= 1.0 / SAMPLE_RATE - 1e-6 || is_cut) {
            if motion.len() >= MAX_SAMPLES {
                return Err("Motion analysis exceeds the supported duration".into());
            }
            let (activity, confidence) = previous_motion
                .as_ref()
                .map(|previous| motion_score(previous, &sampled, sample_w, sample_h))
                .unwrap_or((0.0, 0.0));
            motion.push(MotionSample {
                time,
                activity,
                confidence,
            });
            previous_motion = Some(sampled);
            last_motion_time = time;
        }
        previous_frame = Some((time, valid));
        let profile_bytes = motion.len() * std::mem::size_of::<MotionSample>()
            + fragments.len() * 16
            + (cuts.len() + motion_resets.len()) * 8;
        if profile_bytes > CACHE_BYTES {
            return Err("Scene analysis profile exceeds the 32 MiB limit".into());
        }
        let fraction = (time / info.duration).clamp(0.0, 1.0);
        if fraction - last_progress >= 0.01 {
            progress(fraction);
            last_progress = fraction;
        }
    }
    check_cancelled(&cancelled)?;
    child.wait_success("Scene analysis")?;
    if let Some((time, valid)) = previous_frame {
        if valid {
            run_start.get_or_insert(time);
        } else if let Some(start) = run_start.take() {
            fragments.push((start, time));
        }
    } else {
        return Err("Source contains no decoded video frames".into());
    }
    if let Some(start) = run_start {
        if info.duration > start {
            fragments.push((start, info.duration));
        }
    }
    if !remove_blank {
        fragments = vec![(0.0, info.duration)];
    }
    cuts.push(info.duration);
    motion_resets.sort_by(f64::total_cmp);
    smooth_motion(&mut motion, &motion_resets);
    progress(1.0);
    Ok(SourceAnalysis {
        fingerprint,
        info,
        fragments,
        motion,
        cuts,
        decoded_frames,
        removed_frames,
    })
}

/// Classify every native pixel; only the motion image is sampled down.
fn read_frame(
    reader: &mut impl Read,
    layout: &FrameLayout,
    offsets: &[usize],
    buffer: &mut [u8],
    cancelled: &AtomicBool,
) -> Result<Option<(bool, Vec<f32>)>, String> {
    let mut consumed = 0;
    let mut black = layout.classify;
    let mut white = layout.classify;
    let mut sample_index = 0;
    let count_pixels = offsets.len() / if layout.rgb { 3 } else { 1 };
    let mut image = vec![0.0; count_pixels];
    while consumed < layout.bytes {
        check_cancelled(cancelled)?;
        let count = buffer.len().min(layout.bytes - consumed);
        let mut filled = 0;
        while filled < count {
            match reader.read(&mut buffer[filled..count]) {
                Ok(0) if consumed == 0 && filled == 0 => return Ok(None),
                Ok(0) => return Err("Incomplete source frame during analysis".into()),
                Ok(read) => filled += read,
                Err(error) => {
                    check_cancelled(cancelled)?;
                    return Err(format!("Cannot read source frame: {error}"));
                }
            }
        }
        let chunk = &buffer[..count];
        if black || white {
            for (index, sample) in chunk.chunks_exact(layout.sample_bytes).enumerate() {
                let value = if layout.sample_bytes == 1 {
                    sample[0] as u16
                } else {
                    u16::from_le_bytes([sample[0], sample[1]])
                };
                let chroma = layout
                    .chroma
                    .filter(|_| consumed + index * layout.sample_bytes >= layout.plane_bytes);
                black &= value == chroma.unwrap_or(layout.black);
                white &= value == chroma.unwrap_or(layout.white);
                if !black && !white {
                    break;
                }
            }
        }
        while sample_index < offsets.len() && offsets[sample_index] < consumed + count {
            let offset = offsets[sample_index] - consumed;
            let value = if layout.sample_bytes == 1 {
                chunk[offset] as u16
            } else {
                u16::from_le_bytes([chunk[offset], chunk[offset + 1]])
            };
            let weight = if layout.rgb {
                [0.7152, 0.0722, 0.2126][sample_index / count_pixels]
            } else {
                1.0
            };
            image[sample_index % count_pixels] +=
                value as f32 * 255.0 / ((1u32 << layout.depth) - 1) as f32 * weight;
            sample_index += 1;
        }
        consumed += count;
    }
    Ok(Some((black || white, image)))
}

fn motion_score(previous: &[f32], current: &[f32], width: usize, height: usize) -> (f64, f64) {
    if width < 12 || height < 12 || previous.len() != current.len() {
        return (0.0, 0.0);
    }
    let previous_mean = previous.iter().sum::<f32>() / previous.len() as f32;
    let current_mean = current.iter().sum::<f32>() / current.len() as f32;
    let exposure = current_mean - previous_mean;
    let mut activity = 0.0;
    let mut tracked = 0.0;
    let mut blocks = 0.0;
    for y in (3..height - 8).step_by(8) {
        for x in (3..width - 8).step_by(8) {
            let mut texture = 0.0f32;
            for dy in (0..6).step_by(2) {
                for dx in (0..6).step_by(2) {
                    let index = (y + dy) * width + x + dx;
                    texture += (previous[index] - previous[index + 1]).abs();
                }
            }
            if texture < 12.0 {
                continue;
            }
            let error = |sx: isize, sy: isize| -> f32 {
                let mut sum = 0.0;
                for dy in (0..6).step_by(2) {
                    for dx in (0..6).step_by(2) {
                        let previous_index = ((y + dy) as isize + sy) as usize * width
                            + ((x + dx) as isize + sx) as usize;
                        sum += (current[(y + dy) * width + x + dx]
                            - exposure
                            - previous[previous_index])
                            .abs();
                    }
                }
                sum / 9.0
            };
            let baseline = error(0, 0);
            blocks += 1.0;
            if baseline < 2.0 {
                continue;
            }
            let mut best = baseline;
            let mut distance = 0.0;
            for sy in -3isize..=3 {
                for sx in -3isize..=3 {
                    let cost = error(sx, sy);
                    if cost + 0.5 < best {
                        best = cost;
                        distance = ((sx * sx + sy * sy) as f64).sqrt();
                    }
                }
            }
            let confidence = ((baseline - best) / baseline).clamp(0.0, 1.0) as f64;
            if confidence > 0.2 && best < 24.0 {
                activity += (distance / 3.0).min(1.0) * confidence;
                tracked += confidence;
            }
        }
    }
    if blocks == 0.0 {
        (0.0, 0.0)
    } else {
        (
            (activity / blocks).clamp(0.0, 1.0),
            (tracked / blocks).clamp(0.0, 1.0),
        )
    }
}

fn smooth_motion(samples: &mut [MotionSample], cuts: &[f64]) {
    let raw = samples.to_vec();
    for (index, sample) in samples.iter_mut().enumerate() {
        let neighbors: Vec<_> = raw[index.saturating_sub(1)..(index + 2).min(raw.len())]
            .iter()
            .filter(|other| {
                (other.time - sample.time).abs() <= 0.2
                    && cuts
                        .get(cuts.partition_point(|&cut| cut <= other.time.min(sample.time)))
                        .is_none_or(|&cut| cut > other.time.max(sample.time))
            })
            .collect();
        sample.activity =
            neighbors.iter().map(|other| other.activity).sum::<f64>() / neighbors.len() as f64;
        sample.confidence =
            neighbors.iter().map(|other| other.confidence).sum::<f64>() / neighbors.len() as f64;
    }
}

fn action_windows(samples: &[MotionSample], fragments: &[SourceRange]) -> Vec<ActionWindow> {
    let peak = samples
        .iter()
        .map(|sample| sample.activity * sample.confidence)
        .fold(0.0, f64::max);
    let threshold = (peak * 0.35).max(0.015);
    let mut result: Vec<ActionWindow> = Vec::new();
    for fragment in fragments {
        for sample in samples.iter().filter(|sample| {
            sample.time >= fragment.start && sample.time < fragment.start + fragment.duration
        }) {
            let activity = sample.activity * sample.confidence;
            if activity < threshold {
                continue;
            }
            if let Some(window) = result.last_mut() {
                if sample.time <= window.end + 0.2 && window.start >= fragment.start {
                    window.end = (sample.time + 0.4).min(fragment.start + fragment.duration);
                    if activity > window.activity {
                        window.peak = sample.time;
                        window.activity = activity;
                    }
                    continue;
                }
            }
            result.push(ActionWindow {
                start: (sample.time - 0.35).max(fragment.start),
                end: (sample.time + 0.4).min(fragment.start + fragment.duration),
                peak: sample.time,
                activity,
            });
        }
    }
    result.sort_by(|a, b| {
        b.activity
            .total_cmp(&a.activity)
            .then(a.start.total_cmp(&b.start))
    });
    result.truncate(64);
    result.sort_by(|a, b| a.start.total_cmp(&b.start));
    result
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisProgress {
    pub job_id: String,
    pub percent: u32,
    pub message: String,
}
pub fn emit_progress(app: &tauri::AppHandle, id: &str, fraction: f64, message: &str) {
    let _ = app.emit(
        "scene-analysis-progress",
        AnalysisProgress {
            job_id: id.into(),
            percent: (fraction.clamp(0.0, 1.0) * 100.0).round() as u32,
            message: message.into(),
        },
    );
}

#[tauri::command(async)]
pub fn prepare_scene_selection(
    app: tauri::AppHandle,
    video_paths: Vec<String>,
    selections: Vec<SceneSelection>,
    mode: String,
    job_id: String,
) -> Result<Vec<PreparedScene>, String> {
    if !matches!(mode.as_str(), "scenepack" | "long_clip") || selections.is_empty() {
        return Err("Select at least one scene for analysis".into());
    }
    let job = AnalysisJob::begin(job_id)?;
    let _permit = AnalysisPermit::acquire_cancelled(&job.cancelled)?;
    let ffmpeg = get_ffmpeg_binary(&app)?;
    let ffprobe = get_ffprobe_binary(&app)?;
    let mut sources = std::collections::BTreeMap::new();
    for selection in &selections {
        if selection.source_id >= video_paths.len()
            || !selection.start.is_finite()
            || !selection.end.is_finite()
            || selection.start < 0.0
            || selection.end <= selection.start
        {
            return Err("Invalid selected scene".into());
        }
        sources.insert(selection.source_id, ());
    }
    let count = sources.len();
    let mut result = Vec::new();
    for (index, &source_id) in sources.keys().enumerate() {
        let analysis = analyze_source(
            &ffmpeg,
            &ffprobe,
            Path::new(&video_paths[source_id]),
            mode == "scenepack",
            job.cancelled.clone(),
            &mut |fraction| {
                emit_progress(
                    &app,
                    &job.id,
                    (index as f64 + fraction) / count as f64,
                    "Analyzing source motion...",
                );
            },
        )?;
        for selection in selections
            .iter()
            .filter(|selection| selection.source_id == source_id)
        {
            if selection.end > analysis.info.duration + 0.051 {
                return Err("Selected scene exceeds the current source duration".into());
            }
            if mode == "long_clip" {
                for (cut_index, bounds) in analysis.cuts.windows(2).enumerate() {
                    let request = SceneSelection {
                        scene_id: cut_index,
                        source_id,
                        start: bounds[0].max(selection.start),
                        end: bounds[1].min(selection.end),
                    };
                    if request.end > request.start {
                        if let Some(scene) = analysis.prepare(&request) {
                            result.push(scene);
                        }
                    }
                }
            } else if let Some(scene) = analysis.prepare(selection) {
                result.push(scene);
            }
        }
    }
    check_cancelled(&job.cancelled)?;
    for scene in &result {
        if fingerprint(Path::new(&video_paths[scene.source_id]))? != scene.fingerprint {
            return Err("Source changed during analysis".into());
        }
    }
    if result.is_empty() {
        return Err("Selected scenes contain only pure black or white frames".into());
    }
    result.sort_by_key(|scene| {
        selections
            .iter()
            .position(|selection| {
                selection.scene_id == scene.scene_id && selection.source_id == scene.source_id
            })
            .unwrap_or(scene.scene_id)
    });
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_blank_classification_preserves_one_distinct_pixel() {
        let flag = AtomicBool::new(false);
        let mut buffer = vec![0; 12];
        let layout = FrameLayout::native("gray16le", "pc", 12, 1);
        for level in [0, 255] {
            let pixels = vec![level; 24];
            assert!(
                read_frame(
                    &mut pixels.as_slice(),
                    &layout,
                    &[0, 18],
                    &mut buffer,
                    &flag
                )
                .unwrap()
                .unwrap()
                .0
            );
            let mut pixels = pixels;
            pixels[12] = 1;
            assert!(
                !read_frame(
                    &mut pixels.as_slice(),
                    &layout,
                    &[0, 18],
                    &mut buffer,
                    &flag
                )
                .unwrap()
                .unwrap()
                .0
            );
        }
        assert!(
            !read_frame(
                &mut vec![254; 24].as_slice(),
                &layout,
                &[0, 18],
                &mut buffer,
                &flag
            )
            .unwrap()
            .unwrap()
            .0
        );
    }
    #[test]
    fn blank_levels_respect_native_yuv_range_and_depth() {
        let flag = AtomicBool::new(false);
        let mut buffer = vec![0; 12];
        for (format, range) in [
            ("yuv420p", "tv"),
            ("yuv420p", "pc"),
            ("yuvj420p", ""),
            ("yuv420p10le", "tv"),
        ] {
            let layout = FrameLayout::native(format, range, 4, 2);
            for level in [layout.black, layout.white] {
                let mut pixels = Vec::new();
                for index in 0..layout.bytes / layout.sample_bytes {
                    let value = if index * layout.sample_bytes < layout.plane_bytes {
                        level
                    } else {
                        layout.chroma.unwrap()
                    };
                    pixels.extend_from_slice(&value.to_le_bytes()[..layout.sample_bytes]);
                }
                assert!(
                    read_frame(&mut pixels.as_slice(), &layout, &[0], &mut buffer, &flag)
                        .unwrap()
                        .unwrap()
                        .0,
                    "{format}/{range}"
                );
                pixels[0] ^= 1;
                assert!(
                    !read_frame(&mut pixels.as_slice(), &layout, &[0], &mut buffer, &flag)
                        .unwrap()
                        .unwrap()
                        .0,
                    "{format}/{range} must retain detail"
                );
            }
        }
        let layout = FrameLayout::native("gbrpf32le", "pc", 2, 2);
        assert!(!layout.classify);
    }
    #[test]
    fn exposure_change_and_static_texture_are_not_motion() {
        let width = 64;
        let image: Vec<_> = (0..64 * 40)
            .map(|index| ((index % width) * 3 + (index / width) % 5) as f32)
            .collect();
        assert_eq!(motion_score(&image, &image, width, 40), (0.0, 0.0));
        let bright: Vec<_> = image.iter().map(|pixel| pixel + 25.0).collect();
        assert_eq!(motion_score(&image, &bright, width, 40), (0.0, 0.0));
        let moving: Vec<_> = (0..64 * 40)
            .map(|index| image[(index + 2).min(image.len() - 1)])
            .collect();
        assert!(motion_score(&image, &moving, width, 40).0 > 0.05);
    }
}
