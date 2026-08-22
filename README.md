# cia app

cia app is a fast, local Windows desktop application for creating beat-synced jugg edits directly on your machine.

Your media stays entirely local. cia app performs media analysis, onset/beat tracking, geometric remapping, transform stacking, and encoding locally without cloud dependencies.

![cia app preview](docs/preview.png)

## Features

- **3-Source Input**: Drag-and-drop or select **Scene Video**, **Drums Audio**, and **Target Audio** with automated media probing.
- **Beat & Downbeat Detection**: High-precision musical alignment powered by local onset detection and tempo tracking.
- **Remap Styles**:
  - **HARD**: Fast snaps, dynamic reverse cuts on downbeats, one-framers, and aggressive shakes.
  - **SMOOTH**: Gentle continuous curves, soft zoom transitions, and zero reverse remaps.
  - **HYBRID**: Balanced alternating snap and saddle curves with medium shake intensity.
- **DETAILS Modal with Live Preview & Custom Parameters**:
  - Hover over any effect to preview the algorithm's visual output on a generated 256x256 test pattern.
  - Toggle individual effects on or off.
  - Fine-tune 28 custom parameters across **SHAKES** (harmonic amplitude/decay, bouncy, dissolve, skew, squish, optics, stretch), **ZOOM**, **AMBIANCE** (flicker, exposure flash, echo/trail, RGB tint, vignette, scanlines), and **TRANSITIONS** (warp bubble, wave warp, slide shake).
  - One-click **RESET TO STYLE DEFAULTS**.
- **FULL FX Toggle**: Instantly toggle between full visual effects and lightweight motion-only mode.
- **Flexible Export Options**:
  - Video Codec: **H.264**, **H.265 (HEVC)**, **VP9**.
  - Target Bitrate: 5 to 50 Mbps via interactive slider.
  - Container Format: **MP4**, **MKV**, **WEBM**.
- **Render Analytics**: Post-render statistics reporting total render time, file size, target FPS, and total effect count.

## Getting started

Download the latest Windows installer from the [releases page](https://github.com/ssdme/cia-app-jugg/releases). Launching the application provides immediate access to beat analysis and rendering — no cloud login or setup wizard required.

See [Install and usage](docs/INSTALL.md) for details.

## Build from source

Prerequisites: current Node.js, Rust stable, and Windows build tools.

```powershell
npm ci
npm run tauri dev
```

Create an NSIS installer:

```powershell
npm run tauri build
```

## Licence and notices

cia app source code is MIT licensed. Third-party software keeps its own licence; see [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
