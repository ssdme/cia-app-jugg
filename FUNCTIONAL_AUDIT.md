# Functional audit — CIA Jugg

Last reviewed: 2026-08-28

This document records what a visible control does **all the way through to the delivered output**. A control is not classified as functional simply because it changes front-end state or has unit tests.

## Classification

- ✅ **Functional** — reaches a Tauri command and changes the saved plan or rendered output.
- ⚠️ **Partial** — works only in some paths, has no user-facing proof, or its result is incomplete.
- ❌ **Decorative** — changes UI state only, is simulated, or has no effect on a delivered result.

## App shell

| Surface / control | Status | Evidence and action |
| --- | --- | --- |
| Window controls | ✅ | Native Tauri controls are used when the desktop runtime is present. |
| Update check and install | ✅ | Uses the Tauri updater; failure is reported as a toast. |
| About, GitHub and dependency links | ✅ | Whitelisted URLs are opened by the native host. |
| Undo / Redo | ⚠️ | A history manager exists, but edits were never recorded by the UI. Phase 0 wires snapshots to real editor changes. |
| Shortcuts modal | ⚠️ | The modal is functional, but timeline playback has no visual media output. |

## Quick and Studio input

| Surface / control | Status | Evidence and action |
| --- | --- | --- |
| Source video, drums and master-audio pickers | ✅ | File validation, probing and beat detection feed plan generation. |
| Drag and drop | ✅ | Native drag/drop resolves a target zone and validates its extension. |
| Media Pool import / refresh | ⚠️ | The index and basic probing work, but import failures had no user-facing message and cached analysis is not used by the render flow. |
| Media Pool assign Scene / Drums | ❌ → ✅ | The handlers called undefined functions. Phase 0 replaces them with explicit probing. |
| Media Pool remove | ⚠️ | Removes only the pool index, not the source file; this must be stated in the UI. |
| One-Click Jugg | ⚠️ | It runs a real Dumper → plan → render pipeline, but its required inputs and output choices are not clear enough. |
| Manual settings entry | ✅ | Probes all three sources and detects beats before opening Studio. |

## Studio render controls

| Surface / control | Status | Evidence and action |
| --- | --- | --- |
| Style, frame rate, aspect ratio, codec, bitrate and container | ✅ | Sent to `generate_plan`; export configuration reaches FFmpeg. |
| Full FX and per-effect toggles | ✅ | Sent to `generate_plan` as effect overrides. |
| Effect custom parameters | ✅ | Sent to `generate_plan` and stored in `ProjectPlan.customParams`. |
| Echo / trail | ✅ | Applied as a runtime override by the renderer. |
| Sidechain, varispeed, staccato and ducking level | ❌ | A UI state and audio helpers exist, but the render command passes the target audio directly to FFmpeg. Hidden until the audio pipeline is connected. |
| Run Process / Render Final Jugg | ⚠️ | Both launch a render and their distinction is unclear. UX restructuring will leave one primary action per state. |
| Cancel render | ✅ | Cancels the FFmpeg job and cleans temporary output. |
| Render progress and completion | ✅ | Driven by native render events; completion exposes the output folder. |
| Pre-flight information | ❌ | BPM, output size, disk space and photosensitivity warning are not presented before launch. |

## Plan, preview and persistence

| Surface / control | Status | Evidence and action |
| --- | --- | --- |
| Plan summary | ⚠️ | It was built separately from the full plan and could disagree with it. Phase 0 keeps a single complete current plan. |
| Timeline curve / cuts / scrubber | ❌ → ⚠️ | It received only a summary, not segments; scrub output was synthetic metadata rather than visible media. Phase 0 binds it to the complete plan. A source-frame preview remains future work. |
| Parameters page (28 controls) | ❌ | `remapParams` was saved but was not sent to `generate_plan` or the renderer. The page is hidden until controls are implemented or mapped to the real custom-parameter model. |
| “Live Preview (256×256), 30 FPS, 8 ms” | ❌ | The canvas is blank and not connected to the renderer. Hidden; no real-time claim may remain. |
| Save / Load Project | ❌ → ⚠️ | Saving used a display summary where Rust expects a full `ProjectPlan`; loading did not rebuild editor state. Phase 0 saves the complete plan. Re-probing missing source media and restoring all Studio controls remains required. |
| Preset save / load | ⚠️ | Presets persist the currently decorative parameter model. Hidden with that model. |

## Specialist workflows

| Surface / control | Status | Evidence and action |
| --- | --- | --- |
| Dumper analysis and report | ✅ | Scene, beat, motion and profile analysis run locally and produce report files. |
| Generate Edit Plan / Apply as Project | ⚠️ | A plan is generated, but it must share the same current-plan state as Studio and clearly explain the consequence. |
| Composition operations | ⚠️ | Blend operations affect composition render. Camera/post-FX sliders only affect the mesh preview path, not every composition output. |
| Character segmentation | ⚠️ | Uses a real pipeline but needs an upfront NVIDIA/CUDA requirement and recovery guidance. |
| Batch Processor | ❌ | The executor sleeps for 150 ms per file and reports fabricated success/output paths; action, preset and export settings are not used. Hidden until a real batch render exists. |

## Release gate

No feature may be marked ready unless its row is ✅, or the UI explicitly says what is unavailable. Before every release, repeat this audit using a real media fixture and verify: input → plan → preview where claimed → render → saved project → reopened project → rerender.
