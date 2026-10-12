# ScenePack update plan

Date: 2026-10-05

Status: approved by the user; implemented locally. Validation results are recorded in `SCENEPACK_UPDATE_REPORT_2026-10-05.md`.

Revision: adaptive scheduling with a flexible recurrence target around 8 seconds. Source-length examples illustrate intent; they do not prescribe exact output timelines.

## Scope

- Add the default-enabled `no repeatitive scene` option to the ScenePack gallery and Settings, using one shared value.
- Discourage early ScenePack repetition and adapt shot length, source coverage and remapping to available motion and musical timing.
- Guide time remapping with motion analysis in ScenePack and Long Clip.
- Remove exactly pure black and pure white source frames from ScenePack.
- Add `OPEN FILE` beside `OPEN FOLDER` and `RENDER AGAIN`.
- Start rendering directly when `RENDER AGAIN` is clicked.

Keep existing layouts, controls, effects and presets except for these requested additions. Keep code and technical documentation in English. Preserve the existing audit patch in the working tree.

## 1. Scene scheduling

### Current failure

In `src-tauri/src/plan.rs`, rhythm-based switching advances to the next selected scene and resets its source cursor to the beginning. With a small selection, the planner repeatedly samples the same beginnings while leaving the rest of each scene unused.

### Option behavior

The gallery and Settings checkboxes use the exact label `no repeatitive scene`. New sessions default to enabled. Store the option with newly generated plans. Changing it does not require decoding media again. Style presets must not overwrite it.

Use one adaptive scheduler rather than switching between strict beat cuts and fixed full-scene blocks. The goal is readable action, musical timing and variety. Complete scene coverage and equal or proportional shot lengths are not mandatory.

### Candidate construction

1. After blank-frame removal, identify motion windows: buildup, sustained activity, peaks and recovery. Retain context around peaks so an action remains understandable.
2. At each possible transition, compare continuing the current scene, extending its action across several beats, moving to another unused window, switching to another selected scene, and reusing footage when needed.
3. Offer shot lengths spanning several nearby beats or downbeats, informed by FAST/MID/SLOW. These controls express a pacing preference; a beat does not require a scene switch when the current action benefits from continuation.
4. Track consumed intervals by source ID and source timestamps, and recent appearances by parent scene ID. A fresh passage from a previously shown scene differs from replaying the same footage. Overlapping scene selections and splits around removed frames must not disguise a repeated passage as new footage.
5. Preserve source continuity within a shot. Prefer a continuous unused extension to an unnecessary jump. Use selection order as a stable tie-breaker rather than prescribing identical duration or order in every case.

### Selection score and lookahead

Rank candidate shots using:

- Sustained, reliable motion and a readable action window.
- Alignment of useful motion phases and transitions with beats or downbeats.
- Continuity when an action is still developing.
- Variety across selected scenes and source windows.
- Playback quality, including the amount of stretching and time spent on low-motion frames.
- A strong cost for repeating footage, especially a recently shown beginning.

Use bounded lookahead across upcoming musical accents, plus an estimate of remaining useful unseen footage. This prevents consuming all strong windows through rapid cuts at the start and then having to repeat them. Keep candidate counts and search width bounded; score deterministically, with stable source-position tie-breaks.

Use the existing beat and downbeat timestamps for musical alignment. Do not claim musical intensity or drop detection from these timestamps. Audio-energy analysis is outside this patch.

### Scarcity response

Adapt gradually to the amount of useful footage relative to the remaining output:

- With ample footage, allow shorter, varied shots and stronger rhythm-based switching.
- With fewer useful windows, prefer longer action continuations and fewer scene changes while retaining beat effects inside each shot.
- A whole scene is a strong candidate when its action remains useful and its natural duration fits the music. A scene with a long idle beginning or ending can be shortened instead.
- Keep selected static scenes eligible as brief, neutral passages. Do not convert noise into false action or silently discard a scene solely because it is static.
- Favor representing the selected scenes when the available output permits it, without forcing a burst of meaningless cuts to include everything.
- When there is more footage than the audio can use, choose useful distinct windows rather than compressing all source footage into the output.

No formula assigns each scene a fixed share of the audio. Short selections should tend toward fuller, longer scene use; the actual durations come from action windows, music and playback quality.

### Flexible repetition containment

Use roughly 8 seconds of output as the target for avoiding an initial replay, and as a reference for recency penalties afterward. This is a smooth scoring preference, not a prohibition ending at an exact timestamp or an instruction to loop at that point.

- Penalize actual footage reuse more heavily than returning to a scene for a genuinely unused passage.
- Prefer unseen usable content, but allow a meaningful replay when alternatives would produce a prolonged still shot or an excessive slowdown.
- Choose a nearby musical accent for a necessary replay. It may land somewhat before or after the target.
- After that target, keep a cost for overlap and recent repetition; replay does not become free.
- Where alternatives exist, avoid consecutive replay of the same source window and rotate among different scenes or actions.
- With extremely short or entirely static material, keep playback usable rather than forcing an artificial first pass to last 8 seconds. Record the fallback in planning diagnostics for validation.
- On a short audio track, prefer a single varied passage when feasible; do not create a loop just to fit a pacing template.

Firm safeguards are valid source ranges, exact output duration, no added freeze padding, bounded base playback stretching, scene-boundary safety and exclusion of removed frames. Set a conservative initial stretch ceiling, then tune it against native-frame-rate fixtures; existing brief stylized speed ramps remain available. Intentional local reverse effects are distinct from restarting a shot and remain supported within valid allocated ranges.

When disabled, restore the existing rhythm-based switching policy. Motion guidance and pure-frame filtering remain active as independent requested features. Existing saved plans keep their recorded timing when replayed.

## 2. Motion-guided remapping

Analyze selected ScenePack scenes and the Long Clip source. No Change bypasses motion analysis.

- Produce a time-indexed motion profile using a small image stream, temporal smoothing and exposure-normalized block movement. Track motion level, change, persistence and confidence.
- Combine an absolute activity floor with relative peaks within each scene. A static scene must not acquire a strong action score simply because one noisy frame is its local maximum.
- Prefer sustained movement over isolated brightness flashes or noise. Reset comparisons at scene cuts and removed-frame gaps. Real camera movement remains usable visual activity.
- Form windows around motion buildup, peaks and recovery; retain enough neighboring footage to avoid isolated, unreadable peak frames.
- Place brief slow phases near useful movement. Apply stronger remapping when the activity and confidence support it, and lighter or linear remapping on weak or static material. Respect the selected style, explicit settings and disabled effects; adaptation must not exceed the requested intensity. Do not repeatedly end every beat segment in a prolonged low-motion phase.
- Keep the chosen source window continuous and its action readable. Extend it across beats when warranted, instead of resetting its beginning for another speed ramp.
- In Long Clip, replace evenly spaced source anchors with motion-rich anchors distributed through the video. Keep individual sampled windows within detected scene boundaries and discourage overlapping repeats.
- Keep genuinely static selected scenes eligible. Use a neutral, linear remap where no useful motion is detected.

This analysis estimates visual activity; it does not label the semantic meaning of an action. Use deterministic scoring and source-position tie-breaks so the same inputs produce the same plan. Do not add a Python runtime or machine-learning dependency.

## 3. Exact blank-frame filtering

For ScenePack, inspect decoded source frames before scaling, padding, frame-rate normalization or generated effects. Remove a frame only when every pixel is exactly pure black or exactly pure white, respecting the source's color range and bit depth. Keep near-black, near-white and detailed frames.

Inspect frames at native resolution and precision. A reduced-resolution motion stream cannot establish that every original pixel is blank. Stream analysis with bounded memory rather than saving full-resolution decoded video to disk.

Remove blank frames wherever they occur, including the middle of a scene. Represent the remaining material as continuous source-local fragments grouped under the original scene ID. Keep selection IDs stable. Recompute usable duration and choose thumbnails from retained frames. Exclude wholly blank scenes; return a clear error if the selection has no usable material.

Use actual source timestamps for variable-frame-rate footage. Compact video gaps only; retain the original audio and requested output duration. FFmpeg provides timestamp-preserving processing controls in its [official documentation](https://www.ffmpeg.org/ffmpeg.html#Advanced-Video-options).

Generated flashes remain controlled by the existing effect settings; this filter applies to source frames.

## 4. Analysis contracts and performance

Introduce a small prepared-scene model containing the original scene ID, source ID, retained fragments, usable duration, motion profile, action windows and thumbnail position. Keep each render source range continuous; split ranges at removed gaps while retaining their parent scene identity for scheduling. Save selected-window and reuse diagnostics with new plans so scheduling decisions can be inspected without adding controls to the UI.

- Cache analysis by canonical source path, file size, modification time, analysis version and relevant options.
- Reuse results for style, rhythm and checkbox changes. Analyze missing sources or selected ranges only.
- Reuse bounded analysis concurrency and managed child-process cancellation.
- Check source fingerprints before and after analysis; reject stale results after source replacement or selection changes.
- Show analysis progress using the existing progress UI, and allow cancellation.
- Persist versioned planning metadata while retaining compatibility with existing saved plans. Do not silently regenerate old plans when loading them.

Measure cold analysis, cached replanning, peak memory and render duration on representative small and large selections. The full-resolution blank-frame scan adds work on the first analysis; cached runs must avoid rescanning unchanged media.

## 5. Completed-render actions

### OPEN FILE

Add the button on the same row as the existing two actions. Open the completed output in the Windows default associated application through a native command, using the exact validated file path. Report a missing file or unavailable association through the existing error UI. Do not automatically open a player after every render.

### RENDER AGAIN

Call the shared render-start handler immediately, with the current audio, selected scenes and settings. Reuse cached analysis. Allocate a new job ID and output path, guard double clicks and concurrent jobs, and retain cancellation and error handling. Return directly to the running progress state without requiring `RUN PROCESS`.

## 6. Implementation sequence

1. Add prepared-scene contracts, analysis caching and deterministic media fixtures.
2. Implement exact blank-frame filtering and motion profiles.
3. Add bounded adaptive shot selection, smooth recurrence costs and motion-guided remapping.
4. Connect the shared checkbox, `OPEN FILE` and immediate `RENDER AGAIN` behavior.
5. Run regression checks, benchmark analysis and render paths, and document results.

Keep analysis and scheduling helpers in focused Rust modules rather than extending the main planner with another large inline pipeline. Use small typed frontend helpers where they reduce duplicated state or native calls.

## 7. Acceptance checks

- Unequal scene lengths, uneven action distribution and audio durations unrelated to source lengths. Do not assert proportional shot lengths or predetermined source-length timelines.
- Continued action across several beats with scarce footage; shorter varied shots with ample footage; quiet tails not extended simply to consume whole scenes.
- Strong early-reuse penalties when useful unseen footage exists; necessary returns near musical accents around the recurrence target; no abrupt scheduling change at exactly 8 seconds.
- Different treatment of a new window from an already shown scene and an overlapping replay of the same window.
- No excessive base slowdown used solely to reach the recurrence target; usable fallback for one very short scene or fully static material.
- Source continuity, bounded source windows, exact output duration and deterministic choices. No added freeze padding or return to invalid or removed frames.
- FAST/MID/SLOW behavior with the option disabled; pacing preferences with the option enabled; reverse-effect source bounds.
- Pure black/white frames at the beginning, middle and end; a single nonblank pixel; near-white and near-black footage; fully blank selections.
- Variable frame rates, higher bit depths and discontinuous fragments retaining one scene identity.
- Motion surrounded by still footage, multiple separate actions in one scene, a brightness flash without movement, real cuts and fully static sources. Validate peak confidence and readable context around selected action windows.
- No Change bypassing the new analysis, and unchanged output audio duration.
- Cancellation, replaced media, cache reuse and stale-result rejection.
- Checkbox synchronization and default state; preserved current selection and settings when rerendering.
- `OPEN FILE` opening the completed output; `RENDER AGAIN` starting one job directly and creating a distinct output.
- Existing frontend checks, Rust tests, formatting, Clippy and native smoke renders. Inspect representative renders visually for readable action, smooth pacing, natural recurrence and lack of repeated beginnings; numerical scoring alone does not establish the best edit.

## Approval

The user approved adaptive shot selection, motion-guided emphasis and repetition containment around a flexible 8-second target, then explicitly requested implementation. There is no mandatory full first pass, proportional scene allocation or forced slowdown to reach a clock threshold. The other requested features remain in scope.
