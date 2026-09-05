# cia app

> **Fast, local, beat-synchronized video time remap & kinetic edit studio for Windows.**

Your media stays entirely local. **cia app** performs media analysis, onset/beat tracking, geometric remapping, transform stacking, and hardware-accelerated encoding locally without cloud dependencies.

[![Release](https://img.shields.io/github/v/release/ssdme/cia-app-jugg?color=00f2fe&style=flat-square)](https://github.com/ssdme/cia-app-jugg/releases)
[![Platform](https://img.shields.io/badge/platform-Windows%20x64-18181b?style=flat-square&logo=windows)](https://github.com/ssdme/cia-app-jugg/releases)
[![License](https://img.shields.io/badge/license-MIT-3f3f46?style=flat-square)](LICENSE)
[![Zero Setup](https://img.shields.io/badge/setup-100%25%20standalone-22c55e?style=flat-square)](https://github.com/ssdme/cia-app-jugg/releases)

---

## Interface Preview

![cia app preview](docs/preview.png)

---

## Render Previews

Here is how the beat-synchronized time remap engine renders:

<div align="center">
  <table>
    <tr>
      <td align="center" width="50%">
        <b>⚡ Quick Teaser (5s — Fast Snaps &amp; One-Framers)</b><br><br>
        <video src="https://github.com/ssdme/cia-app-jugg/releases/download/v1.0.2/preview1.mp4" controls width="100%"></video>
        <br>
        <sub><a href="https://github.com/ssdme/cia-app-jugg/releases/download/v1.0.2/preview1.mp4">▶ Direct Link / Download preview1.mp4 (2 MB)</a></sub>
      </td>
      <td align="center" width="50%">
        <b>🔥 Full Showcase (28s — Complete Beat Sync &amp; Color Grading)</b><br><br>
        <video src="https://github.com/ssdme/cia-app-jugg/releases/download/v1.0.2/preview2.mp4" controls width="100%"></video>
        <br>
        <sub><a href="https://github.com/ssdme/cia-app-jugg/releases/download/v1.0.2/preview2.mp4">▶ Direct Link / Download preview2.mp4 (15 MB)</a></sub>
      </td>
    </tr>
  </table>
</div>

---

## Core Features

- **3-Source Input Pipeline** :
  - **SCENE** : Source video clip (MP4, MKV, WEBM, MOV, AVI).
  - **DRUMS** : Isolated drum stem or beat audio for onset extraction.
  - **AUDIO** : Target master music track for final sync and audio multiplexing.
- **Local Neural Onset & Beat Tracking** :
  - Embedded `beat_this` neural network ONNX engine for microsecond-accurate beat and downbeat tracking.
  - No external Python environment or network requests needed.
- **Dynamic Remap Curves** :
  - **HARD** : Aggressive snaps, dynamic reverse cuts on downbeats, one-framers, and high-energy shakes.
  - **SMOOTH** : Continuous flowing curves, gentle zoom transitions, zero reverse remaps.
  - **HYBRID** : Balanced alternating snap and saddle curves with medium shake intensity.
- **Pure Rust Matrix Effects Engine** :
  - **Camera Shakes** : Harmonic amplitude/decay, bouncy bounce, dissolve skew, and squish-pop.
  - **Ambiance Styling** : Highlight bloom flash, echo-trail motion blending, and CC Deep Dark tone curve.
  - **Anti-Flash Mode** : Clean photosensitive-safe rendering that suppresses white/black strobes while preserving kinetic motion.
- **Flexible Export Options** :
  - High-efficiency codecs : **H.264**, **H.265 (HEVC)**, and **VP9**.
  - Adjustable bitrates from 5 to 50 Mbps.
  - Automated borderless stretch and crop-to-fill geometry.

---

## Getting Started

### Standalone Windows Installer (Recommended)

Download the latest standalone setup from the **[Releases Page](https://github.com/ssdme/cia-app-jugg/releases/latest)** :

📥 **[Download cia app v1.0.2 Installer (.exe)](https://github.com/ssdme/cia-app-jugg/releases/download/v1.0.2/cia.app_1.0.2_x64-setup.exe)**

- **100% Offline & Standalone** : Bundles the full media and beat detection runtimes (FFmpeg, FFprobe, and ONNX models).
- **Zero Configuration** : Runs out-of-the-box on clean Windows installations without administrative UAC prompts, Scoop, or Python setup.

---

## Build from Source

Prerequisites : Node.js (v18+), Rust stable, and Visual Studio MSVC build tools.

```powershell
# 1. Clone the repository
git clone https://github.com/ssdme/cia-app-jugg.git
cd cia-app-jugg

# 2. Install dependencies
npm ci

# 3. Launch development server
npm run tauri dev

# 4. Compile production NSIS installer
npm run tauri build
```

---

## Licence

cia app source code is [MIT licensed](LICENSE). Embedded third-party tools retain their respective licenses (see [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)).
