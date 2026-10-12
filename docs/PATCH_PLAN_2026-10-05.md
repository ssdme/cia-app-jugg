# Audit patch plan â€” 5 October 2026

Historical implementation plan for the accepted local baseline `feat/jugg-classic` at `958fefd`, version 1.0.3. Preserve pre-existing Cargo manifest changes. Work on `codex/audit-render-hardening`; publication, releases and tags are separate operations. See [audit](AUDIT_2026-10-05.md) and [implementation report](PATCH_REPORT_2026-10-05.md).

The target is the shipped video workflow. Keep Text Studio disabled and preserve the existing screens, layout and navigation. Composition, timeline, batch, RIFE and GPU encoding require separate work; do not merge experimental main wholesale.

## Design decisions

| Contract | Decision |
| --- | --- |
| Effects | One camelCase property per option; explicit gallery mapping |
| Presets | Rust supplies numeric values and activation flags |
| Style changes | Replace artistic preset values; preserve explicit Anti-Flash and CC Deep Dark choices |
| FULL FX | Resolve complete preset families before user overrides; Anti-Flash has final priority |
| Sources | Stable IDs and source-local extraction time, distinct from compact montage time |
| Jobs | One heavy render; reject overlapping starts |
| Cancellation | Per-job signal, filtered progress, cancelling state until children stop and cleanup finishes |
| Frontend types | Extract typed contracts/services; keep strict checks in remaining components |
| Compatibility | Explicit plan schema and documented schema-1/2 migration; reject ambiguous mappings |
| Image quality | Preserve output dimensions and established geometry; do not lower resolution to fit a budget |

Proposed module names are design suggestions; the implementation report records the actual delivered layout.

## Preparation

Capture dependency/native-runtime versions and working baseline renders. Generate deterministic portable sources with distinct colors, durations, 24/30/60 FPS, audio, Unicode and spaced paths. Separate unit tests, native integration tests and optional benchmarks. Missing required tools/fixtures must fail; optional personal-media benchmarks must be explicitly ignored.

## 1. Settings and validation â€” A1, A2, A7, A8

Remove duplicate effect keys and free-form gallery properties. Add complete Rust presets and reject stale style responses. Define FULL FX OFF for every relevant family, preserve user overrides, and consume Warp Bubble, Wave Warp and Slide Shake parameters in preview/export.

Validate codecs/containers, even dimensions, FPS, duration, finite parameters and non-empty segments before allocation. Add typed IPC contracts and regression tests for each toggle, all styles, preset race ordering, transition extremes, unsupported WEBM combinations and malformed plans.

Primary files: App.svelte, plan.rs, effects.rs, preview.rs, render.rs, contracts.ts and render-settings.ts.

## 2. Explicit source identity â€” A3, A8

Use `{ sourceId, start, duration }` ranges tied to an immutable full path list. Never apply cumulative gallery timestamps directly as an individual source seek. Compact the montage from selected source-local ranges.

Declare pairs/boundaries explicitly and migrate legacy plans by splitting global ranges at source boundaries. Reject missing/inconsistent mappings.

Validate selecting only the second or third source, discontinuous selections, mixed FPS, unequal durations, reverse at boundaries, short sources and schema round trips.

## 3. Render lifecycle â€” A4, A6

Register an immutable job ID, scoped cancellation and dedicated temporary directory. Refuse overlapping renders. Keep the UI cancelling until the job has stopped native children, closed handles and removed its files. Filter stale progress by job ID.

Drain bounded stderr and verify every process and decoded source. Use temporary output and atomic publication without replacing existing files. Separate the render core from AppHandle through progress/cancellation adapters.

Test cancellation during decode, sampling, writes and finalization; immediate early restart rejection; restart after cleanup; noisy stderr; corrupt later media; inaccessible output; blocked pipes; and competing output reservations.

Primary new modules: jobs.rs and process.rs.

## 4. Responsive analysis â€” A5

Move beats, metadata, scenes, thumbnails and rendering onto controlled workers. Begin with two concurrent analysis jobs and one render. Tag requests with generation/source identity so late responses cannot overwrite new selections.

Cache by normalized path, file size, precise modification time, algorithm/options and content fingerprint when justified. Fix listener/timer cleanup, including registration after unmount. Limit progress to roughly ten events/s while retaining phase and terminal events. Stop idle slider animation loops.

Validate responsiveness, replacement at the same path, source changes during analysis, stable listener counts and final progress delivery.

## 5. Bounded rendering costs

Measure preparation, decoding, sampling/effects, encoding and finalization. Report actual processing throughput separately from output FPS. Decode only merged required ranges with effect margins.

Start with a 256 MiB frame LRU and 4 GiB raw disk-cache budget; these are cache limits, not total process-memory guarantees. Use indexed blocks, eviction and controlled re-decoding for loops/reverse; do not decode whole long sources for small extracts. Preserve resolution, monitor writes/free space and reuse buffers only for enabled effects.

Benchmark identical release inputs/settings: short 1080p, a small extract from long media, multiple sources, 4K and reverse. Record several-run medians, phase times, working set, decoded bytes, storage, hits/misses and cancellation. Compare pre-encoding sampled frames to distinguish codec variation. Explain regressions above ten percent rather than promising unmeasured acceleration.

Primary new module: frame_cache.rs; provide a portable benchmark harness.

## 6. Quality and distribution

Add source-scoped svelte-check without disabling checkJs. Extract contracts/services before visual refactoring. Upgrade compatible development dependencies and lock chosen versions. Make formatting and all-target Clippy warning-free, with narrow justified exceptions.

Describe native payload origins, versions, hashes and licenses. Verify FFmpeg, FFprobe, beat runtime and model before packaging. Fix fallback lookup and remove arbitrary port-owner termination. Apply a compatible CSP and validate native inputs/paths.

Align README, metadata, notices and technical debt with delivered behavior. Keep unsigned CI installers separate from release signing; verify artifact signatures against the configured updater key. A release must synchronize manifest versions.

## Validation criteria

The following were planned checks, not claims of execution:

```powershell
npm ci
npm run check
npm test
npm run build
npm run verify:payload
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --locked --all-targets --manifest-path src-tauri/Cargo.toml -- -D warnings
cargo test --locked --manifest-path src-tauri/Cargo.toml
```

The regression matrix includes settings, source identity, temporal boundaries, cancellation, native errors, release performance and distribution. Installation qualification requires a clean Windows machine, offline import/analysis/export/cancellation, and an actual update between two correctly signed test versions.

Order: preparation, contracts, sources, jobs, analysis, rendering budgets, then distribution. Fixtures/check tooling can begin early. Completion requires tested A1â€“A8 fixes, honest test execution, measured cache budgets and a working clean-install video path; outstanding limitations remain explicit.
