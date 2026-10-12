# cia jugg v1.0.4

This release updates the Windows application and the main source branch to the validated rendering engine.

## Changes

- Fix `Invalid action window` render failures caused by floating-point timestamp rounding. Analysis bounds action windows to the selected scene, and validation tolerates numerical drift while rejecting invalid metadata.
- Add motion-aware ScenePack and Long Clip scheduling. The default-enabled `no repeatitive scene` option favors fresh useful footage and discourages early repetition with a flexible eight-second target.
- Remove exactly pure black and white source frames in ScenePack while retaining near-blank detail and native timestamps.
- Add `OPEN FILE` and make `RENDER AGAIN` start processing directly.
- Correct effect toggles, complete style presets, transition parameters and source-local extraction.
- Scope render jobs, cancellation, native processes and temporary files; bound raw/frame caches and retain output resolution.
- Verify the native FFmpeg/FFprobe and beat/ONNX payload and updater artifact signatures.

## Download

[Windows x64 installer](https://github.com/ssdme/cia-app-jugg/releases/download/v1.0.4/cia.jugg_1.0.4_x64-setup.exe)

The installer includes the native video and beat-analysis runtime. The release also provides its updater signature, `latest.json` and `SHA256SUMS.txt`.

The updater uses the existing public key. Source code and installation assets target the same release commit. Text Studio remains disabled; the separate compositing experiments are not included in this release.
