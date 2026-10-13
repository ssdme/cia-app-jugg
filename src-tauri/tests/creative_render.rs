use cia_render_lib::{
    creative::CreativeKind,
    creative_schedule::{CreativeEvent, CreativeOptions},
    effects::default_segment_effects,
    jobs::RenderJob,
    plan::{create_plan_internal, generate_plan, validate_plan, ProjectPlan},
    render::{render_pipeline_core, RenderEnvironment},
    sources::SourceRange,
};
use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

const WIDTH: usize = 320;
const HEIGHT: usize = 180;
const FPS: usize = 16;
const FRAME_BYTES: usize = WIDTH * HEIGHT * 3;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn options(mask: u8) -> CreativeOptions {
    CreativeOptions {
        time_slice: mask & 1 != 0,
        ink_pulse: mask & 2 != 0,
        prism_drift: mask & 4 != 0,
        ..Default::default()
    }
}

fn generated(options: Option<CreativeOptions>) -> String {
    generated_with_audio(
        options,
        20.0,
        (1..80).map(|beat| beat as f64 * 0.25).collect(),
        (1..20).map(|beat| beat as f64).collect(),
    )
}

fn generated_with_audio(
    options: Option<CreativeOptions>,
    duration: f64,
    beats: Vec<f64>,
    downbeats: Vec<f64>,
) -> String {
    generate_plan(
        "HARD".into(),
        FPS as u32,
        beats,
        downbeats,
        duration,
        duration,
        WIDTH as u32,
        HEIGHT as u32,
        120.0,
        false,
        None,
        None,
        None,
        Some("no_change".into()),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        options,
    )
    .unwrap()
}

#[test]
fn short_clips_and_missing_beats_keep_enabled_effects_available() {
    for duration in [0.13, 0.5, 2.0] {
        for mask in [1, 2, 4] {
            for beats in [Vec::new(), vec![duration - 0.01]] {
                let plan: ProjectPlan = serde_json::from_str(&generated_with_audio(
                    Some(options(mask)),
                    duration,
                    beats.clone(),
                    beats,
                ))
                .unwrap();
                validate_plan(&plan).unwrap();
                assert!(
                    !plan.creative_events.is_empty(),
                    "Mask {mask}, duration {duration}"
                );
                assert!(plan
                    .creative_events
                    .iter()
                    .all(|event| event.t0 >= 0.0 && event.t1 <= duration));
            }
        }
    }
}

#[test]
fn creative_accents_leave_clean_pixels_outside_their_envelope() {
    let frame: Vec<u8> = (0..FRAME_BYTES).map(|index| (index % 251) as u8).collect();
    let alternate: Vec<u8> = frame.iter().map(|value| 255 - value).collect();
    let mut output = vec![0; FRAME_BYTES];
    for kind in [
        CreativeKind::TimeSlice,
        CreativeKind::InkPulse,
        CreativeKind::PrismDrift,
    ] {
        for phase in [-0.1, 0.0, 1.0, 1.1] {
            cia_render_lib::creative::apply(
                kind,
                &frame,
                Some(&alternate),
                &mut output,
                WIDTH,
                HEIGHT,
                phase,
                37,
                0.85,
            );
            assert_eq!(output, frame);
        }
        cia_render_lib::creative::apply(
            kind,
            &frame,
            Some(&alternate),
            &mut output,
            WIDTH,
            HEIGHT,
            0.5,
            37,
            0.0,
        );
        assert_eq!(output, frame);
    }
}

#[test]
fn creative_options_are_independent_deterministic_and_sparse() {
    let omitted = generated(None);
    assert_eq!(omitted, generated(Some(CreativeOptions::default())));
    let baseline: ProjectPlan = serde_json::from_str(&omitted).unwrap();
    assert!(baseline.creative_events.is_empty());
    assert_eq!(
        serde_json::to_value(options(7)).unwrap(),
        serde_json::json!({"timeSlice": true, "inkPulse": true, "prismDrift": true,
            "focusPull": false, "reliefLight": false, "paperGate": false})
    );
    for mask in 1..=7 {
        let json = generated(Some(options(mask)));
        assert_eq!(json, generated(Some(options(mask))));
        let plan: ProjectPlan = serde_json::from_str(&json).unwrap();
        validate_plan(&plan).unwrap();
        assert!(
            !plan.full_fx,
            "Creative options remain independent of Full FX"
        );
        assert!(!plan.creative_events.is_empty());
        let mut seen = [false; 3];
        for event in &plan.creative_events {
            let index = match event.kind {
                CreativeKind::TimeSlice => 0,
                CreativeKind::InkPulse => 1,
                CreativeKind::PrismDrift => 2,
                _ => panic!("Legacy options must not enable trial effects"),
            };
            assert!(mask & (1 << index) != 0, "Unexpected effect in mask {mask}");
            seen[index] = true;
            assert!(event.t0.is_finite() && event.t1.is_finite());
            assert!(event.t0 >= 0.0 && event.t1 > event.t0);
            assert!(event.t1 <= plan.target_duration);
            assert!(event.strength.is_finite() && event.strength > 0.0);
            assert!(event.strength <= 1.0);
            assert!(
                event.t1 - event.t0 <= 2.0,
                "An accent became a persistent filter"
            );
        }
        for (index, found) in seen.into_iter().enumerate() {
            assert_eq!(found, mask & (1 << index) != 0, "Mask {mask}");
        }
        for (index, event) in plan.creative_events.iter().enumerate() {
            for other in &plan.creative_events[index + 1..] {
                if event.kind == other.kind {
                    assert!(event.t1 <= other.t0 || other.t1 <= event.t0);
                }
            }
        }
        let active_frames = (0..FPS * 20)
            .filter(|frame| {
                let time = *frame as f64 / FPS as f64;
                plan.creative_events
                    .iter()
                    .any(|event| time >= event.t0 && time < event.t1)
            })
            .count();
        assert!(
            active_frames < FPS * 14,
            "Mask {mask}: {active_frames}/{} active frames; creative accents need breathing room",
            FPS * 20
        );
        let mut ordinary = serde_json::to_value(plan).unwrap();
        ordinary.as_object_mut().unwrap().remove("creative_events");
        let mut expected = serde_json::to_value(&baseline).unwrap();
        expected.as_object_mut().unwrap().remove("creative_events");
        assert_eq!(
            ordinary, expected,
            "Creative options changed existing remapping"
        );
    }
}

fn neutral_plan() -> ProjectPlan {
    let mut plan = create_plan_internal(
        "SMOOTH",
        FPS as u32,
        &[],
        &[],
        2.0,
        2.0,
        WIDTH as u32,
        HEIGHT as u32,
        120.0,
        false,
        None,
        None,
        Some("no_change"),
        None,
        None,
    )
    .unwrap();
    plan.schema_version = 3;
    plan.ambiance = None;
    plan.transitions.clear();
    plan.one_framers.clear();
    plan.motion_blur = false;
    plan.creative_events.clear();
    plan.source_ranges = Some(vec![
        SourceRange {
            source_id: 0,
            start: 0.5,
            duration: 1.0,
        },
        SourceRange {
            source_id: 0,
            start: 2.0,
            duration: 1.0,
        },
    ]);
    for segment in &mut plan.segments {
        segment.s0 = segment.t0;
        segment.s1 = segment.t1;
        segment.effects = default_segment_effects();
        segment.curve = "linear".into();
        segment.remap = None;
    }
    plan
}

fn event(kind: CreativeKind) -> CreativeEvent {
    CreativeEvent {
        t0: 0.75,
        t1: 1.25,
        kind,
        seed: 37,
        strength: 0.85,
    }
}

#[test]
fn legacy_plans_load_and_invalid_creative_events_fail_validation() {
    let plan = neutral_plan();
    let mut legacy = serde_json::to_value(&plan).unwrap();
    legacy.as_object_mut().unwrap().remove("creative_events");
    let restored: ProjectPlan = serde_json::from_value(legacy).unwrap();
    assert_eq!(restored, plan);
    for (start, end, strength) in [
        (f64::NAN, 1.0, 0.8),
        (0.5, f64::INFINITY, 0.8),
        (0.5, 1.0, f64::NAN),
        (-0.01, 0.5, 0.8),
        (0.5, 2.01, 0.8),
        (0.5, 0.5, 0.8),
        (1.0, 0.5, 0.8),
        (0.5, 1.0, -0.01),
        (0.5, 1.0, 1.01),
        (0.0, 1.51, 0.8),
    ] {
        let mut invalid = plan.clone();
        let mut accent = event(CreativeKind::InkPulse);
        accent.t0 = start;
        accent.t1 = end;
        accent.strength = strength;
        invalid.creative_events.push(accent);
        assert!(
            validate_plan(&invalid).is_err(),
            "Accepted {start}, {end}, {strength}"
        );
    }
    let mut unknown = serde_json::to_value(event(CreativeKind::InkPulse)).unwrap();
    unknown["kind"] = "unknown".into();
    assert!(serde_json::from_value::<CreativeEvent>(unknown).is_err());
    let mut overlapping = plan;
    overlapping.creative_events =
        vec![event(CreativeKind::InkPulse), event(CreativeKind::InkPulse)];
    assert!(validate_plan(&overlapping).is_err());
}

fn run_ffmpeg(ffmpeg: &Path, args: &[&str], output: &Path) {
    let result = Command::new(ffmpeg)
        .args(["-nostdin", "-hide_banner", "-loglevel", "error", "-y"])
        .args(args)
        .arg(output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

fn create_media(directory: &Path, ffmpeg: &Path) -> (String, String) {
    let source = directory.join("motion and fragment guards.mp4");
    let audio = directory.join("audio.wav");
    let mut raw = Vec::with_capacity(FRAME_BYTES * FPS * 4);
    for frame in 0..FPS * 4 {
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let edge = ((x + frame * 11) / 21 + (y + frame * 3) / 17) % 2;
                let color = if (FPS / 2..FPS * 3 / 2).contains(&frame) {
                    [
                        30 + edge as u8 * 55,
                        165 + edge as u8 * 65,
                        25 + edge as u8 * 35,
                    ]
                } else if (FPS * 2..FPS * 3).contains(&frame) {
                    [
                        30 + edge as u8 * 55,
                        25 + edge as u8 * 35,
                        165 + edge as u8 * 65,
                    ]
                } else {
                    [255, 0, 255]
                };
                raw.extend_from_slice(&color);
            }
        }
    }
    let raw_path = directory.join("source.rgb");
    let mut file = std::fs::File::create(&raw_path).unwrap();
    file.write_all(&raw).unwrap();
    drop(file);
    run_ffmpeg(
        ffmpeg,
        &[
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "-s",
            "320x180",
            "-r",
            "16",
            "-i",
            raw_path.to_str().unwrap(),
            "-c:v",
            "libx264",
            "-crf",
            "0",
            "-preset",
            "ultrafast",
            "-pix_fmt",
            "yuv444p",
        ],
        &source,
    );
    run_ffmpeg(
        ffmpeg,
        &[
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=44100:duration=2",
            "-c:a",
            "pcm_s16le",
        ],
        &audio,
    );
    std::fs::remove_file(raw_path).unwrap();
    (
        source.to_string_lossy().into_owned(),
        audio.to_string_lossy().into_owned(),
    )
}

fn render_pixels(
    environment: &RenderEnvironment,
    source: &str,
    audio: &str,
    id: &str,
    plan: &ProjectPlan,
) -> Vec<u8> {
    let job = RenderJob::begin(id.into()).unwrap();
    let stats = render_pipeline_core(
        environment,
        &job,
        &serde_json::to_string(plan).unwrap(),
        source,
        audio,
        false,
        None,
        &mut |_| {},
    )
    .unwrap();
    let result = Command::new(&environment.ffmpeg)
        .args(["-v", "error", "-i"])
        .arg(stats.output_path)
        .args(["-f", "rawvideo", "-pix_fmt", "rgb24", "pipe:1"])
        .stdout(Stdio::piped())
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(result.stdout.len(), FRAME_BYTES * FPS * 2);
    result.stdout
}

fn difference(first: &[u8], second: &[u8]) -> f64 {
    first
        .iter()
        .zip(second)
        .map(|(a, b)| a.abs_diff(*b) as u64)
        .sum::<u64>() as f64
        / first.len() as f64
}

/// One sequential native harness avoids the process-wide render registry racing.
#[test]
fn native_creative_renders_are_visible_independent_and_fragment_bounded() {
    let directory = root().join(format!("tests/generated/creative-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let environment = RenderEnvironment {
        ffmpeg: root().join("src-tauri/binaries/ffmpeg.exe"),
        ffprobe: root().join("src-tauri/binaries/ffprobe.exe"),
        cache_root: directory.join("cache"),
        output_dir: directory.join("output"),
    };
    let (source, audio) = create_media(&directory, &environment.ffmpeg);
    let baseline = neutral_plan();
    let ordinary = render_pixels(
        &environment,
        &source,
        &audio,
        "creative-baseline",
        &baseline,
    );
    let mut legacy = serde_json::to_value(&baseline).unwrap();
    legacy.as_object_mut().unwrap().remove("creative_events");
    let legacy: ProjectPlan = serde_json::from_value(legacy).unwrap();
    assert_eq!(
        ordinary,
        render_pixels(&environment, &source, &audio, "creative-legacy", &legacy)
    );
    for mask in [1, 2, 4, 7] {
        let mut plan = baseline.clone();
        for (bit, kind) in [
            (1, CreativeKind::TimeSlice),
            (2, CreativeKind::InkPulse),
            (4, CreativeKind::PrismDrift),
        ] {
            if mask & bit != 0 {
                plan.creative_events.push(event(kind));
            }
        }
        validate_plan(&plan).unwrap();
        let changed = render_pixels(
            &environment,
            &source,
            &audio,
            &format!("creative-mask-{mask}"),
            &plan,
        );
        let mut visible = 0;
        for (index, (original, creative)) in ordinary
            .chunks_exact(FRAME_BYTES)
            .zip(changed.chunks_exact(FRAME_BYTES))
            .enumerate()
        {
            let time = index as f64 / FPS as f64;
            let delta = difference(original, creative);
            if (0.75..1.25).contains(&time) {
                visible += usize::from(delta > 0.5);
            } else {
                // Inter-frame H.264 prediction can change decoded pixels despite identical RGB input.
                assert!(
                    delta < 1.0,
                    "Mask {mask} leaked at {time:.3}s: mean delta {delta}"
                );
            }
            if mask == 1 {
                let mut channels = [0u64; 3];
                for pixel in creative.chunks_exact(3) {
                    for channel in 0..3 {
                        channels[channel] += pixel[channel] as u64;
                    }
                }
                let selected = if time < 1.0 { 1 } else { 2 };
                let other = if time < 1.0 { 2 } else { 1 };
                assert!(channels[selected] > channels[0] * 2);
                assert!(
                    channels[selected] > channels[other] * 2,
                    "Time Slice sampled across a selected fragment at {time:.3}s"
                );
            }
        }
        assert!(
            visible >= 2,
            "Mask {mask} produced no visible multi-frame accent"
        );
    }
    let mut freeze = baseline.clone();
    for segment in &mut freeze.segments {
        segment.s0 = 1.0 - 5e-8;
        segment.s1 = segment.s0;
    }
    freeze.creative_events.push(event(CreativeKind::TimeSlice));
    let frozen = render_pixels(
        &environment,
        &source,
        &audio,
        "creative-boundary-hold",
        &freeze,
    );
    for (index, frame) in frozen.chunks_exact(FRAME_BYTES).enumerate() {
        let mut green = 0u64;
        let mut blue = 0u64;
        for pixel in frame.chunks_exact(3) {
            green += pixel[1] as u64;
            blue += pixel[2] as u64;
        }
        assert!(
            green > blue * 2,
            "Time Slice crossed the held fragment boundary at frame {index}"
        );
    }
    let mut reverse = baseline;
    for segment in &mut reverse.segments {
        std::mem::swap(&mut segment.s0, &mut segment.s1);
    }
    reverse.creative_events.push(event(CreativeKind::TimeSlice));
    let reversed = render_pixels(&environment, &source, &audio, "creative-reverse", &reverse);
    for (index, frame) in reversed.chunks_exact(FRAME_BYTES).enumerate() {
        let mut channels = [0u64; 3];
        for pixel in frame.chunks_exact(3) {
            for channel in 0..3 {
                channels[channel] += pixel[channel] as u64;
            }
        }
        let (selected, other) = if index < FPS { (2, 1) } else { (1, 2) };
        assert!(
            channels[selected] > channels[other] * 2,
            "Reverse Time Slice crossed a selected fragment at frame {index}"
        );
    }
    assert_eq!(
        std::fs::read_dir(&environment.cache_root).unwrap().count(),
        0
    );
}
