use cia_render_lib::{
    analysis::{self, AnalysisJob},
    frame_cache::FrameCache,
    jobs::RenderJob,
    plan::{default_effects_for_style, generate_plan, ProjectPlan},
    probe::probe_media_internal,
    render::{render_pipeline_core, RenderEnvironment},
    scene_analysis::{analyze_source, SceneSelection},
};
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Instant,
};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}
fn source(name: &str) -> PathBuf {
    root().join("tests/generated/scenes").join(name)
}
fn binaries() -> (PathBuf, PathBuf) {
    (
        root().join("src-tauri/binaries/ffmpeg.exe"),
        root().join("src-tauri/binaries/ffprobe.exe"),
    )
}

#[test]
fn native_precision_gaps_timestamps_cache_and_cancellation() {
    let (ffmpeg, ffprobe) = binaries();
    let path = source("native gaps.mkv");
    assert!(path.exists(), "Run npm run test:media first");
    let start = Instant::now();
    let analysis = analyze_source(
        &ffmpeg,
        &ffprobe,
        &path,
        true,
        Arc::new(AtomicBool::new(false)),
        &mut |_| {},
    )
    .unwrap();
    let cold = start.elapsed();
    assert_eq!(analysis.decoded_frames, 20);
    assert_eq!(
        analysis.removed_frames, 6,
        "Near-white, near-black and one distinct 16-bit pixel must survive"
    );
    assert_eq!(analysis.fragments.len(), 2);
    assert!((analysis.fragments[0].0 - 0.2).abs() < 0.001);
    assert!((analysis.fragments[0].1 - 0.8).abs() < 0.001);
    assert!((analysis.fragments[1].0 - 1.0).abs() < 0.001);
    assert!((analysis.fragments[1].1 - 1.8).abs() < 0.001);
    assert!(analysis
        .motion
        .iter()
        .any(|sample| sample.activity * sample.confidence > 0.015));
    let range = analysis
        .prepare(&SceneSelection {
            scene_id: 0,
            source_id: 0,
            start: 0.0,
            end: 2.0,
        })
        .unwrap()
        .fragments
        .remove(0);
    std::fs::create_dir_all(source("trim-cache")).unwrap();
    let mut frames = FrameCache::new(
        ffmpeg.clone(),
        vec![path.to_string_lossy().into_owned()],
        vec![range.clone()],
        source("trim-cache"),
        64,
        40,
        10.0,
        Arc::new(AtomicBool::new(false)),
        65536,
        65536,
    )
    .unwrap();
    let (last, _, _) = frames.frame_at_time(range.duration - 1e-6);
    let mut pixels = vec![0; 64 * 40 * 3];
    frames.get_frame(last, &mut pixels).unwrap();
    assert!(
        pixels.iter().any(|&pixel| pixel < 240),
        "FPS conversion must not read the excluded next white frame"
    );
    let start = Instant::now();
    let cached = analyze_source(
        &ffmpeg,
        &ffprobe,
        &path,
        true,
        Arc::new(AtomicBool::new(false)),
        &mut |_| {},
    )
    .unwrap();
    assert!(Arc::ptr_eq(&analysis, &cached));
    println!(
        "Cold analysis: {:?}; cached: {:?}; frames: {}",
        cold,
        start.elapsed(),
        analysis.decoded_frames
    );
    let vfr = analyze_source(
        &ffmpeg,
        &ffprobe,
        &source("variable gaps.mkv"),
        true,
        Arc::new(AtomicBool::new(false)),
        &mut |_| {},
    )
    .unwrap();
    assert_eq!(vfr.removed_frames, 6);
    assert!(
        (vfr.fragments[1].1 - 2.6).abs() < 0.001,
        "Use VFR timestamps rather than a nominal frame count"
    );
    for name in ["black.mkv", "white.mkv"] {
        let blank = analyze_source(
            &ffmpeg,
            &ffprobe,
            &source(name),
            true,
            Arc::new(AtomicBool::new(false)),
            &mut |_| {},
        )
        .unwrap();
        assert!(
            blank.fragments.is_empty(),
            "{name} should contain no usable frames"
        );
        assert!(blank
            .prepare(&SceneSelection {
                scene_id: 7,
                source_id: 0,
                start: 0.0,
                end: blank.info.duration
            })
            .is_none());
    }
    let cancelled = Arc::new(AtomicBool::new(false));
    let flag = cancelled.clone();
    let cancel_path = source("cancel-motion.mp4");
    std::fs::copy(source("motion.mp4"), &cancel_path).unwrap();
    let result = analyze_source(
        &ffmpeg,
        &ffprobe,
        &cancel_path,
        true,
        cancelled,
        &mut |_| {
            flag.store(true, Ordering::Release);
        },
    );
    assert!(result.unwrap_err().contains("cancelled"));
    std::fs::remove_file(cancel_path).unwrap();
    analysis::cancel("analysis-before-start").unwrap();
    assert!(AnalysisJob::begin("analysis-before-start".into())
        .err()
        .unwrap()
        .contains("cancelled"));
}

#[test]
fn analyzed_plan_roundtrip_motion_render_and_stale_source_guard() {
    let (ffmpeg, ffprobe) = binaries();
    let path = source("motion.mp4");
    let analysis = analyze_source(
        &ffmpeg,
        &ffprobe,
        &path,
        true,
        Arc::new(AtomicBool::new(false)),
        &mut |_| {},
    )
    .unwrap();
    let mut scenes = Vec::new();
    for (scene_id, (start, end)) in [(0.0, 2.3), (2.3, 5.5), (5.5, 8.0)].into_iter().enumerate() {
        scenes.push(
            analysis
                .prepare(&SceneSelection {
                    scene_id,
                    source_id: 0,
                    start,
                    end,
                })
                .unwrap(),
        );
    }
    let beats: Vec<_> = (1..16).map(|index| index as f64 / 2.0).collect();
    let mut effects = default_effects_for_style("HARD", false);
    effects.shakes = false;
    effects.zoom = false;
    effects.flicker = false;
    effects.bouncy_shake = false;
    effects.dissolve_shake = false;
    effects.skew_shake = false;
    effects.squish_pop = false;
    effects.optics_bounce = false;
    effects.buildup_chain = false;
    effects.warp_stretch = false;
    effects.zoom_beat_offset = false;
    let json = generate_plan(
        "HARD".into(),
        12,
        beats.clone(),
        vec![2.0, 4.0, 6.0],
        8.0,
        8.0,
        160,
        90,
        120.0,
        false,
        Some(effects.clone()),
        None,
        None,
        Some("scenepack".into()),
        None,
        Some("fast".into()),
        None,
        None,
        Some(scenes.clone()),
        Some(true),
    )
    .unwrap();
    let mut plan: ProjectPlan = serde_json::from_str(&json).unwrap();
    assert_eq!(plan.schema_version, 4);
    assert!(plan
        .segments
        .iter()
        .any(|segment| segment.remap.as_ref().unwrap().strength > 0.0));
    assert!(plan
        .planning
        .as_ref()
        .unwrap()
        .shots
        .iter()
        .all(|shot| shot.reused_fraction < 0.01));
    plan.motion_blur = false;
    let json = serde_json::to_string(&plan).unwrap();
    let environment = RenderEnvironment {
        ffmpeg,
        ffprobe,
        output_dir: source("renders"),
        cache_root: source("cache"),
    };
    let job = RenderJob::begin("motion-native-smoke".into()).unwrap();
    let paths = Some(vec![path.to_string_lossy().into_owned()]);
    let result = render_pipeline_core(
        &environment,
        &job,
        &json,
        &path.to_string_lossy(),
        &source("audio.wav").to_string_lossy(),
        false,
        paths.clone(),
        &mut |_| {},
    )
    .unwrap();
    println!(
        "Motion smoke render: {}; {:.2}s",
        result.output_path, result.render_time_secs
    );
    let output = probe_media_internal(&result.output_path, Some(&environment.ffprobe)).unwrap();
    assert!((output.duration - 8.0).abs() < 0.1);
    plan.planning.as_mut().unwrap().source_fingerprints[0].fingerprint = "stale".into();
    let stale = render_pipeline_core(
        &environment,
        &job,
        &serde_json::to_string(&plan).unwrap(),
        &path.to_string_lossy(),
        &source("audio.wav").to_string_lossy(),
        false,
        paths,
        &mut |_| {},
    );
    assert!(stale.unwrap_err().contains("changed after motion analysis"));
    drop(job);
    let legacy = generate_plan(
        "HARD".into(),
        12,
        beats.clone(),
        vec![2.0, 4.0, 6.0],
        8.0,
        8.0,
        160,
        90,
        120.0,
        false,
        Some(effects),
        None,
        None,
        Some("scenepack".into()),
        None,
        Some("fast".into()),
        None,
        None,
        Some(scenes),
        Some(false),
    )
    .unwrap();
    let legacy: ProjectPlan = serde_json::from_str(&legacy).unwrap();
    assert!(!legacy.planning.as_ref().unwrap().no_repetitive_scene);
    assert!(
        legacy.planning.as_ref().unwrap().shots.len() > plan.planning.as_ref().unwrap().shots.len()
    );
    let long = analyze_source(
        &environment.ffmpeg,
        &environment.ffprobe,
        &path,
        false,
        Arc::new(AtomicBool::new(false)),
        &mut |_| {},
    )
    .unwrap();
    assert_eq!(long.removed_frames, 0);
    assert!(
        long.decoded_frames < analysis.decoded_frames,
        "Long Clip needs only a small motion stream"
    );
}
