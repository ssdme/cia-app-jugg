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
    videoDuration: 2, mediaVersion: 1, pauseSceneAnalysis: false, rejectSceneAnalysis: null, staleProgress: () => {
    for (const handler of events.values()) if (handler.event === 'render-progress') callbacks.get(handler.callback)({ payload: {jobId:'stale',phase:'SAMPLING',percent:88,message:'STALE'} });
  } };
  window.__UI_TEST__ = state;
  window.__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
    transformCallback: callback => { callbacks.set(++callbackId, callback); return callbackId; },
    unregisterCallback: id => callbacks.delete(id), convertFileSrc: file => file,
    invoke: async (command, args = {}) => {
      state.calls.push({ command, args });
      if (command === 'get_app_version') return '1.0.3';
      if (command === 'plugin:event|listen') { events.set(callbackId, {event:args.event,callback:args.handler}); return callbackId; }
      if (command === 'plugin:event|unlisten') { events.delete(args.eventId); return; }
      if (command === 'plugin:window|is_maximized') return false;
      if (command === 'cmd_get_style_defaults') return defaults(args.style);
      if (command === 'cmd_get_style_preset') {
        await new Promise(resolve => setTimeout(resolve, args.style === 'SMOOTH' ? 80 : 5));
        return { effects: { ...Object.fromEntries(effects.map(id => [key(id), !['scanlines','transitions','cc_deep_dark'].includes(id)])),
          oneFramers: args.fullFx, tint: args.fullFx, vignette: args.fullFx, ccDeepDark: false, antiFlash: false, echoTrail:false }, params: defaults(args.style) };
      }
      if (command === 'get_effect_previews') return effects.map(id => ({id, name:id.toUpperCase(), category:'TEST', description:'Preview', previewDataUrl:'data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7'}));
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
        if (state.pauseSceneAnalysis) return new Promise((_, reject) => { state.rejectSceneAnalysis = reject; });
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
        if (state.rejectSceneAnalysis) { state.rejectSceneAnalysis('Analysis cancelled by user'); state.rejectSceneAnalysis = null; }
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
  // A slower old-style response must never replace the current preset.
  await page.locator('.style-card').filter({hasText:'SMOOTH'}).click();
  await page.locator('.style-card').filter({hasText:'HYBRID'}).click();
  await page.waitForTimeout(150);
  await page.getByRole('button', {name:/DETAILS/}).click();
  assert.equal(await page.locator('#cp-shake-a0').inputValue(), '5');
  const toggle = page.locator('.effect-card').filter({hasText:'ONE_FRAMERS'});
  await toggle.click();
  await page.getByRole('button', {name:/CLOSE/}).last().click();
  await page.getByRole('button', {name:/RUN PROCESS/}).click();
  await page.getByRole('button', {name:'CANCEL',exact:true}).waitFor();
  const request = await page.evaluate(() => window.__UI_TEST__.calls.findLast(call => call.command === 'generate_plan'));
  assert.equal(request.args.effectOverrides.oneFramers, false);
  assert.equal('one_framers' in request.args.effectOverrides, false);
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
  await galleryOption.waitFor();
  assert.equal(await galleryOption.isChecked(), true);
  await galleryOption.uncheck();
  await page.getByRole('button', {name:/APPLY & CONTINUE/}).click();
  assert.equal(await page.locator('#scene-no-repeat-settings').isChecked(), false);
  await page.locator('#scene-no-repeat-settings').check();
  await page.getByRole('button', {name:/SELECT CLIPS/}).click();
  assert.equal(await galleryOption.isChecked(), true);
  await page.getByRole('button', {name:'DESELECT ALL',exact:true}).click();
  await page.locator('.scene-card').filter({hasText:'CLIP #5'}).click();
  await page.screenshot({path:resolve(output,'scene-gallery.png')});
  await page.getByRole('button', {name:/APPLY & CONTINUE/}).click();
  // Cancellation while source analysis is still running must prevent render launch.
  await page.evaluate(() => { window.__UI_TEST__.pauseSceneAnalysis = true; });
  await page.getByRole('button', {name:'RENDER AGAIN',exact:true}).click();
  await page.waitForFunction(() => Boolean(window.__UI_TEST__.rejectSceneAnalysis));
  await page.getByRole('button', {name:'CANCEL',exact:true}).click();
  await page.getByRole('button', {name:/RUN PROCESS/}).waitFor();
  assert.equal(await page.evaluate(() => window.__UI_TEST__.renderCount), 3);
  await page.evaluate(() => { window.__UI_TEST__.pauseSceneAnalysis = false; });
  await page.getByRole('button', {name:/RUN PROCESS/}).click();
  await page.waitForFunction(() => window.__UI_TEST__.renderCount === 4);
  const analyzed = await page.evaluate(() => window.__UI_TEST__.calls.findLast(call => call.command === 'prepare_scene_selection'));
  assert.deepEqual(analyzed.args.selections, [{sceneId:4,sourceId:0,start:8,end:11}]);
  const scenePlan = await page.evaluate(() => window.__UI_TEST__.calls.findLast(call => call.command === 'generate_plan'));
  assert.equal(scenePlan.args.noRepetitiveScene, true);
  assert.equal(scenePlan.args.preparedScenes[0].sceneId, 4);
  assert.deepEqual(scenePlan.args.sourceRanges, [{sourceId:0,start:8,duration:.75},{sourceId:0,start:9,duration:2}]);
  await page.evaluate(() => window.__UI_TEST__.finishRender());
  await page.getByRole('button', {name:'OPEN FILE',exact:true}).waitFor();
  await page.getByRole('button', {name:'OPEN FILE',exact:true}).scrollIntoViewIfNeeded();
  await page.locator('.toast').waitFor({state:'hidden'});
  await page.screenshot({path:resolve(output,'scene-render-done.png')});
  assert.deepEqual(errors, []);
  console.log('UI layout, presets, shared scene options, stable selection IDs, output actions, rerender and cancellation checks passed.');
} finally {
  if (browser) await browser.close();
  server.kill(); if (baselineServer) baselineServer.kill();
}
