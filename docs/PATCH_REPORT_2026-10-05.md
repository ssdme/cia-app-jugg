# Audit patch report â€” 5 October 2026

Historical implementation and measurements for `feat/jugg-classic` at `958fefd`, version 1.0.3, prepared on `codex/audit-render-hardening`. The original patch was local and unpublished when this report was recorded. The pre-existing Cargo.toml change had no textual difference and was retained.

## User experience

Existing screens, layout, navigation and controls were preserved. Playwright comparisons with the audited baseline matched title bar, tabs, content and import-area geometry at 960 Ã— 580. Generated measurements/screenshots are under tests/generated/ui.

Corrected behavior includes disabled effects remaining disabled, complete style presets, FULL FX families, applied transition/Squish Pop settings and engine-consistent dissolve/wave units. CANCEL remains CANCELLING until terminal cleanup; AVG FPS reports measured throughput. Invalid export settings fail explicitly.

## Delivered fixes

| Finding | Implementation and verification |
| --- | --- |
| A1 â€” effect keys | Canonical camelCase contract and explicit gallery mapping; real toggle and IPC-key tests |
| A2 â€” presets | Complete Rust preset and stale-response rejection; rapid SMOOTH/HYBRID tests |
| A3 â€” source time | Schema-3 source-local ranges/stable IDs; isolated later-source, discontinuous, mixed-FPS and reverse-boundary exports |
| A4 â€” cancellation | Single active job, scoped progress/cancellation/cleanup and confirmed restart |
| A5 â€” blocking work | Bounded analysis workers, spawn_blocking render, file fingerprints and request generations |
| A6 â€” processes | Drained bounded stderr, per-process status, blocked-pipe cancellation and atomic output publication |
| A7 â€” transitions | Rendered Warp Bubble, Wave Warp and Slide Shake parameters; zero/active amplitude frame comparisons |
| A8 â€” validation | Central style/dimension/FPS/duration/codec/segment/effect checks and explicit scene formats/migrations |

Probe failures no longer fabricate metadata. Short plans retain actual selected durations. Raw caches read at most two-second blocks with disk eviction and a frame LRU. Active-effect buffers are reused; idle sliders stop animation; late listeners are released.

## Historical release measurements

Ryzen 5 5600H, approximately 28 GiB usable RAM, Windows x64, Node 24.15.0, Rust 1.96.0 and FFmpeg 8.1.1. Medians of three runs per scenario. Inputs were synthetic testsrc2 at 30 FPS; output was one second, HARD/FULL FX, 16 FPS, H.264/MP4, 12 Mb/s. The 4K/reverse canvas remained 4K.

The baseline harness extracts the 958fefd render loop and pixel functions, adapting only paths, Tauri events and DTOs. Both sides use identical plans, media and executables. This measures the windowless engine, not full application latency.

| Scenario | Before | After | Render working set before/after | Decoded RGB before/after |
| --- | ---: | ---: | ---: | ---: |
| 1080p, two-second source | 1.36 s | 1.29 s | 181.1 / 163.1 MiB | 356.0 / 356.0 MiB |
| 1080p, short extract from twelve-second source | 4.10 s | 1.27 s | 181.2 / 163.2 MiB | 2135.7 / 356.0 MiB |
| 4K, two-second source, reverse | 7.56 s | 5.20 s | 703.4 / 489.7 MiB | 1423.8 / 1423.8 MiB |

Working-set sampling used a 10 ms interval and excludes FFmpeg children/frontend. The short-1080p difference is small relative to measurement variability. Other improvements describe these cases, not a universal speedup.

| Patched scenario | Decode | Sampling/effects/pipe | Finalization | Frame-LRU peak |
| --- | ---: | ---: | ---: | ---: |
| Short 1080p | 0.308 s | 0.583 s | 0.390 s | 94.9 MiB |
| Long source / short extract | 0.308 s | 0.582 s | 0.377 s | 94.9 MiB |
| 4K/reverse | 1.087 s | 2.825 s | 1.263 s | 237.3 MiB |

Sampling includes waiting for the concurrent encoder pipe, so it does not isolate FFmpeg CPU time. Of 38 frame requests, the 1080p LRU had nine hits/29 misses; 4K had seven/31. Peak disk usage matches decoded RGB above. Budgets are 256 MiB frame LRU and 4 GiB raw cache, not total application RAM. Eviction testing deliberately uses a four-frame disk budget.

Reproduce with npm run benchmark, then npm run benchmark:baseline. Generated plans/results remain in tests/generated/benchmark.

## Checks recorded on 5 October

- Clean npm install, source checks, four frontend tests and Vite build passed; npm audit reported none.
- Headless Chrome/IPC simulation covered initial layout, asynchronous presets, toggles, stale events and cancellation.
- Rust: 56 unit tests and three integrations passed; six historical personal-media tests were explicitly ignored. Formatting and Clippy with -D warnings passed.
- Native integrations executed beat/ONNX on synthetic audio and checked seven supported codec/container pairs, metadata, FPS, resolution and duration.
- Cancellation covered decoding, sampling, encoding and blocked pipes, followed by cleanup/restart. Sources included Unicode and spaces.
- Six native files passed SHA-256 verification. The pinned FFmpeg/FFprobe archive was verified in a clean reference directory.
- Tauri/NSIS produced an unsigned test installer including six native files and FFmpeg notices.
- CI checks became blocking; release-key generation was removed. Updater signature tests covered correct signatures, modified files and altered comments.

Pinned npm versions: Vite 8.3.2, Svelte 5.57.1, svelte-check 4.7.6, Tauri API 2.11.0 and CLI 2.12.1. Rust lockfile retained.

The historical unsigned installer was cia jugg_1.0.3_x64-setup.exe, 136100360 bytes (129.80 MiB), SHA-256 834e188e7ed2d8e9df49c24808a61f41649db0d766600af784dfcfb63b190508. It is a local build artifact, excluded from this source publication.

## Release qualification still required

The installer was built, not installed in a clean Windows VM. Mocked IPC does not replace WebView2/CSP/native-dialog validation. A real update between two signed versions was not executed.

The application remains version 1.0.3; a separate release must synchronize versions and validate installation/update signing. No installer or updater manifest was published by this audit work.

Remaining limits: disabled Text Studio, large App component, undocumented upstream beat/ONNX provenance and no automatic recovery cleanup after abrupt process termination. npm audit does not cover Rust or native executable advisories. See [technical debt](TECH_DEBT.md) and the later [ScenePack report](SCENEPACK_UPDATE_REPORT_2026-10-05.md).
