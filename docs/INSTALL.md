# Install and first launch

cia app is a standalone Windows desktop application for beat-synced video editing.

## Prerequisites

- Windows 10 / 11 (64-bit).
- Built-in or system-available FFmpeg/FFprobe binaries (bundled in release installer).
- Standard audio/video media files:
  - Video: `mp4`, `mkv`, `webm`, `mov`, `avi`
  - Audio: `mp3`, `wav`, `flac`, `m4a`, `ogg`

## Launch and usage

1. Launch **cia app**.
2. Drag and drop (or browse for) three sources:
   - **Scene Video**: The footage to be geometrically remapped and transformed.
   - **Drums Audio**: The percussive or beat track used for beat and downbeat tracking.
   - **Target Audio**: The soundtrack to accompany the rendered output.
3. Choose your **Remap Style** (`HARD`, `SMOOTH`, or `HYBRID`), target framerate, aspect ratio, FX mode, and export options.
4. Click **RUN PROCESS** to generate the plan and render the finished video.

## Output naming and preservation

Rendered outputs are saved to the application's output directory (`output/`).

Existing files are never overwritten:
- Initial render: `cia_jugg_<timestamp>.<ext>`
- If a file with the same name already exists: `cia_jugg_<timestamp>-1.<ext>`, `cia_jugg_<timestamp>-2.<ext>`, and so on.
