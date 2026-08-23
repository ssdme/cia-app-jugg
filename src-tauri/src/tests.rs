use super::*;

#[test]
fn test_shake_envelope_boundaries() {
    let fps = 16.0;
    let duration = 2.0;
    let dt_frame = 1.0 / fps;
    let two_frames = 2.0 * dt_frame;

    let env_start = compute_shake_envelope(0.0, duration, fps);
    assert_eq!(env_start, 0.0, "Envelope at t=0 must be 0.0");

    let env_2_frames = compute_shake_envelope(two_frames, duration, fps);
    assert!((env_2_frames - 1.0).abs() < 1e-6, "Envelope at t=2 frames must be 1.0 (got {})", env_2_frames);

    let env_mid = compute_shake_envelope(duration / 2.0, duration, fps);
    assert!((env_mid - 1.0).abs() < 1e-6, "Envelope in mid segment must be 1.0 (got {})", env_mid);

    let env_end_minus_2 = compute_shake_envelope(duration - two_frames, duration, fps);
    assert!((env_end_minus_2 - 1.0).abs() < 1e-6, "Envelope at t=end-2 frames must be 1.0 (got {})", env_end_minus_2);

    let env_end = compute_shake_envelope(duration, duration, fps);
    assert_eq!(env_end, 0.0, "Envelope at t=end must be 0.0");
}

#[test]
fn test_zoom_continuity() {
    let beats = vec![0.5, 1.0, 1.5, 2.0, 2.5, 3.0];
    let downbeats = vec![1.0, 2.0, 3.0];

    for style in ["HARD", "SMOOTH", "HYBRID"] {
        let plan = create_plan_internal(style, 16, &beats, &downbeats, 5.0, 3.5, 1080, 1080, 120.0, true, None, None).unwrap();

        for win in plan.segments.windows(2) {
            let seg_n = &win[0];
            let seg_n1 = &win[1];
            assert_eq!(
                seg_n.effects.zoom.scale_end,
                seg_n1.effects.zoom.scale_start,
                "Zoom continuity broken between segments in style {}",
                style
            );
        }
    }
}

#[test]
fn test_reverse_remap_planner() {
    let beats = vec![
        0.42, 1.14, 1.88, 2.60, 3.32, 4.04, 4.76, 5.50, 6.20, 6.94, 7.64, 8.38, 9.10, 9.82,
        10.54, 11.26, 11.98, 12.70, 13.42, 14.14,
    ];
    let downbeats = vec![2.60, 5.50, 8.38, 11.26, 14.14];

    let hard_plan = create_plan_internal("HARD", 16, &beats, &downbeats, 10.773, 14.315, 1080, 1080, 83.33, true, None, None).unwrap();
    let mut reverse_found = false;

    for seg in &hard_plan.segments {
        if seg.effects.reverse {
            assert!(seg.s1 < seg.s0, "Reversed segment must have s1 < s0 (got s0={}, s1={})", seg.s0, seg.s1);
            reverse_found = true;
        } else {
            assert!(seg.s0 < seg.s1, "Normal segment must have s0 < s1 (got s0={}, s1={})", seg.s0, seg.s1);
        }
    }
    assert!(reverse_found, "HARD style on fixture must contain at least one reversed downbeat segment");

    let smooth_plan = create_plan_internal("SMOOTH", 16, &beats, &downbeats, 10.773, 14.315, 1080, 1080, 83.33, true, None, None).unwrap();
    for seg in &smooth_plan.segments {
        assert!(!seg.effects.reverse, "SMOOTH style must not contain reversed segments");
        assert!(seg.s0 < seg.s1);
    }
}

#[test]
fn test_mirror_coordinate_and_sample_pixel_mirrored() {
    let w = 100usize;
    let h = 100usize;

    assert_eq!(mirror_coordinate(-5, w), 5);
    assert_eq!(mirror_coordinate(0, w), 0);
    assert_eq!(mirror_coordinate((w - 1) as i64, w), w - 1);
    assert_eq!(mirror_coordinate((w + 3) as i64, w), w - 4);

    assert_eq!(mirror_coordinate(-5, h), 5);
    assert_eq!(mirror_coordinate(0, h), 0);
    assert_eq!(mirror_coordinate((h - 1) as i64, h), h - 1);
    assert_eq!(mirror_coordinate((h + 3) as i64, h), h - 4);

    let mut test_image = vec![0u8; w * h * 3];
    let idx_left = (10 * w + 5) * 3;
    test_image[idx_left] = 255;
    test_image[idx_left + 1] = 128;
    test_image[idx_left + 2] = 64;

    let sampled_left = sample_pixel_mirrored(&test_image, w, h, -5, 10);
    assert_eq!(sampled_left, [255, 128, 64]);

    let idx_right = (10 * w + (w - 4)) * 3;
    test_image[idx_right] = 10;
    test_image[idx_right + 1] = 20;
    test_image[idx_right + 2] = 30;

    let sampled_right = sample_pixel_mirrored(&test_image, w, h, (w + 3) as i64, 10);
    assert_eq!(sampled_right, [10, 20, 30]);

    let idx_top = (5 * w + 10) * 3;
    test_image[idx_top] = 40;
    test_image[idx_top + 1] = 50;
    test_image[idx_top + 2] = 60;

    let sampled_top = sample_pixel_mirrored(&test_image, w, h, 10, -5);
    assert_eq!(sampled_top, [40, 50, 60]);

    let idx_bottom = ((h - 4) * w + 10) * 3;
    test_image[idx_bottom] = 70;
    test_image[idx_bottom + 1] = 80;
    test_image[idx_bottom + 2] = 90;

    let sampled_bottom = sample_pixel_mirrored(&test_image, w, h, 10, (h + 3) as i64);
    assert_eq!(sampled_bottom, [70, 80, 90]);
}

#[test]
fn test_motion_blur_frame_blending_logic() {
    assert_eq!(compute_motion_blur_frames(1.0, true), 1);
    assert_eq!(compute_motion_blur_frames(0.5, true), 1);
    assert_eq!(compute_motion_blur_frames(3.2, true), 4);
    assert_eq!(compute_motion_blur_frames(2.1, true), 3);
    assert_eq!(compute_motion_blur_frames(5.0, true), 4);
    assert_eq!(compute_motion_blur_frames(3.2, false), 1);

    let f1 = vec![100u8, 100u8, 100u8];
    let f2 = vec![150u8, 150u8, 150u8];
    let f3 = vec![200u8, 200u8, 200u8];
    let f4 = vec![250u8, 250u8, 250u8];
    let frames: Vec<&[u8]> = vec![&f1, &f2, &f3, &f4];
    let mut out = vec![0u8; 3];
    blend_full_frames(&frames, &mut out);
    assert_eq!(out, vec![175, 175, 175]);
}

#[test]
fn test_schema_v1_and_v2_parsing_and_retrocompat() {
    let v1_json = r#"{
        "schema_version": 1,
        "style": "HARD",
        "fps": 16,
        "aspect": { "w": 1080, "h": 1080 },
        "borderless": true,
        "bpm": 120.0,
        "target_duration": 10.0,
        "video_duration": 10.0,
        "audio_duration": 10.0,
        "loops": 0,
        "segments": [
            {
                "t0": 0.0,
                "t1": 10.0,
                "s0": 0.0,
                "s1": 10.0,
                "curve": "snap"
            }
        ]
    }"#;

    let parsed_v1: ProjectPlan = serde_json::from_str(v1_json).expect("Schema v1 must parse cleanly");
    assert_eq!(parsed_v1.schema_version, 1);
    assert_eq!(parsed_v1.motion_blur, false);
    assert_eq!(parsed_v1.segments[0].effects.reverse, false);
    assert_eq!(parsed_v1.one_framers.len(), 0);
    assert_eq!(parsed_v1.transitions.len(), 0);
    assert_eq!(parsed_v1.segments[0].transition, None);

    let v2_plan = create_plan_internal("HARD", 16, &[1.0, 2.0], &[1.0], 5.0, 5.0, 1080, 1080, 120.0, true, None, None).unwrap();
    assert_eq!(v2_plan.schema_version, 2);
    assert_eq!(v2_plan.motion_blur, true);
    assert_eq!(v2_plan.segments[0].effects.shake.a0, 8.0);
    assert!(v2_plan.one_framers.len() > 0);
    assert!(v2_plan.transitions.len() > 0);

    let v2_serialized = serde_json::to_string(&v2_plan).unwrap();
    assert!(v2_serialized.contains("\"one_framers\":["));
    assert!(v2_serialized.contains("\"transitions\":["));
    let v2_deserialized: ProjectPlan = serde_json::from_str(&v2_serialized).unwrap();
    assert_eq!(v2_deserialized.one_framers.len(), v2_plan.one_framers.len());
    assert_eq!(v2_deserialized.transitions.len(), v2_plan.transitions.len());
}

#[test]
fn test_one_framers_library_diff() {
    let width = 64usize;
    let height = 64usize;
    let mut frame_in = vec![0u8; width * height * 3];
    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) * 3;
            frame_in[idx] = ((x * 4) % 256) as u8;
            frame_in[idx + 1] = ((y * 4) % 256) as u8;
            frame_in[idx + 2] = (((x + y) * 2) % 256) as u8;
        }
    }

    for framer_type in ONE_FRAMER_TYPES {
        let mut frame_out = vec![0u8; width * height * 3];
        apply_one_framer(framer_type, &frame_in, &mut frame_out, width, height);

        let diff: i64 = frame_in
            .iter()
            .zip(frame_out.iter())
            .map(|(&a, &b)| (a as i64 - b as i64).abs())
            .sum();

        println!("One-Framer [{}] produced total pixel diff: {}", framer_type, diff);
        assert!(diff > 0, "One-framer {} must modify frame (diff > 0)", framer_type);
    }
}

#[test]
fn test_one_framers_auto_placement() {
    let video_duration = 10.773;
    let audio_duration = 14.315;
    let beats = vec![
        0.42, 1.14, 1.88, 2.60, 3.32, 4.04, 4.76, 5.50, 6.20, 6.94, 7.64, 8.38, 9.10, 9.82,
        10.54, 11.26, 11.98, 12.70, 13.42, 14.14,
    ];
    let downbeats = vec![2.60, 5.50, 8.38, 11.26, 14.14];
    let fps = 16u32;
    let bpm = 83.33;

    let plan_hard = create_plan_internal(
        "HARD",
        fps,
        &beats,
        &downbeats,
        video_duration,
        audio_duration,
        1080,
        1080,
        bpm,
        true,
        None,
        None,
    )
    .unwrap();

    let num_cuts = plan_hard.segments.len();
    let num_downbeats = downbeats.len();
    let total_raw_candidates = num_cuts * 4 + num_downbeats;
    println!(
        "HARD raw placement check: {} cuts * 4 + {} downbeats = {} candidates",
        num_cuts, num_downbeats, total_raw_candidates
    );
    assert_eq!(num_cuts, 21);
    assert_eq!(total_raw_candidates, 89);
    assert!(plan_hard.one_framers.len() > 50, "Valid deduped framers in [0, target] should be substantial");

    for win in plan_hard.one_framers.windows(2) {
        assert!(win[0].t <= win[1].t, "one_framers list must be sorted ascending by t");
        assert!(win[0].t >= 0.0 && win[1].t <= audio_duration + 0.1);
    }

    let plan_smooth = create_plan_internal(
        "SMOOTH",
        fps,
        &beats,
        &downbeats,
        video_duration,
        audio_duration,
        1080,
        1080,
        bpm,
        true,
        None,
        None,
    )
    .unwrap();
    assert!(plan_smooth.one_framers.len() < plan_hard.one_framers.len());

    let plan_hybrid = create_plan_internal(
        "HYBRID",
        fps,
        &beats,
        &downbeats,
        video_duration,
        audio_duration,
        1080,
        1080,
        bpm,
        true,
        None,
        None,
    )
    .unwrap();
    assert!(plan_hybrid.one_framers.len() > plan_smooth.one_framers.len());
}

#[test]
fn test_one_framers_reproducibility() {
    let video_duration = 10.773;
    let audio_duration = 14.315;
    let beats = vec![
        0.42, 1.14, 1.88, 2.60, 3.32, 4.04, 4.76, 5.50, 6.20, 6.94, 7.64, 8.38, 9.10, 9.82,
        10.54, 11.26, 11.98, 12.70, 13.42, 14.14,
    ];
    let downbeats = vec![2.60, 5.50, 8.38, 11.26, 14.14];
    let fps = 16u32;
    let bpm = 83.33;

    let plan1 = create_plan_internal("HARD", fps, &beats, &downbeats, video_duration, audio_duration, 1080, 1080, bpm, true, None, None).unwrap();
    let plan2 = create_plan_internal("HARD", fps, &beats, &downbeats, video_duration, audio_duration, 1080, 1080, bpm, true, None, None).unwrap();

    assert_eq!(plan1.one_framers, plan2.one_framers);
    for (f1, f2) in plan1.one_framers.iter().zip(plan2.one_framers.iter()) {
        assert_eq!(f1.t, f2.t);
        assert_eq!(f1.framer_type, f2.framer_type);
    }
}

#[test]
fn test_curves_monotonicity_and_bounds() {
    assert_eq!(evaluate_curve("snap", 0.0), 0.0);
    assert_eq!(evaluate_curve("snap", 1.0), 1.0);
    assert_eq!(evaluate_curve("saddle", 0.0), 0.0);
    assert_eq!(evaluate_curve("saddle", 1.0), 1.0);

    let steps = 1000;
    let mut prev_snap = -1.0;
    let mut prev_saddle = -1.0;

    for i in 0..=steps {
        let x = (i as f64) / (steps as f64);
        let y_snap = evaluate_curve("snap", x);
        let y_saddle = evaluate_curve("saddle", x);

        assert!(y_snap >= 0.0 && y_snap <= 1.0);
        assert!(y_saddle >= 0.0 && y_saddle <= 1.0);

        if i > 0 {
            assert!(y_snap > prev_snap);
            assert!(y_saddle >= prev_saddle);
        }
        prev_snap = y_snap;
        prev_saddle = y_saddle;
    }
}

#[test]
fn test_crop_to_fill_maths() {
    let crop_1_1 = compute_crop_to_fill(1080, 1920, 1080, 1080);
    assert_eq!(crop_1_1.x, 0);
    assert_eq!(crop_1_1.y, 0);
    assert_eq!(crop_1_1.width, 1080);
    assert_eq!(crop_1_1.height, 1920);
    assert_eq!(crop_1_1.out_w, 1080);
    assert_eq!(crop_1_1.out_h, 1080);

    let crop_16_9 = compute_crop_to_fill(1080, 1920, 16, 9);
    assert_eq!(crop_16_9.x, 0);
    assert_eq!(crop_16_9.y, 0);
    assert_eq!(crop_16_9.width, 1080);
    assert_eq!(crop_16_9.height, 1920);
    assert_eq!(crop_16_9.out_w, 1920);
    assert_eq!(crop_16_9.out_h, 1080);

    let crop_9_16 = compute_crop_to_fill(1080, 1920, 9, 16);
    assert_eq!(crop_9_9_16(crop_9_16.x, crop_9_16.y, crop_9_16.width, crop_9_16.height, crop_9_16.out_w, crop_9_16.out_h), (0, 0, 1080, 1920, 1080, 1920));
}

fn crop_9_9_16(x: u32, y: u32, w: u32, h: u32, ow: u32, oh: u32) -> (u32, u32, u32, u32, u32, u32) {
    (x, y, w, h, ow, oh)
}

#[test]
fn test_borderless_stretch_scale() {
    let (sx1, sy1) = compute_borderless_scale(1920, 1080, 1080, 1080);
    assert_eq!(sx1, 0.5625);
    assert_eq!(sy1, 1.0);

    let (sx2, sy2) = compute_borderless_scale(1080, 1920, 1080, 1080);
    assert_eq!(sx2, 1.0);
    assert_eq!(sy2, 0.5625);

    let crop = compute_crop_to_fill(1080, 1920, 1080, 1080);
    assert_eq!(crop.x, 0);
    assert_eq!(crop.y, 0);
    assert_eq!(crop.width, 1080);
    assert_eq!(crop.height, 1920);
    assert_eq!(crop.out_w, 1080);
    assert_eq!(crop.out_h, 1080);
}

#[test]
fn test_probe_media_video_pure_rust() {
    let video_path = r"C:\Users\cia\Downloads\jugg video & audio tester\snaptik_7674387013243538721_v3.mp4";
    if std::path::Path::new(video_path).exists() {
        let res = probe_media_internal(video_path, None).expect("Probe should succeed");
        println!("Video probe result: {:?}", res);
        assert!(res.duration > 10.0);
        assert_eq!(res.width, 1080);
        assert_eq!(res.height, 1920);
        assert_eq!(res.fps, 30.0);
    }
}

#[test]
fn test_probe_media_audio_pure_rust() {
    let drums_path = r"C:\Users\cia\Downloads\jugg video & audio tester\audio [drums].mp3";
    if std::path::Path::new(drums_path).exists() {
        let res = probe_media_internal(drums_path, None).expect("Probe should succeed");
        println!("Drums audio probe result: {:?}", res);
        assert!(res.duration > 14.0);
        assert_eq!(res.audio_channels, 2);
        assert_eq!(res.audio_sample_rate, 44100);
    }

    let audio_path = r"C:\Users\cia\Downloads\jugg video & audio tester\curiosos.mp3";
    if std::path::Path::new(audio_path).exists() {
        let res = probe_media_internal(audio_path, None).expect("Probe should succeed");
        println!("Target audio probe result: {:?}", res);
        assert!(res.duration > 14.0);
        assert_eq!(res.audio_channels, 2);
        assert_eq!(res.audio_sample_rate, 44100);
    }
}

#[test]
fn test_generate_plan_fixture_invariants() {
    let video_duration = 10.773;
    let audio_duration = 14.315;
    let beats = vec![
        0.42, 1.14, 1.88, 2.60, 3.32, 4.04, 4.76, 5.50, 6.20, 6.94, 7.64, 8.38, 9.10, 9.82,
        10.54, 11.26, 11.98, 12.70, 13.42, 14.14,
    ];
    let downbeats = vec![2.60, 5.50, 8.38, 11.26, 14.14];
    let fps = 16u32;
    let bpm = 83.33;
    let min_seg_dur = 3.0 / (fps as f64);

    for style in ["HARD", "SMOOTH", "HYBRID"] {
        let plan = create_plan_internal(
            style,
            fps,
            &beats,
            &downbeats,
            video_duration,
            audio_duration,
            1080,
            1080,
            bpm,
            true,
            None,
            None,
        )
        .expect("Plan generation must succeed");

        println!("--- Tested Style: {} (Segments: {}, Loops: {}) ---", style, plan.segments.len(), plan.loops);

        assert_eq!(plan.segments.first().unwrap().t0, 0.0);
        assert!((plan.segments.last().unwrap().t1 - audio_duration).abs() < 0.01);

        for win in plan.segments.windows(2) {
            assert!((win[0].t1 - win[1].t0).abs() < 1e-4);
        }

        for seg in &plan.segments {
            let s_min = seg.s0.min(seg.s1);
            let s_max = seg.s0.max(seg.s1);
            assert!(s_min >= 0.0);
            assert!(s_max > s_min);
            assert!(s_max <= video_duration + 1e-4);
            let dur = seg.t1 - seg.t0;
            assert!(dur >= min_seg_dur - 1e-4);
        }

        if style == "HARD" || style == "HYBRID" {
            assert!(plan.loops >= 1);
        }
    }
}

#[test]
fn test_ambiance_flicker_oscillation() {
    let fps = 16.0;
    let seg = PlanSegment {
        t0: 0.0, t1: 1.0, s0: 0.0, s1: 1.0,
        curve: "snap".to_string(),
        effects: crate::SegmentEffects {
            shake: crate::ShakeEffect { a0: 0.0, omega: 0.0, k: 0.0, seed: 0 },
            zoom: crate::ZoomEffect { scale_start: 1.0, scale_end: 1.0 },
            reverse: false,
            ..crate::default_segment_effects()
        },
        transition: None,
        color_hints: None,
    };

    let amplitude = 0.15;
    let freq = 12.0;
    let mut min_val = f64::MAX;
    let mut max_val = f64::MIN;
    for i in 0..128 {
        let t = (i as f64) / fps;
        let seg_phase = (seg.effects.shake.seed as f64) * 0.0012345;
        let v = amplitude * (2.0 * std::f64::consts::PI * freq * t + seg_phase).sin();
        if v < min_val { min_val = v; }
        if v > max_val { max_val = v; }
    }
    assert!(min_val >= -amplitude - 1e-9, "Flicker must stay >= -A");
    assert!(max_val <= amplitude + 1e-9, "Flicker must stay <= +A");
    let range = max_val - min_val;
    assert!(range > 0.8 * 2.0 * amplitude, "Flicker must oscillate across most of [-A, +A]");
}

#[test]
fn test_ambiance_exposure_flash() {
    let fps: f64 = 16.0;
    let peak: f64 = 0.5;
    let dt: f64 = 1.0 / fps;

    let env_center: f64 = (1.0f64 - (0.0f64).abs() / 2.0f64).max(0.0f64);
    let flash_center: f64 = peak * env_center;
    assert!((flash_center - peak).abs() < 1e-6, "Flash at downbeat must equal peak");

    let env_edge: f64 = (1.0f64 - ((2.0f64 * dt * fps).abs() / 2.0f64)).max(0.0f64);
    let flash_edge: f64 = peak * env_edge;
    assert!(flash_edge.abs() < 1e-6, "Flash at ±2 frames must be 0");

    let env_half: f64 = (1.0f64 - ((-1.0f64).abs() / 2.0f64)).max(0.0f64);
    let flash_half: f64 = peak * env_half;
    assert!((flash_half - peak * 0.5f64).abs() < 1e-6, "Flash at ±1 frame must be peak/2");
}

#[test]
fn test_ambiance_echo_trail_blend() {
    let width = 4usize;
    let height = 4usize;
    let n = width * height * 3;
    let frame_in = vec![200u8; n];
    let mut frame_out = vec![0u8; n];

    let alpha = 0.3;
    let k = 3u32;
    let mut echo_ring: Vec<Vec<u8>> = (0..3).map(|_| vec![100u8; n]).collect();
    let mut echo_head = 0usize;

    let vignette_lut = vec![255u8; width * height];
    let seg = PlanSegment {
        t0: 0.0, t1: 1.0, s0: 0.0, s1: 1.0,
        curve: "snap".to_string(),
        effects: crate::SegmentEffects {
            shake: crate::ShakeEffect { a0: 0.0, omega: 0.0, k: 0.0, seed: 0 },
            zoom: crate::ZoomEffect { scale_start: 1.0, scale_end: 1.0 },
            reverse: false,
            ..crate::default_segment_effects()
        },
        transition: None,
        color_hints: None,
    };

    let amb = AmbianceConfig {
        flicker: FlickerConfig { amplitude: 0.0, f: 0.0, phase: 0.0 },
        exposure_flash: ExposureFlashConfig { peak: 0.0, times: vec![] },
        echo_trail: EchoTrailConfig { enabled: true, alpha, k },
        tint: TintConfig { offset_rgb: [0, 0, 0] },
        vignette: VignetteConfig { strength: 0.0 },
        scanlines: ScanlinesConfig { opacity: 0.0 },
    };

    apply_ambiance_effects(
        &frame_in,
        &mut frame_out,
        width, height,
        &amb,
        &mut echo_ring,
        &mut echo_head,
        &vignette_lut,
        0.0,
        0.0,
        &seg,
        16.0,
    );

    let expected = 169u8;
    for px in 0..(width * height) {
        let idx = px * 3;
        assert_eq!(frame_out[idx], expected, "Echo trail blend mismatch at px {}", px);
    }
}

#[test]
fn test_ambiance_tint_vignette_scanlines() {
    let width = 16usize;
    let height = 16usize;
    let n = width * height * 3;
    let frame_in = vec![128u8; n];
    let mut frame_out = vec![0u8; n];

    let vignette_lut = vec![255u8; width * height];
    let mut echo_ring: Vec<Vec<u8>> = (0..3).map(|_| vec![0u8; n]).collect();
    let mut echo_head = 0usize;

    let seg = PlanSegment {
        t0: 0.0, t1: 1.0, s0: 0.0, s1: 1.0,
        curve: "snap".to_string(),
        effects: crate::SegmentEffects {
            shake: crate::ShakeEffect { a0: 0.0, omega: 0.0, k: 0.0, seed: 0 },
            zoom: crate::ZoomEffect { scale_start: 1.0, scale_end: 1.0 },
            reverse: false,
            ..crate::default_segment_effects()
        },
        transition: None,
        color_hints: None,
    };

    let tint_r = 10i16;
    let amb = AmbianceConfig {
        flicker: FlickerConfig { amplitude: 0.0, f: 0.0, phase: 0.0 },
        exposure_flash: ExposureFlashConfig { peak: 0.0, times: vec![] },
        echo_trail: EchoTrailConfig { enabled: false, alpha: 0.3, k: 3 },
        tint: TintConfig { offset_rgb: [tint_r, 0, 0] },
        vignette: VignetteConfig { strength: 0.0 },
        scanlines: ScanlinesConfig { opacity: 0.15 },
    };

    apply_ambiance_effects(
        &frame_in, &mut frame_out, width, height, &amb,
        &mut echo_ring, &mut echo_head, &vignette_lut, 0.15,
        0.0, &seg, 16.0,
    );

    let non_scanline_r = frame_out[width * 3];
    assert_eq!(non_scanline_r, 137u8, "Tint R+10 with vignette should yield 137 for non-scanline rows");

    let scanline_r = frame_out[0];
    assert_eq!(scanline_r, 116u8, "Scanline row should be dimmed by vignette+opacity combined");

    let diff: i64 = frame_in.iter().zip(frame_out.iter())
        .map(|(&a, &b)| (a as i64 - b as i64).abs())
        .sum();
    assert!(diff > 0, "Ambiance effects must change the frame");
}

#[test]
fn test_transitions_warp_bubble() {
    let fps = 16.0;
    let t_cut = 2.0;

    let env_center = compute_warp_bubble_env(t_cut, t_cut, fps);
    assert!((env_center - 0.5).abs() < 1e-4, "Peak warp bubble envelope should be 0.5");

    let env_edge_left = compute_warp_bubble_env(t_cut - 2.0 / fps, t_cut, fps);
    assert!((env_edge_left - 0.0).abs() < 1e-4, "Warp bubble env at -2 frames should be 0.0");

    let env_edge_right = compute_warp_bubble_env(t_cut + 2.0 / fps, t_cut, fps);
    assert!((env_edge_right - 0.0).abs() < 1e-4, "Warp bubble env at +2 frames should be 0.0");

    let width = 64usize;
    let height = 64usize;
    let mut frame_in = vec![0u8; width * height * 3];
    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) * 3;
            frame_in[idx] = (x * 4) as u8;
            frame_in[idx + 1] = (y * 4) as u8;
            frame_in[idx + 2] = ((x + y) * 2) as u8;
        }
    }
    let mut frame_out = vec![0u8; width * height * 3];
    apply_warp_bubble(&frame_in, &mut frame_out, width, height, 0.5, 1.2);

    let diff: i64 = frame_in
        .iter()
        .zip(frame_out.iter())
        .map(|(&a, &b)| (a as i64 - b as i64).abs())
        .sum();
    assert!(diff > 0, "Active warp bubble must produce pixel displacement");
}

#[test]
fn test_transitions_wave_warp() {
    let fps = 16.0;
    let t_cut = 1.0;
    let height = 1080usize;

    let (h0, _, _, t_fr0) = compute_wave_warp_params(t_cut, t_cut, fps, height);
    assert!((h0 - 280.0).abs() < 1e-4, "Wave warp H at t=0 should be 280.0");
    assert!((t_fr0 - 0.0).abs() < 1e-4);

    let (h3, _, _, _) = compute_wave_warp_params(t_cut + 3.0 / fps, t_cut, fps, height);
    assert!((h3 - 140.0).abs() < 1e-4, "Wave warp H at t=3 frames should be 140.0");

    let (h6, _, _, _) = compute_wave_warp_params(t_cut + 6.0 / fps, t_cut, fps, height);
    assert!((h6 - 0.0).abs() < 1e-4, "Wave warp H at t=6 frames should be 0.0");

    let (h7, _, _, _) = compute_wave_warp_params(t_cut + 7.0 / fps, t_cut, fps, height);
    assert_eq!(h7, 0.0, "Wave warp H at t=7 frames should be 0.0");
}

#[test]
fn test_transitions_slide_shake() {
    let fps = 16.0;
    let t_cut = 2.0;
    let dt = 1.0 / fps;

    let shift_before = compute_slide_shake_shift(t_cut - dt, t_cut, fps);
    let shift_after = compute_slide_shake_shift(t_cut + dt, t_cut, fps);

    assert!(shift_before > 0.0, "Shift before cut must be positive");
    assert!(shift_after < 0.0, "Shift after cut must be negative");
    assert!(
        (shift_before.abs() - shift_after.abs()).abs() < 1e-4,
        "Shift magnitude must be continuous across cut (signs inverted)"
    );

    let shift_bound_left = compute_slide_shake_shift(t_cut - 3.0 * dt, t_cut, fps);
    assert!((shift_bound_left - 0.0).abs() < 1e-4);

    let shift_bound_right = compute_slide_shake_shift(t_cut + 3.0 * dt, t_cut, fps);
    assert!((shift_bound_right - 0.0).abs() < 1e-4);
}

#[test]
fn test_transitions_auto_placement() {
    let video_duration = 10.773;
    let audio_duration = 14.315;
    let beats = vec![
        0.42, 1.14, 1.88, 2.60, 3.32, 4.04, 4.76, 5.50, 6.20, 6.94, 7.64, 8.38, 9.10, 9.82,
        10.54, 11.26, 11.98, 12.70, 13.42, 14.14,
    ];
    let downbeats = vec![2.60, 5.50, 8.38, 11.26, 14.14];
    let fps = 16u32;
    let bpm = 83.33;

    let plan_hard = create_plan_internal("HARD", fps, &beats, &downbeats, video_duration, audio_duration, 1080, 1080, bpm, true, None, None).unwrap();

    let wrap_transitions: Vec<_> = plan_hard.transitions.iter().filter(|t| t.is_wrap).collect();
    assert_eq!(wrap_transitions.len(), 1, "HARD plan should have 1 wrap transition");
    assert_eq!(wrap_transitions[0].transition_type, "WARP_BUBBLE");

    let cut_warps = plan_hard.transitions.iter().filter(|t| !t.is_wrap && t.transition_type == "WARP_BUBBLE").count();
    let cut_waves = plan_hard.transitions.iter().filter(|t| !t.is_wrap && t.transition_type == "WAVE_WARP").count();
    let cut_slides = plan_hard.transitions.iter().filter(|t| !t.is_wrap && t.transition_type == "SLIDE_SHAKE").count();

    println!(
        "HARD transitions breakdown: wrap=1, warp_cuts={}, wave_cuts={}, slide_cuts={}",
        cut_warps, cut_waves, cut_slides
    );

    assert!((cut_warps as i32 - 6).abs() <= 2, "Warp cuts count should be ~6");
    assert!((cut_waves as i32 - 4).abs() <= 2, "Wave cuts count should be ~4");
    assert!((cut_slides as i32 - 8).abs() <= 2, "Slide cuts count should be ~8");

    let plan_smooth = create_plan_internal("SMOOTH", fps, &beats, &downbeats, video_duration, audio_duration, 1080, 1080, bpm, true, None, None).unwrap();
    let smooth_warps = plan_smooth.transitions.iter().filter(|t| !t.is_wrap && t.transition_type == "WARP_BUBBLE").count();
    let smooth_waves = plan_smooth.transitions.iter().filter(|t| !t.is_wrap && t.transition_type == "WAVE_WARP").count();
    assert_eq!(smooth_warps, 0, "SMOOTH style has 0% warp on cuts");
    assert_eq!(smooth_waves, 0, "SMOOTH style has 0% wave on cuts");

    let plan_hybrid = create_plan_internal("HYBRID", fps, &beats, &downbeats, video_duration, audio_duration, 1080, 1080, bpm, true, None, None).unwrap();
    assert!(plan_hybrid.transitions.len() > plan_smooth.transitions.len());
}

#[test]
fn test_benchmark_full_effects_pipeline() {
    let video_path = r"C:\Users\cia\Downloads\jugg video & audio tester\snaptik_7674387013243538721_v3.mp4";
    let audio_path = r"C:\Users\cia\Downloads\jugg video & audio tester\curiosos.mp3";

    if !std::path::Path::new(video_path).exists() || !std::path::Path::new(audio_path).exists() {
        println!("Test files not found, skipping benchmark test.");
        return;
    }

    let beats = vec![
        0.42, 1.14, 1.88, 2.60, 3.32, 4.04, 4.76, 5.50, 6.20, 6.94, 7.64, 8.38, 9.10, 9.82,
        10.54, 11.26, 11.98, 12.70, 13.42, 14.14,
    ];
    let downbeats = vec![2.60, 5.50, 8.38, 11.26, 14.14];
    let fps = 16u32;
    let bpm = 83.33;

    let plan = create_plan_internal(
        "HARD",
        fps,
        &beats,
        &downbeats,
        10.773,
        14.315,
        1080,
        1080,
        bpm,
        true,
        None,
        None,
    )
    .expect("Plan generation failed");

    let ffmpeg_bin = if let Ok(output) = std::process::Command::new("where.exe").arg("ffmpeg").output() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        stdout.lines().next().map(|s| std::path::PathBuf::from(s.trim())).unwrap_or_default()
    } else {
        std::path::PathBuf::from("ffmpeg.exe")
    };

    let scene_info = probe_media_internal(video_path, None).unwrap();
    let src_w = scene_info.width;
    let src_h = scene_info.height;
    let src_fps = scene_info.fps;
    let frame_bytes = (src_w * src_h * 3) as usize;

    let temp_dir = std::env::temp_dir().join("cia_app_bench_t10");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let raw_cache = temp_dir.join("test_frames.raw");

    let t_decode_start = std::time::Instant::now();
    let mut decode_cmd = std::process::Command::new(&ffmpeg_bin);
    decode_cmd.args([
        "-y",
        "-i",
        video_path,
        "-f",
        "rawvideo",
        "-pix_fmt",
        "rgb24",
        "-an",
        &raw_cache.to_string_lossy(),
    ]);
    let mut decode_proc = decode_cmd.spawn().expect("Failed to spawn decode");
    let status = decode_proc.wait().expect("Decode failed");
    assert!(status.success());
    let t_decode = t_decode_start.elapsed();

    let total_cached_bytes = std::fs::metadata(&raw_cache).unwrap().len();
    let total_source_frames = (total_cached_bytes / (frame_bytes as u64)) as usize;

    let crop = compute_crop_to_fill(src_w, src_h, plan.aspect.w, plan.aspect.h);
    let output_fps = plan.fps as f64;
    let total_output_frames = (plan.target_duration * output_fps).round() as usize;

    let mut raw_file = std::fs::File::open(&raw_cache).unwrap();
    let cropped_frame_bytes = (crop.width * crop.height * 3) as usize;

    // 1. Baseline T9 Pipeline
    let t_t9_start = std::time::Instant::now();
    let mut t9_reader = CachedFrameReader::new(&mut raw_file, frame_bytes, 16);
    let mut sampled_full_frame = vec![0u8; frame_bytes];
    let mut blend_storage = vec![vec![0u8; frame_bytes]; 4];
    let mut one_framer_buf = vec![0u8; frame_bytes];
    let mut t9_crop = vec![0u8; cropped_frame_bytes];

    for i in 0..total_output_frames {
        let t = (i as f64) / output_fps;
        let seg = plan.segments.iter().find(|s| t >= s.t0 && t <= s.t1).or_else(|| plan.segments.last()).unwrap();
        let seg_dur = (seg.t1 - seg.t0).max(1e-6);
        let t_rel = (t - seg.t0).max(0.0);
        let x = (t_rel / seg_dur).clamp(0.0, 1.0);
        let u = evaluate_curve(&seg.curve, x);
        let u_prime = evaluate_curve_derivative(&seg.curve, x);
        let speed_v = ((seg.s1 - seg.s0).abs() / seg_dur) * u_prime;
        let n_blur = compute_motion_blur_frames(speed_v, plan.motion_blur);

        let src_time = seg.s0 + (seg.s1 - seg.s0) * u;
        let mut base_src_frame = (src_time * src_fps).round() as i64;
        if base_src_frame < 0 { base_src_frame = 0; }
        if base_src_frame >= total_source_frames as i64 { base_src_frame = (total_source_frames - 1) as i64; }

        if n_blur <= 1 {
            t9_reader.get_frame(base_src_frame as u64, &mut sampled_full_frame).unwrap();
        } else {
            let mut slice_ptrs: Vec<&[u8]> = Vec::with_capacity(n_blur);
            for k in 0..n_blur {
                let f_idx = (base_src_frame + (k as i64)).clamp(0, (total_source_frames - 1) as i64) as u64;
                t9_reader.get_frame(f_idx, &mut blend_storage[k]).unwrap();
            }
            for k in 0..n_blur {
                slice_ptrs.push(&blend_storage[k]);
            }
            blend_full_frames(&slice_ptrs, &mut sampled_full_frame);
        }

        let active_framer = plan
            .one_framers
            .iter()
            .find(|f| (t - f.t).abs() < (0.5 / output_fps) + 1e-6);

        let full_frame_ptr = if let Some(framer) = active_framer {
            apply_one_framer(
                &framer.framer_type,
                &sampled_full_frame,
                &mut one_framer_buf,
                src_w as usize,
                src_h as usize,
            );
            &one_framer_buf
        } else {
            &sampled_full_frame
        };

        let params = compute_transform_params(&seg.effects, t_rel, seg_dur, output_fps);
        apply_transform_stack_cropped(
            full_frame_ptr,
            &mut t9_crop,
            src_w as usize,
            src_h as usize,
            crop.x,
            crop.y,
            crop.width,
            crop.height,
            params,
        );
    }
    let t_t9 = t_t9_start.elapsed();

    // 2. T10 Pipeline
    let t_t10_start = std::time::Instant::now();
    let mut t10_reader = CachedFrameReader::new(&mut raw_file, frame_bytes, 16);
    let mut transition_buf = vec![0u8; frame_bytes];
    let mut t10_crop = vec![0u8; cropped_frame_bytes];

    for i in 0..total_output_frames {
        let t = (i as f64) / output_fps;
        let seg = plan.segments.iter().find(|s| t >= s.t0 && t <= s.t1).or_else(|| plan.segments.last()).unwrap();
        let seg_dur = (seg.t1 - seg.t0).max(1e-6);
        let t_rel = (t - seg.t0).max(0.0);
        let x = (t_rel / seg_dur).clamp(0.0, 1.0);
        let u = evaluate_curve(&seg.curve, x);
        let u_prime = evaluate_curve_derivative(&seg.curve, x);
        let speed_v = ((seg.s1 - seg.s0).abs() / seg_dur) * u_prime;
        let n_blur = compute_motion_blur_frames(speed_v, plan.motion_blur);

        let src_time = seg.s0 + (seg.s1 - seg.s0) * u;
        let mut base_src_frame = (src_time * src_fps).round() as i64;
        if base_src_frame < 0 { base_src_frame = 0; }
        if base_src_frame >= total_source_frames as i64 { base_src_frame = (total_source_frames - 1) as i64; }

        if n_blur <= 1 {
            t10_reader.get_frame(base_src_frame as u64, &mut sampled_full_frame).unwrap();
        } else {
            let mut slice_ptrs: Vec<&[u8]> = Vec::with_capacity(n_blur);
            for k in 0..n_blur {
                let f_idx = (base_src_frame + (k as i64)).clamp(0, (total_source_frames - 1) as i64) as u64;
                t10_reader.get_frame(f_idx, &mut blend_storage[k]).unwrap();
            }
            for k in 0..n_blur {
                slice_ptrs.push(&blend_storage[k]);
            }
            blend_full_frames(&slice_ptrs, &mut sampled_full_frame);
        }

        let active_framer = plan
            .one_framers
            .iter()
            .find(|f| (t - f.t).abs() < (0.5 / output_fps) + 1e-6);

        let full_frame_ptr = if let Some(framer) = active_framer {
            apply_one_framer(
                &framer.framer_type,
                &sampled_full_frame,
                &mut one_framer_buf,
                src_w as usize,
                src_h as usize,
            );
            &one_framer_buf
        } else {
            &sampled_full_frame
        };

        let mut active_trans: Option<(&TransitionItem, f64)> = None;
        for trans in &plan.transitions {
            let t_frames = (t - trans.t) * output_fps;
            match trans.transition_type.as_str() {
                "WARP_BUBBLE" => {
                    if t_frames.abs() <= 2.0 + 1e-4 {
                        active_trans = Some((trans, t_frames));
                        break;
                    }
                }
                "WAVE_WARP" => {
                    if t_frames >= -1e-4 && t_frames <= 6.0 + 1e-4 {
                        active_trans = Some((trans, t_frames));
                        break;
                    }
                }
                "SLIDE_SHAKE" => {
                    if t_frames.abs() <= 3.0 + 1e-4 {
                        active_trans = Some((trans, t_frames));
                        break;
                    }
                }
                _ => {}
            }
        }

        let trans_frame_ptr = if let Some((trans, _t_frames)) = active_trans {
            match trans.transition_type.as_str() {
                "WARP_BUBBLE" => {
                    let env_a = compute_warp_bubble_env(t, trans.t, output_fps);
                    apply_warp_bubble(
                        full_frame_ptr,
                        &mut transition_buf,
                        src_w as usize,
                        src_h as usize,
                        env_a,
                        1.2,
                    );
                    &transition_buf
                }
                "WAVE_WARP" => {
                    let (h_t, k, v, t_fr) = compute_wave_warp_params(t, trans.t, output_fps, src_h as usize);
                    apply_wave_warp(
                        full_frame_ptr,
                        &mut transition_buf,
                        src_w as usize,
                        src_h as usize,
                        h_t,
                        k,
                        v,
                        t_fr,
                    );
                    &transition_buf
                }
                "SLIDE_SHAKE" => {
                    let shift_x = compute_slide_shake_shift(t, trans.t, output_fps);
                    apply_slide_shake(
                        full_frame_ptr,
                        &mut transition_buf,
                        src_w as usize,
                        src_h as usize,
                        shift_x,
                    );
                    &transition_buf
                }
                _ => full_frame_ptr,
            }
        } else {
            full_frame_ptr
        };

        let params = compute_transform_params(&seg.effects, t_rel, seg_dur, output_fps);
        apply_transform_stack_cropped(
            trans_frame_ptr,
            &mut t10_crop,
            src_w as usize,
            src_h as usize,
            crop.x,
            crop.y,
            crop.width,
            crop.height,
            params,
        );
    }
    let t_t10 = t_t10_start.elapsed();

    // 3. T11 Ambiance Pipeline
    let amb = plan.ambiance.as_ref().unwrap();
    let vig_strength = amb.vignette.strength;
    let scanline_opacity_bench = amb.scanlines.opacity;
    let rx_b = (src_w as f64) / 2.0;
    let ry_b = (src_h as f64) / 2.0;
    let r_max_b = (rx_b * rx_b + ry_b * ry_b).sqrt();
    let mut vignette_lut_bench = vec![0u8; (src_w * src_h) as usize];
    for vy in 0..(src_h as usize) {
        let dy = (vy as f64) - ry_b;
        for vx in 0..(src_w as usize) {
            let dx = (vx as f64) - rx_b;
            let r = (dx * dx + dy * dy).sqrt();
            let factor = 1.0 - vig_strength * (r / r_max_b).powi(2);
            vignette_lut_bench[vy * (src_w as usize) + vx] = (factor.clamp(0.0, 1.0) * 255.0) as u8;
        }
    }
    let echo_k_b = 3usize;
    let mut echo_ring_b: Vec<Vec<u8>> = (0..echo_k_b).map(|_| vec![128u8; frame_bytes]).collect();
    let mut echo_head_b: usize = 0;
    let mut ambiance_buf_b = vec![0u8; frame_bytes];
    let mut t11_crop = vec![0u8; cropped_frame_bytes];

    let t_t11_start = std::time::Instant::now();
    let mut t11_reader = CachedFrameReader::new(&mut raw_file, frame_bytes, 16);

    for i in 0..total_output_frames {
        let t = (i as f64) / output_fps;
        let seg = plan.segments.iter().find(|s| t >= s.t0 && t <= s.t1).or_else(|| plan.segments.last()).unwrap();
        let seg_dur = (seg.t1 - seg.t0).max(1e-6);
        let t_rel = (t - seg.t0).max(0.0);
        let x = (t_rel / seg_dur).clamp(0.0, 1.0);
        let u = evaluate_curve(&seg.curve, x);
        let u_prime = evaluate_curve_derivative(&seg.curve, x);
        let speed_v = ((seg.s1 - seg.s0).abs() / seg_dur) * u_prime;
        let n_blur = compute_motion_blur_frames(speed_v, plan.motion_blur);

        let src_time = seg.s0 + (seg.s1 - seg.s0) * u;
        let mut base_src_frame = (src_time * src_fps).round() as i64;
        if base_src_frame < 0 { base_src_frame = 0; }
        if base_src_frame >= total_source_frames as i64 { base_src_frame = (total_source_frames - 1) as i64; }

        if n_blur <= 1 {
            t11_reader.get_frame(base_src_frame as u64, &mut sampled_full_frame).unwrap();
        } else {
            let mut slice_ptrs: Vec<&[u8]> = Vec::with_capacity(n_blur);
            for k in 0..n_blur {
                let f_idx = (base_src_frame + (k as i64)).clamp(0, (total_source_frames - 1) as i64) as u64;
                t11_reader.get_frame(f_idx, &mut blend_storage[k]).unwrap();
            }
            for k in 0..n_blur { slice_ptrs.push(&blend_storage[k]); }
            blend_full_frames(&slice_ptrs, &mut sampled_full_frame);
        }

        let active_framer = plan.one_framers.iter().find(|f| (t - f.t).abs() < (0.5 / output_fps) + 1e-6);
        let full_frame_ptr = if let Some(framer) = active_framer {
            apply_one_framer(&framer.framer_type, &sampled_full_frame, &mut one_framer_buf, src_w as usize, src_h as usize);
            &one_framer_buf
        } else { &sampled_full_frame };

        let mut active_trans: Option<(&TransitionItem, f64)> = None;
        for trans in &plan.transitions {
            let t_frames = (t - trans.t) * output_fps;
            match trans.transition_type.as_str() {
                "WARP_BUBBLE" => { if t_frames.abs() <= 2.0 + 1e-4 { active_trans = Some((trans, t_frames)); break; } }
                "WAVE_WARP"   => { if t_frames >= -1e-4 && t_frames <= 6.0 + 1e-4 { active_trans = Some((trans, t_frames)); break; } }
                "SLIDE_SHAKE" => { if t_frames.abs() <= 3.0 + 1e-4 { active_trans = Some((trans, t_frames)); break; } }
                _ => {}
            }
        }
        let trans_frame_ptr_b = if let Some((trans, _)) = active_trans {
            match trans.transition_type.as_str() {
                "WARP_BUBBLE" => {
                    let env_a = compute_warp_bubble_env(t, trans.t, output_fps);
                    apply_warp_bubble(full_frame_ptr, &mut transition_buf, src_w as usize, src_h as usize, env_a, 1.2);
                    &transition_buf as &[u8]
                }
                "WAVE_WARP" => {
                    let (h_t, k, v, t_fr) = compute_wave_warp_params(t, trans.t, output_fps, src_h as usize);
                    apply_wave_warp(full_frame_ptr, &mut transition_buf, src_w as usize, src_h as usize, h_t, k, v, t_fr);
                    &transition_buf
                }
                "SLIDE_SHAKE" => {
                    let shift_x = compute_slide_shake_shift(t, trans.t, output_fps);
                    apply_slide_shake(full_frame_ptr, &mut transition_buf, src_w as usize, src_h as usize, shift_x);
                    &transition_buf
                }
                _ => full_frame_ptr,
            }
        } else { full_frame_ptr };

        apply_ambiance_effects(
            trans_frame_ptr_b, &mut ambiance_buf_b,
            src_w as usize, src_h as usize,
            amb,
            &mut echo_ring_b, &mut echo_head_b,
            &vignette_lut_bench, scanline_opacity_bench,
            t, seg, output_fps,
        );

        let params_b = compute_transform_params(&seg.effects, t_rel, seg_dur, output_fps);
        apply_transform_stack_cropped(&ambiance_buf_b, &mut t11_crop, src_w as usize, src_h as usize, crop.x, crop.y, crop.width, crop.height, params_b);
    }
    let t_t11 = t_t11_start.elapsed();

    let t_t9_total = t_decode + t_t9;
    let t_t10_total = t_decode + t_t10;
    let t_t11_total = t_decode + t_t11;
    let ratio = (t_t11_total.as_secs_f64() / t_t10_total.as_secs_f64()).max(0.01);

    println!("=== T11 AMBIANCE BENCHMARK REPORT ===");
    println!("Total frames rendered: {}", total_output_frames);
    println!("Decode time: {:.3}s", t_decode.as_secs_f64());
    println!("T9 pipeline time: {:.3}s", t_t9_total.as_secs_f64());
    println!("T10 pipeline time: {:.3}s", t_t10_total.as_secs_f64());
    println!("T11 pipeline time: {:.3}s", t_t11_total.as_secs_f64());
    println!("Performance ratio (T11 / T10): {:.3}x", ratio);
    println!("========================================");

    assert!(
        ratio < 1.5,
        "Benchmark check failed: ratio was {:.3}x (expected < 1.5x)",
        ratio
    );
}

#[test]
fn test_adaptive_scale_4k() {
    let w: u32 = 3840;
    let h: u32 = 2160;
    let frames: u64 = 1800;
    let max_cache: u64 = 4 * 1024 * 1024 * 1024;
    let raw_cache = (w as u64) * (h as u64) * 3 * frames;
    assert!(raw_cache > max_cache, "4K source should exceed 4GB cache");

    let s = ((max_cache as f64) / (raw_cache as f64)).sqrt();
    let long_side = w.max(h) as f64;
    let floor_scale = 1080.0 / long_side;
    let s_clamped = s.max(floor_scale).min(1.0);

    let new_w = ((w as f64 * s_clamped) as u32) & !1;
    let new_h = ((h as f64 * s_clamped) as u32) & !1;

    assert!(new_w.max(new_h) >= 1080, "Long side must be >= 1080 after scale");
    let scaled_cache = (new_w as u64) * (new_h as u64) * 3 * frames;
    assert!(scaled_cache <= max_cache, "Scaled cache must fit in 4GB");
}

#[test]
fn test_adaptive_scale_1080p_short() {
    let w: u32 = 1080;
    let h: u32 = 1920;
    let frames: u64 = 323;
    let max_cache: u64 = 4 * 1024 * 1024 * 1024;
    let raw_cache = (w as u64) * (h as u64) * 3 * frames;
    assert!(raw_cache < max_cache, "1080p short source should fit in 4GB without scaling");
}

#[test]
fn test_adaptive_scale_max_duration() {
    let fps: f64 = 30.0;
    let floor_w: u32 = 1080;
    let floor_h: u32 = 608;
    let max_cache: u64 = 4 * 1024 * 1024 * 1024;
    let floor_frame_bytes = (floor_w as u64) * (floor_h as u64) * 3;
    let max_frames = max_cache / floor_frame_bytes;
    let max_seconds = (max_frames as f64) / fps;
    assert!(max_seconds > 60.0, "At 1080x608 @ 30fps, max should be > 60s");
}

#[test]
fn test_full_fx_off_strips_effects() {
    let video_duration = 10.773;
    let audio_duration = 14.315;
    let beats = vec![
        0.42, 1.14, 1.88, 2.60, 3.32, 4.04, 4.76, 5.50, 6.20, 6.94,
        7.64, 8.38, 9.10, 9.82, 10.54, 11.26, 11.98, 12.70, 13.42, 14.14,
    ];
    let downbeats = vec![2.60, 5.50, 8.38, 11.26, 14.14];

    let plan = create_plan_internal(
        "HARD", 16, &beats, &downbeats,
        video_duration, audio_duration, 1080, 1080, 83.33, false, None, None,
    ).expect("Plan generation must succeed");

    assert_eq!(plan.full_fx, false);
    assert!(plan.one_framers.is_empty(), "full_fx=false must produce empty one_framers");
    assert!(!plan.transitions.is_empty(), "full_fx=false must still have geometric transitions");

    let amb = plan.ambiance.as_ref().unwrap();
    assert!(amb.flicker.amplitude > 0.0);
    assert!(amb.exposure_flash.times.is_empty());
    assert!(!amb.echo_trail.enabled);
    assert_eq!(amb.vignette.strength, 0.0);
    assert_eq!(amb.scanlines.opacity, 0.0);
    assert_eq!(amb.tint.offset_rgb, [0, 0, 0]);
}

#[test]
fn test_full_fx_on_matches_head() {
    let video_duration = 10.773;
    let audio_duration = 14.315;
    let beats = vec![
        0.42, 1.14, 1.88, 2.60, 3.32, 4.04, 4.76, 5.50, 6.20, 6.94,
        7.64, 8.38, 9.10, 9.82, 10.54, 11.26, 11.98, 12.70, 13.42, 14.14,
    ];
    let downbeats = vec![2.60, 5.50, 8.38, 11.26, 14.14];

    let plan = create_plan_internal(
        "HARD", 16, &beats, &downbeats,
        video_duration, audio_duration, 1080, 1080, 83.33, true, None, None,
    ).expect("Plan generation must succeed");

    assert_eq!(plan.full_fx, true);
    assert!(!plan.one_framers.is_empty());
    assert!(!plan.transitions.is_empty());
    let amb = plan.ambiance.as_ref().unwrap();
    assert!(amb.flicker.amplitude > 0.0);
    assert!(!amb.exposure_flash.times.is_empty());
    assert!(amb.vignette.strength > 0.0);
    assert!(amb.scanlines.opacity > 0.0);
}

#[test]
fn test_bouncy_shake_pattern() {
    let v0 = compute_bouncy_shake(0.0);
    let v1 = compute_bouncy_shake(1.0);
    let v10 = compute_bouncy_shake(10.0);
    let out = compute_bouncy_shake(11.0);
    assert!(v0 < 0.0,  "frame 0 should be negative, got {}", v0);
    assert!(v1 > 0.0,  "frame 1 should be positive, got {}", v1);
    assert!((v10).abs() < 1e-9, "frame 10 should be 0, got {}", v10);
    assert_eq!(out, 0.0, "out of range should be 0");

    let v05 = compute_bouncy_shake(0.5);
    assert!(v05 > v0 && v05 < v1, "frame 0.5 should be between v0 and v1");
}

#[test]
fn test_skew_shake_zero_at_end() {
    let duration = 1.0;
    let s0_deg = 10.0;
    let skew_end = compute_skew_shake(duration, duration, s0_deg);
    assert!(skew_end.abs() < 0.1, "skew at T should be near 0");
    let skew_start = compute_skew_shake(0.0, duration, s0_deg);
    assert!(skew_start.abs() > 0.05, "skew at t=0 should be non-zero");
}

#[test]
fn test_squish_pop_returns_to_one() {
    let (sx5, sy5) = compute_squish_pop(5.0);
    assert!((sx5 - 1.0).abs() < 1e-9);
    assert!((sy5 - 1.0).abs() < 1e-9);
    let (sx1, sy1) = compute_squish_pop(1.0);
    assert!((sy1 - 0.88).abs() < 1e-9);
    assert!((sx1 - 1.10).abs() < 1e-9);
}

#[test]
fn test_optics_k_monotone_decreasing() {
    let dur = 1.0;
    let k0 = 0.08;
    let samples: Vec<f64> = (0..=10).map(|i| compute_optics_k(i as f64 * 0.1, dur, k0)).collect();
    for w in samples.windows(2) {
        assert!(w[0] >= w[1] - 1e-12);
    }
    assert!((samples[10]).abs() < 1e-9);
    assert!((samples[0] - k0).abs() < 1e-9);
}

#[test]
fn test_stretch_ends_at_one() {
    let dur = 1.0;
    let scale_start = 1.4;
    let s_end = compute_stretch_scale(dur, dur, scale_start);
    assert!((s_end - 1.0).abs() < 1e-9);
    let s_start = compute_stretch_scale(0.0, dur, scale_start);
    assert!((s_start - scale_start).abs() < 1e-6);
}

#[test]
fn test_buildup_chain_continuity() {
    let fps = 30.0;
    let dur = 1.0;
    let v_tail = compute_chain_envelope_mult(dur - 0.001, dur, fps, false, true);
    assert!(v_tail >= 0.59 && v_tail <= 0.61);

    let v_head = compute_chain_envelope_mult(0.0, dur, fps, true, false);
    assert!((v_head - 0.6).abs() < 0.01);
}

#[test]
fn test_t14_seed_reproducibility() {
    let beats = vec![0.42, 1.14, 1.88, 2.60, 3.32, 4.04];
    let downbeats = vec![2.60];
    let plan1 = create_plan_internal("HARD", 16, &beats, &downbeats, 10.0, 6.0, 1080, 1080, 120.0, true, None, None)
        .expect("plan1 ok");
    let plan2 = create_plan_internal("HARD", 16, &beats, &downbeats, 10.0, 6.0, 1080, 1080, 120.0, true, None, None)
        .expect("plan2 ok");
    assert_eq!(plan1.segments, plan2.segments);
}

#[test]
fn test_t14_adv_shakes_present_in_hard() {
    let beats: Vec<f64> = (0..20).map(|i| i as f64 * 0.72).collect();
    let downbeats = vec![2.88, 5.76, 8.64];
    let plan = create_plan_internal("HARD", 16, &beats, &downbeats, 14.0, 14.4, 1080, 1080, 83.33, true, None, None)
        .expect("plan ok");
    let has_bouncy = plan.segments.iter().any(|s| s.effects.bouncy_shake.is_some());
    let has_squish = plan.segments.iter().any(|s| s.effects.squish_pop.is_some());
    let has_zoom_off = plan.segments.iter().any(|s| s.effects.zoom_beat_offset > 0);
    assert!(has_bouncy);
    assert!(has_squish);
    assert!(has_zoom_off);
}

#[test]
fn test_render_stats_computation() {
    let beats: Vec<f64> = (0..20).map(|i| i as f64 * 0.72).collect();
    let downbeats = vec![2.88, 5.76, 8.64];
    let plan = create_plan_internal("HARD", 16, &beats, &downbeats, 14.0, 14.4, 1080, 1080, 83.33, true, None, None)
        .expect("plan ok");

    let temp_dir = std::env::temp_dir();
    let dummy_mp4 = temp_dir.join("test_stats_fixture.mp4");
    std::fs::write(&dummy_mp4, vec![0u8; 1024 * 512]).expect("write dummy file");

    let stats = compute_render_stats(&plan, &dummy_mp4, 2.45);
    let _ = std::fs::remove_file(&dummy_mp4);

    assert!(stats.render_time_secs > 0.0);
    assert!(stats.file_size_mb > 0.0);
    assert_eq!(stats.target_fps, 16);
    assert!(stats.effects_count > 0);
}

#[test]
fn test_generic_preview_frame_pattern() {
    let w = 256usize;
    let h = 256usize;
    let frame = generate_generic_preview_frame(w, h);
    assert_eq!(frame.len(), w * h * 3);

    assert_eq!(frame[0], 0);
    assert_eq!(frame[1], 0);
    assert_eq!(frame[2], 0);

    let last_idx = ((h - 1) * w + (w - 1)) * 3;
    assert_eq!(frame[last_idx], 255);
    assert_eq!(frame[last_idx + 1], 255);
    assert_eq!(frame[last_idx + 2], 255);

    for y in 0..h {
        for x in 0..w {
            let idx1 = (y * w + x) * 3;
            let idx2 = (x * w + y) * 3;
            assert_eq!(frame[idx1], frame[idx2]);
        }
    }

    for i in 0..(w - 1) {
        let idx_curr = (i * w + i) * 3;
        let idx_next = ((i + 1) * w + (i + 1)) * 3;
        assert!(frame[idx_curr] <= frame[idx_next]);
    }
}

#[test]
fn test_all_18_effect_previews_produce_diff() {
    let previews = get_effect_previews().expect("get_effect_previews must succeed");
    assert_eq!(previews.len(), 18);

    let base_frame = generate_generic_preview_frame(256, 256);

    for item in &previews {
        assert!(item.preview_data_url.starts_with("data:image/bmp;base64,"));

        let preview_frame = render_effect_preview(&item.id, 256, 256);
        let diff: i64 = base_frame
            .iter()
            .zip(preview_frame.iter())
            .map(|(&a, &b)| (a as i64 - b as i64).abs())
            .sum();

        assert!(diff > 0);
    }
}

#[test]
fn test_plan_with_manual_effect_overrides() {
    let beats: Vec<f64> = (0..20).map(|i| i as f64 * 0.72).collect();
    let downbeats = vec![2.88, 5.76, 8.64];

    let mut ov = default_effects_for_style("HARD", true);
    ov.shakes = false;
    ov.zoom = false;
    ov.one_framers = false;
    ov.transitions = false;
    ov.flicker = false;

    let plan = create_plan_internal(
        "HARD", 16, &beats, &downbeats, 14.0, 14.4, 1080, 1080, 83.33, true, Some(ov), None,
    ).expect("Plan generation with overrides ok");

    for seg in &plan.segments {
        assert_eq!(seg.effects.shake.a0, 0.0);
        assert_eq!(seg.effects.zoom.scale_start, 1.0);
        assert_eq!(seg.effects.zoom.scale_end, 1.0);
    }
    assert!(plan.one_framers.is_empty());
    assert!(plan.transitions.is_empty());
    if let Some(ref amb) = plan.ambiance {
        assert_eq!(amb.flicker.amplitude, 0.0);
    }

    let mut ov_smooth = default_effects_for_style("SMOOTH", true);
    ov_smooth.bouncy_shake = true;

    let plan_smooth = create_plan_internal(
        "SMOOTH", 16, &beats, &downbeats, 14.0, 14.4, 1080, 1080, 83.33, true, Some(ov_smooth), None,
    ).expect("SMOOTH plan with bouncy override ok");

    let has_bouncy = plan_smooth.segments.iter().any(|s| s.effects.bouncy_shake.is_some());
    assert!(has_bouncy);
}

#[test]
fn test_custom_params_override() {
    let beats: Vec<f64> = (0..16).map(|i| i as f64 * 0.72).collect();
    let downbeats = vec![2.88, 5.76, 8.64];

    let mut cp = get_style_defaults("HARD");
    cp.shake_a0 = 99.0;

    let plan = create_plan_internal(
        "HARD", 16, &beats, &downbeats, 14.0, 14.4, 1080, 1080, 83.33, true, None, Some(cp),
    ).expect("T18 custom override plan ok");

    for (i, seg) in plan.segments.iter().enumerate() {
        assert_eq!(seg.effects.shake.a0, 99.0, "Segment {} mismatch", i);
    }
}

#[test]
fn test_toggle_priority_over_custom_params() {
    let beats: Vec<f64> = (0..16).map(|i| i as f64 * 0.72).collect();
    let downbeats = vec![2.88, 5.76, 8.64];

    let mut ov = default_effects_for_style("HARD", true);
    ov.shakes = false;

    let mut cp = get_style_defaults("HARD");
    cp.shake_a0 = 99.0;

    let plan = create_plan_internal(
        "HARD", 16, &beats, &downbeats, 14.0, 14.4, 1080, 1080, 83.33, true, Some(ov), Some(cp),
    ).expect("T18 toggle priority plan ok");

    for (i, seg) in plan.segments.iter().enumerate() {
        assert_eq!(seg.effects.shake.a0, 0.0, "Segment {} mismatch", i);
    }
}

#[test]
fn test_get_style_defaults_values() {
    let hard = get_style_defaults("HARD");
    let smooth = get_style_defaults("SMOOTH");
    let hybrid = get_style_defaults("HYBRID");

    assert!(hard.shake_a0 > 0.0);
    assert!(hard.zoom_scale_start > 1.0 || hard.zoom_scale_end > 1.0);
    assert!(hard.flicker_amplitude > 0.0);
    assert!(hard.exposure_flash_peak > 0.0);

    assert!(smooth.shake_a0 <= hard.shake_a0);
    assert!(smooth.flicker_amplitude <= hard.flicker_amplitude);

    assert!(hybrid.shake_a0 >= smooth.shake_a0);
    assert!(hybrid.shake_a0 <= hard.shake_a0);

    assert!(hard.vignette_strength >= 0.0);
    assert!(hard.scanlines_opacity >= 0.0);
    assert!(hard.warp_bubble_amplitude > 0.0);
    assert!(hard.wave_warp_height > 0.0);
    assert!(hard.slide_shake_pixels > 0.0);
}

#[test]
fn test_export_config_custom_values() {
    let beats = vec![0.42, 1.14, 1.88, 2.60];
    let downbeats = vec![2.60];
    let mut plan = create_plan_internal(
        "HARD", 16, &beats, &downbeats, 10.0, 5.0, 1080, 1080, 120.0, true, None, None,
    ).expect("Plan generation must succeed");

    let custom_export = ExportConfig {
        codec: "H265".to_string(),
        bitrate_mbps: 30,
        format: "MKV".to_string(),
    };
    plan.export = custom_export.clone();

    let json = serde_json::to_string(&plan).expect("Serialization must succeed");
    let deserialized: ProjectPlan = serde_json::from_str(&json).expect("Deserialization must succeed");

    assert_eq!(deserialized.export.codec, "H265");
    assert_eq!(deserialized.export.bitrate_mbps, 30);
    assert_eq!(deserialized.export.format, "MKV");
}

#[test]
fn test_export_config_retrocompat_default() {
    let beats = vec![0.42, 1.14, 1.88, 2.60];
    let downbeats = vec![2.60];
    let plan = create_plan_internal(
        "HARD", 16, &beats, &downbeats, 10.0, 5.0, 1080, 1080, 120.0, true, None, None,
    ).expect("Plan generation must succeed");

    let mut val = serde_json::to_value(&plan).expect("To JSON value");
    if let Some(obj) = val.as_object_mut() {
        obj.remove("export");
    }

    let legacy_json = serde_json::to_string(&val).expect("Legacy JSON serialization");
    let parsed: ProjectPlan = serde_json::from_str(&legacy_json).expect("Retrocompatible deserialization");

    assert_eq!(parsed.export.codec, "H264");
    assert_eq!(parsed.export.bitrate_mbps, 12);
    assert_eq!(parsed.export.format, "MP4");
}

#[test]
fn test_export_render_h265_fixture() {
    let video_path = r"C:\Users\cia\Downloads\jugg video & audio tester\snaptik_7674387013243538721_v3.mp4";
    let audio_path = r"C:\Users\cia\Downloads\jugg video & audio tester\curiosos.mp3";
    if !std::path::Path::new(video_path).exists() || !std::path::Path::new(audio_path).exists() {
        println!("Test fixture files not found, skipping H.265 render test.");
        return;
    }

    let temp_dir = std::env::temp_dir().join("cia_t19_h265_test");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let out_file = temp_dir.join("test_output_h265.mkv");

    let mut encode_cmd = std::process::Command::new("ffmpeg");
    encode_cmd.args([
        "-y",
        "-i", video_path,
        "-i", audio_path,
        "-t", "2.0",
        "-c:v", "libx265",
        "-b:v", "12M",
        "-pix_fmt", "yuv420p",
        "-c:a", "aac",
        "-shortest",
        &out_file.to_string_lossy(),
    ]);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        encode_cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let status = encode_cmd.status().expect("ffmpeg H.265 encode failed to run");
    assert!(status.success(), "FFmpeg H.265 encode must succeed");
    assert!(out_file.exists(), "H.265 output file must exist");

    let mut probe_cmd = std::process::Command::new("ffprobe");
    probe_cmd.args([
        "-v", "error",
        "-select_streams", "v:0",
        "-show_entries", "stream=codec_name",
        "-of", "default=noprint_wrappers=1:nokey=1",
        &out_file.to_string_lossy(),
    ]);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        probe_cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let probe_out = probe_cmd.output().expect("ffprobe must run");
    let codec_name = String::from_utf8_lossy(&probe_out.stdout).trim().to_lowercase();
    assert!(codec_name == "hevc" || codec_name == "h265", "Codec must be hevc/h265, got: {}", codec_name);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_export_render_vp9_fixture() {
    let video_path = r"C:\Users\cia\Downloads\jugg video & audio tester\snaptik_7674387013243538721_v3.mp4";
    let audio_path = r"C:\Users\cia\Downloads\jugg video & audio tester\curiosos.mp3";
    if !std::path::Path::new(video_path).exists() || !std::path::Path::new(audio_path).exists() {
        println!("Test fixture files not found, skipping VP9 render test.");
        return;
    }

    let temp_dir = std::env::temp_dir().join("cia_t19_vp9_test");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let out_file = temp_dir.join("test_output_vp9.webm");

    let mut encode_cmd = std::process::Command::new("ffmpeg");
    encode_cmd.args([
        "-y",
        "-i", video_path,
        "-i", audio_path,
        "-t", "2.0",
        "-c:v", "libvpx-vp9",
        "-b:v", "12M",
        "-pix_fmt", "yuv420p",
        "-c:a", "libopus",
        "-shortest",
        &out_file.to_string_lossy(),
    ]);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        encode_cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let status = encode_cmd.status().expect("ffmpeg VP9 encode failed to run");
    assert!(status.success(), "FFmpeg VP9 encode must succeed");
    assert!(out_file.exists(), "VP9 output file must exist");

    let mut probe_cmd = std::process::Command::new("ffprobe");
    probe_cmd.args([
        "-v", "error",
        "-select_streams", "v:0",
        "-show_entries", "stream=codec_name",
        "-of", "default=noprint_wrappers=1:nokey=1",
        &out_file.to_string_lossy(),
    ]);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        probe_cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let probe_out = probe_cmd.output().expect("ffprobe must run");
    let codec_name = String::from_utf8_lossy(&probe_out.stdout).trim().to_lowercase();
    assert_eq!(codec_name, "vp9", "Codec must be vp9, got: {}", codec_name);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_export_render_bitrate_comparison() {
    let video_path = r"C:\Users\cia\Downloads\jugg video & audio tester\snaptik_7674387013243538721_v3.mp4";
    let audio_path = r"C:\Users\cia\Downloads\jugg video & audio tester\curiosos.mp3";
    if !std::path::Path::new(video_path).exists() || !std::path::Path::new(audio_path).exists() {
        println!("Test fixture files not found, skipping bitrate comparison test.");
        return;
    }

    let temp_dir = std::env::temp_dir().join("cia_t19_bitrate_test");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let out_12m = temp_dir.join("test_12m.mp4");
    let out_30m = temp_dir.join("test_30m.mp4");

    let mut cmd12 = std::process::Command::new("ffmpeg");
    cmd12.args([
        "-y", "-i", video_path, "-i", audio_path, "-t", "3.0",
        "-c:v", "libx264", "-b:v", "12M", "-pix_fmt", "yuv420p", "-c:a", "aac", "-shortest",
        &out_12m.to_string_lossy(),
    ]);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd12.creation_flags(CREATE_NO_WINDOW);
    }
    let st12 = cmd12.status().expect("12M encode failed");
    assert!(st12.success());

    let mut cmd30 = std::process::Command::new("ffmpeg");
    cmd30.args([
        "-y", "-i", video_path, "-i", audio_path, "-t", "3.0",
        "-c:v", "libx264", "-b:v", "30M", "-pix_fmt", "yuv420p", "-c:a", "aac", "-shortest",
        &out_30m.to_string_lossy(),
    ]);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd30.creation_flags(CREATE_NO_WINDOW);
    }
    let st30 = cmd30.status().expect("30M encode failed");
    assert!(st30.success());

    let size_12m = std::fs::metadata(&out_12m).unwrap().len();
    let size_30m = std::fs::metadata(&out_30m).unwrap().len();

    println!("T19 bitrate comparison: 12M={} bytes ({:.2} MB), 30M={} bytes ({:.2} MB)",
        size_12m, (size_12m as f64) / (1024.0 * 1024.0),
        size_30m, (size_30m as f64) / (1024.0 * 1024.0)
    );

    assert!(size_30m > size_12m, "30 Mbps file must be larger than 12 Mbps file");
    let ratio = (size_30m as f64) / (size_12m as f64);
    assert!(ratio > 1.2, "30 Mbps file must be significantly larger than 12 Mbps file (ratio: {:.2})", ratio);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_resolve_unique_output_path() {
    let temp_dir = std::env::temp_dir().join("cia_test_unique_path");
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).unwrap();

    let base_name = "test_clip";
    let ext = "mp4";

    let p0 = resolve_unique_output_path(&temp_dir, base_name, ext);
    assert_eq!(p0, temp_dir.join("test_clip.mp4"));
    std::fs::write(&p0, b"data0").unwrap();

    let p1 = resolve_unique_output_path(&temp_dir, base_name, ext);
    assert_eq!(p1, temp_dir.join("test_clip-1.mp4"));
    std::fs::write(&p1, b"data1").unwrap();

    let p2 = resolve_unique_output_path(&temp_dir, base_name, ext);
    assert_eq!(p2, temp_dir.join("test_clip-2.mp4"));
    std::fs::write(&p2, b"data2").unwrap();

    let p3 = resolve_unique_output_path(&temp_dir, base_name, ext);
    assert_eq!(p3, temp_dir.join("test_clip-3.mp4"));

    let _ = std::fs::remove_dir_all(&temp_dir);
}

// ─── T23: Dumper Tests ───────────────────────────────────────────────────────

#[test]
fn test_luminance_mad_exact() {
    let width = 4usize;
    let height = 4usize;
    let n = width * height * 3;

    // Frame 1: Pure Gray 100 -> Y = 100
    let f1 = vec![100u8; n];
    // Frame 2: Pure Gray 150 -> Y = 150
    let f2 = vec![150u8; n];

    let mad = compute_luminance_mad(&f1, &f2, width, height);
    assert_eq!(mad, 50.0, "MAD between gray 100 and gray 150 must be exactly 50.0");

    // Identical frames -> MAD = 0.0
    let mad_zero = compute_luminance_mad(&f1, &f1, width, height);
    assert_eq!(mad_zero, 0.0, "MAD of identical frames must be 0.0");
}

#[test]
fn test_lab_stats_gradient() {
    let width = 64usize;
    let height = 64usize;
    let mut frame = vec![0u8; width * height * 3];

    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) * 3;
            frame[idx] = (x * 4) as u8;
            frame[idx + 1] = (y * 4) as u8;
            frame[idx + 2] = 128u8;
        }
    }

    let stats = downsample_and_compute_lab_stats(&frame, width, height);

    assert!(stats.mean[0] > 0.0 && stats.mean[0] < 100.0, "L mean must be in (0, 100), got {}", stats.mean[0]);
    assert!(stats.std[0] > 0.0, "L std must be > 0 on gradient, got {}", stats.std[0]);
    assert!(stats.std[1] > 0.0, "a std must be > 0 on gradient, got {}", stats.std[1]);
    assert!(stats.std[2] > 0.0, "b std must be > 0 on gradient, got {}", stats.std[2]);
}

#[test]
fn test_cut_beat_sync_synthetic() {
    let beats = vec![1.0, 2.0, 3.0, 4.0, 5.0];

    // 0 cuts -> (0.0, true)
    let (sync_empty, is_na) = compute_cut_beat_sync(&[], &beats);
    assert_eq!(sync_empty, 0.0);
    assert!(is_na);

    // Cuts within +/- 60 ms vs outside +/- 60 ms:
    // cut 1: 1.03 (delta 30ms <= 60ms) -> synced
    // cut 2: 2.05 (delta 50ms <= 60ms) -> synced
    // cut 3: 3.10 (delta 100ms > 60ms) -> NOT synced
    let cuts = vec![1.03, 2.05, 3.10];
    let (sync, is_na) = compute_cut_beat_sync(&cuts, &beats);
    assert!(!is_na);
    assert_eq!(sync, 0.6667); // 2 out of 3 = 0.6667
}

#[test]
fn test_dump_analysis_schema_serde() {
    let analysis = DumpAnalysis {
        schema_version: 1,
        source: "C:/test/edit.mp4".to_string(),
        duration: 10.5,
        fps: 60.0,
        cuts: vec![2.0, 5.0, 8.0],
        scenes: vec![
            SceneItem { start: 0.0, end: 2.0 },
            SceneItem { start: 2.0, end: 5.0 },
            SceneItem { start: 5.0, end: 8.0 },
            SceneItem { start: 8.0, end: 10.5 },
        ],
        beats: BeatResult {
            bpm: 128.0,
            beats: vec![2.0, 4.0, 6.0, 8.0, 10.0],
            downbeats: vec![2.0, 6.0, 10.0],
        },
        cut_beat_sync: 0.6667,
        sync_na: false,
        sync_tolerance_ms: Some(60.0),
        detected_style: StyleDecision {
            style_name: "jugg".to_string(),
            sub_style: Some("JUGG (Standard)".to_string()),
            archetype: Some(Archetype::JUGG),
            confidence: 0.90,
            sync_tolerance_ms: Some(60.0),
            justifications: vec!["High shake energy".to_string()],
        },
        one_framers: vec![1.5, 4.2],
        one_framers_v2: Some(vec![1.5, 4.2]),
        segments: vec![
            DumpSegment {
                start: 0.0,
                end: 2.0,
                lab: LabStats {
                    mean: [55.2, 12.4, -8.1],
                    std: [8.5, 4.2, 3.1],
                },
                mad_mean: 14.5,
                mad_peak: 42.1,
                motion: Some(SegmentMotion {
                    shake_energy: 0.025,
                    zoom_presence: true,
                    mean_divergence: 0.012,
                    mean_curl: 0.002,
                }),
                one_framer_count: 1,
                speed_hint: "normal".to_string(),
            },
        ],
        motion_warning: None,
        json_path: Some("C:/test/analysis.json".to_string()),
        report_path: Some("C:/test/edit_report.md".to_string()),
        reusable_project_path: Some("C:/test/reusable_project.json".to_string()),
    };

    let json_str = serde_json::to_string(&analysis).expect("Serialization must succeed");
    let deserialized: DumpAnalysis = serde_json::from_str(&json_str).expect("Deserialization must succeed");

    assert_eq!(deserialized.schema_version, 1);
    assert_eq!(deserialized.cuts.len(), 3);
    assert_eq!(deserialized.scenes.len(), 4);
    assert_eq!(deserialized.beats.bpm, 128.0);
    assert_eq!(deserialized.cut_beat_sync, 0.6667);
    assert!(!deserialized.sync_na);
    assert_eq!(deserialized.detected_style.style_name, "jugg");
    assert_eq!(deserialized.one_framers.len(), 2);
    assert_eq!(deserialized.segments.len(), 1);
    assert_eq!(deserialized.segments[0].lab.mean[0], 55.2);
    assert!(deserialized.segments[0].motion.is_some());
}

#[test]
fn test_motion_extraction_synthetic() {
    // Synthetic TRF line with median translation of dx=10, dy=-5 on 1000x500 frame
    let trf_sample = "VID.STAB 1\nFrame 1 (List 0 [])\nFrame 2 (List 3 [(LM 9 -5 400 200 32 0.5 0.5),(LM 10 -5 500 250 32 0.5 0.5),(LM 11 -4 600 300 32 0.5 0.5)])";
    let frames = parse_trf_content(trf_sample, 1000.0, 500.0, 30.0);
    assert_eq!(frames.len(), 2);

    // Frame 1 is empty
    assert_eq!(frames[0].tx, 0.0);
    assert_eq!(frames[0].ty, 0.0);

    // Frame 2: median dx=10 -> tx = 10/1000 = 0.01, median dy=-5 -> ty = -5/500 = -0.01
    assert!((frames[1].tx - 0.01).abs() < 1e-4);
    assert!((frames[1].ty - (-0.01)).abs() < 1e-4);
}

#[test]
fn test_one_framer_detection_synthetic() {
    // Baseline ~5.0 with an isolated spike to 45.0 at index 5 (t=2.5s)
    let mad_series = vec![5.0, 5.2, 4.8, 5.1, 5.0, 45.0, 5.2, 5.0, 4.9, 5.1];
    let timestamps = vec![0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 4.5];

    let one_framers = detect_one_framers(&mad_series, &timestamps);
    assert_eq!(one_framers.len(), 1);
    assert!((one_framers[0] - 2.5).abs() < 1e-4);
}

#[test]
fn test_one_framer_v2_detection() {
    // Synthetic fixture: 3 frames (Noir - Blanc - Noir)
    let mad_series = vec![0.0, 100.0, 0.0];
    let timestamps = vec![0.0, 0.03333, 0.06666];

    let detected = detect_one_framers_v2(&mad_series, &timestamps);
    assert_eq!(detected.len(), 1, "Must detect exactly 1 one-framer v2 spike");
    assert!((detected[0] - 0.03333).abs() < 1e-4, "Spike must be at center frame t=0.03333s");
}

#[test]
fn test_sync_tolerance_bpm_scaling() {
    // 60 BPM -> 120ms (max bound)
    let tol_60 = compute_sync_tolerance_ms(60.0);
    assert!((tol_60 - 120.0).abs() < 1e-4, "60 BPM must give 120ms tolerance, got {}", tol_60);

    // 100 BPM -> 120ms (12000 / 100 = 120)
    let tol_100 = compute_sync_tolerance_ms(100.0);
    assert!((tol_100 - 120.0).abs() < 1e-4, "100 BPM must give 120ms tolerance, got {}", tol_100);

    // 160 BPM -> 75ms (12000 / 160 = 75)
    let tol_160 = compute_sync_tolerance_ms(160.0);
    assert!((tol_160 - 75.0).abs() < 1e-4, "160 BPM must give 75ms tolerance, got {}", tol_160);

    // 180 BPM -> ~66.67ms (12000 / 180 = 66.6667)
    let tol_180 = compute_sync_tolerance_ms(180.0);
    assert!((tol_180 - 66.6667).abs() < 0.01, "180 BPM must give ~66.67ms tolerance, got {}", tol_180);

    // 200 BPM -> 60ms (12000 / 200 = 60)
    let tol_200 = compute_sync_tolerance_ms(200.0);
    assert!((tol_200 - 60.0).abs() < 1e-4, "200 BPM must give 60ms tolerance, got {}", tol_200);

    // 300 BPM -> 40ms (min bound: 12000 / 300 = 40)
    let tol_300 = compute_sync_tolerance_ms(300.0);
    assert!((tol_300 - 40.0).abs() < 1e-4, "300 BPM must give 40ms tolerance, got {}", tol_300);
}

#[test]
fn test_classifier_flow_signature() {
    // Simulate "Flow" features: sync > 0.5, low shake, no one-framers, slowdowns present
    let features = ClassifierFeatures {
        cuts_count: 4,
        cut_density: 0.4,
        shake_energy: 0.008,
        one_framer_density: 0.0,
        one_framer_density_v2: 0.0,
        sync: 0.65,
        sync_downbeats_only: false,
        zoom_presence: false,
        slowdown_presence: true,
        motion_available: true,
        bpm: 120.0,
        sync_tolerance_ms: 100.0,
    };

    let decision = classify_style(&features);
    assert_eq!(decision.archetype, Some(Archetype::FLOW));
    assert_eq!(decision.style_name, "velocity/flow");
    assert_eq!(decision.sub_style.as_deref(), Some("FLOW (Liquid)"));
}

#[test]
fn test_style_classifier_5_archetypes() {
    // 1. basic/clean
    let feat_basic = ClassifierFeatures {
        cuts_count: 1,
        cut_density: 0.1,
        shake_energy: 0.005,
        one_framer_density: 0.0,
        one_framer_density_v2: 0.0,
        sync: 0.0,
        sync_downbeats_only: false,
        zoom_presence: false,
        slowdown_presence: false,
        motion_available: true,
        bpm: 120.0,
        sync_tolerance_ms: 100.0,
    };
    assert_eq!(classify_style(&feat_basic).style_name, "basic/clean");

    // 2. jugg
    let feat_jugg = ClassifierFeatures {
        cuts_count: 8,
        cut_density: 0.8,
        shake_energy: 0.020,
        one_framer_density: 0.40,
        one_framer_density_v2: 0.40,
        sync: 0.65,
        sync_downbeats_only: false,
        zoom_presence: true,
        slowdown_presence: false,
        motion_available: true,
        bpm: 140.0,
        sync_tolerance_ms: 85.7,
    };
    assert_eq!(classify_style(&feat_jugg).style_name, "jugg");

    // 3. glitch-leaning
    let feat_glitch = ClassifierFeatures {
        cuts_count: 20,
        cut_density: 2.2,
        shake_energy: 0.015,
        one_framer_density: 0.20,
        one_framer_density_v2: 0.20,
        sync: 0.30,
        sync_downbeats_only: false,
        zoom_presence: false,
        slowdown_presence: false,
        motion_available: true,
        bpm: 130.0,
        sync_tolerance_ms: 92.3,
    };
    assert_eq!(classify_style(&feat_glitch).style_name, "glitch-leaning");

    // 4. velocity/flow
    let feat_velocity = ClassifierFeatures {
        cuts_count: 4,
        cut_density: 0.4,
        shake_energy: 0.008,
        one_framer_density: 0.05,
        one_framer_density_v2: 0.05,
        sync: 0.70,
        sync_downbeats_only: false,
        zoom_presence: false,
        slowdown_presence: true,
        motion_available: true,
        bpm: 120.0,
        sync_tolerance_ms: 100.0,
    };
    assert_eq!(classify_style(&feat_velocity).style_name, "velocity/flow");

    // 5. hybrid/unclassified
    let feat_hybrid = ClassifierFeatures {
        cuts_count: 5,
        cut_density: 0.5,
        shake_energy: 0.008,
        one_framer_density: 0.05,
        one_framer_density_v2: 0.05,
        sync: 0.20,
        sync_downbeats_only: false,
        zoom_presence: false,
        slowdown_presence: false,
        motion_available: true,
        bpm: 120.0,
        sync_tolerance_ms: 100.0,
    };
    assert_eq!(classify_style(&feat_hybrid).style_name, "hybrid/unclassified");
}

#[test]
fn test_markdown_report_mandatory_sections() {
    let analysis = DumpAnalysis {
        schema_version: 1,
        source: "C:/test/edit.mp4".to_string(),
        duration: 12.0,
        fps: 30.0,
        cuts: vec![2.0, 6.0],
        scenes: vec![
            SceneItem { start: 0.0, end: 2.0 },
            SceneItem { start: 2.0, end: 6.0 },
            SceneItem { start: 6.0, end: 12.0 },
        ],
        beats: BeatResult { bpm: 120.0, beats: vec![1.0, 2.0, 3.0], downbeats: vec![1.0] },
        cut_beat_sync: 0.50,
        sync_na: false,
        sync_tolerance_ms: Some(100.0),
        detected_style: StyleDecision {
            style_name: "velocity/flow".to_string(),
            sub_style: Some("FLOW (Liquid)".to_string()),
            archetype: Some(Archetype::FLOW),
            confidence: 0.85,
            sync_tolerance_ms: Some(100.0),
            justifications: vec!["Speed ramping detected".to_string()],
        },
        one_framers: vec![1.5],
        one_framers_v2: Some(vec![1.5]),
        segments: vec![
            DumpSegment {
                start: 0.0,
                end: 2.0,
                lab: LabStats { mean: [50.0, 0.0, 0.0], std: [10.0, 2.0, 2.0] },
                mad_mean: 8.0,
                mad_peak: 20.0,
                motion: Some(SegmentMotion { shake_energy: 0.010, zoom_presence: false, mean_divergence: 0.0, mean_curl: 0.0 }),
                one_framer_count: 1,
                speed_hint: "normal".to_string(),
            }
        ],
        motion_warning: None,
        json_path: Some("C:/test/analysis.json".to_string()),
        report_path: Some("C:/test/edit_report.md".to_string()),
        reusable_project_path: Some("C:/test/reusable_project.json".to_string()),
    };

    let project = ReusableProject {
        schema_version: "dumper_project_v1".to_string(),
        source: "C:/test/edit.mp4".to_string(),
        beats: analysis.beats.clone(),
        cuts: analysis.cuts.clone(),
        segments: vec![],
        suggested_style: "velocity/flow".to_string(),
        fps_suggestion: 30.0,
    };

    let report = generate_markdown_report(&analysis, &project);

    // Verify all mandatory section headers exist in generated report
    assert!(report.contains("# Dump Report"));
    assert!(report.contains("## Detected style"));
    assert!(report.contains("## Cuts & sync"));
    assert!(report.contains("## Beats"));
    assert!(report.contains("## Segments (signatures)"));
    assert!(report.contains("## Color signatures"));
    assert!(report.contains("## One-framers"));
    assert!(report.contains("## Motion"));
    assert!(report.contains("## Reusable vs descriptive"));
}

#[test]
fn test_reusable_project_json_schema() {
    let proj = ReusableProject {
        schema_version: "dumper_project_v1".to_string(),
        source: "C:/test/cut.mp4".to_string(),
        beats: BeatResult { bpm: 115.4, beats: vec![0.5, 1.0], downbeats: vec![0.5] },
        cuts: vec![0.68, 2.57],
        segments: vec![
            ReusableSegment {
                start: 0.0,
                end: 0.68,
                lab_mean: [46.5, -13.2, 9.0],
                lab_std: [22.2, 19.6, 25.1],
                speed_hint: "normal".to_string(),
            }
        ],
        suggested_style: "jugg".to_string(),
        fps_suggestion: 60.0,
    };

    let json_str = serde_json::to_string(&proj).expect("Serialize ReusableProject");
    let deserialized: ReusableProject = serde_json::from_str(&json_str).expect("Deserialize ReusableProject");

    assert_eq!(deserialized.schema_version, "dumper_project_v1");
    assert_eq!(deserialized.suggested_style, "jugg");
    assert_eq!(deserialized.fps_suggestion, 60.0);
    assert_eq!(deserialized.segments.len(), 1);
    assert_eq!(deserialized.segments[0].speed_hint, "normal");
}

#[test]
fn test_detect_scenes_snaptik_fixture() {
    let video_path = r"C:\Users\cia\Downloads\jugg video & audio tester\snaptik_7674387013243538721_v3.mp4";
    if std::path::Path::new(video_path).exists() {
        let exe_path = std::env::current_dir().unwrap_or_default().join("binaries").join("scenedetect.exe");
        if exe_path.exists() || std::path::Path::new(r"C:\Users\cia\Music\cia-app-jugg\src-tauri\binaries\scenedetect.exe").exists() {
            let bin = if exe_path.exists() {
                exe_path
            } else {
                std::path::PathBuf::from(r"C:\Users\cia\Music\cia-app-jugg\src-tauri\binaries\scenedetect.exe")
            };

            let mut cmd = std::process::Command::new(bin);
            cmd.arg(video_path);
            let out = cmd.output().expect("scenedetect execution failed");
            assert!(out.status.success(), "scenedetect must exit with 0");
            let parsed: serde_json::Value = serde_json::from_slice(&out.stdout).expect("Valid JSON");
            assert!(parsed.get("cuts").is_some());
        }
    }
}

#[test]
fn test_benchmark_dumper_analysis() {
    use std::io::Read;
    #[cfg(target_os = "windows")]
    use std::os::windows::process::CommandExt;

    let video_path = r"C:\Users\cia\Downloads\jugg video & audio tester\snaptik_7674387013243538721_v3.mp4";
    if !std::path::Path::new(video_path).exists() {
        return;
    }

    let probe = probe_media_internal(video_path, None).expect("Probe failed");
    let analysis_fps = probe.fps.min(30.0);
    let width = 640usize;
    let height = ((probe.height as f64 * (640.0 / probe.width as f64)).round() as usize) & !1;
    let frame_bytes = width * height * 3;

    // 1. Measure pure decode time
    let mut decode_cmd = std::process::Command::new("ffmpeg");
    decode_cmd.args([
        "-v", "error",
        "-i", video_path,
        "-vf", &format!("fps={},scale={}:{}", analysis_fps, width, height),
        "-f", "rawvideo",
        "-pix_fmt", "rgb24",
        "-",
    ]);
    #[cfg(target_os = "windows")]
    decode_cmd.creation_flags(CREATE_NO_WINDOW);
    decode_cmd.stdout(std::process::Stdio::piped());

    let t_dec_start = std::time::Instant::now();
    let mut decode_child = decode_cmd.spawn().expect("Decode spawn failed");
    let mut decode_stdout = decode_child.stdout.take().unwrap();
    let mut buf = vec![0u8; frame_bytes];
    let mut _decode_frame_count = 0usize;
    while decode_stdout.read_exact(&mut buf).is_ok() {
        _decode_frame_count += 1;
    }
    let _ = decode_child.wait();
    let decode_time = t_dec_start.elapsed().as_secs_f64();

    // 2. Measure analysis time (decode + MAD + CIELAB)
    let mut ana_cmd = std::process::Command::new("ffmpeg");
    ana_cmd.args([
        "-v", "error",
        "-i", video_path,
        "-vf", &format!("fps={},scale={}:{}", analysis_fps, width, height),
        "-f", "rawvideo",
        "-pix_fmt", "rgb24",
        "-",
    ]);
    #[cfg(target_os = "windows")]
    ana_cmd.creation_flags(CREATE_NO_WINDOW);
    ana_cmd.stdout(std::process::Stdio::piped());

    let t_ana_start = std::time::Instant::now();
    let mut ana_child = ana_cmd.spawn().expect("Ana spawn failed");
    let mut ana_stdout = ana_child.stdout.take().unwrap();
    let mut curr_frame = vec![0u8; frame_bytes];
    let mut prev_frame = vec![0u8; frame_bytes];
    let mut is_first = true;
    let mut ana_frame_count = 0usize;

    while ana_stdout.read_exact(&mut curr_frame).is_ok() {
        let _mad = if is_first {
            is_first = false;
            0.0
        } else {
            compute_luminance_mad(&curr_frame, &prev_frame, width, height)
        };
        let _lab = downsample_and_compute_lab_stats(&curr_frame, width, height);
        prev_frame.copy_from_slice(&curr_frame);
        ana_frame_count += 1;
    }
    let _ = ana_child.wait();
    let analysis_time = t_ana_start.elapsed().as_secs_f64();

    let ratio = analysis_time / decode_time.max(0.001);
    println!(
        "\n[BENCH] Dumper Profile Pass ({} frames):\n  Pure Decode Time:   {:.3}s\n  Full Analysis Time: {:.3}s\n  Overhead Ratio:     {:.2}x (< 3.0x threshold)\n",
        ana_frame_count, decode_time, analysis_time, ratio
    );

    assert!(ratio < 3.0, "Analysis time ({:.3}s) must be < 3x decode time ({:.3}s), ratio was {:.2}x", analysis_time, decode_time, ratio);
}

#[test]
fn test_benchmark_motion_pass() {
    #[cfg(target_os = "windows")]
    use std::os::windows::process::CommandExt;

    let video_path = r"C:\Users\cia\Downloads\cut.mp4";
    if !std::path::Path::new(video_path).exists() {
        return;
    }

    let temp_trf = std::env::temp_dir().join(format!("bench_motion_{}.trf", std::process::id()));
    let temp_trf_str = temp_trf.to_string_lossy().replace('\\', "/").replace(':', "\\:");

    let t_start = std::time::Instant::now();
    let mut cmd = std::process::Command::new("ffmpeg");
    cmd.args([
        "-v", "error",
        "-y",
        "-i", video_path,
        "-vf", &format!("scale=640:-2,fps=30,vidstabdetect=result='{}':fileformat=ascii:shakiness=5:accuracy=9:stepsize=12", temp_trf_str),
        "-an",
        "-f", "null",
        "-",
    ]);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let status = cmd.status().expect("FFmpeg motion bench command failed");
    let motion_time = t_start.elapsed().as_secs_f64();

    if temp_trf.exists() {
        let _ = std::fs::remove_file(&temp_trf);
    }

    assert!(status.success(), "Motion command must succeed");
    println!(
        "\n[BENCH] Motion Extraction Pass on cut.mp4:\n  Motion Time: {:.3}s (< 2x T23 analysis threshold ~30s)\n",
        motion_time
    );
    assert!(motion_time < 60.0, "Motion extraction time ({:.3}s) must be < 60s", motion_time);
}

#[test]
fn test_full_dump_pipeline_fixtures() {
    let fixtures = [
        r"C:\Users\cia\Downloads\jugg video & audio tester\snaptik_7674387013243538721_v3.mp4",
        r"C:\Users\cia\Downloads\cut.mp4",
    ];

    for video_path in fixtures {
        if !std::path::Path::new(video_path).exists() {
            println!("\n[FIXTURE] File not found: {}, skipping test.", video_path);
            continue;
        }

        println!("\n=======================================================");
        println!(">>> RUNNING FULL DUMP PIPELINE ON: {}", video_path);
        println!("=======================================================");

        let t_start = std::time::Instant::now();
        let res = run_dump_pipeline_internal(None, video_path);
        let elapsed = t_start.elapsed().as_secs_f64();

        match res {
            Ok(analysis) => {
                println!("[DUMP SUCCESS in {:.2}s]", elapsed);
                println!("  Source:            {}", analysis.source);
                println!("  Duration:          {:.2}s", analysis.duration);
                println!("  FPS:               {:.2}", analysis.fps);
                println!("  Detected Style:    {} (Confidence: {:.0}%)", analysis.detected_style.style_name, analysis.detected_style.confidence * 100.0);
                println!("  Justifications:    {:?}", analysis.detected_style.justifications);
                println!("  Cuts count:        {} -> {:?}", analysis.cuts.len(), analysis.cuts);
                println!("  Scenes count:      {}", analysis.scenes.len());
                println!("  BPM:               {:.1}", analysis.beats.bpm);
                println!("  Beats count:       {}", analysis.beats.beats.len());
                println!("  Downbeats count:   {}", analysis.beats.downbeats.len());
                if analysis.sync_na {
                    println!("  Cut-Beat Sync:     N/A (0 cuts)");
                } else {
                    println!("  Cut-Beat Sync:     {:.1}% (±60ms)", analysis.cut_beat_sync * 100.0);
                }
                println!("  One-framers count: {}", analysis.one_framers.len());
                println!("  Segments count:    {}", analysis.segments.len());
                if let Some(first_seg) = analysis.segments.first() {
                    println!("  First segment:     [{:.2}s - {:.2}s] LAB mean: {:?}, std: {:?}, MAD mean: {:.2}, peak: {:.2}, hint: {}",
                        first_seg.start, first_seg.end, first_seg.lab.mean, first_seg.lab.std, first_seg.mad_mean, first_seg.mad_peak, first_seg.speed_hint
                    );
                }
                println!("  JSON saved to:     {:?}", analysis.json_path);
                println!("  Report saved to:   {:?}", analysis.report_path);
                println!("  Reusable proj to:  {:?}", analysis.reusable_project_path);

                assert_eq!(analysis.schema_version, 1);
                assert!(analysis.duration > 0.0);
                assert!(analysis.json_path.is_some());
                if let Some(json_p) = analysis.json_path {
                    assert!(std::path::Path::new(&json_p).exists(), "Saved JSON file must exist");
                }
                if let Some(rep_p) = analysis.report_path {
                    assert!(std::path::Path::new(&rep_p).exists(), "Saved Report file must exist");
                }
                if let Some(proj_p) = analysis.reusable_project_path {
                    assert!(std::path::Path::new(&proj_p).exists(), "Saved Reusable Project file must exist");
                }
            }
            Err(e) => {
                panic!("Dump pipeline failed for {}: {}", video_path, e);
            }
        }
    }
}

#[test]
fn test_convert_dumper_project_style_mapping_5_archetypes() {
    assert_eq!(map_dumper_style_to_jugg_style("jugg"), "HARD");
    assert_eq!(map_dumper_style_to_jugg_style("glitch-leaning"), "HARD");
    assert_eq!(map_dumper_style_to_jugg_style("velocity/flow"), "SMOOTH");
    assert_eq!(map_dumper_style_to_jugg_style("basic/clean"), "SMOOTH");
    assert_eq!(map_dumper_style_to_jugg_style("hybrid/unclassified"), "HYBRID");
    assert_eq!(map_dumper_style_to_jugg_style("unknown-style"), "HYBRID");

    // Full plan conversion style verification
    let styles = [
        ("jugg", "HARD"),
        ("glitch-leaning", "HARD"),
        ("velocity/flow", "SMOOTH"),
        ("basic/clean", "SMOOTH"),
        ("hybrid/unclassified", "HYBRID"),
    ];

    for (dumper_style, expected_jugg_style) in styles {
        let proj = ReusableProject {
            schema_version: "dumper_project_v1".to_string(),
            source: "C:/test/edit.mp4".to_string(),
            beats: BeatResult { bpm: 120.0, beats: vec![1.0, 2.0], downbeats: vec![1.0] },
            cuts: vec![2.0],
            segments: vec![
                ReusableSegment {
                    start: 0.0,
                    end: 2.0,
                    lab_mean: [50.0, 0.0, 0.0],
                    lab_std: [10.0, 5.0, 5.0],
                    speed_hint: "normal".to_string(),
                },
                ReusableSegment {
                    start: 2.0,
                    end: 4.0,
                    lab_mean: [60.0, 2.0, -1.0],
                    lab_std: [8.0, 4.0, 4.0],
                    speed_hint: "fast".to_string(),
                },
            ],
            suggested_style: dumper_style.to_string(),
            fps_suggestion: 30.0,
        };

        let plan = convert_dumper_project_to_plan(&proj).expect("Conversion must succeed");
        assert_eq!(plan.schema_version, 2);
        assert_eq!(plan.style, expected_jugg_style, "Style mismatch for {}", dumper_style);
    }
}

#[test]
fn test_convert_dumper_project_fps_clamp_bounds() {
    // 1. Lower bound (< 12 -> 12)
    assert_eq!(clamp_dumper_fps(5.0), 12);
    assert_eq!(clamp_dumper_fps(11.4), 12);
    assert_eq!(clamp_dumper_fps(12.0), 12);

    // 2. Upper bound (> 60 -> 60)
    assert_eq!(clamp_dumper_fps(60.0), 60);
    assert_eq!(clamp_dumper_fps(120.0), 60);
    assert_eq!(clamp_dumper_fps(240.0), 60);

    // 3. Normal values rounded
    assert_eq!(clamp_dumper_fps(29.97), 30);
    assert_eq!(clamp_dumper_fps(24.0), 24);
}

#[test]
fn test_convert_dumper_project_contiguity_and_color_hints() {
    let proj = ReusableProject {
        schema_version: "dumper_project_v1".to_string(),
        source: "C:/test/edit.mp4".to_string(),
        beats: BeatResult {
            bpm: 128.0,
            beats: vec![0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0],
            downbeats: vec![0.5, 2.5],
        },
        cuts: vec![0.85, 2.10, 3.45],
        segments: vec![
            ReusableSegment {
                start: 0.0,
                end: 0.85,
                lab_mean: [42.1, 10.5, -5.2],
                lab_std: [15.2, 8.1, 7.3],
                speed_hint: "normal".to_string(),
            },
            ReusableSegment {
                start: 0.85,
                end: 2.10,
                lab_mean: [55.3, -8.4, 12.1],
                lab_std: [18.4, 9.2, 11.0],
                speed_hint: "snap".to_string(),
            },
            ReusableSegment {
                start: 2.10,
                end: 3.45,
                lab_mean: [30.2, 4.1, 2.0],
                lab_std: [12.0, 4.5, 3.8],
                speed_hint: "slow".to_string(),
            },
            ReusableSegment {
                start: 3.45,
                end: 5.00,
                lab_mean: [65.0, 1.0, -1.0],
                lab_std: [20.0, 6.0, 5.0],
                speed_hint: "fast".to_string(),
            },
        ],
        suggested_style: "jugg".to_string(),
        fps_suggestion: 120.0, // Should clamp to 60
    };

    let plan = convert_dumper_project_to_plan(&proj).expect("Conversion must succeed");

    // 1. Schema version and parameters
    assert_eq!(plan.schema_version, 2);
    assert_eq!(plan.style, "HARD");
    assert_eq!(plan.fps, 60);
    assert_eq!(plan.bpm, 128.0);
    assert_eq!(plan.target_duration, 5.00);

    // 2. Strict contiguity from 0.0 to target_duration
    assert_eq!(plan.segments.len(), 4);
    assert_eq!(plan.segments[0].t0, 0.0);
    assert_eq!(plan.segments.last().unwrap().t1, 5.00);

    for win in plan.segments.windows(2) {
        let seg_n = &win[0];
        let seg_n1 = &win[1];
        assert_eq!(
            seg_n.t1, seg_n1.t0,
            "Segment boundary contiguity broken between [{:.3}-{:.3}] and [{:.3}-{:.3}]",
            seg_n.t0, seg_n.t1, seg_n1.t0, seg_n1.t1
        );
    }

    // 3. Color hints present on every segment
    for (idx, seg) in plan.segments.iter().enumerate() {
        assert!(seg.color_hints.is_some(), "Segment {} must have color_hints", idx);
        let ch = seg.color_hints.as_ref().unwrap();
        assert_eq!(ch.lab_mean, proj.segments[idx].lab_mean);
        assert_eq!(ch.lab_std, proj.segments[idx].lab_std);
    }
}

#[test]
fn test_apply_dumper_project_on_cut_fixture_flow() {
    let video_path = r"C:\Users\cia\Downloads\cut.mp4";
    if !std::path::Path::new(video_path).exists() {
        return;
    }

    println!("\n=======================================================");
    println!(">>> FULL FLOW: DUMP cut.mp4 -> APPLY AS PROJECT -> JUGG PLAN");
    println!("=======================================================");

    let t_start = std::time::Instant::now();
    let analysis = run_dump_pipeline_internal(None, video_path).expect("Dump pipeline failed");
    let dump_elapsed = t_start.elapsed().as_secs_f64();

    println!("[1. DUMP COMPLETED in {:.2}s]", dump_elapsed);
    println!("  Source:            {}", analysis.source);
    println!("  Raw Detected Style: {} ({:.0}%)", analysis.detected_style.style_name, analysis.detected_style.confidence * 100.0);
    println!("  Raw Source FPS:    {:.2}", analysis.fps);
    println!("  Cuts:              {} cuts", analysis.cuts.len());
    println!("  Reusable Path:     {:?}", analysis.reusable_project_path);

    let reusable_path = analysis.reusable_project_path.expect("Reusable project path must exist");
    assert!(std::path::Path::new(&reusable_path).exists());

    // 2. Apply as project (conversion)
    let plan = apply_dumper_project(Some(reusable_path.clone()), None).expect("Apply dumper project failed");

    println!("\n[2. APPLIED AS JUGG PROJECT PLAN]");
    println!("  Plan Schema:       v{}", plan.schema_version);
    println!("  Mapped Jugg Style: {}", plan.style);
    println!("  Clamped Target FPS:{}", plan.fps);
    println!("  Target Duration:   {:.3}s", plan.target_duration);
    println!("  Segments Count:    {}", plan.segments.len());
    println!("  One-Framers Count: {}", plan.one_framers.len());
    println!("  Transitions Count: {}", plan.transitions.len());
    println!("  Ambiance Config:   {}", if plan.ambiance.is_some() { "Present" } else { "None" });
    if let Some(first_seg) = plan.segments.first() {
        println!("  First Segment:     [{:.3}s - {:.3}s] Curve: {}, ColorHints: {:?}",
            first_seg.t0, first_seg.t1, first_seg.curve, first_seg.color_hints
        );
    }

    // Invariant assertions
    assert_eq!(plan.schema_version, 2);
    assert_eq!(plan.style, "HARD"); // glitch-leaning -> HARD
    assert_eq!(plan.fps, 60);       // 120 clamped to 60
    assert!(plan.target_duration > 0.0);
    assert!(!plan.segments.is_empty());
    assert_eq!(plan.segments[0].t0, 0.0);
    assert!((plan.segments.last().unwrap().t1 - plan.target_duration).abs() < 1e-3);

    for win in plan.segments.windows(2) {
        let seg_n = &win[0];
        let seg_n1 = &win[1];
        assert_eq!(
            seg_n.t1, seg_n1.t0,
            "Contiguity broken between segments [{:.3}-{:.3}] and [{:.3}-{:.3}]",
            seg_n.t0, seg_n.t1, seg_n1.t0, seg_n1.t1
        );
    }
}

// ─── T27 COMPOSITION & SEE-THROUGH TESTS ───────────────────────────────────

#[test]
fn test_composition_layers_json_schema() {
    let raw_json = r#"[
        { "name": "hair_back", "file": "hair_back.png", "zOrder": 0, "hasContent": true },
        { "name": "body", "file": "body.png", "zOrder": 1, "hasContent": true },
        { "name": "clothes_lower", "file": "clothes_lower.png", "zOrder": 2, "hasContent": true },
        { "name": "clothes_upper", "file": "clothes_upper.png", "zOrder": 3, "hasContent": true },
        { "name": "face", "file": "face.png", "zOrder": 4, "hasContent": false },
        { "name": "mouth", "file": "mouth.png", "zOrder": 5, "hasContent": true },
        { "name": "eyes", "file": "eyes.png", "zOrder": 6, "hasContent": true },
        { "name": "hair_front", "file": "hair_front.png", "zOrder": 7, "hasContent": true },
        { "name": "accessories", "file": "accessories.png", "zOrder": 8, "hasContent": false }
    ]"#;

    let layers: Vec<LayerItem> = serde_json::from_str(raw_json).expect("layers.json must parse correctly");
    assert_eq!(layers.len(), 9);
    assert_eq!(layers[0].name, "hair_back");
    assert_eq!(layers[0].z_order, 0);
    assert_eq!(layers[0].has_content, Some(true));

    let proj = CompProject {
        schema_version: "comp_project_v1".to_string(),
        character_path: "C:/test/character.png".to_string(),
        background_path: Some("C:/test/bg.mp4".to_string()),
        audio_path: None,
        layers: layers.clone(),
        parallax_strength: None,
        beat_punch_intensity: None,
        light_wrap_intensity: None,
        chromatic_aberration: None,
        impact_blur_strength: None,
    };

    let serialized = serde_json::to_string(&proj).expect("CompProject must serialize");
    let deserialized: CompProject = serde_json::from_str(&serialized).expect("CompProject must deserialize");
    assert_eq!(deserialized.schema_version, "comp_project_v1");
    assert_eq!(deserialized.layers.len(), 9);
}

#[test]
fn test_gpu_detection_clean_status() {
    let gpu_res = check_nvidia_gpu_internal();
    match gpu_res {
        Ok(info) => {
            println!("NVIDIA GPU successfully detected: {}", info);
            assert!(!info.is_empty());
        }
        Err(err) => {
            println!("NVIDIA GPU not detected: {}", err);
            assert!(err.contains("NVIDIA GPU"));
        }
    }
}

#[test]
fn test_see_through_segmentation_and_recomposition_accuracy() {
    let character_path = r"C:\Users\cia\Downloads\spider-man-11530958085nzzlmiz6hg-732305370.png";
    if !std::path::Path::new(character_path).exists() {
        return;
    }

    let temp_out = std::env::temp_dir().join("cia_t27_test_comp");
    let _ = std::fs::remove_dir_all(&temp_out);

    let res = segment_character_internal(None, character_path, Some(temp_out.to_str().unwrap()))
        .expect("Character segmentation must succeed");

    assert_eq!(res.status, "success");
    assert!(res.layers_count >= 8);
    assert!(std::path::Path::new(&res.layers_json_path).exists());

    // Verify all layer PNGs exist
    for layer in &res.layers {
        let p = temp_out.join(&layer.file);
        assert!(p.exists(), "Layer file {} must exist", layer.file);
    }

    // LE TEST DE JUSTESSE: Recomposition test in Rust
    // Verify each layer loads, composite in z-order on transparent background, compare with original
    let mut py_test = std::process::Command::new("py");
    py_test.arg("-3.11");
    py_test.arg("-c");
    py_test.arg(format!(
        r#"
from PIL import Image
import numpy as np
import json, os, sys

orig = Image.open(r"{char_p}").convert("RGBA")
orig_arr = np.array(orig)

out_dir = r"{out_d}"
with open(os.path.join(out_dir, "layers.json")) as f:
    layers = json.load(f)

layers_sorted = sorted(layers, key=lambda x: x["zOrder"])

recomp = Image.new("RGBA", orig.size, (0, 0, 0, 0))
for l in layers_sorted:
    l_path = os.path.join(out_dir, l["file"])
    if os.path.exists(l_path):
        l_img = Image.open(l_path).convert("RGBA")
        recomp.alpha_composite(l_img)

recomp_arr = np.array(recomp)
opaque = orig_arr[:, :, 3] > 10
diff = np.abs(orig_arr[opaque].astype(int) - recomp_arr[opaque].astype(int))
max_diff = np.max(diff)
print(f"Max pixel difference on opaque regions: {{max_diff}}")
if max_diff > 2:
    sys.exit(1)
sys.exit(0)
"#,
        char_p = character_path,
        out_d = temp_out.to_str().unwrap().replace('\\', "/")
    ));

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        py_test.creation_flags(0x08000000);
    }

    let py_out = py_test.output().expect("Failed to run python recomposition verification");
    println!("Python verification output: {}", String::from_utf8_lossy(&py_out.stdout));
    assert!(
        py_out.status.success(),
        "Recomposition error exceeds tolerance (2/channel): {}",
        String::from_utf8_lossy(&py_out.stderr)
    );
}

#[test]
fn test_save_composition_project() {
    let layers = vec![
        LayerItem {
            name: "face".to_string(),
            file: "face.png".to_string(),
            z_order: 4,
            has_content: Some(true),
            full_path: Some("C:/test/face.png".to_string()),
            thumbnail_base64: None,
            z_depth: None,
        },
        LayerItem {
            name: "body".to_string(),
            file: "body.png".to_string(),
            z_order: 1,
            has_content: Some(true),
            full_path: Some("C:/test/body.png".to_string()),
            thumbnail_base64: None,
            z_depth: None,
        },
    ];

    let proj = CompProject {
        schema_version: "comp_project_v1".to_string(),
        character_path: "C:/test/spiderman.png".to_string(),
        background_path: Some("C:/test/bg.mp4".to_string()),
        audio_path: None,
        layers,
        parallax_strength: None,
        beat_punch_intensity: None,
        light_wrap_intensity: None,
        chromatic_aberration: None,
        impact_blur_strength: None,
    };

    let target = std::env::temp_dir().join("test_comp_proj.json");
    let saved_path = save_composition_project(proj, Some(target.to_str().unwrap().to_string()))
        .expect("Saving comp project must succeed");

    assert!(std::path::Path::new(&saved_path).exists());
    let content = std::fs::read_to_string(&saved_path).unwrap();
    assert!(content.contains("comp_project_v1"));
    assert!(content.contains("spiderman.png"));
}

#[test]
fn test_alpha_over_exact() {
    // Semi-transparent red (200, 0, 0, 128) over solid blue (0, 0, 200, 255)
    let bg = [0u8, 0, 200, 255];
    let fg = [200u8, 0, 0, 128]; // a ~ 0.50196

    let blended = alpha_over_pixel(bg, fg);
    // Expected: R ≈ 200 * 0.50196 ≈ 100, B ≈ 200 * (1 - 0.50196) ≈ 100
    assert_eq!(blended[3], 255);
    assert!((blended[0] as i32 - 100).abs() <= 2);
    assert_eq!(blended[1], 0);
    assert!((blended[2] as i32 - 100).abs() <= 2);

    // Fully transparent FG over solid BG
    let transparent_fg = [255u8, 255, 255, 0];
    let b1 = alpha_over_pixel(bg, transparent_fg);
    assert_eq!(b1, bg);

    // Fully opaque FG over BG
    let opaque_fg = [50u8, 150, 250, 255];
    let b2 = alpha_over_pixel(bg, opaque_fg);
    assert_eq!(b2, opaque_fg);
}

#[test]
fn test_drop_shadow_in_alpha_only() {
    let w = 50usize;
    let h = 50usize;

    // Create a 50x50 white background
    let mut bg = vec![255u8; w * h * 4];

    // Create a character with a 10x10 square in center [20..30, 20..30]
    let mut char_data = vec![0u8; w * h * 4];
    for y in 20..30 {
        for x in 20..30 {
            let idx = (y * w + x) * 4;
            char_data[idx] = 200;
            char_data[idx + 1] = 50;
            char_data[idx + 2] = 50;
            char_data[idx + 3] = 255;
        }
    }
    let char_img = RawImage {
        width: w,
        height: h,
        data: char_data,
    };

    let ops = vec![
        CompositionOp {
            id: "drop_shadow".to_string(),
            name: "Drop Shadow".to_string(),
            op_type: "drop_shadow".to_string(),
            blend_mode: BlendMode::Multiply,
            opacity: 0.60,
            mask_by_alpha: false,
            enabled: true,
            params: serde_json::json!({
                "offsetX": 5.0,
                "offsetY": 5.0,
                "blurRadius": 4.0
            }),
        }
    ];

    composite_frame_with_ops(&mut bg, &char_img, &ops, w, h);

    // 1. Unaffected corner (x=2, y=2) should still be pure white (255, 255, 255)
    let idx_corner = (2 * w + 2) * 4;
    assert_eq!(bg[idx_corner], 255);
    assert_eq!(bg[idx_corner + 1], 255);
    assert_eq!(bg[idx_corner + 2], 255);

    // 2. Center of character (x=25, y=25) should have character color
    let idx_center = (25 * w + 25) * 4;
    assert_eq!(bg[idx_center], 200);
    assert_eq!(bg[idx_center + 1], 50);
    assert_eq!(bg[idx_center + 2], 50);

    // 3. Shadow area at offset (x=33, y=33) should be darker than 255 (shadowed background)
    let idx_shadow = (33 * w + 33) * 4;
    assert!(bg[idx_shadow] < 255, "Shadow must darken background");
}

#[test]
fn test_light_wrap_mord_uniquement_sur_bords() {
    let w = 60usize;
    let h = 60usize;

    let mut bg = vec![100u8; w * h * 4];

    // Character: solid square in [15..45, 15..45]
    let mut char_data = vec![0u8; w * h * 4];
    for y in 15..45 {
        for x in 15..45 {
            let idx = (y * w + x) * 4;
            char_data[idx] = 50;
            char_data[idx + 1] = 50;
            char_data[idx + 2] = 50;
            char_data[idx + 3] = 255;
        }
    }
    let char_img = RawImage {
        width: w,
        height: h,
        data: char_data,
    };

    let alpha_channel: Vec<f32> = (0..(w * h)).map(|i| char_img.data[i * 4 + 3] as f32 / 255.0).collect();
    let edge_mask = extract_inner_edge_mask(&alpha_channel, w, h, 6.0);

    // Far outside (5, 5) -> edge mask must be exactly 0
    assert_eq!(edge_mask[5 * w + 5], 0.0);

    // Deep interior (30, 30) -> edge mask must be near zero
    assert!(edge_mask[30 * w + 30] < 0.05, "Interior must be near zero");

    // At edge boundary (16, 30) -> edge mask must be high
    assert!(edge_mask[16 * w + 30] > 0.40, "Inner edge must be high");

    let ops = vec![
        CompositionOp {
            id: "light_wrap".to_string(),
            name: "Light Wrap".to_string(),
            op_type: "light_wrap".to_string(),
            blend_mode: BlendMode::Screen,
            opacity: 0.8,
            mask_by_alpha: true,
            enabled: true,
            params: serde_json::json!({
                "blurRadius": 10.0,
                "edgeWidth": 6.0
            }),
        }
    ];

    composite_frame_with_ops(&mut bg, &char_img, &ops, w, h);

    // Interior (30, 30) unaffected by light wrap -> retains character base color [50, 50, 50]
    let idx_inner = (30 * w + 30) * 4;
    assert!((bg[idx_inner] as i32 - 50).abs() <= 1);

    // Edge (16, 30) affected by light wrap -> brighter due to background bleed
    let idx_edge = (16 * w + 30) * 4;
    assert!(bg[idx_edge] > 50, "Edge must receive light wrap bleed");
}

#[test]
fn test_mask_by_alpha_isoles_op() {
    let w = 40usize;
    let h = 40usize;

    // Background is green [0, 200, 0, 255]
    let mut bg = vec![0u8; w * h * 4];
    for i in 0..(w * h) {
        bg[i * 4 + 1] = 200;
        bg[i * 4 + 3] = 255;
    }

    // Character in right half only [20..40, 0..40]
    let mut char_data = vec![0u8; w * h * 4];
    for y in 0..40 {
        for x in 20..40 {
            let idx = (y * w + x) * 4;
            char_data[idx] = 180;
            char_data[idx + 1] = 180;
            char_data[idx + 2] = 180;
            char_data[idx + 3] = 255;
        }
    }
    let char_img = RawImage {
        width: w,
        height: h,
        data: char_data,
    };

    let default_ops = get_default_composition_ops();
    composite_frame_with_ops(&mut bg, &char_img, &default_ops, w, h);

    // Left half (x=5, y=20) where alpha=0:
    // Drop shadow might reach nearby pixels, but far pixel (x=2, y=2) has alpha=0 and no ops modifying it
    let idx_bg_left = (2 * w + 2) * 4;
    assert_eq!(bg[idx_bg_left], 0);
    assert_eq!(bg[idx_bg_left + 1], 200); // Green channel preserved
    assert_eq!(bg[idx_bg_left + 2], 0);
}

#[test]
fn test_png_without_alpha_returns_clean_error() {
    let temp_solid = std::env::temp_dir().join("solid_no_alpha.png");
    let solid_raw = RawImage {
        width: 50,
        height: 50,
        data: vec![255u8; 50 * 50 * 4], // all A=255
    };
    save_image_rgba(&solid_raw, &temp_solid, None).unwrap();

    let res = validate_and_load_character_png(&temp_solid, None);
    assert!(res.is_err());
    let err = res.err().unwrap();
    assert_eq!(err, "PNG sans canal alpha — détourage requis");

    // Also check spider-man fixture if it has baked solid background
    let spiderman_p = std::path::Path::new(r"C:\Users\cia\Downloads\spider-man-11530958085nzzlmiz6hg-732305370.png");
    if spiderman_p.exists() {
        let res_sp = validate_and_load_character_png(spiderman_p, None);
        assert!(res_sp.is_err());
        assert_eq!(res_sp.err().unwrap(), "PNG sans canal alpha — détourage requis");
    }
}

#[test]
fn test_render_composition_image_and_video_pipeline() {
    let bg_vid_path = r"C:\Users\cia\Downloads\jugg video & audio tester\snaptik_7674387013243538721_v3.mp4";

    // Create a real transparent character fixture PNG (200x200 with transparent background and red character in center)
    let temp_char = std::env::temp_dir().join("test_char_alpha.png");
    let mut char_data = vec![0u8; 200 * 200 * 4];
    for y in 50..150 {
        for x in 50..150 {
            let idx = (y * 200 + x) * 4;
            char_data[idx] = 220;
            char_data[idx + 1] = 40;
            char_data[idx + 2] = 40;
            char_data[idx + 3] = 255;
        }
    }
    let char_raw = RawImage {
        width: 200,
        height: 200,
        data: char_data,
    };
    save_image_rgba(&char_raw, &temp_char, None).unwrap();

    // 1. Test Static Image Background Composite
    let temp_bg_img = std::env::temp_dir().join("test_bg_solid.png");
    let bg_solid = RawImage {
        width: 200,
        height: 200,
        data: vec![100u8; 200 * 200 * 4],
    };
    save_image_rgba(&bg_solid, &temp_bg_img, None).unwrap();

    let out_dir = std::env::temp_dir().join("cia_t27_test_render_comp");
    let _ = std::fs::remove_dir_all(&out_dir);

    let res_img = render_composition_internal(
        None,
        temp_char.to_str().unwrap(),
        temp_bg_img.to_str().unwrap(),
        None,
        Some(out_dir.to_str().unwrap()),
    ).expect("Image composition render must succeed");

    assert!(std::path::Path::new(&res_img).exists());
    assert!(res_img.ends_with(".png"));

    // 2. Test Video Background Composite if fixture exists
    if std::path::Path::new(bg_vid_path).exists() {
        let res_vid = render_composition_internal(
            None,
            temp_char.to_str().unwrap(),
            bg_vid_path,
            None,
            Some(out_dir.to_str().unwrap()),
        ).expect("Video composition render must succeed");

        assert!(std::path::Path::new(&res_vid).exists());
        assert!(res_vid.ends_with(".mp4"));
        println!("Rendered composition video: {}", res_vid);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// T28 MESH DEFORMATION & PROCEDURAL ANIMATION TESTS
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_mesh_identity_zero_deformation() {
    let w = 120;
    let h = 140;
    let mut data = vec![0u8; w * h * 4];

    // Draw some distinct colored pattern with alpha
    for y in 20..120 {
        for x in 20..100 {
            let idx = (y * w + x) * 4;
            data[idx] = ((x * 2) % 256) as u8;
            data[idx + 1] = ((y * 2) % 256) as u8;
            data[idx + 2] = 180;
            data[idx + 3] = 255;
        }
    }

    let src_img = RawImage { width: w, height: h, data: data.clone() };
    let mesh = build_layer_mesh("body", &src_img);

    // Un-deformed vertices: orig_x, orig_y
    let undeformed_verts: Vec<(f32, f32)> = mesh.vertices.iter().map(|v| (v.orig_x, v.orig_y)).collect();
    let rendered = render_deformed_mesh(&src_img, &mesh, &undeformed_verts);

    assert_eq!(rendered.width, w);
    assert_eq!(rendered.height, h);

    // Verify perfect identity reconstruction inside the bounding box
    let mut max_diff = 0i32;
    for y in 25..115 {
        for x in 25..95 {
            let idx = (y * w + x) * 4;
            for c in 0..4 {
                let diff = (src_img.data[idx + c] as i32 - rendered.data[idx + c] as i32).abs();
                if diff > max_diff {
                    max_diff = diff;
                }
            }
        }
    }

    assert_eq!(max_diff, 0, "Identity reconstruction must have delta 0 vs original PNG");
}

#[test]
fn test_mesh_coverage_and_no_nan() {
    let w = 200;
    let h = 200;
    let mut data = vec![0u8; w * h * 4];
    for y in 30..170 {
        for x in 30..170 {
            let idx = (y * w + x) * 4;
            data[idx] = 200;
            data[idx + 3] = 255;
        }
    }
    let img = RawImage { width: w, height: h, data };

    let layer_names = ["hair_front", "hair_back", "eyes", "body", "clothes_upper", "accessories", "face"];
    let config = AnimationConfig::default();
    let beats = vec![0.5, 1.0, 1.5, 2.0];
    let downbeats = vec![0.5, 2.0];

    for name in &layer_names {
        let mesh = build_layer_mesh(name, &img);
        assert!(!mesh.vertices.is_empty(), "Mesh must have vertices");
        assert!(!mesh.triangles.is_empty(), "Mesh must have triangles");

        if name.contains("hair") {
            assert_eq!(mesh.grid_w, 12);
            assert_eq!(mesh.grid_h, 14);
        } else {
            assert_eq!(mesh.grid_w, 10);
            assert_eq!(mesh.grid_h, 10);
        }

        // Test multiple timestamps
        let camera = CameraState::default();
        for &t in &[0.0, 0.25, 0.5, 1.0, 2.1, 3.2, 5.0] {
            let deformed = compute_deformed_vertices(&mesh, t, (t * 30.0) as u32, 30.0, &beats, &downbeats, &config, 1, 0.5, &camera, w, h);
            assert_eq!(deformed.len(), mesh.vertices.len());

            for &(x, y) in &deformed {
                assert!(!x.is_nan(), "Vertex X cannot be NaN at t={t} for layer {name}");
                assert!(!y.is_nan(), "Vertex Y cannot be NaN at t={t} for layer {name}");
                assert!(!x.is_infinite(), "Vertex X cannot be Infinite at t={t}");
                assert!(!y.is_infinite(), "Vertex Y cannot be Infinite at t={t}");
            }

            for tri in &mesh.triangles {
                let p0 = deformed[tri.v_indices[0]];
                let p1 = deformed[tri.v_indices[1]];
                let p2 = deformed[tri.v_indices[2]];
                let det = (p1.0 - p0.0) * (p2.1 - p0.1) - (p2.0 - p0.0) * (p1.1 - p0.1);
                assert!(!det.is_nan(), "Triangle determinant cannot be NaN");
            }
        }
    }
}

#[test]
fn test_eyes_blink_controller() {
    let w = 100;
    let h = 100;
    let mut data = vec![0u8; w * h * 4];
    for y in 40..60 {
        for x in 30..70 {
            let idx = (y * w + x) * 4;
            data[idx] = 30;
            data[idx + 1] = 30;
            data[idx + 2] = 30;
            data[idx + 3] = 255;
        }
    }
    let img = RawImage { width: w, height: h, data };
    let mesh = build_layer_mesh("eyes", &img);

    let mut config = AnimationConfig::default();
    config.entrance_enabled = false;
    config.blink_interval_sec = 3.0;

    let fps = 30.0;
    let beats = vec![];
    let downbeats = vec![];
    let camera = CameraState::default();

    // Between blinks (e.g. t = 1.5s): scaleY is normal ~1.0
    let verts_open = compute_deformed_vertices(&mesh, 1.5, 45, fps, &beats, &downbeats, &config, 6, 0.7, &camera, w, h);
    let top_y_open = verts_open.iter().map(|v| v.1).fold(f32::INFINITY, f32::min);
    let bot_y_open = verts_open.iter().map(|v| v.1).fold(f32::NEG_INFINITY, f32::max);
    let height_open = bot_y_open - top_y_open;

    // Peak of blink (at cycle start t = 3.05s, frame 1.5 into 3-frame blink):
    // cycle_t = 0.05s (halfway through 3/30 = 0.10s blink duration)
    let verts_closed = compute_deformed_vertices(&mesh, 3.05, 91, fps, &beats, &downbeats, &config, 6, 0.7, &camera, w, h);
    let top_y_closed = verts_closed.iter().map(|v| v.1).fold(f32::INFINITY, f32::min);
    let bot_y_closed = verts_closed.iter().map(|v| v.1).fold(f32::NEG_INFINITY, f32::max);
    let height_closed = bot_y_closed - top_y_closed;

    assert!(height_closed < height_open * 0.20, "Eyes height at peak blink must collapse towards 0 (open: {:.1}, closed: {:.1})", height_open, height_closed);
}

#[test]
fn test_camera_identity() {
    let w = 120;
    let h = 140;
    let mut data = vec![0u8; w * h * 4];

    for y in 20..120 {
        for x in 20..100 {
            let idx = (y * w + x) * 4;
            data[idx] = ((x * 3) % 256) as u8;
            data[idx + 1] = ((y * 3) % 256) as u8;
            data[idx + 2] = 210;
            data[idx + 3] = 255;
        }
    }

    let src_img = RawImage { width: w, height: h, data: data.clone() };
    let mesh = build_layer_mesh("body", &src_img);

    let mut undeformed_verts = Vec::new();
    for v in &mesh.vertices {
        undeformed_verts.push((v.orig_x, v.orig_y));
    }

    let rendered = render_deformed_mesh(&src_img, &mesh, &undeformed_verts);
    assert_eq!(rendered.width, w);
    assert_eq!(rendered.height, h);

    let mut max_diff = 0i32;
    for y in 25..115 {
        for x in 25..95 {
            let idx = (y * w + x) * 4;
            for c in 0..4 {
                let diff = (src_img.data[idx + c] as i32 - rendered.data[idx + c] as i32).abs();
                if diff > max_diff {
                    max_diff = diff;
                }
            }
        }
    }

    assert_eq!(max_diff, 0, "Camera at identity (0,0,0) and zoom 1.0 must produce zero delta vs original PNG");
}

#[test]
fn test_parallax_separation() {
    let w = 1920usize;
    let h = 1080usize;
    let img = RawImage { width: w, height: h, data: vec![255u8; w * h * 4] };
    let mesh = build_layer_mesh("body", &img);

    let camera = CameraState { pan_x: 0.1, pan_y: 0.0, zoom: 1.0, roll: 0.0 };
    let mut config = AnimationConfig::default();
    config.entrance_enabled = false;
    config.parallax_strength = 1.0;

    let beats = vec![];
    let downbeats = vec![];

    // Layer 0: z=0.0
    let verts_z0 = compute_deformed_vertices(
        &mesh, 0.0, 0, 30.0, &beats, &downbeats, &config, 0, 0.0, &camera, w, h
    );

    // Layer 1: z=1.0
    let verts_z1 = compute_deformed_vertices(
        &mesh, 0.0, 0, 30.0, &beats, &downbeats, &config, 1, 1.0, &camera, w, h
    );

    // For any vertex, difference in X coordinate between z=1.0 and z=0.0:
    let diff_x = verts_z1[0].0 - verts_z0[0].0;
    let expected_separation = 0.1 * (w as f32); // 192.0

    assert!((diff_x - expected_separation).abs() < 1e-3, "Parallax separation (actual: {:.3}, expected: {:.3}) must be exactly 0.1 * viewport_width", diff_x, expected_separation);
}

#[test]
fn test_beat_punch_decay() {
    let mut config = AnimationConfig::default();
    config.beat_punch_intensity = 0.6;

    let downbeats = vec![1.0]; // Downbeat at 1.0s
    let beats = vec![1.0];

    // Spike at downbeat (t = 1.0s)
    let cam_spike = compute_camera_state(1.0, &beats, &downbeats, &config);
    assert!(cam_spike.zoom > 1.025, "Camera zoom must punch on downbeat (got {:.4})", cam_spike.zoom);

    // At 200 ms after downbeat (t = 1.200s), zoom must return within ±0.001 of 1.0
    let cam_decayed = compute_camera_state(1.200, &beats, &downbeats, &config);
    let diff = (cam_decayed.zoom - 1.0).abs();
    assert!(diff <= 0.001, "Camera zoom must return to ±0.001 of 1.0 within 200 ms (at 200ms diff is {:.6}, zoom is {:.6})", diff, cam_decayed.zoom);
}

#[test]
fn test_mesh_render_benchmark_1080p() {
    let w = 1920;
    let h = 1080;

    // Create 9 mock semantic layers at 1080p
    let layer_names = [
        "hair_back", "body", "clothes_lower", "clothes_upper",
        "face", "mouth", "eyes", "hair_front", "accessories",
    ];

    let mut layers = Vec::new();
    let total = layer_names.len();
    for (i, name) in layer_names.iter().enumerate() {
        let mut data = vec![0u8; w * h * 4];
        let y_min = 100 + i * 80;
        let y_max = (y_min + 300).min(h - 50);
        let x_min = 400 + i * 50;
        let x_max = (x_min + 600).min(w - 50);

        for y in y_min..y_max {
            for x in x_min..x_max {
                let idx = (y * w + x) * 4;
                data[idx] = (50 + i * 20) as u8;
                data[idx + 1] = (80 + i * 15) as u8;
                data[idx + 2] = (120 + i * 10) as u8;
                data[idx + 3] = 255;
            }
        }

        let raw = RawImage { width: w, height: h, data };
        let mesh = build_layer_mesh(name, &raw);
        let z_depth = i as f32 / (total - 1) as f32;
        layers.push((name.to_string(), raw, mesh, i, z_depth));
    }

    let config = AnimationConfig::default();
    let beats = vec![0.5, 1.0, 1.5, 2.0, 2.5];
    let downbeats = vec![0.5, 2.0];
    let fps = 30.0;

    let camera = compute_camera_state(0.0, &beats, &downbeats, &config);

    // Warm up
    let _ = render_animated_character_frame(&layers, 0.0, 0, fps, &beats, &downbeats, &config, &camera, w, h);

    // Measure 15 frames
    let num_frames = 15;
    let start_time = std::time::Instant::now();

    for f in 0..num_frames {
        let t = (f as f64) / fps;
        let cam = compute_camera_state(t, &beats, &downbeats, &config);
        let frame = render_animated_character_frame(&layers, t, f, fps, &beats, &downbeats, &config, &cam, w, h);
        assert_eq!(frame.width, w);
        assert_eq!(frame.height, h);
    }

    let elapsed = start_time.elapsed();
    let ms_per_frame = (elapsed.as_secs_f64() * 1000.0) / (num_frames as f64);

    println!("\n[BENCHMARK 1080p MESH + CAMERA PARALLAX]");
    println!("  Resolution: {}x{}", w, h);
    println!("  Layers Count: {}", layers.len());
    println!("  Total Time for {} frames: {:.2} ms", num_frames, elapsed.as_secs_f64() * 1000.0);
    println!("  Performance: {:.2} ms / frame (ceiling < 25 ms/frame)", ms_per_frame);

    if ms_per_frame > 25.0 {
        panic!("BENCHMARK FAILED: {:.2} ms/frame exceeds 25 ms/frame limit! STOP + plan optimization", ms_per_frame);
    }

    assert!(ms_per_frame < 25.0, "Performance must be under 25 ms/frame");
}

#[test]
fn test_light_wrap_edge_bleed() {
    let w = 200usize;
    let h = 200usize;
    let bg_buf = vec![255u8; w * h * 4]; // Pure white background

    // Black circle with soft edge gradient from radius 35 to 55
    let mut char_alpha = vec![0.0f32; w * h];
    let mut composite_buf = bg_buf.clone();

    let cx = 100.0f32;
    let cy = 100.0f32;

    for y in 0..h {
        for x in 0..w {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let dist = (dx * dx + dy * dy).sqrt();

            let a = if dist <= 35.0 {
                1.0f32
            } else if dist >= 55.0 {
                0.0f32
            } else {
                ((55.0 - dist) / 20.0).clamp(0.0, 1.0)
            };

            char_alpha[y * w + x] = a;

            // Character is black [0, 0, 0] over white background [255, 255, 255]
            let px_val = ((1.0 - a) * 255.0).round() as u8;
            let idx = (y * w + x) * 4;
            composite_buf[idx] = px_val;
            composite_buf[idx + 1] = px_val;
            composite_buf[idx + 2] = px_val;
            composite_buf[idx + 3] = 255;
        }
    }

    // Inspect initial brightness of a ring of edge pixels at radius ~42
    let edge_idx = (100 * w + 142) * 4; // x=142, y=100 (radius 42)
    let initial_edge_val = composite_buf[edge_idx];

    // Apply Light Wrap Post-FX
    apply_light_wrap_post_fx(&mut composite_buf, &bg_buf, &char_alpha, w, h, 0.8);

    let final_edge_val = composite_buf[edge_idx];

    // Center pixel (x=100, y=100, radius 0)
    let center_idx = (100 * w + 100) * 4;
    let center_val = composite_buf[center_idx];

    // Center should remain pitch black (0)
    assert!(center_val <= 5, "Deep circle center should not be affected by light wrap, got {}", center_val);

    // Edge pixel brightness must strictly increase due to background light wrap bleeding
    assert!(
        final_edge_val > initial_edge_val,
        "Light wrap must increase brightness on circle edges (before: {}, after: {})",
        initial_edge_val,
        final_edge_val
    );
}

#[test]
fn test_chromatic_aberration_identity() {
    let w = 150usize;
    let h = 150usize;
    let mut data = vec![0u8; w * h * 4];

    for y in 0..h {
        for x in 0..w {
            let idx = (y * w + x) * 4;
            data[idx] = ((x * 7) % 256) as u8;
            data[idx + 1] = ((y * 11) % 256) as u8;
            data[idx + 2] = (((x + y) * 5) % 256) as u8;
            data[idx + 3] = 255;
        }
    }

    let original = data.clone();
    apply_chromatic_aberration_post_fx(&mut data, w, h, 0.0);

    let mut max_diff = 0i32;
    for i in 0..data.len() {
        let diff = (data[i] as i32 - original[i] as i32).abs();
        if diff > max_diff {
            max_diff = diff;
        }
    }

    assert_eq!(max_diff, 0, "Chromatic aberration at intensity 0.0 must be strictly identical (delta = 0)");
}

#[test]
fn test_post_fx_benchmark_1080p() {
    let w = 1920;
    let h = 1080;

    let layer_names = [
        "hair_back", "body", "clothes_lower", "clothes_upper",
        "face", "mouth", "eyes", "hair_front", "accessories",
    ];

    let mut layers = Vec::new();
    let total = layer_names.len();
    for (i, name) in layer_names.iter().enumerate() {
        let mut data = vec![0u8; w * h * 4];
        let y_min = 100 + i * 80;
        let y_max = (y_min + 300).min(h - 50);
        let x_min = 400 + i * 50;
        let x_max = (x_min + 600).min(w - 50);

        for y in y_min..y_max {
            for x in x_min..x_max {
                let idx = (y * w + x) * 4;
                data[idx] = (50 + i * 20) as u8;
                data[idx + 1] = (80 + i * 15) as u8;
                data[idx + 2] = (120 + i * 10) as u8;
                data[idx + 3] = 255;
            }
        }

        let raw = RawImage { width: w, height: h, data };
        let mesh = build_layer_mesh(name, &raw);
        let z_depth = i as f32 / (total - 1) as f32;
        layers.push((name.to_string(), raw, mesh, i, z_depth));
    }

    let mut config = AnimationConfig::default();
    config.parallax_strength = 0.5;
    config.beat_punch_intensity = 0.6;
    config.light_wrap_intensity = 0.5;
    config.chromatic_aberration = 0.5;
    config.impact_blur_strength = 0.5;

    let beats = vec![0.5, 1.0, 1.5, 2.0, 2.5];
    let downbeats = vec![0.5, 2.0];
    let fps = 30.0;

    let ops = get_default_composition_ops();
    let bg_frame = RawImage { width: w, height: h, data: vec![30u8; w * h * 4] };

    // Warm up
    let camera = compute_camera_state(0.0, &beats, &downbeats, &config);
    let char_frame = render_animated_character_frame(&layers, 0.0, 0, fps, &beats, &downbeats, &config, &camera, w, h);
    let precomputed = precompute_composition_masks(&char_frame, &ops, w, h);
    let mut frame_buf = bg_frame.data.clone();
    composite_frame_fast(&mut frame_buf, &char_frame, &ops, &precomputed, w, h);
    apply_light_wrap_post_fx(&mut frame_buf, &bg_frame.data, &precomputed.alpha_channel, w, h, config.light_wrap_intensity);
    apply_impact_motion_blur_post_fx(&mut frame_buf, w, h, config.impact_blur_strength);
    apply_chromatic_aberration_post_fx(&mut frame_buf, w, h, config.chromatic_aberration);

    // Measure 15 frames with full stack: 9 layers + mesh deformation + 2.5D camera + composite ops + Post-FX
    let num_frames = 15;
    let mut dur_char = 0.0;
    let mut dur_precompute = 0.0;
    let mut dur_comp = 0.0;
    let mut dur_lw = 0.0;
    let mut dur_blur = 0.0;
    let mut dur_chroma = 0.0;

    let start_time = std::time::Instant::now();

    for f in 0..num_frames {
        let t = (f as f64) / fps;
        let cam = compute_camera_state(t, &beats, &downbeats, &config);

        let t0 = std::time::Instant::now();
        let char_f = render_animated_character_frame(&layers, t, f, fps, &beats, &downbeats, &config, &cam, w, h);
        dur_char += t0.elapsed().as_secs_f64() * 1000.0;

        let t1 = std::time::Instant::now();
        let pre_masks = precompute_composition_masks(&char_f, &ops, w, h);
        dur_precompute += t1.elapsed().as_secs_f64() * 1000.0;

        let t2 = std::time::Instant::now();
        let mut buf = bg_frame.data.clone();
        composite_frame_fast(&mut buf, &char_f, &ops, &pre_masks, w, h);
        dur_comp += t2.elapsed().as_secs_f64() * 1000.0;

        // 1. Light Wrap
        let t3 = std::time::Instant::now();
        apply_light_wrap_post_fx(&mut buf, &bg_frame.data, &pre_masks.alpha_channel, w, h, config.light_wrap_intensity);
        dur_lw += t3.elapsed().as_secs_f64() * 1000.0;

        // 2. Impact Downbeat Pulse
        let mut chromatic_spike = 0.0f32;
        let mut impact_blur_spike = 0.0f32;
        for &db in &downbeats {
            if t >= db {
                let dt = (t - db) as f32;
                if dt < 0.10 {
                    let decay = (-35.0 * dt).exp();
                    chromatic_spike += 0.35 * decay;
                    impact_blur_spike += decay;
                }
            }
        }

        // 3. Impact Motion Blur
        let t4 = std::time::Instant::now();
        let total_blur = config.impact_blur_strength * impact_blur_spike;
        if total_blur > 0.001 {
            apply_impact_motion_blur_post_fx(&mut buf, w, h, total_blur);
        }
        dur_blur += t4.elapsed().as_secs_f64() * 1000.0;

        // 4. Chromatic Aberration
        let t5 = std::time::Instant::now();
        let total_chroma = (config.chromatic_aberration + chromatic_spike).clamp(0.0, 1.0);
        if total_chroma > 0.001 {
            apply_chromatic_aberration_post_fx(&mut buf, w, h, total_chroma);
        }
        dur_chroma += t5.elapsed().as_secs_f64() * 1000.0;
    }

    let elapsed = start_time.elapsed();
    let ms_per_frame = (elapsed.as_secs_f64() * 1000.0) / (num_frames as f64);

    println!("\n[BENCHMARK 1080p FULL POST-FX STACK BREAKDOWN]");
    println!("  Char frame render:       {:.2} ms/frame", dur_char / (num_frames as f64));
    println!("  Precompute masks (ops):  {:.2} ms/frame", dur_precompute / (num_frames as f64));
    println!("  Composite frame fast:    {:.2} ms/frame", dur_comp / (num_frames as f64));
    println!("  Light Wrap Post-FX:      {:.2} ms/frame", dur_lw / (num_frames as f64));
    println!("  Impact Blur Post-FX:     {:.2} ms/frame", dur_blur / (num_frames as f64));
    println!("  Chromatic Aberration FX: {:.2} ms/frame", dur_chroma / (num_frames as f64));
    println!("  --------------------------------------------");
    println!("  Total Frame Time:        {:.2} ms / frame", ms_per_frame);

    if ms_per_frame > 50.0 {
        panic!("BENCHMARK FAILED: {:.2} ms/frame exceeds 50 ms/frame hard limit! STOP + plan optimization", ms_per_frame);
    }

    assert!(ms_per_frame < 35.0, "Performance must be under 35 ms/frame");
}



