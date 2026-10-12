use cia_render_lib::{
    plan::create_plan_internal,
    scene_analysis::{analyze_source, SceneSelection},
    scene_schedule::apply_schedule,
};
use std::{
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc},
    time::Instant,
};
mod shared;

fn main() -> Result<(), String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();
    let directory = root.join("tests/generated/benchmark");
    let ffmpeg = root.join("src-tauri/binaries/ffmpeg.exe");
    let ffprobe = root.join("src-tauri/binaries/ffprobe.exe");
    let mut results = Vec::new();
    for name in ["1080p.mp4", "long.mp4", "4k.mp4"] {
        let path = directory.join(name);
        if !path.is_file() {
            return Err(format!(
                "Missing {}; run npm run benchmark first",
                path.display()
            ));
        }
        for remove_blank in [true, false] {
            let start = Instant::now();
            let (analysis, peak_ram) = shared::measure(|| {
                analyze_source(
                    &ffmpeg,
                    &ffprobe,
                    &path,
                    remove_blank,
                    Arc::new(AtomicBool::new(false)),
                    &mut |_| {},
                )
            })?;
            let cold_ms = start.elapsed().as_secs_f64() * 1000.0;
            let start = Instant::now();
            let cached = analyze_source(
                &ffmpeg,
                &ffprobe,
                &path,
                remove_blank,
                Arc::new(AtomicBool::new(false)),
                &mut |_| {},
            )?;
            let cached_ms = start.elapsed().as_secs_f64() * 1000.0;
            assert!(Arc::ptr_eq(&analysis, &cached));
            for count in if name == "long.mp4" {
                vec![3, 24]
            } else {
                vec![3]
            } {
                let scenes: Vec<_> = (0..count)
                    .filter_map(|scene_id| {
                        analysis.prepare(&SceneSelection {
                            scene_id,
                            source_id: 0,
                            start: analysis.info.duration * scene_id as f64 / count as f64,
                            end: analysis.info.duration * (scene_id + 1) as f64 / count as f64,
                        })
                    })
                    .collect();
                let beats: Vec<_> = (1..40).map(|index| index as f64 / 2.0).collect();
                let downbeats: Vec<_> = beats.iter().step_by(4).copied().collect();
                let template = create_plan_internal(
                    "HARD", 24, &beats, &downbeats, 22.0, 20.0, 320, 180, 120.0, false, None, None,
                    None, None, None,
                )?;
                let start = Instant::now();
                let mut shots = 0;
                for _ in 0..5 {
                    let mut plan = template.clone();
                    apply_schedule(
                        &mut plan,
                        &scenes,
                        &beats,
                        &downbeats,
                        "fast",
                        true,
                        if remove_blank {
                            "scenepack"
                        } else {
                            "long_clip"
                        },
                    )?;
                    shots = plan.planning.unwrap().shots.len();
                }
                let planning_ms = start.elapsed().as_secs_f64() * 1000.0 / 5.0;
                let measurement = serde_json::json!({"source":name,"mode":if remove_blank {"scenepack"} else {"long_clip"},
                "width":analysis.info.width,"height":analysis.info.height,"durationSeconds":analysis.info.duration,
                "coldAnalysisMs":cold_ms,"cachedAnalysisMs":cached_ms,"meanPlanningMs":planning_ms,
                "peakProcessRamBytesExcludingFfmpeg":peak_ram,"decodedFrames":analysis.decoded_frames,"selectedScenes":count,"shots":shots});
                println!("{measurement}");
                results.push(measurement);
            }
        }
    }
    std::fs::write(
        directory.join("scene-analysis-results.json"),
        serde_json::to_string_pretty(&results).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())
}
