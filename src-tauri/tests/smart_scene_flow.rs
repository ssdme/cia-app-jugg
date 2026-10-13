use cia_render_lib::{
    jobs::RenderJob,
    plan::{default_effects_for_style, generate_plan, validate_plan, ProjectPlan},
    probe::probe_media_internal,
    render::{render_pipeline_core, RenderEnvironment},
    scene_analysis::{analyze_source, ActionWindow, MotionSample, PreparedScene, SceneSelection},
    sources::SourceRange,
};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    process::Command,
    sync::{atomic::AtomicBool, Arc},
};

fn beats(duration: f64) -> Vec<f64> {
    (1..(duration / 0.48) as usize)
        .map(|index| index as f64 * 0.48 + if index % 4 == 0 { 0.02 } else { 0.0 })
        .collect()
}

fn scenes() -> Vec<PreparedScene> {
    [(2.2, 4.3), (8.4, 5.7), (15.6, 3.6)]
        .into_iter()
        .enumerate()
        .map(|(id, (start, duration))| PreparedScene {
            scene_id: id,
            source_id: id,
            source_start: start,
            source_end: start + duration,
            fingerprint: format!("fixture-{id}"),
            fps: 30.0,
            fragments: vec![
                SourceRange {
                    source_id: id,
                    start,
                    duration: duration * 0.45,
                },
                SourceRange {
                    source_id: id,
                    start: start + duration * 0.45 + 0.2,
                    duration: duration * 0.55 - 0.2,
                },
            ],
            motion: (0..(duration * 12.0) as usize)
                .map(|index| {
                    let local = index as f64 / 12.0;
                    MotionSample {
                        time: start + local,
                        activity: if (local - duration * 0.21).abs() < duration * 0.11
                            || (local - duration * 0.72).abs() < duration * 0.13
                        {
                            0.65
                        } else {
                            0.015
                        },
                        confidence: 0.9,
                    }
                })
                .collect(),
            actions: [(0.1, 0.33, 0.21), (0.59, 0.85, 0.72)]
                .into_iter()
                .map(|(from, to, peak)| ActionWindow {
                    start: start + duration * from,
                    end: start + duration * to,
                    peak: start + duration * peak,
                    activity: 0.585,
                })
                .collect(),
        })
        .collect()
}

fn generated(
    scenes: Option<Vec<PreparedScene>>,
    mode: &str,
    no_repeat: Option<bool>,
    smart: Option<bool>,
    duration: f64,
) -> ProjectPlan {
    generated_with_style(scenes, mode, no_repeat, smart, duration, "SMOOTH")
}

fn generated_with_style(
    scenes: Option<Vec<PreparedScene>>,
    mode: &str,
    no_repeat: Option<bool>,
    smart: Option<bool>,
    duration: f64,
    style: &str,
) -> ProjectPlan {
    let beats = beats(duration);
    let downbeats = beats.iter().skip(3).step_by(4).copied().collect();
    let mut effects = default_effects_for_style(style, false);
    effects.shakes = false;
    effects.zoom = false;
    effects.flicker = false;
    effects.buildup_chain = false;
    effects.zoom_beat_offset = false;
    effects.bouncy_shake = false;
    effects.dissolve_shake = false;
    effects.skew_shake = false;
    effects.squish_pop = false;
    effects.optics_bounce = false;
    effects.warp_stretch = false;
    let json = generate_plan(
        style.into(),
        12,
        beats,
        downbeats,
        8.0,
        duration,
        320,
        180,
        125.0,
        false,
        Some(effects),
        None,
        None,
        Some(mode.into()),
        None,
        Some("fast".into()),
        None,
        None,
        scenes,
        no_repeat,
        smart,
        None,
    )
    .unwrap();
    let plan = serde_json::from_str(&json).unwrap();
    validate_plan(&plan).unwrap();
    plan
}

fn visible_shot_count(plan: &ProjectPlan) -> usize {
    let shots = &plan.planning.as_ref().unwrap().shots;
    1 + shots
        .windows(2)
        .filter(|pair| {
            let last = pair[0].ranges.last().unwrap();
            let first = &pair[1].ranges[0];
            last.source_id != first.source_id
                || (last.start + last.duration - first.start).abs() >= 0.003
        })
        .count()
}

#[test]
fn smart_flow_balances_beat_pacing_with_motion_and_fresh_source_ranges() {
    for style in ["HARD", "SMOOTH"] {
        assert_smart_pacing(style);
    }
}

fn assert_smart_pacing(style: &str) {
    let scenes = scenes();
    let duration = 19.3;
    let generate = |no_repeat, smart| {
        generated_with_style(
            Some(scenes.clone()),
            "scenepack",
            no_repeat,
            smart,
            duration,
            style,
        )
    };
    let no_repeat = generate(Some(true), None);
    let classic = generate(Some(false), None);
    let smart = generate(Some(false), Some(true));
    let planning = smart.planning.as_ref().unwrap();
    assert!(planning.smart_scene_flow);
    assert!(!planning.no_repetitive_scene);
    assert!(planning.motion_guided);
    let counts = [
        visible_shot_count(&no_repeat),
        visible_shot_count(&smart),
        visible_shot_count(&classic),
    ];
    println!("{style} visible shots, No Repeat / Smart / Classic: {counts:?}");
    assert!(
        counts[0] < counts[1] && counts[1] < counts[2],
        "{style} Smart should pace between whole-scene coverage and dense beat cuts: {counts:?}"
    );
    assert!(planning
        .shots
        .iter()
        .any(|shot| { shot.ranges[0].start > scenes[shot.scene_id].source_start + 0.4 }));
    let grid = beats(duration);
    let aligned = planning
        .shots
        .iter()
        .skip(1)
        .filter(|shot| grid.iter().any(|beat| (shot.t0 - beat).abs() < 1e-6))
        .count();
    assert!(aligned * 4 >= (planning.shots.len() - 1) * 3);
    let mut end = 0.0;
    for shot in &planning.shots {
        assert!((shot.t0 - end).abs() < 1e-7);
        end = shot.t1;
        let span: f64 = shot.ranges.iter().map(|range| range.duration).sum();
        let rate = span / (shot.t1 - shot.t0);
        assert!((0.67 - 1e-6..=1.5 + 1e-6).contains(&rate));
        for range in &shot.ranges {
            assert!(
                scenes[shot.scene_id].fragments.iter().any(|fragment| {
                    range.source_id == fragment.source_id
                        && range.start >= fragment.start - 1e-7
                        && range.start + range.duration <= fragment.start + fragment.duration + 1e-7
                }),
                "A shot must never sample a filtered blank gap"
            );
        }
    }
    assert!((end - duration).abs() < 1e-7);
    assert_eq!(smart, generate(Some(false), Some(true)));
}

#[test]
fn smart_precedence_and_legacy_json_defaults_are_stable() {
    let scenes = scenes();
    let smart = generated(
        Some(scenes.clone()),
        "scenepack",
        Some(false),
        Some(true),
        12.7,
    );
    assert_eq!(
        smart,
        generated(
            Some(scenes.clone()),
            "scenepack",
            Some(true),
            Some(true),
            12.7
        )
    );
    let default = generated(Some(scenes.clone()), "scenepack", None, None, 12.7);
    assert_eq!(
        default,
        generated(
            Some(scenes.clone()),
            "scenepack",
            Some(true),
            Some(false),
            12.7
        )
    );
    assert!(default.planning.as_ref().unwrap().no_repetitive_scene);
    assert!(!default.planning.as_ref().unwrap().smart_scene_flow);
    let classic = generated(Some(scenes), "scenepack", Some(false), Some(false), 12.7);
    assert!(!classic.planning.as_ref().unwrap().no_repetitive_scene);
    assert!(!classic.planning.as_ref().unwrap().smart_scene_flow);
    let mut json = serde_json::to_value(&default).unwrap();
    assert_eq!(json["planning"]["smartSceneFlow"], false);
    json["planning"]
        .as_object_mut()
        .unwrap()
        .remove("smartSceneFlow");
    assert_eq!(
        serde_json::from_value::<ProjectPlan>(json).unwrap(),
        default
    );
    assert_eq!(
        serde_json::from_str::<ProjectPlan>(&serde_json::to_string(&smart).unwrap()).unwrap(),
        smart
    );
}

#[test]
fn smart_scene_setting_does_not_change_long_clip_or_no_change() {
    for mode in ["long_clip", "no_change"] {
        let scenes = (mode == "long_clip").then(scenes);
        let original = generated(scenes.clone(), mode, Some(true), Some(false), 8.0);
        let with_setting = generated(scenes, mode, Some(true), Some(true), 8.0);
        assert_eq!(
            original, with_setting,
            "Smart Scene Flow only applies to ScenePack"
        );
    }
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

#[test]
fn analyzed_smart_plan_renders_moving_video_with_audio() {
    let root = root();
    let media = root.join("tests/generated/scenes");
    let source = media.join("motion.mp4");
    assert!(source.exists(), "Run npm run test:media first");
    let environment = RenderEnvironment {
        ffmpeg: root.join("src-tauri/binaries/ffmpeg.exe"),
        ffprobe: root.join("src-tauri/binaries/ffprobe.exe"),
        output_dir: media.join("smart-render"),
        cache_root: media.join("smart-cache"),
    };
    let analysis = analyze_source(
        &environment.ffmpeg,
        &environment.ffprobe,
        &source,
        true,
        Arc::new(AtomicBool::new(false)),
        &mut |_| {},
    )
    .unwrap();
    let scenes = [(0.0, 1.35), (1.35, 3.95), (3.95, 8.0)]
        .into_iter()
        .enumerate()
        .map(|(scene_id, (start, end))| {
            analysis
                .prepare(&SceneSelection {
                    scene_id,
                    source_id: 0,
                    start,
                    end,
                })
                .unwrap()
        })
        .collect();
    let mut plan = generated(Some(scenes), "scenepack", Some(true), Some(true), 8.0);
    assert!(plan.planning.as_ref().unwrap().smart_scene_flow);
    plan.motion_blur = false;
    let source_path = source.to_string_lossy().into_owned();
    let job = RenderJob::begin("smart-scene-smoke".into()).unwrap();
    let result = render_pipeline_core(
        &environment,
        &job,
        &serde_json::to_string(&plan).unwrap(),
        &source_path,
        &media.join("audio.wav").to_string_lossy(),
        false,
        Some(vec![source_path.clone()]),
        &mut |_| {},
    )
    .unwrap();
    let output = probe_media_internal(&result.output_path, Some(&environment.ffprobe)).unwrap();
    assert!((output.duration - 8.0).abs() < 0.1);
    assert_eq!((output.width, output.height), (320, 180));
    assert!(output.audio_channels > 0);
    // Container edit lists can skew the lightweight MP4 probe's FPS estimate.
    let metadata = Command::new(&environment.ffprobe)
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=avg_frame_rate,nb_frames",
            "-of",
            "json",
            &result.output_path,
        ])
        .output()
        .unwrap();
    assert!(metadata.status.success());
    let metadata: serde_json::Value = serde_json::from_slice(&metadata.stdout).unwrap();
    let stream = &metadata["streams"][0];
    let (numerator, denominator) = stream["avg_frame_rate"]
        .as_str()
        .unwrap()
        .split_once('/')
        .unwrap();
    let fps = numerator.parse::<f64>().unwrap() / denominator.parse::<f64>().unwrap();
    assert!(
        (fps - plan.fps as f64).abs() < 0.01,
        "Expected {} FPS, got {fps}",
        plan.fps
    );
    assert_eq!(stream["nb_frames"].as_str().unwrap(), "96");
    let decoded = Command::new(&environment.ffmpeg)
        .args([
            "-v",
            "error",
            "-i",
            &result.output_path,
            "-map",
            "0:v:0",
            "-f",
            "framemd5",
            "pipe:1",
        ])
        .output()
        .unwrap();
    assert!(decoded.status.success());
    let text = String::from_utf8(decoded.stdout).unwrap();
    let hashes: Vec<_> = text
        .lines()
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| line.rsplit(',').next())
        .collect();
    assert_eq!(hashes.len(), 96);
    assert!(
        hashes.iter().collect::<BTreeSet<_>>().len() > hashes.len() / 2,
        "The output must retain motion rather than freeze selected shots"
    );
}
