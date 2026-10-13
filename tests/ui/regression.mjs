import { chromium } from 'playwright';
import { spawn } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import assert from 'node:assert/strict';

const server = spawn(process.execPath, [resolve('node_modules/vite/bin/vite.js'), '--host', '127.0.0.1', '--port', '5187'], { stdio: 'pipe', windowsHide: true });
let baselineServer;
const logs = new Map();
function collectLogs(child) {
  logs.set(child, '');
  for (const stream of [child.stdout, child.stderr]) stream.on('data', chunk => logs.set(child, (logs.get(child) + chunk).slice(-8192)));
  child.on('error', error => logs.set(child, `${logs.get(child)}\n${error.message}`));
}
collectLogs(server);
const output = resolve('tests/generated/ui'); mkdirSync(output, { recursive: true });
const waitFor = async (url, child) => {
  for (let attempt = 0; attempt < 600; attempt++) {
    if (child.exitCode !== null) throw new Error(`UI test server exited (${child.exitCode}): ${logs.get(child)}`);
    try { if ((await fetch(url)).ok) return; } catch {}
    await new Promise(resolve => setTimeout(resolve, 100));
  }
  throw new Error(`UI test server did not start: ${url}\n${logs.get(child)}`);
};

function mockNative() {
  const callbacks = new Map(); let callbackId = 0; const events = new Map();
  const effects = ['shakes','zoom','flicker','one_framers','transitions','tint','vignette','scanlines', 'exposure_flash','bouncy_shake','dissolve_shake','skew_shake','squish_pop','optics_bounce','buildup_chain','warp_stretch','zoom_beat_offset','cc_deep_dark'];
  const key = id => id.replace(/_([a-z])/g, (_, letter) => letter.toUpperCase());
  const defaults = style => ({ shakeA0: style === 'SMOOTH' ? 3 : style === 'HYBRID' ? 5 : 8, shakeOmega: 15, shakeK: 3,
    bouncyAmplitude: 40, dissolvePct: 30, skewS0: 10, squishScaleYMin: .85, squishScaleXMax: 1.18, opticsK0: .08, stretchScale: 1.4,
    zoomScaleStart: 1, zoomScaleEnd: 1.15, zoomBeatOffsetFrames: 1, flickerAmplitude: .15, flickerFrequencyHz: 12, exposureFlashPeak: .5,
    echoAlpha: .3, echoKDepth: 3, tintROffset: 0, tintGOffset: 0, tintBOffset: 0, vignetteStrength: .3, scanlinesOpacity: .15,
    warpBubbleAmplitude: .5, warpBubbleFrequency: 1.2, waveWarpHeight: 280, waveWarpSpeed: 20, slideShakePixels: 100 });
  const state = { calls: [], audioPicks: 0, rejectRender: null, finishRender: null, renderCount: 0,
    videoDuration: 2, mediaVersion: 1, pauseSceneAnalysis: false, rejectSceneAnalysis: null, finishSceneAnalysis: null, staleProgress: () => {
    for (const handler of events.values()) if (handler.event === 'render-progress') callbacks.get(handler.callback)({ payload: {jobId:'stale',phase:'SAMPLING',percent:88,message:'STALE'} });
  } };
  window.__UI_TEST__ = state;
  window.__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
    transformCallback: callback => { callbacks.set(++callbackId, callback); return callbackId; },
    unregisterCallback: id => callbacks.delete(id), convertFileSrc: file => file,
    invoke: async (command, args = {}) => {
      state.calls.push({ command, args });
      if (command === 'get_app_version') return '1.0.5';
      if (command === 'plugin:event|listen') { events.set(callbackId, {event:args.event,callback:args.handler}); return callbackId; }
      if (command === 'plugin:event|unlisten') { events.delete(args.eventId); return; }
      if (command === 'plugin:window|is_maximized') return false;
      if (command === 'cmd_get_style_defaults') return defaults(args.style);
      if (command === 'cmd_get_style_preset') {
        await new Promise(resolve => setTimeout(resolve, args.style === 'SMOOTH' ? 80 : 5));
        return { effects: { ...Object.fromEntries(effects.map(id => [key(id), !['scanlines','transitions','cc_deep_dark'].includes(id)])),
          oneFramers: args.fullFx, tint: args.fullFx, vignette: args.fullFx, ccDeepDark: false, antiFlash: false, echoTrail:false }, params: defaults(args.style) };
      }
      if (command === 'get_effect_previews') {
        const creative = ['time_slice', 'ink_pulse', 'prism_drift'];
        const beta = {
          focus_pull: 'A moving sharp focus plane softens the rest of the frame.',
          relief_light: 'Raking light brings raised image details into relief.',
          paper_gate: 'A torn-paper aperture reveals the image on accented cuts.',
        };
        return [...effects, ...creative, ...Object.keys(beta)].map(id => ({id,
          name: creative.includes(id) || id in beta ? id.replaceAll('_', ' ').toUpperCase() : id.toUpperCase(),
          category: id in beta ? 'CREATIVE BETA' : creative.includes(id) ? 'CREATIVE' : 'TEST', description:beta[id] || 'Preview',
          previewDataUrl:'data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7'}));
      }
      if (command === 'pick_files') return ['C:/test/source.mp4'];
      if (command === 'pick_file') return `C:/test/${++state.audioPicks === 1 ? 'drums' : 'audio'}.wav`;
      if (command === 'media_fingerprints') return args.paths.map(path => `${path}:1:${state.mediaVersion}`);
      if (command === 'probe_media') return {duration:args.filePath.endsWith('.mp4') ? state.videoDuration : 2,width:1920,height:1080,fps:30,audioChannels:2,audioSampleRate:44100};
      if (command === 'get_scene_clips') return [0,2,4].map((index, position) => ({index, sourceId:0, sourceDuration:12,
        startTime:position*4,endTime:position*4+3,duration:2.75,thumbnail:'data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7',
        usableRanges:[{sourceId:0,start:position*4,duration:.75},{sourceId:0,start:position*4+1,duration:2}]}));
      if (command === 'prepare_scene_selection') {
        const prepared = args.selections.map(scene => ({sceneId:scene.sceneId,sourceId:scene.sourceId,sourceStart:scene.start,sourceEnd:scene.end,
          fingerprint:'test',fps:30,fragments:[{sourceId:scene.sourceId,start:scene.start,duration:scene.end-scene.start}],motion:[],actions:[]}));
        if (state.pauseSceneAnalysis) return new Promise((resolve, reject) => {
          state.rejectSceneAnalysis = reject;
          state.finishSceneAnalysis = () => { state.rejectSceneAnalysis = null; state.finishSceneAnalysis = null; resolve(prepared); };
        });
        return prepared;
      }
      if (command === 'detect_beats') return {beats:[0,.5,1,1.5],downbeats:[0,1],bpm:120};
      if (command === 'generate_plan') return JSON.stringify({segments:[],one_framers:[],transitions:[],style:args.style,fps:args.fps,aspect:{w:args.aspectW,h:args.aspectH},target_duration:2,loops:0,export:args.exportConfig});
      if (command === 'save_plan') return 'C:/test/project.json';
      if (command === 'run_render_pipeline') return new Promise((resolve, reject) => {
        state.renderCount++;
        state.rejectRender = () => reject('Render cancelled by user');
        state.finishRender = () => resolve({outputPath:`C:/test/render-${state.renderCount}.mp4`,renderTimeSecs:.2,fileSizeMb:1,
          targetFps:16,effectsCount:0,actualFps:16,decodedBytes:0,peakCacheBytes:0,decodeTimeSecs:0,samplingTimeSecs:0,
          finalizationTimeSecs:0,peakFrameRamBytes:0,frameCacheHits:0,frameCacheMisses:0});
      });
      if (command === 'cancel_render') {
        if (state.rejectSceneAnalysis) { state.rejectSceneAnalysis('Analysis cancelled by user'); state.rejectSceneAnalysis = null; state.finishSceneAnalysis = null; }
        return;
      }
      if (command === 'plugin:updater|check') return null;
      return null;
    },
  };
}

let browser;
try {
  await waitFor('http://127.0.0.1:5187', server);
  browser = await chromium.launch({ headless: true, channel: process.env.CIA_BROWSER_CHANNEL || undefined });
  const page = await browser.newPage({ viewport: { width: 960, height: 580 } });
  const errors = []; page.on('pageerror', error => errors.push(error.message));
  await page.addInitScript(mockNative);
  await page.goto('http://127.0.0.1:5187'); await page.evaluate(() => document.fonts.ready);
  await page.locator('.remap-drop-zone').first().waitFor();
  const selectors = ['.titlebar', '.tab-bar', '.content-area', '.remap-drop-zone'];
  const geometry = async () => Object.fromEntries(await Promise.all(selectors.map(async selector => [selector, await page.locator(selector).first().boundingBox()])));
  const after = await geometry();
  await page.screenshot({path:resolve(output,'after.png')});
  if (process.env.CIA_UI_BASELINE) {
    const baseline = process.env.CIA_UI_BASELINE;
    baselineServer = spawn(process.execPath, [resolve(baseline,'node_modules/vite/bin/vite.js'), '--host','127.0.0.1','--port','5188'], {cwd:baseline,stdio:'pipe',windowsHide:true});
    collectLogs(baselineServer);
    await waitFor('http://127.0.0.1:5188', baselineServer); await page.goto('http://127.0.0.1:5188'); await page.evaluate(() => document.fonts.ready);
    const before = await geometry(); assert.deepEqual(after, before, 'Initial UI layout changed');
    await page.screenshot({path:resolve(output,'before.png')});
    writeFileSync(resolve(output,'layout.json'), JSON.stringify({before,after},null,2));
    await page.goto('http://127.0.0.1:5187');
  }
  for (const zone of ['scene','drums','audio']) await page.locator(`.remap-drop-zone[data-zone="${zone}"]`).click();
  await page.getByRole('button', {name:/CONTINUE/}).click();
  await page.getByRole('button', {name:'HARD',exact:false}).waitFor();
  const portraitRatio = page.getByRole('button', {name:'3:4',exact:true});
  assert.equal(await portraitRatio.evaluate(button => button.classList.contains('active')), true);
  await page.getByRole('button', {name:'1:1',exact:true}).click();
  assert.equal(await portraitRatio.evaluate(button => button.classList.contains('active')), false);
  await portraitRatio.click();
  const creativeKeys = ['timeSlice', 'inkPulse', 'prismDrift', 'focusPull', 'reliefLight', 'paperGate'];
  const creativeInputs = ['time-slice', 'ink-pulse', 'prism-drift', 'focus-pull', 'relief-light', 'paper-gate']
    .map(id => page.locator(`#creative-${id}`));
  const detailsButton = page.getByRole('button', {name:'DETAILS', exact:true});
  const openDetails = () => detailsButton.click();
  const closeDetails = () => page.getByRole('button', {name:'APPLY & CLOSE', exact:true}).click();
  const creativeState = async () => {
    const wasOpen = await page.locator('.details-modal-card').count() > 0;
    if (!wasOpen) await openDetails();
    const state = Object.fromEntries(await Promise.all(creativeInputs.map(async (input, index) => [creativeKeys[index], await input.isChecked()])));
    if (!wasOpen) await closeDetails();
    return state;
  };
  const defaultCreative = {timeSlice:true,inkPulse:true,prismDrift:true,focusPull:true,reliefLight:true,paperGate:true};
  const allCreative = Object.fromEntries(creativeKeys.map(key => [key, true]));
  const noCreative = Object.fromEntries(creativeKeys.map(key => [key, false]));
  assert.equal(await page.locator('.creative-mix').count(), 0);
  assert.equal(await page.locator('[id^="creative-"]').count(), 0, 'Creative toggles belong only in Details');
  await openDetails();
  assert.deepEqual(await creativeState(), defaultCreative);
  for (const input of creativeInputs) assert.equal(await input.evaluate(input => Boolean(input.closest('.effect-card'))), true);
  const totalEffects = await page.locator('.effect-card').count();
  assert.equal(totalEffects, 24);
  assert.equal(await page.locator('.effect-category-pill').filter({hasText:'CREATIVE BETA'}).count(), 3);
  await page.getByRole('button', {name:'DESELECT ALL', exact:true}).click();
  assert.equal(await page.locator('.effects-count-badge').textContent(), `0 / ${totalEffects} ACTIVE`);
  assert.deepEqual(await creativeState(), noCreative);
  await page.getByRole('button', {name:'SELECT ALL', exact:true}).click();
  assert.equal(await page.locator('.effects-count-badge').textContent(), `${totalEffects} / ${totalEffects} ACTIVE`);
  assert.deepEqual(await creativeState(), allCreative);
  await page.getByRole('button', {name:'RESET TO STYLE',exact:true}).click();
  await page.waitForTimeout(50);
  assert.equal(await page.locator('.effects-count-badge').textContent(), `21 / ${totalEffects} ACTIVE`);
  assert.deepEqual(await creativeState(), defaultCreative);
  // Established creative options remain independent of the additional effects.
  for (let mask = 1; mask <= 7; mask++) {
    const expected = { ...defaultCreative };
    for (let index = 0; index < 3; index++) {
      const enabled = Boolean(mask & (1 << index));
      expected[creativeKeys[index]] = enabled;
      await creativeInputs[index].setChecked(enabled);
    }
    assert.deepEqual(await creativeState(), expected);
  }
  // Every beta combination preserves the established creative preferences.
  for (let mask = 1; mask <= 7; mask++) {
    const expected = { ...defaultCreative };
    for (let index = 3; index < creativeInputs.length; index++) {
      const enabled = Boolean(mask & (1 << (index - 3)));
      expected[creativeKeys[index]] = enabled;
      await creativeInputs[index].setChecked(enabled);
    }
    assert.deepEqual(await creativeState(), expected);
  }
  const timeSliceCard = page.locator('.effect-card[data-effect-id="time_slice"]');
  await timeSliceCard.focus();
  await page.keyboard.press('Enter');
  assert.equal(await creativeInputs[0].isChecked(), false);
  await page.keyboard.press('Space');
  assert.equal(await creativeInputs[0].isChecked(), true);
  await timeSliceCard.click();
  assert.equal(await creativeInputs[0].isChecked(), false);
  await timeSliceCard.click();
  assert.equal(await creativeInputs[0].isChecked(), true);
  await creativeInputs[0].focus();
  await page.keyboard.press('Space');
  assert.equal(await creativeInputs[0].isChecked(), false);
  await page.keyboard.press('Space');
  assert.equal(await creativeInputs[0].isChecked(), true);
  await creativeInputs[1].uncheck();
  const focusCard = page.locator('.effect-card[data-effect-id="focus_pull"]');
  await focusCard.focus();
  await page.keyboard.press('Enter');
  assert.equal(await creativeInputs[3].isChecked(), false);
  await page.keyboard.press('Space');
  assert.equal(await creativeInputs[3].isChecked(), true);
  await focusCard.click();
  assert.equal(await creativeInputs[3].isChecked(), false);
  await focusCard.click();
  assert.equal(await creativeInputs[3].isChecked(), true);
  await creativeInputs[3].focus();
  await page.keyboard.press('Space');
  assert.equal(await creativeInputs[3].isChecked(), false);
  await page.keyboard.press('Space');
  assert.equal(await creativeInputs[3].isChecked(), true);
  const customCreative = { ...allCreative, inkPulse:false };
  await creativeInputs[3].scrollIntoViewIfNeeded();
  await page.screenshot({path:resolve(output,'creative-details.png')});
  await closeDetails();
  // A slower old-style response must never replace the current preset.
  await page.locator('.style-card').filter({hasText:'SMOOTH'}).click();
  await page.locator('.style-card').filter({hasText:'HYBRID'}).click();
  await page.waitForTimeout(150);
  assert.deepEqual(await creativeState(), customCreative);
  await page.locator('#toggle-full-fx').click();
  await page.locator('#toggle-full-fx').click();
  await page.waitForTimeout(50);
  assert.deepEqual(await creativeState(), customCreative);
  await page.screenshot({path:resolve(output,'creative-settings.png')});
  await openDetails();
  await page.getByRole('button', {name:'RESET TO STYLE',exact:true}).click();
  await page.waitForTimeout(50);
  assert.equal(await page.locator('#cp-shake-a0').inputValue(), '5');
  const toggle = page.locator('.effect-card').filter({hasText:'ONE_FRAMERS'});
  await toggle.click();
  await page.getByRole('button', {name:/CLOSE/}).last().click();
  assert.deepEqual(await creativeState(), defaultCreative);
  await page.getByRole('button', {name:/RUN PROCESS/}).click();
  await page.getByRole('button', {name:'CANCEL',exact:true}).waitFor();
  const request = await page.evaluate(() => window.__UI_TEST__.calls.findLast(call => call.command === 'generate_plan'));
  assert.equal(request.args.aspectW, 1080);
  assert.equal(request.args.aspectH, 1440);
  assert.equal(request.args.effectOverrides.oneFramers, false);
  assert.equal('one_framers' in request.args.effectOverrides, false);
  assert.equal(request.args.smartSceneFlow, true);
  assert.equal(request.args.noRepetitiveScene, false);
  assert.deepEqual(request.args.creativeOptions, defaultCreative);
  assert.equal(creativeKeys.some(key => key in request.args.effectOverrides), false);
  await page.evaluate(() => window.__UI_TEST__.staleProgress());
  assert.equal(await page.locator('.render-percent').textContent(), '0%');
  await page.getByRole('button', {name:'CANCEL',exact:true}).click();
  const cancel = page.getByRole('button', {name:'CANCELLING...',exact:true}); await cancel.waitFor(); assert.equal(await cancel.isDisabled(), true);
  assert.equal(await page.getByRole('button', {name:/RUN PROCESS/}).count(), 0);
  await page.evaluate(() => window.__UI_TEST__.rejectRender());
  await page.getByRole('button', {name:/RUN PROCESS/}).waitFor();
  // Completed output actions: open the exact file and start rerendering directly.
  await page.getByRole('button', {name:/RUN PROCESS/}).click();
  await page.waitForFunction(() => window.__UI_TEST__.renderCount === 2);
  await page.evaluate(() => window.__UI_TEST__.finishRender());
  await page.getByRole('button', {name:'OPEN FILE',exact:true}).click();
  const opened = await page.evaluate(() => window.__UI_TEST__.calls.findLast(call => call.command === 'open_output_file'));
  assert.equal(opened.args.path, 'C:/test/render-2.mp4');
  const firstJob = await page.evaluate(() => window.__UI_TEST__.calls.findLast(call => call.command === 'run_render_pipeline').args.jobId);
  await page.getByRole('button', {name:'RENDER AGAIN',exact:true}).click();
  await page.waitForFunction(() => window.__UI_TEST__.renderCount === 3);
  assert.equal(await page.getByRole('button', {name:/RUN PROCESS/}).count(), 0);
  const nextJob = await page.evaluate(() => window.__UI_TEST__.calls.findLast(call => call.command === 'run_render_pipeline').args.jobId);
  assert.notEqual(firstJob, nextJob);
  await page.evaluate(() => window.__UI_TEST__.finishRender());
  await page.getByRole('button', {name:'OPEN FILE',exact:true}).waitFor();
  assert.equal(await page.evaluate(() => window.__UI_TEST__.calls.filter(call => call.command === 'prepare_scene_selection').length), 0);
  await page.getByRole('button', {name:'Back',exact:true}).click();
  await page.evaluate(() => { window.__UI_TEST__.videoDuration = 12; window.__UI_TEST__.mediaVersion = 2; });
  await page.getByRole('button', {name:/CONTINUE/}).click();
  await page.locator('.long-video-card').filter({hasText:'SCENEPACK'}).click();
  const galleryOption = page.locator('#scene-no-repeat-gallery');
  const gallerySmart = page.locator('#scene-smart-flow-gallery');
  const settingsOption = page.locator('#scene-no-repeat-settings');
  const settingsSmart = page.locator('#scene-smart-flow-settings');
  await galleryOption.waitFor();
  assert.equal(await galleryOption.isChecked(), false);
  assert.equal(await gallerySmart.isChecked(), true);
  await galleryOption.check();
  assert.equal(await gallerySmart.isChecked(), false);
  await gallerySmart.check();
  assert.equal(await galleryOption.isChecked(), false);
  await page.getByRole('button', {name:/APPLY & CONTINUE/}).click();
  assert.equal(await settingsOption.isChecked(), false);
  assert.equal(await settingsSmart.isChecked(), true);
  await settingsOption.check();
  assert.equal(await settingsSmart.isChecked(), false);
  await page.getByRole('button', {name:/SELECT CLIPS/}).click();
  assert.equal(await galleryOption.isChecked(), true);
  assert.equal(await gallerySmart.isChecked(), false);
  // Leaving both options unchecked retains classic ScenePack behavior.
  await galleryOption.uncheck();
  assert.equal(await gallerySmart.isChecked(), false);
  await page.getByRole('button', {name:/APPLY & CONTINUE/}).click();
  assert.equal(await settingsOption.isChecked(), false);
  assert.equal(await settingsSmart.isChecked(), false);
  await page.getByRole('button', {name:'RENDER AGAIN',exact:true}).click();
  await page.waitForFunction(() => window.__UI_TEST__.renderCount === 4);
  const classicPlan = await page.evaluate(() => window.__UI_TEST__.calls.findLast(call => call.command === 'generate_plan'));
  assert.equal(classicPlan.args.longVideoMode, 'scenepack');
  assert.equal(classicPlan.args.noRepetitiveScene, false);
  assert.equal(classicPlan.args.smartSceneFlow, false);
  await page.evaluate(() => window.__UI_TEST__.finishRender());
  await page.getByRole('button', {name:'OPEN FILE',exact:true}).waitFor();
  await settingsSmart.check();
  assert.equal(await settingsOption.isChecked(), false);
  await page.getByRole('button', {name:/SELECT CLIPS/}).click();
  assert.equal(await gallerySmart.isChecked(), true);
  assert.equal(await galleryOption.isChecked(), false);
  await page.getByRole('button', {name:'DESELECT ALL',exact:true}).click();
  await page.locator('.scene-card').filter({hasText:'CLIP #5'}).click();
  await page.screenshot({path:resolve(output,'scene-gallery.png')});
  await page.getByRole('button', {name:/APPLY & CONTINUE/}).click();
  // Cancellation while source analysis is still running must prevent render launch.
  await openDetails();
  await creativeInputs[1].uncheck();
  await creativeInputs[3].check();
  await creativeInputs[5].check();
  await closeDetails();
  const capturedCreative = { ...defaultCreative, inkPulse:false, focusPull:true, paperGate:true };
  await page.evaluate(() => { window.__UI_TEST__.pauseSceneAnalysis = true; });
  await page.getByRole('button', {name:'RENDER AGAIN',exact:true}).click();
  await page.waitForFunction(() => Boolean(window.__UI_TEST__.rejectSceneAnalysis));
  await page.getByRole('button', {name:'CANCEL',exact:true}).click();
  await page.getByRole('button', {name:/RUN PROCESS/}).waitFor();
  assert.equal(await page.evaluate(() => window.__UI_TEST__.renderCount), 4);
  await page.getByRole('button', {name:/RUN PROCESS/}).click();
  await page.waitForFunction(() => Boolean(window.__UI_TEST__.finishSceneAnalysis));
  assert.equal(await settingsSmart.evaluate(input => Boolean(input.closest('[inert]'))), true);
  assert.equal(await detailsButton.evaluate(button => Boolean(button.closest('[inert]'))), true);
  // Simulate a later preference update; an in-flight render keeps its captured policy.
  await settingsOption.evaluate(input => input.click());
  assert.equal(await settingsSmart.isChecked(), false);
  await detailsButton.evaluate(button => button.click());
  for (const input of creativeInputs) await input.evaluate(input => input.click());
  assert.deepEqual(await creativeState(), Object.fromEntries(creativeKeys.map(key => [key, !capturedCreative[key]])));
  await closeDetails();
  await page.evaluate(() => window.__UI_TEST__.finishSceneAnalysis());
  await page.waitForFunction(() => window.__UI_TEST__.renderCount === 5);
  const analyzed = await page.evaluate(() => window.__UI_TEST__.calls.findLast(call => call.command === 'prepare_scene_selection'));
  assert.deepEqual(analyzed.args.selections, [{sceneId:4,sourceId:0,start:8,end:11}]);
  const scenePlan = await page.evaluate(() => window.__UI_TEST__.calls.findLast(call => call.command === 'generate_plan'));
  assert.equal(scenePlan.args.noRepetitiveScene, false);
  assert.equal(scenePlan.args.smartSceneFlow, true);
  assert.deepEqual(scenePlan.args.creativeOptions, capturedCreative);
  assert.equal(scenePlan.args.preparedScenes[0].sceneId, 4);
  assert.deepEqual(scenePlan.args.sourceRanges, [{sourceId:0,start:8,duration:.75},{sourceId:0,start:9,duration:2}]);
  await page.evaluate(() => window.__UI_TEST__.finishRender());
  await page.getByRole('button', {name:'OPEN FILE',exact:true}).waitFor();
  await page.getByRole('button', {name:'OPEN FILE',exact:true}).scrollIntoViewIfNeeded();
  await page.locator('.toast').waitFor({state:'hidden'});
  await page.screenshot({path:resolve(output,'scene-render-done.png')});
  assert.deepEqual(errors, []);
  console.log('UI layout, presets, independent creative combinations, shared scene options, render snapshots, stable selection IDs, output actions, rerender and cancellation checks passed.');
} finally {
  if (browser) await browser.close();
  server.kill(); if (baselineServer) baselineServer.kill();
}
