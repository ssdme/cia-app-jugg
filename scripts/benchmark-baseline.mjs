// Extract the audited engine into an isolated harness; never checkout or modify the baseline.
// Only Tauri paths/events are adapted. Pixel kernels and the render loop remain those of 958fefd.
import { execFileSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync, copyFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve, join } from 'node:path';

const root = resolve('.');
const directory = resolve('tests/generated/benchmark');
const temporary = mkdtempSync(join(tmpdir(), 'cia-jugg-benchmark-baseline-'));
const original = path => execFileSync('git', ['show', `958fefd:${path}`], { encoding: 'utf8', windowsHide: true });
const render = original('src-tauri/src/render.rs');
let prefix = render.split('#[tauri::command]\npub fn cancel_render')[0]
  .replace('use tauri::{Emitter, Manager};', '')
  .replace('compute_crop_to_fill, get_ffmpeg_binary, probe_media', 'compute_crop_to_fill');
let core = render.substring(render.indexOf('#[tauri::command]\npub async fn run_render_pipeline'))
  .replace('#[tauri::command]\npub async fn run_render_pipeline', 'pub fn run_render_pipeline')
  .replace('app: tauri::AppHandle', 'app: BenchmarkApp')
  .replace('get_ffmpeg_binary(&app)?', 'app.ffmpeg.clone()')
  .replace('probe_media(app.clone(), primary_path.clone())?', 'crate::probe::probe_media_internal(primary_path, Some(&app.ffprobe))?');
// Add counters without changing the original algorithm or its allocations.
prefix = prefix.replace('pub effects_count: usize,', 'pub effects_count: usize,\n    pub decoded_bytes: u64,');
prefix = prefix.replace('        effects_count,', '        effects_count,\n        decoded_bytes: 0,');
core = core.replace('let stats = compute_render_stats(', 'let mut stats = compute_render_stats(')
  .replace('    Ok(stats)', '    stats.decoded_bytes = total_cached_bytes;\n    Ok(stats)');
const originalEffects = original('src-tauri/src/effects.rs');
// Alias DTOs to the compiled library so its plans can enter the original kernels.
writeFileSync(join(temporary, 'effects.rs'), 'pub use cia_render_lib::effects::*;\nuse crate::probe::{mirror_coordinate, sample_pixel_mirrored};\n'
  + originalEffects.substring(originalEffects.indexOf('pub fn evaluate_curve(')));
writeFileSync(join(temporary, 'baseline.rs'), prefix + `
pub struct BenchmarkApp { pub ffmpeg: std::path::PathBuf, pub ffprobe: std::path::PathBuf, pub directory: std::path::PathBuf }
impl BenchmarkApp {
    fn path(&self) -> &Self { self }
    fn app_cache_dir(&self) -> Result<std::path::PathBuf, String> { Ok(self.directory.join("cache")) }
    fn app_data_dir(&self) -> Result<std::path::PathBuf, String> { Ok(self.directory.clone()) }
    fn emit(&self, _: &str, _: RenderProgressPayload) -> Result<(), String> { Ok(()) }
}
` + core);
mkdirSync(join(temporary, 'src'));
writeFileSync(join(temporary, 'Cargo.toml'), `[package]
name = "cia-jugg-benchmark-baseline"
version = "0.0.0"
edition = "2021"
[dependencies]
cia-render = {path = ${JSON.stringify(resolve('src-tauri'))}}
serde = {version = "1.0", features = ["derive"]}
serde_json = "1.0"
`);
copyFileSync(resolve('src-tauri/Cargo.lock'), join(temporary, 'Cargo.lock'));
writeFileSync(join(temporary, 'src/main.rs'), `
#![allow(dead_code, unused_imports)]
pub use cia_render_lib::{plan, probe};
#[path=${JSON.stringify(join(temporary, 'effects.rs'))}] mod effects;
#[path=${JSON.stringify(join(temporary, 'baseline.rs'))}] mod baseline;
#[path=${JSON.stringify(resolve('src-tauri/examples/shared/mod.rs'))}] mod shared;
fn main() -> Result<(), String> {
    let root = std::path::PathBuf::from(${JSON.stringify(root)});
    let directory = std::path::PathBuf::from(${JSON.stringify(directory)});
    let mut measurements = Vec::new();
    for (scenario, source) in [("1080p", "1080p.mp4"), ("long-source-short-excerpt", "long.mp4"), ("4k-reverse", "4k.mp4")] {
        let plan_json = std::fs::read_to_string(directory.join(format!("{scenario}-plan.json"))).map_err(|e| e.to_string())?;
        for repetition in 0..3 {
            let app = baseline::BenchmarkApp {
                ffmpeg: root.join("src-tauri/binaries/ffmpeg.exe"), ffprobe: root.join("src-tauri/binaries/ffprobe.exe"),
                directory: directory.join("baseline"),
            };
            let (stats, peak_ram) = shared::measure(|| baseline::run_render_pipeline(app, plan_json.clone(),
                directory.join(source).to_string_lossy().into_owned(), root.join("tests/generated/audio.wav").to_string_lossy().into_owned(), false, None))?;
            measurements.push(serde_json::json!({"scenario":scenario,"repetition":repetition,"peakProcessRamBytes":peak_ram,"stats":stats}));
            std::fs::remove_file(&stats.output_path).map_err(|e| e.to_string())?;
            let cache = directory.join("baseline/cache").join(format!("frames_{}.raw", std::process::id()));
            if cache.exists() { std::fs::remove_file(cache).map_err(|e| e.to_string())?; }
        }
    }
    std::fs::write(directory.join("baseline-results.json"), serde_json::to_string_pretty(&measurements).unwrap()).map_err(|e| e.to_string())?;
    println!("Audited baseline engine measurements saved."); Ok(())
}
`);
console.log(`Isolated baseline benchmark harness: ${temporary}`);
execFileSync('cargo', ['run', '--offline', '--release', '--manifest-path', join(temporary, 'Cargo.toml'), '--target-dir', resolve('src-tauri/target')], { stdio: 'inherit', windowsHide: true });
