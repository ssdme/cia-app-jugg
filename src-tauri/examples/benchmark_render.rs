use cia_render_lib::{jobs::RenderJob, plan::*, render::*, sources::SourceRange};
use std::path::PathBuf;
mod shared;
fn main() -> Result<(), String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();
    let directory = root.join("tests/generated/benchmark");
    let environment = RenderEnvironment {
        ffmpeg: root.join("src-tauri/binaries/ffmpeg.exe"),
        ffprobe: root.join("src-tauri/binaries/ffprobe.exe"),
        cache_root: directory.join("cache"),
        output_dir: directory.join("output"),
    };
    let mut measurements = Vec::new();
    for (scenario, width, height, source, duration, reverse) in [
        ("1080p", 1920, 1080, "1080p.mp4", 2.0, false),
        (
            "long-source-short-excerpt",
            1920,
            1080,
            "long.mp4",
            12.0,
            false,
        ),
        ("4k-reverse", 3840, 2160, "4k.mp4", 2.0, true),
    ] {
        for repetition in 0..3 {
            let mut plan = create_plan_internal(
                "HARD",
                16,
                &[0.5],
                &[],
                duration,
                1.0,
                width,
                height,
                120.0,
                true,
                None,
                None,
                None,
                None,
                None,
            )?;
            plan.schema_version = 3;
            plan.video_duration = duration;
            plan.source_intervals = Some(vec![(0.0, duration)]);
            plan.source_ranges = Some(vec![SourceRange {
                source_id: 0,
                start: 0.0,
                duration,
            }]);
            if reverse {
                for segment in &mut plan.segments {
                    std::mem::swap(&mut segment.s0, &mut segment.s1);
                }
            }
            let path = directory.join(source).to_string_lossy().into_owned();
            std::fs::write(
                directory.join(format!("{scenario}-plan.json")),
                serde_json::to_string(&plan).unwrap(),
            )
            .map_err(|e| e.to_string())?;
            let job = RenderJob::begin(format!("benchmark-{scenario}-{repetition}"))?;
            let (stats, peak_process_ram) = shared::measure(|| {
                render_pipeline_core(
                    &environment,
                    &job,
                    &serde_json::to_string(&plan).map_err(|e| e.to_string())?,
                    &path,
                    &root.join("tests/generated/audio.wav").to_string_lossy(),
                    false,
                    None,
                    &mut |_| {},
                )
            })?;
            measurements.push(
                serde_json::json!({"scenario":scenario,"repetition":repetition,"peakProcessRamBytes":peak_process_ram,"stats":stats}),
            );
            std::fs::remove_file(&stats.output_path).map_err(|e| e.to_string())?;
        }
    }
    std::fs::write(
        directory.join("results.json"),
        serde_json::to_string_pretty(&measurements).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    println!(
        "Release benchmark saved to {}",
        directory.join("results.json").display()
    );
    Ok(())
}
