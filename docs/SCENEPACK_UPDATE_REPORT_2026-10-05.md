# ScenePack update validation

## Action-window rounding fix — 12 October 2026

Prepared action windows are bounded to the selected source interval; peaks and
activity are clamped, and degenerate windows are removed. Schedule validation
allows 0.0001 seconds of boundary rounding while still rejecting genuine
overruns and non-finite values. Two regression tests cover infinitesimal boundary
drift and invalid action metadata.

Publication checks passed: clean npm installation, source diagnostics, four
frontend tests, UI regressions, production frontend build, six native payload
hashes, Rust formatting, warning-free all-target Clippy and 72 native tests.
Six historical personal-media tests remain explicitly ignored.

Date: 2026-10-05. Implemented locally on `codex/audit-render-hardening`; the preceding audit patch is preserved.

## Behavior

- `no repeatitive scene` defaults to enabled and shares one value between the gallery and Settings. Preset changes retain it.
- ScenePack scores motion windows, musical boundaries, continuous action, playback speed and previously used source intervals. It discourages early replays around eight seconds with a smooth penalty. Scarcity favors longer useful passages; source-length examples do not prescribe shot lengths.
- Reuse is tracked by source timestamps, including overlapping selections and fragments of the same parent scene. Base shot speed stays within 0.67–1.5×; insufficient footage can loop earlier rather than adding freeze padding.
- Motion guides ScenePack and Long Clip anchors and remap strength. Low-motion portions use linear sampling. Long Clip searches across each source and uses cuts as analysis boundaries. No Change bypasses the new analysis.
- ScenePack removes exactly pure black/white native frames before resizing or FPS conversion. It preserves near-blank detail, native bit depth, color range and variable timestamps. Internal gaps become continuous retained fragments; wholly blank scenes are excluded. Gallery IDs remain stable.
- `OPEN FILE` uses the validated completed file path and Windows association. `RENDER AGAIN` starts the shared render handler immediately with a new job and output path.
- New schema-4 plans record source fingerprints, remap envelopes and shot/reuse diagnostics. Schemas 1–3 retain their sampling behavior. Replaced media are rejected before rendering.

Analysis runs on existing native workers, supports cancellation and progress, and caches unchanged sources. The profile and shared cache each have a 32 MiB limit; the cache holds at most 16 entries. Native frames stream through a small buffer rather than a full decoded-video file. Unknown pixel precision/layout is retained conservatively rather than classifying converted pixels as exactly blank.

## Checks

- `npm run check`: zero errors and warnings.
- `npm test`: four frontend tests pass.
- `npm run build`: production frontend build passes.
- `npm run test:ui` with `CIA_BROWSER_CHANNEL=chrome`: shared default checkbox, filtered selection IDs, No Change bypass, exact output path, direct rerender with a new job, stale progress and cancellation pass. Gallery and completed-render screenshots were inspected at 960×580.
- `cargo fmt --all -- --check` and `cargo clippy --locked --all-targets -- -D warnings`: pass.
- `cargo test --locked --all-targets`: 70 tests pass; six older personal-media tests remain explicitly ignored and portable rendering regressions cover the supported paths.

New native coverage checks unequal scene lengths, deterministic scheduling, fresh footage before replay, overlapping parent selections, short/static fallback, motion spread across long footage, strictly advancing motion anchors, fragmented source bounds and schema-4 round trips. Exact-frame tests cover 16-bit RGB detail, a single distinct pixel, limited/full YUV levels, 10-bit levels, pure blank sources and variable timestamps. A decoder regression verifies that FPS conversion cannot pull the next excluded white frame into a retained fragment.

An eight-second motion-guided render at 160×90/12 FPS completes successfully and retains the requested duration. Its contact sheet was inspected for changing source timestamps and varied motion. Cancellation during decoding, cancellation before job registration and stale source fingerprints are also checked. The OPEN FILE IPC path is covered in browser tests; the actual Windows media-player association was not launched by these tests.

## Measurements

Local synthetic H.264 sources at 30 FPS; development Rust profile (`opt-level=1`). Analysis timings are one cold measurement and one cache hit per source/mode. Planning is the mean of five runs for a 20-second output. The eight-second native render took approximately 2–3 seconds across validation runs.

| Source | Mode | Cold analysis | Cache hit | Plan, 3 scenes | Plan, 24 scenes |
| --- | --- | ---: | ---: | ---: | ---: |
| 1080p, 2 s | ScenePack | 218 ms | 0.27 ms | 1.39 ms | — |
| 1080p, 2 s | Long Clip | 135 ms | 0.23 ms | 8.69 ms | — |
| 1080p, 12 s | ScenePack | 584 ms | 0.30 ms | 0.32 ms | 7.12 ms |
| 1080p, 12 s | Long Clip | 458 ms | 0.26 ms | 2.44 ms | 45.20 ms |
| 4K, 2 s | ScenePack | 459 ms | 0.25 ms | 1.64 ms | — |
| 4K, 2 s | Long Clip | 376 ms | 0.31 ms | 8.08 ms | — |

ScenePack inspects all 60/360 native frames in these sources. Long Clip emits only 20/120 frames to the motion analyzer; FFmpeg still decodes the input. Sampled peak working set of the Rust benchmark process is 7.45–9.10 MiB, excluding FFmpeg child processes. These short synthetic cases establish cache reuse and bounded analysis behavior, not production throughput or total application memory.

Reproduce after generating the existing benchmark media:

```powershell
npm run test:media
cargo run --locked --manifest-path src-tauri/Cargo.toml --example benchmark_scene_analysis
```

Measurements are written to `tests/generated/benchmark/scene-analysis-results.json`. Browser captures are in `tests/generated/ui/`; native smoke outputs and the inspected contact sheet are in `tests/generated/scenes/`. Generated media are ignored by Git. CI generates fixtures explicitly before native tests.

Visual activity is a deterministic motion estimate, not semantic action recognition. Real footage remains the meaningful check for editorial quality; synthetic coverage verifies the scheduling and rendering safeguards. No package installer, release, commit or push is produced by this patch.
