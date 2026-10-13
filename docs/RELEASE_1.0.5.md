# cia jugg v1.0.5

This release adds varied musical accents and a balanced ScenePack policy. Source code on `main` and the Windows installer target the same release commit.

## Changes

- Add **3:4** output at **1080 x 1440** and select it by default. Existing aspect ratios and custom dimensions remain available.
- Enable **Smart Scene Flow** by default in ScenePack. It balances fresh movement passages with controlled returns and varied shot lengths. `no repeatitive scene` remains available; the two policies are mutually exclusive. Disable both for classic cuts.
- Add six independent effects in **Details**, all enabled by default: **Time Slice** (delayed panels), **Ink Pulse** (ink contours and halftone), **Prism Drift** (color-edge separation), **Focus Pull** (moving focus plane), **Relief Light** (animated contour lighting), and **Paper Gate** (torn-paper reveal).
- Keep creative effects independent of Full FX and style changes. Select All, Deselect All and Reset to Style apply to all effect cards; reset restores the six creative defaults.
- Schedule bounded musical accents with clean intervals. Time Slice samples stay inside the current source fragment, and missing creative options in older plans preserve their existing output.

## Download

[Windows x64 installer](https://github.com/ssdme/cia-app-jugg/releases/download/v1.0.5/cia.jugg_1.0.5_x64-setup.exe)

The installer includes the native video and beat-analysis runtime. The release also provides its updater signature, `latest.json` and `SHA256SUMS.txt`, using the existing trusted updater key.

## Verification

Validation covers frontend types, browser interactions and render request defaults, native scheduling and synthetic video renders, Rust formatting and Clippy, payload hashes and installer signature verification. Historical personal-media benchmarks are excluded from the automated test suite.

Text Studio remains disabled. The separate compositing experiments are not part of this release.
