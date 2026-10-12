# Technical Debt & Code Quality

Updated after the 5 October 2026 audit patch. Clippy and formatting are enforced in CI;
the former warning count from the monolithic `lib.rs` is obsolete.

## Checks

`svelte-check` retains strict JavaScript checking, and Rust uses
`cargo clippy --locked --all-targets -- -D warnings`. Exceptions to argument-count
lints are local to positional pixel/IPC APIs and explain why their contract is retained.
Portable integrations generate their own media. Six historical tests that require
personal files are explicitly ignored instead of reporting a skipped render as a pass.

## Remaining work

- `App.svelte` still owns much of the navigation and media workflow. Typed IPC,
  contracts and preset helpers are extracted; further component/service separation
  should preserve the existing screens and be covered by the UI regression harness.
- Scene thumbnails use a static placeholder if a preview cannot be extracted.
  Metadata and render failures are errors, rather than fabricated successful sources.
- Cache budgets bound raw files and frame LRU storage, not total application/FFmpeg RAM.
  Block eviction can repeat decoding for long reverse/loop-heavy timelines.
- Normal cancellation and errors clean up owned processes/files. Abrupt application
  termination is not yet covered by a Windows job object or stale-job recovery.
- Payload hashes are pinned. The upstream version/build provenance of the existing
  beat_this executable, model and ONNX DLLs remains unrecorded.
- Text Studio stays disabled. Its Python/runtime packaging, media transfer sizes,
  subprocess cancellation and user-specific paths need a separate implementation.
- Release acceptance still requires installation on a clean Windows machine and
  an end-to-end signed update between two test versions. Signature verification
  tests alone do not demonstrate installation or update success.

See [the audit](AUDIT_2026-10-05.md), [implementation plan](PATCH_PLAN_2026-10-05.md)
and [patch report](PATCH_REPORT_2026-10-05.md) for evidence and scope.
