use cia_render_lib::{
    frame_cache::FrameCache, jobs::*, plan::*, process::*, render::*, sources::SourceRange,
};
use std::{
    io::Read,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}
fn fixture_paths() -> Vec<String> {
    (0..3)
        .map(|id| {
            root()
                .join(format!("tests/generated/source {id} été.mp4"))
                .to_string_lossy()
                .into()
        })
        .collect()
}
fn plan(ranges: Vec<SourceRange>) -> ProjectPlan {
    let duration = ranges.iter().map(|range| range.duration).sum();
    let mut overrides = default_effects_for_style("SMOOTH", false);
    overrides.shakes = false;
    overrides.zoom = false;
    overrides.flicker = false;
    overrides.buildup_chain = false;
    overrides.zoom_beat_offset = false;
    let mut plan = create_plan_internal(
        "SMOOTH",
        12,
        &[],
        &[],
        duration,
        1.0,
        320,
        180,
        120.0,
        false,
        Some(overrides),
        None,
        None,
        None,
        None,
    )
    .unwrap();
    plan.schema_version = 3;
    plan.source_ranges = Some(ranges);
    plan.video_duration = duration;
    plan.motion_blur = false;
    plan
}
fn average_rgb(path: &Path) -> [u64; 3] {
    average_rgb_at(path, 0.0)
}
fn average_rgb_at(path: &Path, time: f64) -> [u64; 3] {
    let output = Command::new(root().join("src-tauri/binaries/ffmpeg.exe"))
        .args(["-v", "error", "-ss", &time.to_string(), "-i"])
        .arg(path)
        .args([
            "-frames:v",
            "1",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "pipe:1",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let mut channels = [0; 3];
    for pixel in output.stdout.chunks_exact(3) {
        for index in 0..3 {
            channels[index] += pixel[index] as u64;
        }
    }
    let pixels = output.stdout.len() as u64 / 3;
    assert!(pixels > 0);
    channels.map(|value| value / pixels)
}

/// One sequential integration harness also exercises the single-render registry.
/// Missing tools or failed fixture generation are failures, never skipped passes.
#[test]
fn portable_render_regressions() {
    assert!(Command::new("node")
        .arg(root().join("scripts/generate-test-media.mjs"))
        .current_dir(root())
        .status()
        .expect("Node and the verified FFmpeg payload are required")
        .success());
    let workspace =
        std::env::temp_dir().join(format!("cia-render-regression-{}", std::process::id()));
    std::fs::create_dir_all(&workspace).unwrap();
    let environment = RenderEnvironment {
        ffmpeg: root().join("src-tauri/binaries/ffmpeg.exe"),
        ffprobe: root().join("src-tauri/binaries/ffprobe.exe"),
        cache_root: workspace.join("cache"),
        output_dir: workspace.join("output"),
    };
    let paths = fixture_paths();
    let audio = root()
        .join("tests/generated/audio.wav")
        .to_string_lossy()
        .into_owned();
    let audio_info =
        cia_render_lib::probe::probe_media_internal(&audio, Some(&environment.ffprobe)).unwrap();
    assert!((audio_info.duration - 2.0).abs() < 0.02);
    assert_eq!(audio_info.audio_sample_rate, 44100);
    assert_eq!(audio_info.audio_channels, 1);
    // Exercise the exact native beat payload shipped by the installer, without Python or cloud APIs.
    let beats = Command::new(root().join("src-tauri/binaries/beat_this.exe"))
        .arg(root().join("src-tauri/binaries/beat_this.onnx"))
        .arg(&audio)
        .arg("--json")
        .current_dir(root().join("src-tauri/binaries"))
        .output()
        .unwrap();
    assert!(
        beats.status.success(),
        "{}",
        String::from_utf8_lossy(&beats.stderr)
    );
    let beats: cia_render_lib::beat::BeatResult = serde_json::from_slice(&beats.stdout).unwrap();
    assert!(beats.bpm.is_finite());
    assert!(beats
        .beats
        .iter()
        .chain(&beats.downbeats)
        .all(|time| time.is_finite() && *time >= 0.0 && *time <= 2.0));
    for (id, expected) in [
        (0, [true, false, false]),
        (1, [false, true, false]),
        (2, [false, false, true]),
    ] {
        let job = RenderJob::begin(format!("source-{id}")).unwrap();
        let plan = plan(vec![SourceRange {
            source_id: id,
            start: 0.5,
            duration: 1.0,
        }]);
        let mut events = Vec::new();
        let stats = render_pipeline_core(
            &environment,
            &job,
            &serde_json::to_string(&plan).unwrap(),
            &paths[0],
            &audio,
            false,
            Some(paths.clone()),
            &mut |event| events.push(event),
        )
        .unwrap();
        let rgb = average_rgb(Path::new(&stats.output_path));
        for channel in 0..3 {
            assert_eq!(
                rgb[channel] > 180,
                expected[channel],
                "Wrong selected source: {rgb:?}"
            );
        }
        assert!(events.iter().all(|event| event.job_id == job.id));
        assert_eq!(events.last().unwrap().percent, 100);
        assert!(stats.peak_cache_bytes <= cia_render_lib::frame_cache::DEFAULT_DISK_BUDGET);
        assert!(stats.decoded_bytes < 3 * 160 * 90 * 3 * 60);
    }
    // A reverse at an exact compact source boundary belongs to the left range.
    let mut reversed = plan(vec![
        SourceRange {
            source_id: 0,
            start: 0.5,
            duration: 1.0,
        },
        SourceRange {
            source_id: 2,
            start: 0.5,
            duration: 1.0,
        },
    ]);
    reversed.segments.truncate(1);
    let segment = &mut reversed.segments[0];
    segment.t0 = 0.0;
    segment.t1 = 1.0;
    segment.s0 = 2.0;
    segment.s1 = 0.0;
    segment.curve = "linear".into();
    segment.effects = cia_render_lib::effects::default_segment_effects();
    reversed.ambiance = None;
    reversed.transitions.clear();
    reversed.one_framers.clear();
    let job = RenderJob::begin("reverse-boundary".into()).unwrap();
    let output = render_pipeline_core(
        &environment,
        &job,
        &serde_json::to_string(&reversed).unwrap(),
        &paths[0],
        &audio,
        false,
        Some(paths.clone()),
        &mut |_| {},
    )
    .unwrap();
    assert!(average_rgb_at(Path::new(&output.output_path), 0.0)[2] > 200);
    assert!(average_rgb_at(Path::new(&output.output_path), 0.5)[0] > 200);
    drop(job);

    // Every advertised codec/container pair must actually encode with the payload.
    for (codec, format, expected) in [
        ("H264", "MP4", "h264"),
        ("H265", "MKV", "hevc"),
        ("VP9", "WEBM", "vp9"),
        ("H265", "MP4", "hevc"),
        ("VP9", "MKV", "vp9"),
        ("VP9", "MP4", "vp9"),
        ("H264", "MKV", "h264"),
    ] {
        let job = RenderJob::begin(format!("codec-{codec}-{format}")).unwrap();
        let mut codec_plan = plan(vec![SourceRange {
            source_id: 0,
            start: 0.0,
            duration: 1.0,
        }]);
        codec_plan.export.codec = codec.into();
        codec_plan.export.format = format.into();
        let stats = render_pipeline_core(
            &environment,
            &job,
            &serde_json::to_string(&codec_plan).unwrap(),
            &paths[0],
            &audio,
            false,
            None,
            &mut |_| {},
        )
        .unwrap();
        let probe = Command::new(&environment.ffprobe)
            .args([
                "-v",
                "error",
                "-show_streams",
                "-show_format",
                "-of",
                "json",
            ])
            .arg(&stats.output_path)
            .output()
            .unwrap();
        assert!(probe.status.success());
        let info: serde_json::Value = serde_json::from_slice(&probe.stdout).unwrap();
        let video = info["streams"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["codec_type"] == "video")
            .unwrap();
        assert_eq!(video["codec_name"], expected);
        assert_eq!(video["width"], 320);
        assert_eq!(video["height"], 180);
        assert_eq!(video["r_frame_rate"], "12/1");
        let duration: f64 = info["format"]["duration"]
            .as_str()
            .unwrap()
            .parse()
            .unwrap();
        assert!(
            (duration - 1.0).abs() < 0.15,
            "Unexpected output duration {duration}"
        );
    }

    // These parameters used to be stored but ignored by the render pipeline.
    let pattern = root()
        .join("tests/generated/pattern.mp4")
        .to_string_lossy()
        .into_owned();
    let mut neutral = plan(vec![SourceRange {
        source_id: 0,
        start: 0.0,
        duration: 1.0,
    }]);
    neutral.ambiance = None;
    neutral.transitions.clear();
    neutral.one_framers.clear();
    for segment in &mut neutral.segments {
        segment.effects = cia_render_lib::effects::default_segment_effects();
    }
    let render_pixels = |id: &str, settings: &ProjectPlan| {
        let job = RenderJob::begin(id.replace('_', "-")).unwrap();
        let stats = render_pipeline_core(
            &environment,
            &job,
            &serde_json::to_string(settings).unwrap(),
            &pattern,
            &audio,
            false,
            None,
            &mut |_| {},
        )
        .unwrap();
        let raw = Command::new(&environment.ffmpeg)
            .args(["-v", "error", "-i"])
            .arg(&stats.output_path)
            .args(["-f", "rawvideo", "-pix_fmt", "rgb24", "pipe:1"])
            .output()
            .unwrap();
        assert!(raw.status.success());
        raw.stdout
    };
    let neutral_pixels = render_pixels("transition-neutral", &neutral);
    for (kind, key, amount) in [
        ("WARP_BUBBLE", "amplitude", 0.8),
        ("WAVE_WARP", "height", 100.0),
        ("SLIDE_SHAKE", "amplitude", 100.0),
    ] {
        let mut effect = neutral.clone();
        effect
            .transitions
            .push(cia_render_lib::effects::TransitionItem {
                t: 0.5,
                transition_type: kind.into(),
                duration_frames: 6,
                is_wrap: false,
                params: serde_json::json!({key:0.0}),
            });
        assert_eq!(
            render_pixels(&format!("zero-{kind}"), &effect),
            neutral_pixels
        );
        effect.transitions[0].params = serde_json::json!({key:amount});
        let changed = render_pixels(&format!("active-{kind}"), &effect);
        assert_eq!(changed.len(), neutral_pixels.len());
        assert!(
            changed
                .iter()
                .zip(&neutral_pixels)
                .filter(|(a, b)| a != b)
                .count()
                > changed.len() / 100
        );
    }
    // Mixed FPS, discontinuous sources, tiny disk budget, eviction and reverse reads.
    let directory = JobDirectory::create(&environment.cache_root, "block-test").unwrap();
    let bytes = 160 * 90 * 3;
    let ranges = vec![
        SourceRange {
            source_id: 0,
            start: 0.5,
            duration: 1.0,
        },
        SourceRange {
            source_id: 2,
            start: 1.0,
            duration: 1.0,
        },
    ];
    let mut cache = FrameCache::new(
        environment.ffmpeg.clone(),
        paths.clone(),
        ranges,
        directory.0.clone(),
        160,
        90,
        30.0,
        Arc::new(AtomicBool::new(false)),
        bytes * 2,
        (bytes * 4) as u64,
    )
    .unwrap();
    let mut buffer = vec![0; bytes];
    for time in [0.0, 1.0, 1.9, 0.1, 1.2, 0.9] {
        let (frame, first, last) = cache.frame_at_time(time);
        assert!(frame >= first && frame <= last);
        cache.get_frame(frame, &mut buffer).unwrap();
        assert!(buffer[if time < 1.0 { 0 } else { 2 }] > 200);
        assert!(cache.peak_disk_bytes <= (bytes * 4) as u64);
        assert!(cache.peak_ram_bytes <= (bytes * 2) as u64);
    }
    assert!(cache.cache_misses > 0);
    assert!(cia_render_lib::frame_cache::ensure_disk_space(&directory.0, u64::MAX).is_err());
    // Metadata fingerprints invalidate a replacement even at the same canonical path.
    let replaceable = workspace.join("fingerprint.txt");
    std::fs::write(&replaceable, b"before").unwrap();
    let fingerprints = || {
        cia_render_lib::probe::media_fingerprints(vec![replaceable.to_string_lossy().into_owned()])
            .unwrap()
    };
    let before = fingerprints();
    std::fs::write(&replaceable, b"after replacement").unwrap();
    assert_ne!(before, fingerprints());
    // Cancel from sampling: encoder/decoder cleanup precedes registry release.
    let job = RenderJob::begin("cancel-sampling".into()).unwrap();
    let cancel_flag = job.cancelled.clone();
    let plan_json = serde_json::to_string(&plan(vec![SourceRange {
        source_id: 1,
        start: 0.0,
        duration: 1.0,
    }]))
    .unwrap();
    let result = render_pipeline_core(
        &environment,
        &job,
        &plan_json,
        &paths[0],
        &audio,
        false,
        Some(paths.clone()),
        &mut |event| {
            if event.phase == "SAMPLING" {
                cancel_flag.store(true, Ordering::Release);
            }
        },
    );
    assert!(result.unwrap_err().contains("cancelled"));
    drop(job);
    assert_eq!(
        std::fs::read_dir(&environment.cache_root).unwrap().count(),
        1
    ); // the block-test directory is still owned
    assert!(RenderJob::begin("restart".into()).is_ok());
    for phase in ["DECODING", "ENCODING"] {
        let job = RenderJob::begin(format!("cancel-{phase}")).unwrap();
        let flag = job.cancelled.clone();
        let result = render_pipeline_core(
            &environment,
            &job,
            &plan_json,
            &paths[0],
            &audio,
            false,
            Some(paths.clone()),
            &mut |event| {
                if event.phase == phase {
                    flag.store(true, Ordering::Release);
                }
            },
        );
        assert!(result.unwrap_err().contains("cancelled"));
        drop(job);
        assert_eq!(
            std::fs::read_dir(&environment.cache_root).unwrap().count(),
            1
        );
    }
    let blocked = workspace.join("blocked-output");
    std::fs::write(&blocked, b"file").unwrap();
    let blocked_environment = RenderEnvironment {
        ffmpeg: environment.ffmpeg.clone(),
        ffprobe: environment.ffprobe.clone(),
        cache_root: environment.cache_root.clone(),
        output_dir: blocked,
    };
    let job = RenderJob::begin("blocked-output".into()).unwrap();
    assert!(render_pipeline_core(
        &blocked_environment,
        &job,
        &plan_json,
        &paths[0],
        &audio,
        false,
        Some(paths.clone()),
        &mut |_| {}
    )
    .unwrap_err()
    .contains("output directory"));
    drop(job);
    assert_eq!(
        std::fs::read_dir(&environment.cache_root).unwrap().count(),
        1
    );
    // Publishing never overwrites an existing output.
    let temp = environment.output_dir.join("publish.tmp");
    std::fs::write(&temp, b"first").unwrap();
    let first = publish_output(&temp, &environment.output_dir, "collision", "mp4").unwrap();
    std::fs::write(&temp, b"second").unwrap();
    let second = publish_output(&temp, &environment.output_dir, "collision", "mp4").unwrap();
    assert_ne!(first, second);
    assert_eq!(std::fs::read(first).unwrap(), b"first");
    // A later invalid source must fail rather than reusing preceding frames.
    let job = RenderJob::begin("invalid-second".into()).unwrap();
    let invalid = workspace.join("corrupt.mp4");
    std::fs::write(&invalid, b"invalid media").unwrap();
    let mut invalid_paths = paths;
    invalid_paths[1] = invalid.to_string_lossy().into();
    assert!(render_pipeline_core(
        &environment,
        &job,
        &plan_json,
        &invalid_paths[0],
        &audio,
        false,
        Some(invalid_paths.clone()),
        &mut |_| {}
    )
    .is_err());
    drop(cache);
    drop(directory);
    drop(job);
    std::fs::remove_dir_all(workspace).unwrap();
}

#[test]
fn settings_validation_and_presets() {
    let hard = cmd_get_style_preset("HARD".into(), true).unwrap();
    let smooth = cmd_get_style_preset("SMOOTH".into(), false).unwrap();
    assert_ne!(hard.params, smooth.params);
    assert!(hard.effects.one_framers);
    assert!(!smooth.effects.one_framers);
    let json = serde_json::to_value(hard.effects).unwrap();
    for key in [
        "oneFramers",
        "bouncyShake",
        "dissolveShake",
        "ccDeepDark",
        "antiFlash",
    ] {
        assert!(json.get(key).is_some());
    }
    assert!(json.get("one_framers").is_none());
    let mut plan = plan(vec![SourceRange {
        source_id: 0,
        start: 0.0,
        duration: 1.0,
    }]);
    assert!(validate_plan(&plan).is_ok());
    plan.ambiance = Some(cia_render_lib::effects::default_ambiance("SMOOTH", &[]));
    plan.ambiance.as_mut().unwrap().echo_trail.alpha = 2.0;
    assert!(validate_plan(&plan).is_err());
    plan.ambiance.as_mut().unwrap().echo_trail.alpha = 0.3;
    plan.export.format = "WEBM".into();
    assert!(validate_plan(&plan).is_err());
    plan.export.codec = "VP9".into();
    assert!(validate_plan(&plan).is_ok());
    plan.aspect.w = 0;
    assert!(validate_plan(&plan).is_err());
    plan.aspect.w = 321;
    assert!(validate_plan(&plan).is_err());
    plan.aspect.w = 320;
    plan.segments.clear();
    assert!(validate_plan(&plan).is_err());
    let mut params = get_style_defaults("HARD");
    params.shake_a0 = f64::NAN;
    assert!(validate_custom_params(&params).is_err());
    let (x, y) = cia_render_lib::effects::compute_squish_pop_with_limits(1.0, 0.7, 1.3);
    assert!((x - 1.3).abs() < 1e-10 && (y - 0.7).abs() < 1e-10);
    assert_eq!(
        cia_render_lib::effects::compute_squish_pop_with_limits(1.0, 1.0, 1.0),
        (1.0, 1.0)
    );
    assert_eq!(
        parse_scene_ranges(&[0.0, 10.0, 20.0, 30.0, 40.0, 60.0], 60.0).len(),
        5
    );

    // The command slices all modes into source-local schema-3 ranges.
    for mode in ["no_change", "long_clip", "scenepack"] {
        let generated = generate_plan(
            "HARD".into(),
            16,
            vec![0.5],
            vec![],
            12.0,
            1.0,
            320,
            180,
            120.0,
            true,
            None,
            None,
            None,
            Some(mode.into()),
            None,
            None,
            Some("pairs".into()),
            Some(vec![SourceRange {
                source_id: 2,
                start: 0.5,
                duration: 12.0,
            }]),
            None,
            None,
        )
        .unwrap();
        let plan: ProjectPlan = serde_json::from_str(&generated).unwrap();
        assert_eq!(plan.schema_version, 3);
        let ranges = plan.source_ranges.unwrap();
        assert!(ranges
            .iter()
            .all(|range| range.source_id == 2 && range.start >= 0.5));
        assert!(
            (ranges.iter().map(|range| range.duration).sum::<f64>() - plan.video_duration).abs()
                < 0.002
        );
    }
}

#[test]
#[cfg(windows)]
fn process_stderr_and_blocked_pipe_cancellation() {
    let flag = Arc::new(AtomicBool::new(false));
    let mut command = Command::new("powershell");
    command
        .args([
            "-NoProfile",
            "-Command",
            "[Console]::Error.Write(('x' * 2000000)); [Console]::Out.Write('ready')",
        ])
        .stdout(std::process::Stdio::piped());
    let mut child = ManagedChild::spawn(&mut command, flag.clone()).unwrap();
    let mut stdout = String::new();
    child
        .stdout
        .take()
        .unwrap()
        .read_to_string(&mut stdout)
        .unwrap();
    let (status, tail) = child.wait().unwrap();
    assert!(status.success());
    assert_eq!(stdout, "ready");
    assert!(tail.len() <= 65536);
    let mut command = Command::new("powershell");
    command
        .args(["-NoProfile", "-Command", "Start-Sleep -Seconds 60"])
        .stdout(std::process::Stdio::piped());
    let mut child = ManagedChild::spawn(&mut command, flag.clone()).unwrap();
    let started = Instant::now();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        flag.store(true, Ordering::Release);
    });
    let mut bytes = Vec::new();
    child
        .stdout
        .take()
        .unwrap()
        .read_to_end(&mut bytes)
        .unwrap();
    child.wait().unwrap();
    assert!(started.elapsed() < Duration::from_secs(3));
    // Cancellation also releases an encoder write blocked on a full stdin pipe.
    use std::io::Write;
    let flag = Arc::new(AtomicBool::new(false));
    let mut command = Command::new("powershell");
    command
        .args(["-NoProfile", "-Command", "Start-Sleep -Seconds 60"])
        .stdin(std::process::Stdio::piped());
    let mut child = ManagedChild::spawn(&mut command, flag.clone()).unwrap();
    let started = Instant::now();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        flag.store(true, Ordering::Release);
    });
    assert!(child
        .stdin
        .take()
        .unwrap()
        .write_all(&vec![0; 8 * 1024 * 1024])
        .is_err());
    child.wait().unwrap();
    assert!(started.elapsed() < Duration::from_secs(3));
}
