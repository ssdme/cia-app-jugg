# cia app

Desktop application for beat-synchronized video time remapping and kinetic transform compositing on Windows.

All processing runs locally on the host machine. The application bundles its own ONNX neural inference runtime and FFmpeg toolchain, requiring no cloud services, Python runtime, or external codec installations.

[![Release](https://img.shields.io/github/v/release/ssdme/cia-app-jugg?color=00f2fe&style=flat-square)](https://github.com/ssdme/cia-app-jugg/releases)
[![Platform](https://img.shields.io/badge/platform-Windows%20x64-18181b?style=flat-square&logo=windows)](https://github.com/ssdme/cia-app-jugg/releases)
[![License](https://img.shields.io/badge/license-MIT-3f3f46?style=flat-square)](LICENSE)
[![Build](https://img.shields.io/badge/build-standalone%20nsis-22c55e?style=flat-square)](https://github.com/ssdme/cia-app-jugg/releases)

---

## Render Previews

<table>
  <tr>
    <td align="center" width="50%">
      <video src="https://github.com/user-attachments/assets/960ae91f-d6aa-4c61-9bf7-89dd9c6582b9" controls width="100%"></video>
      <br>
      <sub><a href="https://github.com/ssdme/cia-app-jugg/releases/download/v1.0.2/preview1.mp4">Direct Link: preview1.mp4 (2 MB)</a></sub>
    </td>
    <td align="center" width="50%">
      <video src="https://github.com/user-attachments/assets/925d8312-2a50-4117-867f-065f97be912a" controls width="100%"></video>
      <br>
      <sub><a href="https://github.com/ssdme/cia-app-jugg/releases/download/v1.0.2/preview2.mp4">Direct Link: preview2.mp4 (15 MB)</a></sub>
    </td>
  </tr>
</table>

---

## UX

![cia jugg ux](docs/preview.png?v=1.0.5)

---

## Pipeline & Architecture

### Ingestion Pipeline
- **Scene**: Source footage (MP4, MKV, WEBM, MOV, AVI).
- **Drums**: Isolated percussive audio stem for neural onset and downbeat analysis.
- **Audio**: Target master soundtrack for final audio multiplexing and alignment.

### Neural Beat Detection
- Embedded `beat_this` ONNX model executed via ONNX Runtime C API.
- Beat and downbeat timestamps from the bundled ONNX model.
- Zero external dependencies: no Python runtime, PyTorch, or GPU driver packages required.

### Time Remapping Curves
- **HARD**: High-gradient velocity ramps, one-frame cuts, and beat-aligned reverse cuts.
- **SMOOTH**: Continuous cubic and bezier easing without reverse velocity.
- **HYBRID**: Alternating snap-and-saddle curve profile with dynamic acceleration.

### Scene Selection
- **1.0.4 fix**: Action windows are clamped to selected scene bounds, with numerical tolerance for timestamp rounding, preventing spurious `Invalid action window` render failures.
- **ScenePack**: `no repeatitive scene` adapts shot length to visual activity and beats. It favors unused passages and discourages early replays around a flexible eight-second target. Sparse footage can still loop without forced freeze padding.
- **Smart Scene Flow**: The default ScenePack policy uses shorter musical phrases, varied motion passages and controlled returns. It favors fresh source ranges and discourages consecutive holds without imposing an eight-second replay rule. Available in clip selection and settings; enabling it disables `no repeatitive scene`. Leave both unchecked for classic cuts.
- **Creative Mix**: Three independent options in `Details` add sparse musical accents: `TIME SLICE` splits moving footage into delayed panels, `INK PULSE` adds selective ink contours and halftone, and `PRISM DRIFT` separates color edges before resolving. All three are checked by default, work with any source mode and remain independent of Full FX. Combinations retain clean intervals and bounded intensity.
- **Creative Beta**: `FOCUS PULL` moves a sharp plane through soft focus, `RELIEF LIGHT` animates contour relief with raking light, and `PAPER GATE` opens a torn-paper aperture. These three options are independent and checked by default in `Details`. All six creative effects can be toggled separately; resetting the style enables them again.
- **Motion guidance**: ScenePack and Long Clip select useful motion windows and reduce remap emphasis on still frames. No Change uses the original path.
- **Source filtering**: ScenePack removes exactly pure black/white native frames, including internal gaps, while retaining near-blank detail. Analysis is cached for unchanged media.
- **Output actions**: `OPEN FILE` opens the completed video; `RENDER AGAIN` starts a new render directly.

### Transform & Shader Engine
- **Camera Shakes**: Harmonic damping, bounce, dissolve skew, and squish-pop coordinate transforms.
- **Ambiance Passes**: Highlight bloom, temporal echo-trail frame blending, and CC Deep Dark tone curve.
- **Anti-Flash Mode**: Disables luminance flashes and inversion while preserving geometric effects.

### Export Pipeline
- CPU encoding via embedded FFmpeg (libx264, libx265, libvpx-vp9).
- Target codecs: **H.264 (AVC)**, **H.265 (HEVC)**, and **VP9**.
- Configurable target bitrates from 5 Mbps to 50 Mbps.
- Borderless full-frame scaling to 16:9, 9:16, 3:4, square or a custom even-sized canvas. The default is **3:4 at 1080 x 1440**.

---

## Installation

Download the standalone installer from the [Releases](https://github.com/ssdme/cia-app-jugg/releases/latest) page:

- **[cia.jugg_1.0.5_x64-setup.exe](https://github.com/ssdme/cia-app-jugg/releases/download/v1.0.5/cia.jugg_1.0.5_x64-setup.exe)**

The installer is self-contained. It bundles all required binaries (`ffmpeg.exe`, `ffprobe.exe`, `beat_this.exe`, `onnxruntime.dll`, and ONNX model weights). No administrative privileges or system environment changes are required.

---

## Building from Source

### Prerequisites
- Node.js 22.12+ (Node 24 recommended)
- Rust stable (MSVC toolchain)
- Visual Studio C++ Build Tools

### Build Steps

```powershell
# Clone the repository
git clone https://github.com/ssdme/cia-app-jugg.git
cd cia-app-jugg

# Install frontend dependencies
npm ci

# Verify/download the pinned FFmpeg/FFprobe payload
npm run verify:payload

# Run in development mode
npm run tauri dev

# Compile an unsigned local test installer
npm run tauri build -- --config src-tauri/tauri.ci.conf.json
```

---

## Verification

```powershell
npm run check
npm test
npm run test:ui
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --locked --all-targets --manifest-path src-tauri/Cargo.toml -- -D warnings
cargo test --locked --manifest-path src-tauri/Cargo.toml
```

The integration harness generates synthetic media and requires the verified FFmpeg payload. Legacy personal-media benchmarks are explicitly ignored. Raw frames are decoded in bounded blocks (4 GiB disk cache, up to 256 MiB frame LRU); output dimensions are never reduced to satisfy a cache budget. Text Studio remains disabled.

`npm run benchmark` records three release measurements per scenario. `npm run benchmark:baseline` then runs the audited `958fefd` render loop and pixel kernels in an isolated harness with the same generated plans, media and FFmpeg. Results are written to `tests/generated/benchmark`; process RAM samples exclude FFmpeg child processes.

Schema 3 stores source-local `{ sourceId, start, duration }` ranges. Source IDs refer to the complete, immutable input-path list. Schemas 1 and 2 remain readable: their concatenated global intervals are split at source boundaries; missing or inconsistent source mappings are rejected. Scene arrays explicitly use `scenesFormat: 'pairs'` or `'boundaries'`; omitted format means boundaries for legacy callers. New callers should submit source ranges instead.

Schema 4 adds motion-guided remap envelopes, source fingerprints and shot/reuse diagnostics. Schemas 1–3 keep their existing sampling behavior. Run `cargo run --locked --manifest-path src-tauri/Cargo.toml --example benchmark_scene_analysis` after generating benchmark media to measure cold analysis, cache reuse and planning with 3/24 scenes. See [ScenePack validation](docs/SCENEPACK_UPDATE_REPORT_2026-10-05.md).

Release installers use the configured updater signing key. `build:update` verifies the installer signature against that public key before creating a manifest. CI creates unsigned test installers and never generates a replacement release key.

## License

MIT License (c) 2026 cia213. See [LICENSE](LICENSE) for terms. Bundled third-party binaries and models retain their respective licenses (see [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)).
