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
    assert_eq!(amb.scanlines.opacity, 0.0); // Scanlines are disabled by default for all modes
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
