use cia_render_lib::{
    creative::{apply, CreativeKind},
    creative_schedule::{CreativeEvent, CreativeOptions},
    effects::default_segment_effects,
    jobs::RenderJob,
    plan::{create_plan_internal, generate_plan, validate_plan, ProjectPlan},
    render::{render_pipeline_core, RenderEnvironment},
    scene_analysis::{ActionWindow, MotionSample, PreparedScene},
    sources::SourceRange,
};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

const WIDTH: usize = 320;
const HEIGHT: usize = 180;
const FPS: usize = 16;
const FRAME_BYTES: usize = WIDTH * HEIGHT * 3;
const BETA_KINDS: [CreativeKind; 3] = [
    CreativeKind::FocusPull,
    CreativeKind::ReliefLight,
    CreativeKind::PaperGate,
];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn options(mask: u8) -> CreativeOptions {
    CreativeOptions {
        focus_pull: mask & 1 != 0,
        relief_light: mask & 2 != 0,
        paper_gate: mask & 4 != 0,
        ..CreativeOptions::default()
    }
}

fn prepared_scenes() -> Vec<PreparedScene> {
    (0..3)
        .map(|id| {
            let start = id as f64 * 6.0 + 0.5;
            PreparedScene {
                scene_id: id,
                source_id: 0,
                source_start: start,
                source_end: start + 4.0,
                fingerprint: format!("beta-planning-{id}"),
                fps: FPS as f64,
                fragments: vec![SourceRange {
                    source_id: 0,
                    start,
                    duration: 4.0,
                }],
                motion: (0..48)
                    .map(|index| MotionSample {
                        time: start + index as f64 / 12.0,
                        activity: if (12..30).contains(&index) { 0.7 } else { 0.04 },
                        confidence: 0.9,
                    })
                    .collect(),
                actions: vec![ActionWindow {
                    start: start + 1.0,
                    end: start + 2.5,
                    peak: start + 1.7,
                    activity: 0.63,
                }],
            }
        })
        .collect()
}

fn generated(
    style: &str,
    mode: &str,
    duration: f64,
    beats: Vec<f64>,
    downbeats: Vec<f64>,
    creative: Option<CreativeOptions>,
) -> String {
    generate_plan(
        style.into(),
        FPS as u32,
        beats,
        downbeats,
        22.0,
        duration,
        WIDTH as u32,
        HEIGHT as u32,
        120.0,
        false,
        None,
        None,
        None,
        Some(mode.into()),
        None,
        Some("mid".into()),
        None,
        None,
        (mode != "no_change").then(prepared_scenes),
        Some(false),
        Some(true),
        creative,
    )
    .unwrap()
}

fn full_plan(style: &str, mode: &str, creative: Option<CreativeOptions>) -> String {
    generated(
        style,
        mode,
        20.0,
        (1..80).map(|index| index as f64 * 0.25).collect(),
        (1..20).map(|index| index as f64).collect(),
        creative,
    )
}

fn assert_event_bounds(plan: &ProjectPlan) {
    validate_plan(plan).unwrap();
    let mut previous_start = -1.0;
    for (index, event) in plan.creative_events.iter().enumerate() {
        assert!(event.t0.is_finite() && event.t1.is_finite());
        assert!(event.t0 >= previous_start && event.t0 >= 0.0);
        assert!(event.t1 > event.t0 && event.t1 <= plan.target_duration);
        assert!(event.t1 - event.t0 <= 1.5);
        assert!((0.0..=1.0).contains(&event.strength));
        previous_start = event.t0;
        for other in &plan.creative_events[index + 1..] {
            if event.kind == other.kind {
                assert!(event.t1 <= other.t0 || other.t1 <= event.t0);
            }
        }
    }
}

fn without_creative(plan: &ProjectPlan) -> serde_json::Value {
    let mut value = serde_json::to_value(plan).unwrap();
    value.as_object_mut().unwrap().remove("creative_events");
    value
}

#[test]
fn seven_beta_combinations_preserve_remapping_in_every_style_and_source_mode() {
    for style in ["HARD", "SMOOTH", "HYBRID"] {
        for mode in ["no_change", "scenepack", "long_clip"] {
            let baseline: ProjectPlan =
                serde_json::from_str(&full_plan(style, mode, None)).unwrap();
            assert_eq!(
                full_plan(style, mode, None),
                full_plan(style, mode, Some(CreativeOptions::default()))
            );
            for mask in 1..=7 {
                let json = full_plan(style, mode, Some(options(mask)));
                assert_eq!(json, full_plan(style, mode, Some(options(mask))));
                let plan: ProjectPlan = serde_json::from_str(&json).unwrap();
                assert_event_bounds(&plan);
                for (index, kind) in BETA_KINDS.iter().enumerate() {
                    assert_eq!(
                        plan.creative_events.iter().any(|event| event.kind == *kind),
                        mask & (1 << index) != 0,
                        "{style}, {mode}, mask {mask}: {kind:?}"
                    );
                }
                assert!(plan
                    .creative_events
                    .iter()
                    .all(|event| BETA_KINDS.contains(&event.kind)));
                assert_eq!(without_creative(&plan), without_creative(&baseline));
                let active = (0..FPS * 20)
                    .filter(|frame| {
                        let time = *frame as f64 / FPS as f64;
                        plan.creative_events
                            .iter()
                            .any(|event| time > event.t0 && time < event.t1)
                    })
                    .count();
                assert!(
                    active < FPS * 15,
                    "Beta accents leave too little clean footage"
                );
            }
        }
    }
}

#[test]
fn legacy_three_field_options_keep_their_schedule_and_beta_defaults_off() {
    let legacy: CreativeOptions = serde_json::from_value(serde_json::json!({
        "timeSlice": true,
        "inkPulse": true,
        "prismDrift": true
    }))
    .unwrap();
    assert!(!legacy.focus_pull && !legacy.relief_light && !legacy.paper_gate);
    let explicit = CreativeOptions {
        time_slice: true,
        ink_pulse: true,
        prism_drift: true,
        ..CreativeOptions::default()
    };
    assert_eq!(legacy, explicit);
    assert_eq!(
        full_plan("HARD", "no_change", Some(legacy)),
        full_plan("HARD", "no_change", Some(explicit))
    );
    let all = CreativeOptions {
        time_slice: true,
        ink_pulse: true,
        prism_drift: true,
        ..options(7)
    };
    let mixed: ProjectPlan =
        serde_json::from_str(&full_plan("HARD", "scenepack", Some(all))).unwrap();
    assert_event_bounds(&mixed);
    for kind in [
        CreativeKind::TimeSlice,
        CreativeKind::InkPulse,
        CreativeKind::PrismDrift,
        CreativeKind::FocusPull,
        CreativeKind::ReliefLight,
        CreativeKind::PaperGate,
    ] {
        assert!(mixed.creative_events.iter().any(|event| event.kind == kind));
    }
}

#[test]
fn short_clips_and_sparse_audio_keep_beta_accents_bounded() {
    for duration in [0.13, 0.5, 2.0] {
        for beats in [Vec::new(), vec![duration - 0.01]] {
            for mask in 1..=7 {
                let plan: ProjectPlan = serde_json::from_str(&generated(
                    "HARD",
                    "no_change",
                    duration,
                    beats.clone(),
                    beats.clone(),
                    Some(options(mask)),
                ))
                .unwrap();
                assert_event_bounds(&plan);
                assert!(
                    !plan.creative_events.is_empty(),
                    "No beta accent for duration {duration}, mask {mask}"
                );
            }
        }
    }
}

#[test]
fn beta_accents_have_clean_endpoints_without_an_alternate_frame() {
    let frame: Vec<_> = (0..FRAME_BYTES).map(|index| (index % 251) as u8).collect();
    let mut output = vec![0; FRAME_BYTES];
    for kind in BETA_KINDS {
        for phase in [-0.1, 0.0, 1.0, 1.1, f64::NAN] {
            apply(
                kind,
                &frame,
                None,
                &mut output,
                WIDTH,
                HEIGHT,
                phase,
                37,
                0.9,
            );
            assert_eq!(frame, output, "{kind:?} at phase {phase}");
        }
        apply(kind, &frame, None, &mut output, WIDTH, HEIGHT, 0.5, 37, 0.0);
        assert_eq!(frame, output);
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
    plan.source_ranges = Some(vec![SourceRange {
        source_id: 0,
        start: 0.0,
        duration: 2.0,
    }]);
    for segment in &mut plan.segments {
        segment.s0 = segment.t0;
        segment.s1 = segment.t1;
        segment.effects = default_segment_effects();
        segment.curve = "linear".into();
        segment.remap = None;
    }
    plan
}

fn ffmpeg_file(ffmpeg: &Path, args: &[&str], destination: &Path) {
    let result = Command::new(ffmpeg)
        .args(["-nostdin", "-hide_banner", "-loglevel", "error", "-y"])
        .args(args)
        .arg(destination)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

fn pixels(
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
    let decoded = Command::new(&environment.ffmpeg)
        .args(["-v", "error", "-i"])
        .arg(stats.output_path)
        .args(["-f", "rawvideo", "-pix_fmt", "rgb24", "pipe:1"])
        .output()
        .unwrap();
    assert!(
        decoded.status.success(),
        "{}",
        String::from_utf8_lossy(&decoded.stderr)
    );
    assert_eq!(decoded.stdout.len(), FRAME_BYTES * FPS * 2);
    decoded.stdout
}

fn difference(first: &[u8], second: &[u8]) -> f64 {
    first
        .iter()
        .zip(second)
        .map(|(a, b)| a.abs_diff(*b) as u64)
        .sum::<u64>() as f64
        / first.len() as f64
}

/// All native jobs run sequentially because the render registry is process-wide.
#[test]
fn native_beta_combinations_resolve_to_clean_footage_and_default_off_is_identical() {
    let directory = root().join(format!(
        "tests/generated/creative-beta-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let environment = RenderEnvironment {
        ffmpeg: root().join("src-tauri/binaries/ffmpeg.exe"),
        ffprobe: root().join("src-tauri/binaries/ffprobe.exe"),
        cache_root: directory.join("cache"),
        output_dir: directory.join("output"),
    };
    let source = directory.join("moving source.mp4");
    let audio = directory.join("audio.wav");
    ffmpeg_file(
        &environment.ffmpeg,
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=320x180:rate=16:duration=2",
            "-c:v",
            "libx264",
            "-crf",
            "0",
            "-preset",
            "ultrafast",
            "-pix_fmt",
            "yuv420p",
        ],
        &source,
    );
    ffmpeg_file(
        &environment.ffmpeg,
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
    let source = source.to_string_lossy();
    let audio = audio.to_string_lossy();
    let baseline = neutral_plan();
    let ordinary = pixels(&environment, &source, &audio, "beta-baseline", &baseline);
    let mut default_off = baseline.clone();
    cia_render_lib::creative_schedule::apply_to_plan(
        &mut default_off,
        CreativeOptions::default(),
        &[0.5, 1.0, 1.5],
        &[1.0],
    )
    .unwrap();
    assert_eq!(
        ordinary,
        pixels(
            &environment,
            &source,
            &audio,
            "beta-default-off",
            &default_off
        )
    );
    for mask in [1, 2, 3, 4, 5, 6, 7, 63] {
        let mut plan = baseline.clone();
        let kinds = BETA_KINDS.into_iter().chain([
            CreativeKind::TimeSlice,
            CreativeKind::InkPulse,
            CreativeKind::PrismDrift,
        ]);
        for (index, kind) in kinds.enumerate() {
            if mask & (1 << index) != 0 {
                plan.creative_events.push(CreativeEvent {
                    t0: 0.5,
                    t1: 1.375,
                    kind,
                    seed: 37,
                    strength: 0.9,
                });
            }
        }
        assert_event_bounds(&plan);
        let rendered = pixels(
            &environment,
            &source,
            &audio,
            &format!("beta-mask-{mask}"),
            &plan,
        );
        let mut visible = 0;
        let mut accent_frames = 0;
        let mut accent_delta = 0.0;
        let mut peak_delta = 0.0_f64;
        for (index, (clean, changed)) in ordinary
            .chunks_exact(FRAME_BYTES)
            .zip(rendered.chunks_exact(FRAME_BYTES))
            .enumerate()
        {
            let time = index as f64 / FPS as f64;
            let delta = difference(clean, changed);
            if time > 0.5 && time < 1.375 {
                accent_frames += 1;
                accent_delta += delta;
                peak_delta = peak_delta.max(delta);
                visible += usize::from(delta > 0.5);
            } else {
                // H.264 prediction can change decoded pixels even for unchanged RGB input.
                assert!(
                    delta < 1.5,
                    "Mask {mask} leaked at {time:.3}s: mean delta {delta}"
                );
            }
        }
        let mean_delta = accent_delta / accent_frames as f64;
        println!(
            "Beta mask {mask}: {visible}/{accent_frames} visible accent frames, mean delta {mean_delta:.3}, peak delta {peak_delta:.3}"
        );
        assert!(
            visible >= 3,
            "Mask {mask}: {visible}/{accent_frames} visible accent frames; mean delta {mean_delta:.3}, peak delta {peak_delta:.3}"
        );
    }
    assert_eq!(
        std::fs::read_dir(&environment.cache_root).unwrap().count(),
        0
    );
}
