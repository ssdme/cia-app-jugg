<script>
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { check } from '@tauri-apps/plugin-updater';
  import { relaunch } from '@tauri-apps/plugin-process';
  import { onMount } from 'svelte';
  import GlowSlider from './GlowSlider.svelte';
  import ProjectMark from './ProjectMark.svelte';
  import appLogo from '../src-tauri/icons/128x128@2x.png';

  const appWindow =
    typeof window !== 'undefined' && window.__TAURI_INTERNALS__
      ? getCurrentWindow()
      : null;

  let activePage = $state('remap'); // 'remap' | 'settings' | 'about'
  let toast = $state({ show: false, message: '', type: 'info' });
  let appVersion = $state('1.0.2');
  let discordCopyFeedback = $state(false);

  // Auto-Updater State
  let updateState = $state('idle'); // 'idle' | 'checking' | 'available' | 'downloading' | 'ready' | 'error' | 'up-to-date'
  let availableUpdate = $state(null);
  let updateDownloadedBytes = $state(0);
  let updateContentLength = $state(0);
  let updateErrorMessage = $state('');
  let showUpdateModal = $state(false);

  // Time Remap State (3 drop zones)
  let scenePath = $state('');
  let sceneError = $state('');
  let drumsPath = $state('');
  let drumsError = $state('');
  let audioPath = $state('');
  let audioError = $state('');
  let hoveredZone = $state(null);

  // Probe & Beat Detection State
  let sceneInfo = $state(null);
  let drumsInfo = $state(null);
  let audioInfo = $state(null);
  let beats = $state(null);
  let downbeats = $state(null);
  let bpm = $state(null);
  let isAnalyzing = $state(false);
  let analyzingStep = $state('');

  // Settings Configuration State (T4.5)
  let selectedStyle = $state('HARD'); // 'HARD' | 'SMOOTH' | 'HYBRID'
  let fpsValue = $state(16); // min 12, max 60, step 1, default 16
  let selectedAspectRatio = $state('1:1'); // '16:9' | '9:16' | '1:1' | 'CUSTOM' (default 1:1)
  let customWidth = $state(1080);
  let customHeight = $state(1080);
  let customArError = $state('');
  let borderless = $state(true); // always true internally
  let echoTrailEnabled = $state(false); // T11 echo/trail, default OFF
  let fullFxEnabled = $state(true);    // T13 full fx, default ON
  let renderStats = $state(null);      // T16 render logs & stats

  // T19 Export Options State
  let selectedCodec = $state('H.264'); // 'H.264' | 'H.265' | 'VP9'
  let bitrateValue = $state(12); // min 5, max 50, step 1, default 12
  let selectedFormat = $state('MP4'); // 'MP4' | 'MKV' | 'WEBM'

  // DUMPER Page State
  let dumperVideoPath = $state('');
  let dumperVideoError = $state('');
  let dumperProgress = $state(null); // { phase: 'SCENES' | 'BEATS' | 'PROFILES', percent: 0..100, message: '' }
  let isDumping = $state(false);
  let dumperResult = $state(null);

  // COMPOSITION Page State
  let compCharacterPath = $state('');
  let compCharacterError = $state('');
  let compBackgroundPath = $state('');
  let compBackgroundError = $state('');
  let isSegmenting = $state(false);
  let compResult = $state(null);
  let compGpuError = $state('');
  let isRenderingComposition = $state(false);
  let compRenderProgress = $state(null);
  let compRenderResult = $state(null);
  let isRenderingPreview = $state(false);
  let compPreviewProgress = $state(null);
  let compPreviewResult = $state(null);
  let compParallaxStrength = $state(0.5);
  let compBeatPunchIntensity = $state(0.6);
  let compLightWrapIntensity = $state(0.5);
  let compChromaticAberration = $state(0.3);
  let compImpactBlurStrength = $state(0.5);
  let compOps = $state([
    {
      id: 'drop_shadow',
      name: 'Drop Shadow',
      op_type: 'drop_shadow',
      blend_mode: 'Multiply',
      opacity: 0.60,
      mask_by_alpha: false,
      enabled: true,
      params: { offsetX: 12.0, offsetY: 16.0, blurRadius: 14.0 }
    },
    {
      id: 'light_wrap',
      name: 'Light Wrap',
      op_type: 'light_wrap',
      blend_mode: 'Screen',
      opacity: 0.55,
      mask_by_alpha: true,
      enabled: true,
      params: { blurRadius: 20.0, edgeWidth: 10.0 }
    },
    {
      id: 'tint_raccord',
      name: 'Tint de Raccord',
      op_type: 'tint',
      blend_mode: 'Multiply',
      opacity: 0.07,
      mask_by_alpha: true,
      enabled: true,
      params: {}
    },
    {
      id: 'rim_light',
      name: 'Rim Light',
      op_type: 'rim_light',
      blend_mode: 'Add',
      opacity: 0.65,
      mask_by_alpha: true,
      enabled: true,
      params: { color: [220.0, 240.0, 255.0] }
    }
  ]);

  // T17 Generic Effect Preview and Toggleable Overrides
  let showDetailsModal = $state(false);
  let availableEffects = $state([]);
  let hoveredPreview = $state(null);
  let hoverPos = $state({ x: 0, y: 0 });

  function getDefaultOverrides(style, fullFx) {
    const isSmooth = style === 'SMOOTH';
    const isHybrid = style === 'HYBRID';
    const isHard = !isSmooth && !isHybrid;

    return {
      shakes: true,
      zoom: true,
      flicker: true,
      oneFramers: fullFx && (isHard || isHybrid),
      transitions: true,
      tint: fullFx,
      vignette: fullFx,
      scanlines: fullFx,
      echoTrail: echoTrailEnabled,
      exposureFlash: fullFx && isHard,
      bouncyShake: isHard || isHybrid,
      dissolveShake: isHard || isHybrid,
      skewShake: isHard || isHybrid,
      squishPop: isHard || isHybrid,
      opticsBounce: isHard || isHybrid,
      buildupChain: true,
      warpStretch: isHard || isHybrid,
      zoomBeatOffset: true,
    };
  }

  let effectOverrides = $state(getDefaultOverrides('HARD', true));

  // T18 Custom Params â€” per-effect numeric overrides
  let customParams = $state(null); // null = use style preset defaults

  // Fetch style defaults from Rust when style changes or when opening DETAILS
  async function loadCustomParamsDefaults() {
    try {
      const defaults = await invoke('cmd_get_style_defaults', { style: selectedStyle });
      if (customParams === null) {
        customParams = defaults;
      }
    } catch (e) {
      console.error('[T18] Failed to load style defaults:', e);
    }
  }

  async function handleResetToStyleDefaults() {
    try {
      customParams = await invoke('cmd_get_style_defaults', { style: selectedStyle });
    } catch (e) {
      console.error('[T18] Failed to reset custom params:', e);
    }
  }

  function handleToggleFullFx() {
    fullFxEnabled = !fullFxEnabled;
    effectOverrides = getDefaultOverrides(selectedStyle, fullFxEnabled);
  }

  function handleSelectAllEffects() {
    const next = { ...effectOverrides };
    for (const key of Object.keys(next)) {
      next[key] = true;
    }
    effectOverrides = next;
  }

  function handleDeselectAllEffects() {
    const next = { ...effectOverrides };
    for (const key of Object.keys(next)) {
      next[key] = false;
    }
    effectOverrides = next;
  }

  function handleResetEffectsToPreset() {
    effectOverrides = getDefaultOverrides(selectedStyle, fullFxEnabled);
  }

  const STYLE_OPTIONS = [
    {
      id: 'HARD',
      title: 'HARD',
      desc: 'fast snaps, one-framers, strong shake'
    },
    {
      id: 'SMOOTH',
      title: 'SMOOTH',
      desc: 'soft curves, subtle shake, continuous zoom'
    },
    {
      id: 'HYBRID',
      title: 'HYBRID',
      desc: 'mixed snaps and saddles, medium shake'
    }
  ];

  const ASPECT_RATIO_PRESETS = ['16:9', '9:16', '1:1', 'CUSTOM'];

  const CODEC_OPTIONS = [
    { id: 'H.264', label: 'H.264' },
    { id: 'H.265', label: 'H.265 (HEVC)' },
    { id: 'VP9', label: 'VP9' }
  ];

  const FORMAT_OPTIONS = ['MP4', 'MKV', 'WEBM'];

  const VIDEO_EXTENSIONS = ['mp4', 'mkv', 'webm', 'mov', 'avi'];
  const AUDIO_EXTENSIONS = ['mp3', 'wav', 'flac', 'm4a', 'ogg'];

  let allZonesFilled = $derived(Boolean(scenePath && drumsPath && audioPath));

  const ABOUT_LINKS = [
    { name: 'beat_this (CP-JKU)', detail: 'Beat/downbeat tracking', mark: 'beat', url: 'https://github.com/CPJKU/beat_this' },
    { name: 'ONNX Runtime', detail: 'Neural inference engine', mark: 'onnx', url: 'https://github.com/microsoft/onnxruntime' },
    { name: 'Symphonia', detail: 'Pure-Rust audio decoding', mark: 'symphonia', url: 'https://github.com/pdeljanov/Symphonia' },
    { name: 'mp4', detail: 'Pure-Rust media probing', mark: 'mp4', url: 'https://github.com/alfg/mp4-rust' },
    { name: 'FFmpeg', detail: 'Media tooling', mark: 'ffmpeg', url: 'https://github.com/FFmpeg/FFmpeg' },
    { name: 'Tauri', detail: 'Desktop runtime', mark: 'tauri', url: 'https://github.com/tauri-apps/tauri' },
    { name: 'Svelte', detail: 'Interface framework', mark: 'svelte', url: 'https://github.com/sveltejs/svelte' },
    { name: 'IBM Plex', detail: 'Interface typography', mark: 'plex', url: 'https://github.com/IBM/plex' }
  ];
  const PROJECT_REPOSITORY_URL = 'https://github.com/ssdme/cia-app-jugg';

  function getFileExtension(path) {
    if (!path) return '';
    const parts = path.split('.');
    return parts.length > 1 ? parts.pop().toLowerCase() : '';
  }

  function getFileName(path) {
    if (!path) return '';
    return path.split(/[\\/]/).pop() || path;
  }

  function validateAndSetFile(zone, path) {
    if (!path) return;
    const ext = getFileExtension(path);
    if (zone === 'scene') {
      if (VIDEO_EXTENSIONS.includes(ext)) {
        scenePath = path;
        sceneError = '';
        sceneInfo = null;
      } else {
        sceneError = 'Expected: video — mp4/mkv/webm/mov/avi';
      }
    } else if (zone === 'drums') {
      if (AUDIO_EXTENSIONS.includes(ext)) {
        drumsPath = path;
        drumsError = '';
        drumsInfo = null;
        beats = null;
        downbeats = null;
        bpm = null;
      } else {
        drumsError = 'Expected: audio — mp3/wav/flac/m4a/ogg';
      }
    } else if (zone === 'audio') {
      if (AUDIO_EXTENSIONS.includes(ext)) {
        audioPath = path;
        audioError = '';
        audioInfo = null;
      } else {
        audioError = 'Expected: audio — mp3/wav/flac/m4a/ogg';
      }
    } else if (zone === 'dumper') {
      if (VIDEO_EXTENSIONS.includes(ext)) {
        dumperVideoPath = path;
        dumperVideoError = '';
        dumperResult = null;
        dumperProgress = null;
      } else {
        dumperVideoError = 'Expected: video — mp4/mkv/webm/mov/avi';
      }
    } else if (zone === 'comp-character') {
      if (ext === 'png') {
        compCharacterPath = path;
        compCharacterError = '';
        compResult = null;
        compGpuError = '';
      } else {
        compCharacterError = 'Expected: transparent PNG character image';
      }
    } else if (zone === 'comp-background') {
      if (['png', 'jpg', 'jpeg', 'webp', ...VIDEO_EXTENSIONS].includes(ext)) {
        compBackgroundPath = path;
        compBackgroundError = '';
      } else {
        compBackgroundError = 'Expected: image or video (PNG, JPG, MP4, MKV...)';
      }
    }
  }

  function clearZone(zone, event) {
    if (event) event.stopPropagation();
    if (zone === 'scene') {
      scenePath = '';
      sceneError = '';
      sceneInfo = null;
    } else if (zone === 'drums') {
      drumsPath = '';
      drumsError = '';
      drumsInfo = null;
      beats = null;
      downbeats = null;
      bpm = null;
    } else if (zone === 'audio') {
      audioPath = '';
      audioError = '';
      audioInfo = null;
    } else if (zone === 'dumper') {
      dumperVideoPath = '';
      dumperVideoError = '';
      dumperResult = null;
      dumperProgress = null;
    } else if (zone === 'comp-character') {
      compCharacterPath = '';
      compCharacterError = '';
      compResult = null;
      compGpuError = '';
    } else if (zone === 'comp-background') {
      compBackgroundPath = '';
      compBackgroundError = '';
    }
  }

  async function handlePickFile(zone, event) {
    if (event) event.stopPropagation();
    try {
      let kind = 'video';
      if (zone === 'drums' || zone === 'audio') {
        kind = 'audio';
      } else if (zone === 'comp-character') {
        kind = 'character';
      } else if (zone === 'comp-background') {
        kind = 'background';
      }
      const picked = await invoke('pick_file', { kind });
      if (picked) {
        validateAndSetFile(zone, picked);
      }
    } catch (e) {
      showToast(`Selection cancelled or error: ${e}`, 'error');
    }
  }

  async function handleContinue() {
    if (!scenePath || !drumsPath || !audioPath) return;
    isAnalyzing = true;
    try {
      analyzingStep = 'Probing scene video...';
      sceneInfo = await invoke('probe_media', { filePath: scenePath });

      analyzingStep = 'Probing drums audio...';
      drumsInfo = await invoke('probe_media', { filePath: drumsPath });

      analyzingStep = 'Probing target audio...';
      audioInfo = await invoke('probe_media', { filePath: audioPath });

      analyzingStep = 'Detecting beats with ONNX model...';
      const beatResult = await invoke('detect_beats', { audioPath: drumsPath });
      beats = beatResult.beats;
      downbeats = beatResult.downbeats;
      bpm = beatResult.bpm;

      navigateTo('settings');
    } catch (err) {
      console.error('Processing error:', err);
      const msg = typeof err === 'string' ? err : err?.message || JSON.stringify(err);
      showToast(`Analysis failed: ${msg}`, 'error');
    } finally {
      isAnalyzing = false;
      analyzingStep = '';
    }
  }

  function handleAspectRatioSelect(ar) {
    selectedAspectRatio = ar;
    if (ar === 'CUSTOM') {
      validateCustomDimensions();
    } else {
      customArError = '';
    }
  }

  function validateCustomDimensions() {
    const w = parseInt(String(customWidth), 10);
    const h = parseInt(String(customHeight), 10);
    if (isNaN(w) || w <= 0 || isNaN(h) || h <= 0) {
      customArError = 'Width and height must be integers greater than 0';
    } else {
      customArError = '';
    }
  }

  // Plan Summary State (T5)
  let planSummary = $state(null);

  // Render Execution State (T6)
  let renderState = $state('idle'); // 'idle' | 'running' | 'done' | 'error'
  let renderProgress = $state({
    phase: 'DECODING',
    percent: 0,
    currentFrame: 0,
    totalFrames: 0,
    message: ''
  });
  let renderOutputMp4 = $state('');
  let renderError = $state('');

  function handleCustomWidthInput(e) {
    customWidth = parseInt(e.target.value, 10) || 0;
    validateCustomDimensions();
  }

  function handleCustomHeightInput(e) {
    customHeight = parseInt(e.target.value, 10) || 0;
    validateCustomDimensions();
  }

  async function handleRunProcess() {
    if (selectedAspectRatio === 'CUSTOM') {
      validateCustomDimensions();
      if (customArError) {
        showToast('Please specify valid custom dimensions before running', 'error');
        return;
      }
    }

    if (!sceneInfo || !drumsInfo || !audioInfo) {
      showToast('Media metadata missing. Please re-probe sources.', 'error');
      return;
    }

    renderState = 'running';
    renderError = '';
    renderOutputMp4 = '';
    renderProgress = {
      phase: 'DECODING',
      percent: 0,
      currentFrame: 0,
      totalFrames: 0,
      message: 'Generating project plan...'
    };

    try {
      let aspectW = 1080;
      let aspectH = 1080;
      if (selectedAspectRatio === '16:9') {
        aspectW = 1920;
        aspectH = 1080;
      } else if (selectedAspectRatio === '9:16') {
        aspectW = 1080;
        aspectH = 1920;
      } else if (selectedAspectRatio === '1:1') {
        aspectW = 1080;
        aspectH = 1080;
      } else if (selectedAspectRatio === 'CUSTOM') {
        aspectW = customWidth;
        aspectH = customHeight;
      }

      const planJson = await invoke('generate_plan', {
        style: selectedStyle,
        fps: fpsValue,
        beats: beats || [],
        downbeats: downbeats || [],
        videoDuration: sceneInfo.duration,
        audioDuration: audioInfo.duration,
        aspectW,
        aspectH,
        bpm: bpm || 120.0,
        fullFx: fullFxEnabled,
        effectOverrides,
        customParams: customParams || null,
        exportConfig: {
          codec: selectedCodec,
          bitrateMbps: bitrateValue,
          format: selectedFormat,
        },
      });

      console.log('[PLAN] Generated plan:', planJson);
      const savedPath = await invoke('save_plan', { planJson });
      console.log('[PLAN] Saved project.json to:', savedPath);

      const parsed = JSON.parse(planJson);
      const hasReverse = parsed.segments.some((s) => s.effects && s.effects.reverse);
      const hasOneFramers = parsed.one_framers && parsed.one_framers.length > 0;
      const hasTransitions = (parsed.transitions && parsed.transitions.length > 0) || parsed.segments.some((s) => s.transition);
      const hasAmbiance = !!parsed.ambiance;
      planSummary = {
        segmentsCount: parsed.segments.length,
        loops: parsed.loops,
        targetDuration: parsed.target_duration,
        savedPath,
        style: parsed.style,
        fps: parsed.fps,
        aspect: `${parsed.aspect.w}x${parsed.aspect.h}`,
        motionBlur: parsed.motion_blur,
        fullFx: parsed.full_fx !== false, // default true for retrocompat
        shakes: true,
        zoom: true,
        reverse: hasReverse,
        oneFramers: hasOneFramers,
        transitions: hasTransitions,
        ambiance: hasAmbiance,
        echoTrail: echoTrailEnabled,
        export: parsed.export || {
          codec: selectedCodec,
          bitrateMbps: bitrateValue,
          format: selectedFormat,
        },
      };

      console.log('[RENDER] Launching 3-pass render pipeline...');
      const renderRes = await invoke('run_render_pipeline', {
        planJson,
        scenePath,
        audioPath,
        echoTrail: echoTrailEnabled,
      });

      console.log('[RENDER] Render completed successfully:', renderRes);
      renderStats = typeof renderRes === 'object' && renderRes !== null ? renderRes : {
        outputPath: renderRes,
        renderTimeSecs: 0,
        fileSizeMb: 0,
        targetFps: fpsValue,
        effectsCount: 0
      };
      renderOutputMp4 = renderStats.outputPath;
      renderState = 'done';
      showToast('Render completed successfully!', 'success');
    } catch (err) {
      console.error('Render process failed:', err);
      const msg = typeof err === 'string' ? err : err?.message || JSON.stringify(err);
      if (msg.includes('cancelled') || msg.includes('Cancel')) {
        renderState = 'idle';
        showToast('Render cancelled by user', 'info');
      } else {
        renderState = 'error';
        renderError = msg;
        showToast(`Render failed: ${msg}`, 'error');
      }
    }
  }

  async function handleRenderFinalJugg() {
    if (!scenePath || !audioPath) {
      showToast('Scene video and Audio paths are required for final rendering.', 'error');
      return;
    }

    renderState = 'running';
    renderError = '';
    renderOutputMp4 = '';
    renderProgress = {
      phase: 'DECODING',
      percent: 0,
      currentFrame: 0,
      totalFrames: 0,
      message: 'Preparing final assembly stitch...'
    };

    try {
      let aspectW = 1080;
      let aspectH = 1080;
      if (selectedAspectRatio === '16:9') {
        aspectW = 1920;
        aspectH = 1080;
      } else if (selectedAspectRatio === '9:16') {
        aspectW = 1080;
        aspectH = 1920;
      } else if (selectedAspectRatio === '1:1') {
        aspectW = 1080;
        aspectH = 1080;
      } else if (selectedAspectRatio === 'CUSTOM') {
        aspectW = customWidth;
        aspectH = customHeight;
      }

      const planJson = await invoke('generate_plan', {
        style: selectedStyle,
        fps: fpsValue,
        beats: beats || [],
        downbeats: downbeats || [],
        videoDuration: sceneInfo?.duration || 10.0,
        audioDuration: audioInfo?.duration || 10.0,
        aspectW,
        aspectH,
        bpm: bpm || 120.0,
        fullFx: fullFxEnabled,
        effectOverrides,
        customParams: customParams || null,
        exportConfig: {
          codec: selectedCodec,
          bitrateMbps: bitrateValue,
          format: selectedFormat,
        },
      });

      console.log('[FINAL JUGG] Launching Final Assembly Render...');
      const renderRes = await invoke('render_final_jugg', {
        planJson,
        scenePath,
        audioPath,
        characterProjectPath: compResult?.layersJsonPath || null,
        outputPath: null,
      });

      console.log('[FINAL JUGG] Render completed successfully:', renderRes);
      renderStats = typeof renderRes === 'object' && renderRes !== null ? renderRes : {
        outputPath: renderRes,
        renderTimeSecs: 0,
        fileSizeMb: 0,
        targetFps: fpsValue,
        effectsCount: 0
      };
      renderOutputMp4 = renderStats.outputPath;
      renderState = 'done';
      showToast('Final Jugg assembly rendered successfully!', 'success');
    } catch (err) {
      console.error('Final render process failed:', err);
      const msg = typeof err === 'string' ? err : err?.message || JSON.stringify(err);
      if (msg.includes('cancelled') || msg.includes('Cancel')) {
        renderState = 'idle';
        showToast('Render cancelled by user', 'info');
      } else {
        renderState = 'error';
        renderError = msg;
        showToast(`Render failed: ${msg}`, 'error');
      }
    }
  }

  async function handleCancelRender() {
    try {
      await invoke('cancel_render');
      renderState = 'idle';
      showToast('Render cancelled', 'info');
    } catch (e) {
      console.error('Failed to cancel render:', e);
    }
  }

  async function runDumperPipeline() {
    if (!dumperVideoPath || isDumping) return;
    isDumping = true;
    dumperProgress = { phase: 'SCENES', percent: 5, message: 'Starting analysis pipeline...' };
    dumperResult = null;
    try {
      const res = await invoke('run_dump_pipeline', { videoPath: dumperVideoPath });
      dumperResult = res;
      showToast('Analysis completed successfully', 'success');
    } catch (err) {
      console.error('Dumper error:', err);
      const msg = typeof err === 'string' ? err : err?.message || JSON.stringify(err);
      showToast(`Analysis failed: ${msg}`, 'error');
    } finally {
      isDumping = false;
    }
  }

  async function handleOpenDumperFolder(path) {
    if (!path) return;
    try {
      await invoke('open_target_folder', { path });
      showToast('Opening dump folder in Explorer', 'info');
    } catch (e) {
      console.error('Failed to open folder:', e);
      showToast(`Unable to open folder: ${e}`, 'error');
    }
  }

  async function handleGenerateEditPlan() {
    if (!dumperResult) return;
    try {
      const plan = await invoke('generate_remap_plan', {
        analysis: dumperResult,
        analysisPath: dumperResult.jsonPath || null,
      });
      console.log('[DUMPER] Auto-generated remap edit plan:', plan);

      // Save plan to project.json in app_data
      const planJson = JSON.stringify(plan);
      const savedPath = await invoke('save_plan', { planJson });
      console.log('[DUMPER] Saved generated plan to:', savedPath);

      // Set JUGG inputs and options
      selectedStyle = plan.style;
      fpsValue = plan.fps;
      bpm = plan.bpm;
      if (plan.aspect) {
        if (plan.aspect.w === 1920 && plan.aspect.h === 1080) {
          selectedAspectRatio = '16:9';
        } else if (plan.aspect.w === 1080 && plan.aspect.h === 1920) {
          selectedAspectRatio = '9:16';
        } else if (plan.aspect.w === 1080 && plan.aspect.h === 1080) {
          selectedAspectRatio = '1:1';
        } else {
          selectedAspectRatio = 'CUSTOM';
          customWidth = plan.aspect.w;
          customHeight = plan.aspect.h;
        }
      }

      // Populate file drop zones with the dumped video
      if (dumperResult.source) {
        scenePath = dumperResult.source;
        drumsPath = dumperResult.source;
        audioPath = dumperResult.source;
        sceneInfo = {
          duration: plan.video_duration,
          fps: plan.fps,
          width: plan.aspect ? plan.aspect.w : 1080,
          height: plan.aspect ? plan.aspect.h : 1080,
        };
        audioInfo = {
          duration: plan.audio_duration,
          sample_rate: 48000,
          channels: 2,
        };
      }

      const hasReverse = plan.segments.some((s) => s.effects && s.effects.reverse);
      const hasOneFramers = plan.one_framers && plan.one_framers.length > 0;
      const hasTransitions = (plan.transitions && plan.transitions.length > 0) || plan.segments.some((s) => s.transition);
      const hasAmbiance = !!plan.ambiance;

      planSummary = {
        segmentsCount: plan.segments.length,
        loops: plan.loops || 1,
        targetDuration: plan.target_duration,
        savedPath,
        style: plan.style,
        fps: plan.fps,
        aspect: `${plan.aspect.w}x${plan.aspect.h}`,
        motionBlur: plan.motion_blur,
        fullFx: plan.full_fx !== false,
        shakes: true,
        zoom: true,
        reverse: hasReverse,
        oneFramers: hasOneFramers,
        transitions: hasTransitions,
        ambiance: hasAmbiance,
        echoTrail: echoTrailEnabled,
        export: plan.export || {
          codec: selectedCodec,
          bitrateMbps: bitrateValue,
          format: selectedFormat,
        },
      };

      // Navigate to TIME REMAP / JUGG page
      activePage = 'remap';
      showToast(`Edit plan generated from Dumper v2: style ${plan.style}, ${plan.fps} FPS, ${plan.segments.length} segments`, 'success');
    } catch (err) {
      console.error('Failed to generate edit plan:', err);
      const msg = typeof err === 'string' ? err : err?.message || JSON.stringify(err);
      showToast(`Failed to generate edit plan: ${msg}`, 'error');
    }
  }

  async function handleApplyAsProject() {
    if (!dumperResult || !dumperResult.reusableProjectPath) return;
    try {
      const plan = await invoke('apply_dumper_project', {
        projectPath: dumperResult.reusableProjectPath,
        project: null,
      });
      console.log('[DUMPER] Applied as project plan:', plan);

      // Save plan to project.json in app_data
      const planJson = JSON.stringify(plan);
      const savedPath = await invoke('save_plan', { planJson });
      console.log('[DUMPER] Saved applied plan to:', savedPath);

      // Set JUGG inputs and options
      selectedStyle = plan.style;
      fpsValue = plan.fps;
      bpm = plan.bpm;
      if (plan.aspect) {
        if (plan.aspect.w === 1920 && plan.aspect.h === 1080) {
          selectedAspectRatio = '16:9';
        } else if (plan.aspect.w === 1080 && plan.aspect.h === 1920) {
          selectedAspectRatio = '9:16';
        } else if (plan.aspect.w === 1080 && plan.aspect.h === 1080) {
          selectedAspectRatio = '1:1';
        } else {
          selectedAspectRatio = 'CUSTOM';
          customWidth = plan.aspect.w;
          customHeight = plan.aspect.h;
        }
      }

      // Populate file drop zones with the dumped video
      if (dumperResult.source) {
        scenePath = dumperResult.source;
        drumsPath = dumperResult.source;
        audioPath = dumperResult.source;
        sceneInfo = {
          duration: plan.video_duration,
          fps: plan.fps,
          width: plan.aspect ? plan.aspect.w : 1080,
          height: plan.aspect ? plan.aspect.h : 1080,
        };
        audioInfo = {
          duration: plan.audio_duration,
          sample_rate: 48000,
          channels: 2,
        };
      }

      const hasReverse = plan.segments.some((s) => s.effects && s.effects.reverse);
      const hasOneFramers = plan.one_framers && plan.one_framers.length > 0;
      const hasTransitions = (plan.transitions && plan.transitions.length > 0) || plan.segments.some((s) => s.transition);
      const hasAmbiance = !!plan.ambiance;

      planSummary = {
        segmentsCount: plan.segments.length,
        loops: plan.loops || 1,
        targetDuration: plan.target_duration,
        savedPath,
        style: plan.style,
        fps: plan.fps,
        aspect: `${plan.aspect.w}x${plan.aspect.h}`,
        motionBlur: plan.motion_blur,
        fullFx: plan.full_fx !== false,
        shakes: true,
        zoom: true,
        reverse: hasReverse,
        oneFramers: hasOneFramers,
        transitions: hasTransitions,
        ambiance: hasAmbiance,
        echoTrail: echoTrailEnabled,
        export: plan.export || {
          codec: selectedCodec,
          bitrateMbps: bitrateValue,
          format: selectedFormat,
        },
      };

      // Navigate to TIME REMAP / JUGG page
      activePage = 'remap';
      showToast(`Dumper project applied: style ${plan.style}, ${plan.fps} FPS, ${plan.segments.length} segments`, 'success');
    } catch (err) {
      console.error('Failed to apply dumper project:', err);
      const msg = typeof err === 'string' ? err : err?.message || JSON.stringify(err);
      showToast(`Failed to apply project: ${msg}`, 'error');
    }
  }

  async function runCompositionSegmentation() {
    if (!compCharacterPath || isSegmenting) return;
    isSegmenting = true;
    compGpuError = '';
    compResult = null;
    try {
      // 1. Check GPU status
      await invoke('check_gpu_status');

      // 2. Run segmentation
      const res = await invoke('segment_character', { characterPath: compCharacterPath });
      compResult = res;
      showToast(`Segmentation complete: ${res.layersCount} layers extracted`, 'success');
    } catch (err) {
      console.error('Composition error:', err);
      const msg = typeof err === 'string' ? err : err?.message || JSON.stringify(err);
      compGpuError = msg;
      showToast(`Segmentation failed: ${msg}`, 'error');
    } finally {
      isSegmenting = false;
    }
  }

  async function handleSaveComposition() {
    if (!compResult || !compResult.layers) return;
    try {
      const project = {
        schemaVersion: 'comp_project_v1',
        characterPath: compCharacterPath,
        backgroundPath: compBackgroundPath || null,
        layers: compResult.layers,
        parallax_strength: compParallaxStrength,
        beat_punch_intensity: compBeatPunchIntensity,
        light_wrap_intensity: compLightWrapIntensity,
        chromatic_aberration: compChromaticAberration,
        impact_blur_strength: compImpactBlurStrength,
      };
      const savedPath = await invoke('save_composition_project', { project, targetPath: null });
      showToast(`Composition saved to: ${savedPath}`, 'success');
    } catch (err) {
      console.error('Failed to save composition:', err);
      const msg = typeof err === 'string' ? err : err?.message || JSON.stringify(err);
      showToast(`Failed to save: ${msg}`, 'error');
    }
  }

  async function runCompositionRender() {
    if (!compCharacterPath || !compBackgroundPath || isRenderingComposition) return;
    isRenderingComposition = true;
    compGpuError = '';
    compRenderResult = null;
    compRenderProgress = { phase: 'INIT', percent: 0, current_frame: 0, total_frames: 1, message: 'Initializing layered compositor...' };
    try {
      const outputPath = await invoke('render_composition', {
        characterPath: compCharacterPath,
        backgroundPath: compBackgroundPath,
        ops: compOps
      });
      const ext = getFileExtension(outputPath);
      const isVideo = ['mp4', 'mkv', 'webm', 'mov', 'avi'].includes(ext);
      compRenderResult = {
        outputPath,
        isVideo,
        fileName: getFileName(outputPath),
        timestamp: new Date().toLocaleTimeString()
      };
      showToast('Composition rendered successfully!', 'success');
    } catch (err) {
      console.error('Composition render error:', err);
      const msg = typeof err === 'string' ? err : err?.message || JSON.stringify(err);
      compGpuError = msg;
      showToast(`Composition render failed: ${msg}`, 'error');
    } finally {
      isRenderingComposition = false;
    }
  }

  async function runCompositionMeshPreview() {
    if (!compCharacterPath || isRenderingPreview || isRenderingComposition) return;
    isRenderingPreview = true;
    compGpuError = '';
    compPreviewResult = null;
    compPreviewProgress = { phase: 'MESH_ANIM', percent: 0, current_frame: 0, total_frames: 90, message: 'Initializing procedural mesh animation...' };
    try {
      const outputPath = await invoke('render_mesh_preview', {
        characterPath: compCharacterPath,
        backgroundPath: compBackgroundPath || null,
        audioPath: drumsPath || audioPath || null,
        ops: compOps,
        parallaxStrength: compParallaxStrength,
        beatPunchIntensity: compBeatPunchIntensity,
        lightWrapIntensity: compLightWrapIntensity,
        chromaticAberration: compChromaticAberration,
        impactBlurStrength: compImpactBlurStrength
      });
      compPreviewResult = {
        outputPath,
        isVideo: true,
        fileName: getFileName(outputPath),
        timestamp: new Date().toLocaleTimeString()
      };
      showToast('3s Mesh animation preview rendered successfully!', 'success');
    } catch (err) {
      console.error('Mesh animation render error:', err);
      const msg = typeof err === 'string' ? err : err?.message || JSON.stringify(err);
      compGpuError = msg;
      showToast(`Mesh animation failed: ${msg}`, 'error');
    } finally {
      isRenderingPreview = false;
    }
  }

  async function handleOpenCompFolder(path) {
    if (!path) return;
    try {
      await invoke('open_target_folder', { path });
      showToast('Opening composition folder in Explorer', 'info');
    } catch (e) {
      console.error('Failed to open folder:', e);
      showToast(`Unable to open folder: ${e}`, 'error');
    }
  }

  async function handleOpenTargetFolder() {
    if (!renderOutputMp4) return;
    try {
      await invoke('open_target_folder', { path: renderOutputMp4 });
      showToast('Opening folder in Explorer', 'info');
    } catch (e) {
      console.error('Failed to open folder:', e);
      showToast(`Unable to open folder: ${e}`, 'error');
    }
  }

  function showToast(message, type = 'info') {
    toast = { show: true, message, type };
    setTimeout(() => { toast.show = false; }, 4000);
  }

  async function copyDiscordHandle() {
    try {
      await navigator.clipboard.writeText('cia2013');
      discordCopyFeedback = true;
      showToast('Discord handle copied', 'success');
      setTimeout(() => { discordCopyFeedback = false; }, 2000);
    } catch (e) {
      showToast('Unable to copy the Discord handle', 'error');
    }
  }

  function navigateTo(page) {
    if (activePage !== page) activePage = page;
  }

  async function openAboutLink(url) {
    try {
      await invoke('open_about_link', { url });
    } catch (e) {
      showToast(`Unable to open link: ${e}`, 'error');
    }
  }

  function formatBytes(bytes) {
    if (!bytes || bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`;
  }

  async function checkForAppUpdates(manual = false) {
    if (typeof window === 'undefined' || !window.__TAURI_INTERNALS__) return;
    try {
      updateState = 'checking';
      updateErrorMessage = '';
      const update = await check();
      if (update && update.available) {
        availableUpdate = update;
        updateState = 'available';
        if (manual) {
          showUpdateModal = true;
        }
      } else {
        availableUpdate = null;
        updateState = 'up-to-date';
        if (manual) {
          showToast('cia app is up to date', 'success');
        }
      }
    } catch (err) {
      console.error('Update check failed:', err);
      updateErrorMessage = String(err?.message || err);
      updateState = 'error';
      if (manual) {
        showToast(`Update check failed: ${updateErrorMessage}`, 'error');
      }
    }
  }

  async function installAppUpdate() {
    if (!availableUpdate) return;
    try {
      updateState = 'downloading';
      updateDownloadedBytes = 0;
      updateContentLength = 0;

      let downloaded = 0;
      let contentLength = 0;

      await availableUpdate.downloadAndInstall((event) => {
        switch (event.event) {
          case 'Started':
            contentLength = event.data.contentLength ?? 0;
            updateContentLength = contentLength;
            break;
          case 'Progress':
            downloaded += event.data.chunkLength;
            updateDownloadedBytes = downloaded;
            break;
          case 'Finished':
            updateState = 'ready';
            break;
        }
      });

      showToast('Update installed. Restarting cia app...', 'success');
      await relaunch();
    } catch (err) {
      console.error('Update install failed:', err);
      updateErrorMessage = String(err?.message || err);
      updateState = 'error';
      showToast(`Update installation failed: ${updateErrorMessage}`, 'error');
    }
  }

  $effect(() => {
    const u1 = listen('tauri://drag-drop', (event) => {
      const paths = event.payload?.paths;
      const position = event.payload?.position;
      let targetZone = hoveredZone;
      if (!targetZone && position) {
        const el = document.elementFromPoint(position.x, position.y);
        const dropZoneEl = el?.closest('.remap-drop-zone');
        if (dropZoneEl && dropZoneEl.dataset && dropZoneEl.dataset.zone) {
          targetZone = dropZoneEl.dataset.zone;
        }
      }
      hoveredZone = null;
      if (targetZone && paths && paths.length > 0) {
        validateAndSetFile(targetZone, paths[0]);
      }
    });

    const u2 = listen('tauri://drag-leave', () => {
      hoveredZone = null;
    });

    return () => {
      u1.then(f => f());
      u2.then(f => f());
    };
  });

  let unlistenProgress = null;

  onMount(async () => {
    try {
      appVersion = await invoke('get_app_version');
    } catch (e) {
      console.error('Failed to retrieve app version:', e);
    }

    try {
      availableEffects = await invoke('get_effect_previews');
    } catch (e) {
      console.error('Failed to load effect previews:', e);
    }

    // T18: pre-load custom params defaults for selected style
    await loadCustomParamsDefaults();

    try {
      unlistenProgress = await listen('render-progress', (event) => {
        if (event.payload) {
          renderProgress = event.payload;
        }
      });
    } catch (e) {
      console.error('Failed to listen to render-progress:', e);
    }

    let unlistenDump = null;
    try {
      unlistenDump = await listen('dump-progress', (event) => {
        if (event.payload) {
          dumperProgress = event.payload;
        }
      });
    } catch (e) {
      console.error('Failed to listen to dump-progress:', e);
    }

    let unlistenComp = null;
    try {
      unlistenComp = await listen('comp-progress', (event) => {
        if (event.payload) {
          compRenderProgress = event.payload;
        }
      });
    } catch (e) {
      console.error('Failed to listen to comp-progress:', e);
    }

    checkForAppUpdates(false);

    if (typeof window !== 'undefined' && window.location.hash === '#composition') {
      activePage = 'composition';
      compCharacterPath = 'C:\\Users\\cia\\Downloads\\spider-man-11530958085nzzlmiz6hg-732305370.png';
      compBackgroundPath = 'C:\\Users\\cia\\Downloads\\jugg video & audio tester\\snaptik_7674387013243538721_v3.mp4';
      if (!window.__TAURI_INTERNALS__) {
        compRenderResult = {
          outputPath: 'C:\\Users\\cia\\AppData\\Local\\Temp\\cia_composition\\composition_1787436866339.mp4',
          isVideo: true,
          fileName: 'composition_1787436866339.mp4',
          timestamp: '00:15:10'
        };
        compResult = {
          status: 'success',
          characterPath: compCharacterPath,
          outputDir: 'C:\\Users\\cia\\AppData\\Local\\Temp\\cia_composition\\comp_demo',
          layersCount: 9,
          layers: [
            { name: 'hair_back', file: 'hair_back.png', zOrder: 0, hasContent: true },
            { name: 'body', file: 'body.png', zOrder: 1, hasContent: true },
            { name: 'clothes_lower', file: 'clothes_lower.png', zOrder: 2, hasContent: true },
            { name: 'clothes_upper', file: 'clothes_upper.png', zOrder: 3, hasContent: true },
            { name: 'face', file: 'face.png', zOrder: 4, hasContent: false },
            { name: 'mouth', file: 'mouth.png', zOrder: 5, hasContent: true },
            { name: 'eyes', file: 'eyes.png', zOrder: 6, hasContent: true },
            { name: 'hair_front', file: 'hair_front.png', zOrder: 7, hasContent: true },
            { name: 'accessories', file: 'accessories.png', zOrder: 8, hasContent: false },
          ]
        };
      }
    }

    return () => {
      if (unlistenProgress) unlistenProgress();
      if (unlistenDump) unlistenDump();
      if (unlistenComp) unlistenComp();
    };
  });
</script>

<div class="app-root">
  <!-- Custom Windows Titlebar -->
  <div class="titlebar" data-tauri-drag-region>
    <div class="titlebar-brand">
      <span class="titlebar-text">cia app</span>
    </div>
    <div class="titlebar-controls">
      {#if availableUpdate}
        <button class="titlebar-btn update-badge" onclick={() => showUpdateModal = true} aria-label="Update available">
          <span class="update-badge-dot"></span> UPDATE V{availableUpdate.version}
        </button>
      {/if}
      <button class="titlebar-btn" onclick={() => appWindow?.minimize()} aria-label="Minimize" disabled={!appWindow}>-</button>
      <button class="titlebar-btn close" onclick={() => appWindow?.close()} aria-label="Close" disabled={!appWindow}>X</button>
    </div>
  </div>

  <nav class="tab-bar">
    <button class:active={activePage === 'remap' || activePage === 'settings'} onclick={() => navigateTo('remap')}>TIME REMAP</button>
    <button class:active={activePage === 'dumper'} onclick={() => navigateTo('dumper')}>DUMPER</button>
    <button class:active={activePage === 'composition'} onclick={() => navigateTo('composition')}>COMPOSITION</button>
    <button class:active={activePage === 'about'} onclick={() => navigateTo('about')}>ABOUT</button>
  </nav>

  <!-- Main Content Area -->
  <main class="content-area">
    {#key activePage}
      <div class="page-stage">
        {#if activePage === 'remap'}
          <section class="remap-page" aria-label="Time remap configuration">
            <div class="remap-grid">
              <!-- DROP ZONE 1: SCENE (VIDEO) -->
              <div
                class="remap-drop-zone"
                class:filled={Boolean(scenePath)}
                class:has-error={Boolean(sceneError)}
                class:hovering={hoveredZone === 'scene'}
                data-zone="scene"
                ondragenter={(e) => { e.preventDefault(); hoveredZone = 'scene'; }}
                ondragover={(e) => { e.preventDefault(); hoveredZone = 'scene'; }}
                ondragleave={(e) => { e.preventDefault(); if (hoveredZone === 'scene') hoveredZone = null; }}
                ondrop={(e) => { e.preventDefault(); hoveredZone = null; }}
                onclick={() => !scenePath && handlePickFile('scene')}
                onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && !scenePath && handlePickFile('scene')}
                role="button"
                tabindex="0"
              >
                {#if scenePath}
                  <div class="zone-filled-content">
                    <div class="zone-header">
                      <span class="zone-tag">VIDEO</span>
                      <span class="pro-dot active"></span>
                    </div>
                    <div class="zone-title">SCENE</div>
                    <div class="zone-filename mono" title={scenePath}>{getFileName(scenePath)}</div>
                    <div class="zone-actions">
                      <button class="btn-zone-action" onclick={(e) => handlePickFile('scene', e)}>REPLACE</button>
                      <button class="btn-zone-action danger" onclick={(e) => clearZone('scene', e)}>REMOVE</button>
                    </div>
                  </div>
                {:else}
                  <div class="zone-empty-content">
                    <p class="zone-prompt">DRAG SCENE</p>
                    <span class="zone-sublabel">VIDEO (MP4, MKV, WEBM, MOV, AVI)</span>
                    {#if sceneError}
                      <span class="zone-error-msg">{sceneError}</span>
                    {/if}
                  </div>
                {/if}
              </div>

              <!-- DROP ZONE 2: DRUMS (AUDIO) -->
              <div
                class="remap-drop-zone"
                class:filled={Boolean(drumsPath)}
                class:has-error={Boolean(drumsError)}
                class:hovering={hoveredZone === 'drums'}
                data-zone="drums"
                ondragenter={(e) => { e.preventDefault(); hoveredZone = 'drums'; }}
                ondragover={(e) => { e.preventDefault(); hoveredZone = 'drums'; }}
                ondragleave={(e) => { e.preventDefault(); if (hoveredZone === 'drums') hoveredZone = null; }}
                ondrop={(e) => { e.preventDefault(); hoveredZone = null; }}
                onclick={() => !drumsPath && handlePickFile('drums')}
                onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && !drumsPath && handlePickFile('drums')}
                role="button"
                tabindex="0"
              >
                {#if drumsPath}
                  <div class="zone-filled-content">
                    <div class="zone-header">
                      <span class="zone-tag">AUDIO</span>
                      <span class="pro-dot active"></span>
                    </div>
                    <div class="zone-title">DRUMS</div>
                    <div class="zone-filename mono" title={drumsPath}>{getFileName(drumsPath)}</div>
                    <div class="zone-actions">
                      <button class="btn-zone-action" onclick={(e) => handlePickFile('drums', e)}>REPLACE</button>
                      <button class="btn-zone-action danger" onclick={(e) => clearZone('drums', e)}>REMOVE</button>
                    </div>
                  </div>
                {:else}
                  <div class="zone-empty-content">
                    <p class="zone-prompt">DRAG DRUMS</p>
                    <span class="zone-sublabel">AUDIO (MP3, WAV, FLAC, M4A, OGG)</span>
                    {#if drumsError}
                      <span class="zone-error-msg">{drumsError}</span>
                    {/if}
                  </div>
                {/if}
              </div>

              <!-- DROP ZONE 3: AUDIO (AUDIO) -->
              <div
                class="remap-drop-zone"
                class:filled={Boolean(audioPath)}
                class:has-error={Boolean(audioError)}
                class:hovering={hoveredZone === 'audio'}
                data-zone="audio"
                ondragenter={(e) => { e.preventDefault(); hoveredZone = 'audio'; }}
                ondragover={(e) => { e.preventDefault(); hoveredZone = 'audio'; }}
                ondragleave={(e) => { e.preventDefault(); if (hoveredZone === 'audio') hoveredZone = null; }}
                ondrop={(e) => { e.preventDefault(); hoveredZone = null; }}
                onclick={() => !audioPath && handlePickFile('audio')}
                onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && !audioPath && handlePickFile('audio')}
                role="button"
                tabindex="0"
              >
                {#if audioPath}
                  <div class="zone-filled-content">
                    <div class="zone-header">
                      <span class="zone-tag">AUDIO</span>
                      <span class="pro-dot active"></span>
                    </div>
                    <div class="zone-title">AUDIO</div>
                    <div class="zone-filename mono" title={audioPath}>{getFileName(audioPath)}</div>
                    <div class="zone-actions">
                      <button class="btn-zone-action" onclick={(e) => handlePickFile('audio', e)}>REPLACE</button>
                      <button class="btn-zone-action danger" onclick={(e) => clearZone('audio', e)}>REMOVE</button>
                    </div>
                  </div>
                {:else}
                  <div class="zone-empty-content">
                    <p class="zone-prompt">DRAG AUDIO</p>
                    <span class="zone-sublabel">AUDIO (MP3, WAV, FLAC, M4A, OGG)</span>
                    {#if audioError}
                      <span class="zone-error-msg">{audioError}</span>
                    {/if}
                  </div>
                {/if}
              </div>
            </div>

            {#if allZonesFilled}
              <div class="continue-row">
                <button
                  class="btn-continue"
                  onclick={handleContinue}
                  disabled={isAnalyzing}
                >
                  {#if isAnalyzing}
                    <span class="spinner-inline"></span>
                    <span>{analyzingStep || 'PROCESSING...'}</span>
                  {:else}
                    CONTINUE &gt;
                  {/if}
                </button>
              </div>
            {/if}
          </section>

        {:else if activePage === 'settings'}
          <section class="settings-page" aria-label="Settings configuration">
            <div class="settings-container">
              <!-- Summary Probed Sources -->
              <div class="settings-sources-card">
                <div class="settings-sources-header">
                  <span class="settings-kicker">cia app / TIME REMAP</span>
                  <h1>SETTINGS</h1>
                </div>

                <div class="compact-sources-list">
                  <!-- SCENE -->
                  <div class="source-row">
                    <span class="source-tag">SCENE</span>
                    <span class="source-name mono" title={scenePath}>{getFileName(scenePath)}</span>
                    {#if sceneInfo}
                      <span class="meta-pill mono">
                        {sceneInfo.duration.toFixed(2)}s Â· {sceneInfo.width}x{sceneInfo.height} Â· {sceneInfo.fps.toFixed(0)}fps
                      </span>
                    {/if}
                  </div>

                  <!-- DRUMS -->
                  <div class="source-row">
                    <span class="source-tag">DRUMS</span>
                    <span class="source-name mono" title={drumsPath}>{getFileName(drumsPath)}</span>
                    {#if drumsInfo}
                      <span class="meta-pill mono">
                        {drumsInfo.duration.toFixed(2)}s Â· {drumsInfo.audioSampleRate}Hz Â· {bpm ? bpm.toFixed(1) : 'â€”'} BPM Â· {beats ? beats.length : 0} beats ({downbeats ? downbeats.length : 0} downbeats)
                      </span>
                    {/if}
                  </div>

                  <!-- AUDIO -->
                  <div class="source-row">
                    <span class="source-tag">AUDIO</span>
                    <span class="source-name mono" title={audioPath}>{getFileName(audioPath)}</span>
                    {#if audioInfo}
                      <span class="meta-pill mono">
                        {audioInfo.duration.toFixed(2)}s Â· {audioInfo.audioSampleRate}Hz Â· {audioInfo.audioChannels}ch
                      </span>
                    {/if}
                  </div>
                </div>
              </div>

              <!-- Controls Card -->
              <div class="settings-controls-card">
                <!-- 1. Style Selector (HARD / SMOOTH / HYBRID) -->
                <div class="control-group">
                  <span class="group-label">REMAP STYLE</span>
                  <div class="styles-grid">
                    {#each STYLE_OPTIONS as style}
                      <button
                        class="style-card"
                        class:selected={selectedStyle === style.id}
                        onclick={() => {
                          selectedStyle = style.id;
                          effectOverrides = getDefaultOverrides(selectedStyle, fullFxEnabled);
                        }}
                        type="button"
                      >
                        <div class="style-card-header">
                          <span class="style-name">{style.title}</span>
                          <span class="pro-dot" class:active={selectedStyle === style.id}></span>
                        </div>
                        <p class="style-desc">{style.desc}</p>
                      </button>
                    {/each}
                  </div>
                </div>

                <!-- 2. Framerate (GlowSlider) -->
                <div class="control-group">
                  <GlowSlider
                    bind:value={fpsValue}
                    min={12}
                    max={60}
                    step={1}
                    label="OUTPUT FRAMERATE"
                    unit=" FPS"
                    precision={0}
                  />
                </div>

                <!-- 3. Aspect Ratio Preset Selector -->
                <div class="control-group">
                  <div class="ar-control-header">
                    <span class="group-label">ASPECT RATIO</span>
                    {#if selectedAspectRatio === 'CUSTOM'}
                      <div class="custom-ar-inputs-inline">
                        <div class="input-field">
                          <span class="input-prefix">W:</span>
                          <input
                            type="number"
                            min="1"
                            value={customWidth}
                            oninput={handleCustomWidthInput}
                            class="mono-input"
                            placeholder="W"
                          />
                        </div>
                        <span class="ar-divider">x</span>
                        <div class="input-field">
                          <span class="input-prefix">H:</span>
                          <input
                            type="number"
                            min="1"
                            value={customHeight}
                            oninput={handleCustomHeightInput}
                            class="mono-input"
                            placeholder="H"
                          />
                        </div>
                      </div>
                    {/if}
                  </div>

                  <div class="ar-buttons-row">
                    {#each ASPECT_RATIO_PRESETS as ar}
                      <button
                        class="btn-ar"
                        class:active={selectedAspectRatio === ar}
                        onclick={() => handleAspectRatioSelect(ar)}
                        type="button"
                      >
                        {ar}
                      </button>
                    {/each}
                  </div>
                  {#if selectedAspectRatio === 'CUSTOM' && customArError}
                    <span class="inline-ar-error mono">{customArError}</span>
                  {/if}
                </div>

                <!-- 4. T13 Full FX toggle & T17 Details button -->
                <div class="control-group">
                  <div class="toggle-row">
                    <div class="toggle-row-label">
                      <span class="group-label">FULL FX</span>
                      <span class="toggle-row-desc">All effects â€” one-framers, transitions, tint, vignette, scanlines. Default ON.</span>
                    </div>
                    <div class="toggle-actions-group">
                      <button
                        id="btn-details-fx"
                        class="btn-details-fx"
                        onclick={() => showDetailsModal = true}
                        type="button"
                      >
                        DETAILS
                      </button>
                      <button
                        id="toggle-full-fx"
                        class="toggle-btn"
                        class:active={fullFxEnabled}
                        onclick={handleToggleFullFx}
                        type="button"
                        aria-pressed={fullFxEnabled}
                      >
                        {fullFxEnabled ? 'ON' : 'OFF'}
                      </button>
                    </div>
                  </div>
                </div>

                <!-- 5. T11 Echo/Trail toggle -->
                <div class="control-group">
                  <div class="toggle-row">
                    <div class="toggle-row-label">
                      <span class="group-label">ECHO / TRAIL</span>
                      <span class="toggle-row-desc">Time Blend — blends current frame with 3 previous frames (α=0.3). Default OFF.</span>
                    </div>
                    <button
                      id="toggle-echo-trail"
                      class="toggle-btn"
                      class:active={echoTrailEnabled}
                      onclick={() => echoTrailEnabled = !echoTrailEnabled}
                      type="button"
                      aria-pressed={echoTrailEnabled}
                    >
                      {echoTrailEnabled ? 'ON' : 'OFF'}
                    </button>
                  </div>
                </div>

                <!-- 6. T19 Export Options: Codec, Bitrate, Format -->
                <div class="control-group">
                  <span class="group-label">VIDEO CODEC</span>
                  <div class="options-buttons-row">
                    {#each CODEC_OPTIONS as codec}
                      <button
                        class="btn-option"
                        class:active={selectedCodec === codec.id}
                        onclick={() => selectedCodec = codec.id}
                        type="button"
                      >
                        {codec.label}
                      </button>
                    {/each}
                  </div>
                </div>

                <div class="control-group">
                  <GlowSlider
                    bind:value={bitrateValue}
                    min={5}
                    max={50}
                    step={1}
                    label="BITRATE"
                    unit=" Mbps"
                    precision={0}
                  />
                </div>

                <div class="control-group">
                  <span class="group-label">CONTAINER FORMAT</span>
                  <div class="options-buttons-row">
                    {#each FORMAT_OPTIONS as fmt}
                      <button
                        class="btn-option"
                        class:active={selectedFormat === fmt}
                        onclick={() => selectedFormat = fmt}
                        type="button"
                      >
                        {fmt}
                      </button>
                    {/each}
                  </div>
                </div>
              </div>

              <!-- Render Execution Cards (T6) -->
              {#if renderState === 'running'}
                <div class="render-progress-card">
                  <div class="render-progress-header">
                    <div class="phase-badge-row">
                      <span class="render-phase-badge" class:active={renderProgress.phase === 'DECODING'}>1. DECODE</span>
                      <span class="render-phase-arrow">&gt;</span>
                      <span class="render-phase-badge" class:active={renderProgress.phase === 'SAMPLING'}>2. SAMPLE</span>
                      <span class="render-phase-arrow">&gt;</span>
                      <span class="render-phase-badge" class:active={renderProgress.phase === 'ENCODING'}>3. ENCODE</span>
                    </div>
                    <span class="render-percent mono">{renderProgress.percent}%</span>
                  </div>

                  <div class="progress-bar-container">
                    <div class="progress-bar-fill" style="width: {renderProgress.percent}%;"></div>
                  </div>

                  <div class="render-progress-footer">
                    <span class="render-msg mono" title={renderProgress.message}>{renderProgress.message || 'Processing...'}</span>
                    <button class="btn-cancel-render" onclick={handleCancelRender} type="button">
                      CANCEL
                    </button>
                  </div>
                </div>

              {:else if renderState === 'done'}
                <div class="render-done-card">
                  <div class="render-done-header">
                    <div class="done-title-row">
                      <span class="pro-dot active"></span>
                      <span class="render-done-title">RENDER COMPLETE</span>
                    </div>
                    <span class="done-specs mono">{planSummary?.aspect || '1080x1080'} Â· {renderStats?.targetFps || fpsValue} FPS</span>
                  </div>

                  {#if renderStats}
                  <div class="render-stats-grid">
                    <div class="render-stat-item">
                      <span class="stat-label">RENDER TIME</span>
                      <span class="stat-value mono">{renderStats.renderTimeSecs.toFixed(2)}s</span>
                    </div>
                    <div class="render-stat-item">
                      <span class="stat-label">FILE SIZE</span>
                      <span class="stat-value mono">{renderStats.fileSizeMb.toFixed(2)} MB</span>
                    </div>
                    <div class="render-stat-item">
                      <span class="stat-label">AVG FPS</span>
                      <span class="stat-value mono">{renderStats.targetFps} FPS</span>
                    </div>
                    <div class="render-stat-item">
                      <span class="stat-label">EFFECTS APPLIED</span>
                      <span class="stat-value mono">{renderStats.effectsCount}</span>
                    </div>
                  </div>
                  {/if}

                  <div class="done-path-box">
                    <span class="stat-label">OUTPUT:</span>
                    <span class="saved-path-text mono" title={renderOutputMp4}>{renderOutputMp4}</span>
                  </div>

                  <div class="done-actions-row">
                    <button class="btn-open-folder" onclick={handleOpenTargetFolder} type="button">
                      OPEN FOLDER
                    </button>
                    <button class="btn-pro-secondary" onclick={() => { renderState = 'idle'; }} type="button">
                      RENDER AGAIN
                    </button>
                  </div>
                </div>

              {:else if renderState === 'error'}
                <div class="render-error-card">
                  <div class="render-error-header">
                    <span class="render-error-title">RENDER ERROR</span>
                  </div>
                  <p class="render-error-msg mono">{renderError}</p>
                  <div class="done-actions-row">
                    <button class="btn-pro-secondary" onclick={() => { renderState = 'idle'; }} type="button">
                      DISMISS
                    </button>
                  </div>
                </div>

              {:else if planSummary}
                <div class="plan-summary-card">
                  <div class="plan-summary-header">
                    <span class="plan-summary-title">PLAN SUMMARY</span>
                    <span class="pro-dot active"></span>
                  </div>
                  <div class="plan-summary-grid">
                    <div class="plan-stat">
                      <span class="stat-label">STYLE / FPS</span>
                      <span class="stat-value mono">{planSummary.style} Â· {planSummary.fps} FPS{#if planSummary.motionBlur !== undefined} Â· BLUR {planSummary.motionBlur ? 'ON' : 'OFF'}{/if}</span>
                    </div>
                    <div class="plan-stat">
                      <span class="stat-label">EFFECTS</span>
                      <span class="stat-value mono">SHAKES ON Â· ZOOM ON Â· REVERSE {planSummary.reverse ? 'ON' : 'OFF'} Â· ONE-FRAMERS {planSummary.oneFramers ? 'ON' : 'OFF'} Â· TRANSITIONS {planSummary.transitions ? 'ON' : 'OFF'} Â· AMBIANCE {planSummary.ambiance ? 'ON' : 'OFF'} Â· ECHO {planSummary.echoTrail ? 'ON' : 'OFF'}</span>
                    </div>
                    <div class="plan-stat">
                      <span class="stat-label">FX MODE</span>
                      <span class="stat-value mono" class:fx-motion-only={!planSummary.fullFx}>{planSummary.fullFx ? 'FX: FULL' : 'FX: MOTION ONLY'} · ADV SHAKES ON</span>
                    </div>
                    <div class="plan-stat">
                      <span class="stat-label">EXPORT</span>
                      <span class="stat-value mono">{planSummary.export?.codec || selectedCodec} · {planSummary.export?.bitrateMbps || planSummary.export?.bitrate_mbps || bitrateValue} Mbps · {planSummary.export?.format || selectedFormat}</span>
                    </div>
                    {#if planSummary.ambiance}
                    <div class="plan-stat flicker-warning">
                      <span class="flicker-badge">âš  FLICKER ACTIVE â€” photosensitive epilepsy warning</span>
                    </div>
                    {/if}
                    <div class="plan-stat">
                      <span class="stat-label">SEGMENTS</span>
                      <span class="stat-value mono">{planSummary.segmentsCount} cuts</span>
                    </div>
                    <div class="plan-stat">
                      <span class="stat-label">LOOPS / DURATION</span>
                      <span class="stat-value mono">{planSummary.loops} loop{planSummary.loops === 1 ? '' : 's'} Â· {planSummary.targetDuration.toFixed(2)}s</span>
                    </div>
                  </div>
                  <div class="plan-saved-path">
                    <span class="stat-label">SAVED:</span>
                    <span class="saved-path-text mono" title={planSummary.savedPath}>{planSummary.savedPath}</span>
                  </div>
                </div>
              {/if}

              <!-- Footer Actions (only when idle/error) -->
              {#if renderState !== 'running' && renderState !== 'done'}
                <div class="settings-actions-footer">
                  <button class="btn-pro-secondary" onclick={() => navigateTo('remap')}>
                    &lt; BACK TO SOURCES
                  </button>
                  <button class="btn-pro-secondary" onclick={handleRunProcess}>
                    RUN PROCESS &gt;
                  </button>
                  <button class="btn-run-process btn-final-jugg" onclick={handleRenderFinalJugg}>
                    ⚡ RENDER FINAL JUGG
                  </button>
                </div>
              {/if}
            </div>
          </section>

        {:else if activePage === 'dumper'}
          <section class="dumper-page" aria-label="Dumper analysis page">
            <div class="dumper-container">
              <!-- DROP ZONE FOR EDIT VIDEO -->
              <div
                class="remap-drop-zone dumper-drop-zone"
                class:filled={Boolean(dumperVideoPath)}
                class:has-error={Boolean(dumperVideoError)}
                class:hovering={hoveredZone === 'dumper'}
                data-zone="dumper"
                ondragenter={(e) => { e.preventDefault(); hoveredZone = 'dumper'; }}
                ondragover={(e) => { e.preventDefault(); hoveredZone = 'dumper'; }}
                ondragleave={(e) => { e.preventDefault(); if (hoveredZone === 'dumper') hoveredZone = null; }}
                ondrop={(e) => { e.preventDefault(); hoveredZone = null; }}
                onclick={() => !dumperVideoPath && handlePickFile('dumper')}
                onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && !dumperVideoPath && handlePickFile('dumper')}
                role="button"
                tabindex="0"
              >
                {#if dumperVideoPath}
                  <div class="zone-filled-content">
                    <div class="zone-header">
                      <span class="zone-tag">EDIT VIDEO</span>
                      <span class="pro-dot active"></span>
                    </div>
                    <div class="zone-title">TARGET EDIT</div>
                    <div class="zone-filename mono" title={dumperVideoPath}>{getFileName(dumperVideoPath)}</div>
                    <div class="zone-actions">
                      <button class="btn-zone-action" onclick={(e) => handlePickFile('dumper', e)} disabled={isDumping}>REPLACE</button>
                      <button class="btn-zone-action danger" onclick={(e) => clearZone('dumper', e)} disabled={isDumping}>REMOVE</button>
                    </div>
                  </div>
                {:else}
                  <div class="zone-empty-content">
                    <p class="zone-prompt">DRAG EDIT VIDEO</p>
                    <span class="zone-sublabel">FINISHED EDIT (MP4, MKV, WEBM, MOV, AVI)</span>
                    {#if dumperVideoError}
                      <span class="zone-error-msg">{dumperVideoError}</span>
                    {/if}
                  </div>
                {/if}
              </div>

              <!-- ACTION & PROGRESS & RESULTS -->
              <div class="dumper-actions-panel">
                {#if !isDumping && !dumperResult}
                  <button
                    class="btn-run-process dumper-run-btn"
                    disabled={!dumperVideoPath}
                    onclick={runDumperPipeline}
                  >
                    RUN ANALYSIS &gt;
                  </button>
                {/if}

                <!-- PROGRESS CARD -->
                {#if isDumping && dumperProgress}
                  <div class="render-progress-card dumper-progress-card">
                    <div class="render-progress-header">
                      <div class="phase-badge-row">
                        <span class="render-phase-badge" class:active={dumperProgress.phase === 'SCENES'}>1. SCENES</span>
                        <span class="render-phase-arrow">&gt;</span>
                        <span class="render-phase-badge" class:active={dumperProgress.phase === 'BEATS'}>2. BEATS</span>
                        <span class="render-phase-arrow">&gt;</span>
                        <span class="render-phase-badge" class:active={dumperProgress.phase === 'MOTION'}>3. MOTION</span>
                        <span class="render-phase-arrow">&gt;</span>
                        <span class="render-phase-badge" class:active={dumperProgress.phase === 'PROFILES'}>4. PROFILES</span>
                        <span class="render-phase-arrow">&gt;</span>
                        <span class="render-phase-badge" class:active={dumperProgress.phase === 'REPORT'}>5. REPORT</span>
                      </div>
                      <span class="render-percent mono">{dumperProgress.percent}%</span>
                    </div>

                    <div class="progress-bar-container">
                      <div class="progress-bar-fill" style={`width: ${dumperProgress.percent}%`}></div>
                    </div>

                    <div class="render-status-msg mono">
                      <span class="spinner-inline"></span>
                      {dumperProgress.message}
                    </div>
                  </div>
                {/if}

                <!-- STRUCTURED REPORT VIEW -->
                {#if dumperResult}
                  <div class="dumper-result-card">
                    <!-- 1. HEADER & STYLE ARCHETYPE -->
                    <div class="result-header">
                      <div class="result-title-row">
                        <span class="zone-tag">DUMP REPORT</span>
                        {#if dumperResult.detectedStyle}
                          <span class="style-badge mono">{dumperResult.detectedStyle.styleName.toUpperCase()} ({(dumperResult.detectedStyle.confidence * 100).toFixed(0)}%)</span>
                        {/if}
                        <span class="pro-dot active"></span>
                      </div>
                      <span class="result-timestamp mono">{getFileName(dumperResult.source)}</span>
                    </div>

                    <!-- 2. HIGH-LEVEL METRICS SUMMARY GRID -->
                    <div class="dumper-stats-grid">
                      <div class="stat-box">
                        <span class="stat-label">DURATION & FPS</span>
                        <span class="stat-val mono">{dumperResult.duration}s ({dumperResult.fps} FPS)</span>
                      </div>
                      <div class="stat-box">
                        <span class="stat-label">CUTS & DENSITY</span>
                        <span class="stat-val mono">{dumperResult.cuts.length} cuts ({(dumperResult.cuts.length / (dumperResult.duration || 1)).toFixed(2)}/s)</span>
                      </div>
                      <div class="stat-box">
                        <span class="stat-label">DETECTED BPM</span>
                        <span class="stat-val mono">{dumperResult.beats.bpm > 0 ? dumperResult.beats.bpm.toFixed(1) : 'N/A'} ({dumperResult.beats.beats.length} beats)</span>
                      </div>
                      <div class="stat-box">
                        <span class="stat-label">CUT-BEAT SYNC (±60ms)</span>
                        <span class="stat-val mono highlight-sync">{dumperResult.syncNa ? 'N/A (0 cuts)' : `${(dumperResult.cutBeatSync * 100).toFixed(1)}%`}</span>
                      </div>
                    </div>

                    <!-- 3. DETECTED STYLE & JUSTIFICATIONS -->
                    {#if dumperResult.detectedStyle}
                      <div class="dumper-section-box">
                        <div class="section-box-header">
                          <span class="section-box-title">DETECTED STYLE & JUSTIFICATIONS</span>
                          <span class="badge-accent mono">{dumperResult.detectedStyle.styleName.toUpperCase()}</span>
                        </div>
                        <ul class="justification-list mono">
                          {#each dumperResult.detectedStyle.justifications as just}
                            <li><span class="bullet-dot">·</span> {just}</li>
                          {/each}
                        </ul>
                      </div>
                    {/if}

                    <!-- 4. SEGMENTS (SIGNATURES) TABLE -->
                    {#if dumperResult.segments && dumperResult.segments.length > 0}
                      <div class="dumper-section-box">
                        <div class="section-box-header">
                          <span class="section-box-title">SEGMENTS ({dumperResult.segments.length} SIGNATURES)</span>
                        </div>
                        <div class="table-container">
                          <table class="dumper-data-table mono">
                            <thead>
                              <tr>
                                <th>#</th>
                                <th>RANGE</th>
                                <th>LAB MEAN [L, a, b]</th>
                                <th>LAB STD</th>
                                <th>MAD MEAN</th>
                                <th>MAD PEAK</th>
                                <th>SHAKE ENERGY</th>
                                <th>1-FRAMERS</th>
                                <th>SPEED HINT</th>
                              </tr>
                            </thead>
                            <tbody>
                              {#each dumperResult.segments as seg, idx}
                                <tr>
                                  <td class="col-idx">{idx + 1}</td>
                                  <td>{seg.start.toFixed(2)}s - {seg.end.toFixed(2)}s</td>
                                  <td>[{seg.lab.mean[0].toFixed(1)}, {seg.lab.mean[1].toFixed(1)}, {seg.lab.mean[2].toFixed(1)}]</td>
                                  <td>[{seg.lab.std[0].toFixed(1)}, {seg.lab.std[1].toFixed(1)}, {seg.lab.std[2].toFixed(1)}]</td>
                                  <td>{seg.madMean.toFixed(1)}</td>
                                  <td>{seg.madPeak.toFixed(1)}</td>
                                  <td>{seg.motion ? seg.motion.shakeEnergy.toFixed(3) : 'N/A'}</td>
                                  <td>{seg.oneFramerCount}</td>
                                  <td><span class="badge-hint {seg.speedHint}">{seg.speedHint}</span></td>
                                </tr>
                              {/each}
                            </tbody>
                          </table>
                        </div>
                      </div>
                    {/if}

                    <!-- 5. ONE-FRAMERS & MOTION -->
                    <div class="dumper-two-col-grid">
                      <!-- One-framers -->
                      <div class="dumper-section-box">
                        <div class="section-box-header">
                          <span class="section-box-title">ONE-FRAMERS ({dumperResult.oneFramers ? dumperResult.oneFramers.length : 0})</span>
                        </div>
                        {#if dumperResult.oneFramers && dumperResult.oneFramers.length > 0}
                          <div class="pills-scroll-container">
                            {#each dumperResult.oneFramers as of_t}
                              <span class="framer-pill mono">{of_t.toFixed(3)}s</span>
                            {/each}
                          </div>
                        {:else}
                          <p class="section-empty-hint mono">No isolated 1-frame flashes detected.</p>
                        {/if}
                      </div>

                      <!-- Motion Dynamics -->
                      <div class="dumper-section-box">
                        <div class="section-box-header">
                          <span class="section-box-title">MOTION & REVERSE STATUS</span>
                        </div>
                        <div class="motion-summary-content mono">
                          <div class="motion-row">
                            <span class="motion-label">Shake High-Freq:</span>
                            <span class="motion-val">{dumperResult.segments && dumperResult.segments.length > 0 && dumperResult.segments[0].motion ? (dumperResult.segments.reduce((acc, s) => acc + (s.motion ? s.motion.shakeEnergy : 0), 0) / dumperResult.segments.length).toFixed(4) : 'N/A'}</span>
                          </div>
                          <div class="motion-row">
                            <span class="motion-label">Zoom Present:</span>
                            <span class="motion-val">{dumperResult.segments ? dumperResult.segments.filter(s => s.motion && s.motion.zoomPresence).length : 0} / {dumperResult.segments ? dumperResult.segments.length : 0} segments</span>
                          </div>
                          <div class="motion-note">
                            <span class="note-tag">REVERSE REMAP:</span> Non mesurable depuis la sortie seule (non deviné, laissé aux métadonnées originales).
                          </div>
                        </div>
                      </div>
                    </div>

                    <!-- 6. REUSABLE VS DESCRIPTIVE -->
                    <div class="dumper-section-box reusable-vs-descriptive">
                      <div class="section-box-header">
                        <span class="section-box-title">REUSABLE VS DESCRIPTIVE</span>
                      </div>
                      <div class="reusable-grid mono">
                        <div class="reusable-column">
                          <div class="column-title success">MEASURABLE & REUSABLE (PROJECT SCHEMA)</div>
                          <ul class="feature-checklist">
                            <li>✓ Audio BPM ({dumperResult.beats.bpm.toFixed(1)}), beat grid & downbeats</li>
                            <li>✓ Scene cut timestamps & segment boundaries</li>
                            <li>✓ Color palettes (LAB mean & std per segment)</li>
                            <li>✓ Suggested style preset & target FPS ({dumperResult.fps})</li>
                          </ul>
                        </div>
                        <div class="reusable-column">
                          <div class="column-title muted">DESCRIPTIVE ONLY (NOT RECONSTRUCTIBLE)</div>
                          <ul class="feature-checklist muted">
                            <li>✗ Reverse clip speed remap</li>
                            <li>✗ Pre-FX raw footage layers & compositions</li>
                          </ul>
                        </div>
                      </div>
                    </div>

                    <!-- 7. OUTPUT FILES & ACTION BUTTONS -->
                    <div class="dumper-files-container">
                      <div class="file-path-row mono" title={dumperResult.jsonPath}>
                        <span class="file-badge">ANALYSIS JSON</span>
                        <span class="file-path-text">{dumperResult.jsonPath || 'analysis.json'}</span>
                      </div>
                      <div class="file-path-row mono" title={dumperResult.reportPath}>
                        <span class="file-badge">MARKDOWN REPORT</span>
                        <span class="file-path-text">{dumperResult.reportPath || 'report.md'}</span>
                      </div>
                      <div class="file-path-row mono" title={dumperResult.reusableProjectPath}>
                        <span class="file-badge">REUSABLE PROJECT</span>
                        <span class="file-path-text">{dumperResult.reusableProjectPath || 'reusable_project.json'}</span>
                      </div>
                    </div>

                    <div class="result-footer-actions">
                      <button
                        class="btn-apply-project mono"
                        onclick={handleGenerateEditPlan}
                        title="Auto-generate remap plan from Dumper v2 analysis and load into timeline"
                      >
                        ⚡ GENERATE EDIT PLAN
                      </button>
                      <button
                        class="btn-zone-action mono"
                        onclick={handleApplyAsProject}
                        title="Convert into ProjectPlan and load into JUGG page"
                      >
                        APPLY AS PROJECT
                      </button>
                      <button
                        class="btn-zone-action"
                        onclick={() => handleOpenDumperFolder(dumperResult.jsonPath || dumperResult.reusableProjectPath)}
                      >
                        OPEN FOLDER
                      </button>
                      <button
                        class="btn-zone-action"
                        onclick={() => { dumperResult = null; dumperProgress = null; }}
                      >
                        NEW ANALYSIS
                      </button>
                    </div>
                  </div>
                {/if}
              </div>
            </div>
          </section>

        {:else if activePage === 'composition'}
          <section class="composition-page" aria-label="Composition and layer segmentation">
            <div class="composition-container">
              <!-- DROP ZONES: CHARACTER & BACKGROUND -->
              <div class="composition-grid">
                <!-- DROP ZONE 1: CHARACTER (PNG TRANSPARENT) -->
                <div
                  class="remap-drop-zone comp-drop-zone"
                  class:filled={Boolean(compCharacterPath)}
                  class:has-error={Boolean(compCharacterError)}
                  class:hovering={hoveredZone === 'comp-character'}
                  data-zone="comp-character"
                  ondragenter={(e) => { e.preventDefault(); hoveredZone = 'comp-character'; }}
                  ondragover={(e) => { e.preventDefault(); hoveredZone = 'comp-character'; }}
                  ondragleave={(e) => { e.preventDefault(); if (hoveredZone === 'comp-character') hoveredZone = null; }}
                  ondrop={(e) => { e.preventDefault(); hoveredZone = null; }}
                  onclick={() => !compCharacterPath && handlePickFile('comp-character')}
                  onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && !compCharacterPath && handlePickFile('comp-character')}
                  role="button"
                  tabindex="0"
                >
                  {#if compCharacterPath}
                    <div class="zone-filled-content">
                      <div class="zone-header">
                        <span class="zone-tag">CHARACTER</span>
                        <span class="pro-dot active"></span>
                      </div>
                      <div class="zone-title">CHARACTER</div>
                      <div class="zone-filename mono" title={compCharacterPath}>{getFileName(compCharacterPath)}</div>
                      <div class="zone-actions">
                        <button class="btn-zone-action" onclick={(e) => handlePickFile('comp-character', e)}>REPLACE</button>
                        <button class="btn-zone-action danger" onclick={(e) => clearZone('comp-character', e)}>REMOVE</button>
                      </div>
                    </div>
                  {:else}
                    <div class="zone-empty-content">
                      <p class="zone-prompt">DRAG CHARACTER</p>
                      <span class="zone-sublabel">TRANSPARENT PNG IMAGE (*.png)</span>
                      {#if compCharacterError}
                        <span class="zone-error-msg">{compCharacterError}</span>
                      {/if}
                    </div>
                  {/if}
                </div>

                <!-- DROP ZONE 2: BACKGROUND (IMAGE / VIDEO) -->
                <div
                  class="remap-drop-zone comp-drop-zone"
                  class:filled={Boolean(compBackgroundPath)}
                  class:has-error={Boolean(compBackgroundError)}
                  class:hovering={hoveredZone === 'comp-background'}
                  data-zone="comp-background"
                  ondragenter={(e) => { e.preventDefault(); hoveredZone = 'comp-background'; }}
                  ondragover={(e) => { e.preventDefault(); hoveredZone = 'comp-background'; }}
                  ondragleave={(e) => { e.preventDefault(); if (hoveredZone === 'comp-background') hoveredZone = null; }}
                  ondrop={(e) => { e.preventDefault(); hoveredZone = null; }}
                  onclick={() => !compBackgroundPath && handlePickFile('comp-background')}
                  onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && !compBackgroundPath && handlePickFile('comp-background')}
                  role="button"
                  tabindex="0"
                >
                  {#if compBackgroundPath}
                    <div class="zone-filled-content">
                      <div class="zone-header">
                        <span class="zone-tag">BACKGROUND</span>
                        <span class="pro-dot active"></span>
                      </div>
                      <div class="zone-title">BACKGROUND</div>
                      <div class="zone-filename mono" title={compBackgroundPath}>{getFileName(compBackgroundPath)}</div>
                      <div class="zone-actions">
                        <button class="btn-zone-action" onclick={(e) => handlePickFile('comp-background', e)}>REPLACE</button>
                        <button class="btn-zone-action danger" onclick={(e) => clearZone('comp-background', e)}>REMOVE</button>
                      </div>
                    </div>
                  {:else}
                    <div class="zone-empty-content">
                      <p class="zone-prompt">DRAG BACKGROUND</p>
                      <span class="zone-sublabel">IMAGE OR VIDEO (PNG, JPG, MP4, MKV...)</span>
                      {#if compBackgroundError}
                        <span class="zone-error-msg">{compBackgroundError}</span>
                      {/if}
                    </div>
                  {/if}
                </div>
              </div>

              <!-- COMPOSITION OPS STACK & ACTION PANEL -->
              <div class="composition-actions-panel">
                <!-- 1. COMPOSITION OPS STACK (Manga Recipes) -->
                <div class="dumper-section-box comp-ops-box">
                  <div class="section-box-header">
                    <span class="section-box-title">LAYERED COMPOSITOR OPS (MANGA RECIPES)</span>
                    <span class="pro-dot active"></span>
                  </div>
                  <div class="comp-ops-grid mono">
                    {#each compOps as op}
                      <div class="comp-op-item" class:disabled={!op.enabled}>
                        <div class="op-top-row">
                          <label class="op-label-toggle">
                            <input
                              type="checkbox"
                              bind:checked={op.enabled}
                              class="op-checkbox"
                            />
                            <span class="op-title">{op.name.toUpperCase()}</span>
                          </label>
                          <span class="op-blend-badge">{op.blend_mode.toUpperCase()}</span>
                        </div>
                        <div class="op-control-row">
                          <span class="op-pct-label">OPACITY: {Math.round(op.opacity * 100)}%</span>
                          <input
                            type="range"
                            min="0"
                            max="1"
                            step="0.05"
                            bind:value={op.opacity}
                            class="op-range-slider"
                            disabled={!op.enabled}
                          />
                        </div>
                        <div class="op-desc-row">
                          {#if op.id === 'drop_shadow'}
                            <span class="op-desc">Cast multiply shadow onto background</span>
                          {:else if op.id === 'light_wrap'}
                            <span class="op-desc">Bleed background ambient light onto character edge</span>
                          {:else if op.id === 'tint_raccord'}
                            <span class="op-desc">Color match character to background ambient hue</span>
                          {:else if op.id === 'rim_light'}
                            <span class="op-desc">Additive light contour on character silhouette</span>
                          {/if}
                        </div>
                      </div>
                    {/each}
                  </div>
                </div>

                <!-- CAMERA & 2.5D PARALLAX SECTION -->
                <div class="comp-stack-container" style="margin-top: 14px;">
                  <div class="comp-stack-header">
                    <div class="stack-title-row">
                      <span class="pro-tag">CAMÉRA & PARALLAXE 2.5D</span>
                      <span class="stack-count mono">PROCÉDURAL + BEATS</span>
                    </div>
                  </div>

                  <div class="comp-ops-list">
                    <div class="comp-op-card active">
                      <div class="op-card-top">
                        <div class="op-label-group">
                          <span class="op-name">Parallax Strength</span>
                          <span class="op-blend-badge mono">2.5D DEPTH</span>
                        </div>
                        <span class="op-val-display mono">{Math.round(compParallaxStrength * 100)}%</span>
                      </div>
                      <div class="op-slider-row">
                        <input
                          type="range"
                          min="0.0"
                          max="1.0"
                          step="0.05"
                          bind:value={compParallaxStrength}
                          class="op-slider"
                        />
                      </div>
                      <div class="op-desc-row">
                        <span class="op-desc">Depth separation amplitude across character layers during camera pan/zoom</span>
                      </div>
                    </div>

                    <div class="comp-op-card active">
                      <div class="op-card-top">
                        <div class="op-label-group">
                          <span class="op-name">Beat Punch Intensity</span>
                          <span class="op-blend-badge mono">DOWNBEAT ZOOM</span>
                        </div>
                        <span class="op-val-display mono">{Math.round(compBeatPunchIntensity * 100)}%</span>
                      </div>
                      <div class="op-slider-row">
                        <input
                          type="range"
                          min="0.0"
                          max="1.0"
                          step="0.05"
                          bind:value={compBeatPunchIntensity}
                          class="op-slider"
                        />
                      </div>
                      <div class="op-desc-row">
                        <span class="op-desc">Camera dynamic zoom impulse and micro-shake on detected musical downbeats</span>
                      </div>
                    </div>
                  </div>
                </div>

                <!-- POST-FX SECTION -->
                <div class="comp-stack-container" style="margin-top: 14px;">
                  <div class="comp-stack-header">
                    <div class="stack-title-row">
                      <span class="pro-tag">POST-FX</span>
                      <span class="stack-count mono">LIGHT WRAP + CHROMATIC + IMPACT</span>
                    </div>
                  </div>

                  <div class="comp-ops-list">
                    <div class="comp-op-card active">
                      <div class="op-card-top">
                        <div class="op-label-group">
                          <span class="op-name">Light Wrap Intensity</span>
                          <span class="op-blend-badge mono">CONTOUR BLEED</span>
                        </div>
                        <span class="op-val-display mono">{Math.round(compLightWrapIntensity * 100)}%</span>
                      </div>
                      <div class="op-slider-row">
                        <input
                          type="range"
                          min="0.0"
                          max="1.0"
                          step="0.05"
                          bind:value={compLightWrapIntensity}
                          class="op-slider"
                        />
                      </div>
                      <div class="op-desc-row">
                        <span class="op-desc">Ambient background light bleeding onto character silhouette edges</span>
                      </div>
                    </div>

                    <div class="comp-op-card active">
                      <div class="op-card-top">
                        <div class="op-label-group">
                          <span class="op-name">Chromatic Aberration</span>
                          <span class="op-blend-badge mono">RADIAL RGB SHIFT</span>
                        </div>
                        <span class="op-val-display mono">{Math.round(compChromaticAberration * 100)}%</span>
                      </div>
                      <div class="op-slider-row">
                        <input
                          type="range"
                          min="0.0"
                          max="1.0"
                          step="0.05"
                          bind:value={compChromaticAberration}
                          class="op-slider"
                        />
                      </div>
                      <div class="op-desc-row">
                        <span class="op-desc">Radial RGB channel displacement with reactive spikes on downbeats</span>
                      </div>
                    </div>

                    <div class="comp-op-card active">
                      <div class="op-card-top">
                        <div class="op-label-group">
                          <span class="op-name">Impact Blur Strength</span>
                          <span class="op-blend-badge mono">BEAT SMEAR</span>
                        </div>
                        <span class="op-val-display mono">{Math.round(compImpactBlurStrength * 100)}%</span>
                      </div>
                      <div class="op-slider-row">
                        <input
                          type="range"
                          min="0.0"
                          max="1.0"
                          step="0.05"
                          bind:value={compImpactBlurStrength}
                          class="op-slider"
                        />
                      </div>
                      <div class="op-desc-row">
                        <span class="op-desc">Directional radial smear triggered momentarily on major impact frames</span>
                      </div>
                    </div>
                  </div>
                </div>

                <!-- 2. RENDER ACTION BUTTONS -->
                <div class="comp-actions-row">
                  <button
                    class="btn-render dumper-run-btn"
                    disabled={!compCharacterPath || isRenderingComposition || isRenderingPreview}
                    onclick={runCompositionMeshPreview}
                    type="button"
                    title="Generate a 3-second procedurally animated mesh preview with sway, breathing and blink"
                  >
                    {#if isRenderingPreview}
                      <span>ANIMATING MESH ({compPreviewProgress ? `${compPreviewProgress.percent}%` : 'RUNNING'})...</span>
                    {:else}
                      <span>🎬 RENDER PREVIEW (3s)</span>
                    {/if}
                  </button>

                  <button
                    class="btn-pro-secondary mono comp-sidecar-btn"
                    disabled={!compCharacterPath || !compBackgroundPath || isRenderingComposition || isRenderingPreview}
                    onclick={runCompositionRender}
                    type="button"
                  >
                    {#if isRenderingComposition}
                      <span>COMPOSITING ({compRenderProgress ? `${compRenderProgress.percent}%` : 'RUNNING'})...</span>
                    {:else}
                      <span>⚡ RENDER COMPOSITION</span>
                    {/if}
                  </button>

                  <button
                    class="btn-pro-secondary mono comp-sidecar-btn"
                    disabled={!compCharacterPath || isSegmenting || isRenderingComposition || isRenderingPreview}
                    onclick={runCompositionSegmentation}
                    type="button"
                  >
                    {#if isSegmenting}
                      <span>SEGMENTING (SEE-THROUGH)...</span>
                    {:else}
                      <span>🧩 SEGMENT CHARACTER (SEE-THROUGH)</span>
                    {/if}
                  </button>
                </div>

                <!-- 3. MESH PREVIEW PROGRESS BAR -->
                {#if isRenderingPreview && compPreviewProgress}
                  <div class="dumper-progress-card">
                    <div class="dumper-progress-header">
                      <span class="progress-phase mono">{compPreviewProgress.phase || 'MESH_ANIM'}</span>
                      <span class="progress-pct mono">{compPreviewProgress.percent}%</span>
                    </div>
                    <div class="progress-bar-bg">
                      <div class="progress-bar-fill" style={`width: ${compPreviewProgress.percent}%`}></div>
                    </div>
                    <p class="progress-msg mono">{compPreviewProgress.message || 'Deforming mesh and compositing...'}</p>
                  </div>
                {/if}

                <!-- 3B. MESH PREVIEW RESULT CARD -->
                {#if compPreviewResult}
                  <div class="dumper-result-card comp-render-card">
                    <div class="result-header">
                      <div class="result-title-row">
                        <span class="zone-tag">ANIMATION PREVIEW</span>
                        <span class="style-badge mono">3S MESH PREVIEW (MP4)</span>
                        <span class="pro-dot active"></span>
                      </div>
                      <span class="result-timestamp mono">{compPreviewResult.timestamp}</span>
                    </div>

                    <div class="done-path-box" style="margin: 12px 0;">
                      <span class="stat-label">OUTPUT FILE:</span>
                      <span class="saved-path-text mono" title={compPreviewResult.outputPath}>{compPreviewResult.outputPath}</span>
                    </div>

                    <div class="result-footer-actions">
                      <button
                        class="btn-apply-project mono"
                        onclick={() => handleOpenCompFolder(compPreviewResult.outputPath)}
                        title="Open folder in Windows Explorer"
                      >
                        📂 OPEN FOLDER
                      </button>
                      <button
                        class="btn-zone-action"
                        onclick={() => { compPreviewResult = null; compPreviewProgress = null; }}
                      >
                        PREVIEW AGAIN
                      </button>
                    </div>
                  </div>
                {/if}

                <!-- 3C. FULL COMPOSITION RENDER PROGRESS BAR -->
                {#if isRenderingComposition && compRenderProgress}
                  <div class="dumper-progress-card">
                    <div class="dumper-progress-header">
                      <span class="progress-phase mono">{compRenderProgress.phase || 'COMPOSITING'}</span>
                      <span class="progress-pct mono">{compRenderProgress.percent}%</span>
                    </div>
                    <div class="progress-bar-bg">
                      <div class="progress-bar-fill" style={`width: ${compRenderProgress.percent}%`}></div>
                    </div>
                    <p class="progress-msg mono">{compRenderProgress.message || 'Processing frames...'}</p>
                  </div>
                {/if}

                <!-- 4. RENDER RESULT CARD -->
                {#if compRenderResult}
                  <div class="dumper-result-card comp-render-card">
                    <div class="result-header">
                      <div class="result-title-row">
                        <span class="zone-tag">COMPOSITION RESULT</span>
                        <span class="style-badge mono">{compRenderResult.isVideo ? 'VIDEO (MP4)' : 'IMAGE (PNG)'}</span>
                        <span class="pro-dot active"></span>
                      </div>
                      <span class="result-timestamp mono">{compRenderResult.timestamp}</span>
                    </div>

                    <div class="done-path-box" style="margin: 12px 0;">
                      <span class="stat-label">OUTPUT FILE:</span>
                      <span class="saved-path-text mono" title={compRenderResult.outputPath}>{compRenderResult.outputPath}</span>
                    </div>

                    <div class="result-footer-actions">
                      <button
                        class="btn-apply-project mono"
                        onclick={() => handleOpenCompFolder(compRenderResult.outputPath)}
                        title="Open folder in Windows Explorer"
                      >
                        📂 OPEN FOLDER
                      </button>
                      <button
                        class="btn-zone-action"
                        onclick={() => { compRenderResult = null; compRenderProgress = null; }}
                      >
                        RENDER AGAIN
                      </button>
                    </div>
                  </div>
                {/if}

                <!-- 5. ERROR CARD -->
                {#if compGpuError}
                  <div class="comp-error-card">
                    <div class="comp-error-header">
                      <span class="zone-tag" style="color: #ef4444; border-color: rgba(239,68,68,0.4);">NOTICE</span>
                    </div>
                    <p class="comp-error-msg mono">{compGpuError}</p>
                  </div>
                {/if}

                <!-- 6. POST-SEGMENTATION LAYER STACK VIEW (IF SEGMENTED) -->
                {#if compResult}
                  <div class="dumper-result-card comp-stack-card">
                    <div class="result-header">
                      <div class="result-title-row">
                        <span class="zone-tag">LAYER STACK</span>
                        <span class="style-badge mono">{compResult.layersCount} SEMANTIC LAYERS</span>
                        <span class="pro-dot active"></span>
                      </div>
                      <span class="result-timestamp mono">SCHEMA V1</span>
                    </div>

                    <!-- LAYERS LIST -->
                    <div class="layers-stack-list">
                      {#each compResult.layers as layer}
                        <div class="layer-card">
                          <div class="layer-thumb-box">
                            {#if layer.thumbnailBase64}
                              <img class="layer-thumb" src={layer.thumbnailBase64} alt={layer.name} />
                            {:else}
                              <div class="layer-thumb-placeholder mono">PNG</div>
                            {/if}
                          </div>
                          <div class="layer-info">
                            <div class="layer-name mono">{layer.name.toUpperCase()}</div>
                            <div class="layer-filename mono">{layer.file}</div>
                          </div>
                          <div class="layer-meta">
                            <span class="z-order-badge mono">Z: {layer.zOrder}</span>
                            <span class="layer-status-pill mono" class:active={layer.hasContent !== false}>
                              {layer.hasContent !== false ? 'ACTIVE' : 'EMPTY'}
                            </span>
                          </div>
                        </div>
                      {/each}
                    </div>

                    <!-- FOOTER ACTIONS -->
                    <div class="result-footer-actions">
                      <button
                        class="btn-apply-project mono"
                        onclick={handleSaveComposition}
                        title="Save comp_project.json"
                      >
                        💾 SAVE COMPOSITION
                      </button>
                      <button
                        class="btn-zone-action"
                        onclick={() => handleOpenCompFolder(compResult.outputDir)}
                      >
                        OPEN FOLDER
                      </button>
                      <button
                        class="btn-zone-action"
                        onclick={() => { compResult = null; compGpuError = ''; }}
                      >
                        NEW SEGMENTATION
                      </button>
                    </div>
                  </div>
                {/if}
              </div>
            </div>
          </section>

        {:else if activePage === 'about'}
          <section class="about-page" aria-label="Project credits">
            <header class="about-identity">
              <img class="about-app-logo" src={appLogo} alt="cia app logo" />
              <div class="about-app-copy">
                <h1>cia app <span>V{appVersion}</span></h1>
                <p>Local render workflow, credits and contact.</p>
              </div>
              <div class="about-contacts">
                <button class="about-update-btn" onclick={() => checkForAppUpdates(true)} aria-label="Check for cia app updates" disabled={updateState === 'checking' || updateState === 'downloading'}>
                  {#if updateState === 'checking'}
                    <span>CHECKING...</span>
                  {:else if availableUpdate}
                    <span class="update-ready-text"><span class="pro-dot active"></span> UPDATE V{availableUpdate.version}</span>
                  {:else}
                    <span>CHECK FOR UPDATES</span>
                  {/if}
                </button>
                <button class="discord-contact" onclick={copyDiscordHandle} aria-label="Copy cia app Discord handle">
                  <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M19.5 4.7a16.8 16.8 0 0 0-4.1-1.3l-.5 1.1a15.1 15.1 0 0 0-5.8 0l-.5-1.1A16.9 16.9 0 0 0 4.5 4.7C1.9 8.5 1.2 12.2 1.6 15.8a16.8 16.8 0 0 0 5 2.5l1.2-1.6a9.8 9.8 0 0 1-1.9-.9l.5-.4c3.7 1.7 7.7 1.7 11.4 0l.5.4c-.6.4-1.2.7-1.9.9l1.2 1.6a16.6 16.6 0 0 0 5-2.5c.5-4.2-.8-7.8-3.1-11.1ZM8.7 13.6c-1 0-1.8-.9-1.8-2s.8-2 1.8-2 1.8.9 1.8 2-.8 2-1.8 2Zm6.6 0c-1 0-1.8-.9-1.8-2s.8-2 1.8-2 1.8.9 1.8 2-.8 2-1.8 2Z" /></svg>
                  <span>{discordCopyFeedback ? 'COPIED' : 'cia2013'}</span>
                </button>
                <button class="github-contact" onclick={() => openAboutLink(PROJECT_REPOSITORY_URL)} aria-label="Open cia app on GitHub">
                  <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 .5A11.5 11.5 0 0 0 8.4 22.9c.6.1.8-.3.8-.6v-2.2c-3.4.7-4.1-1.4-4.1-1.4-.6-1.4-1.4-1.8-1.4-1.8-1.1-.8.1-.8.1-.8 1.2.1 1.9 1.3 1.9 1.3 1.1 1.9 2.8 1.3 3.5 1 .1-.8.4-1.3.8-1.6-2.7-.3-5.5-1.3-5.5-6 0-1.3.5-2.4 1.2-3.3-.1-.3-.5-1.6.1-3.3 0 0 1-.3 3.3 1.2a11.3 11.3 0 0 1 6 0c2.3-1.5 3.3-1.2 3.3-1.2.6 1.7.2 3 .1 3.3.8.9 1.2 2 1.2 3.3 0 4.7-2.8 5.7-5.5 6 .4.4.8 1.1.8 2.2v3.2c0 .3.2.7.8.6A11.5 11.5 0 0 0 12 .5Z" /></svg>
                </button>
              </div>
            </header>
            <div class="about-grid">
              {#each ABOUT_LINKS as link}
                <button class="about-link-card" onclick={() => openAboutLink(link.url)} aria-label={`Open ${link.name} website`}>
                  <ProjectMark kind={link.mark} />
                  <div class="about-link-copy">
                    <h2>{link.name}</h2>
                    <p>{link.detail}</p>
                  </div>
                  <span class="about-link-arrow" aria-hidden="true">&gt;</span>
                </button>
              {/each}
            </div>
          </section>
        {/if}
      </div>
    {/key}
  </main>

  <!-- Auto-Updater Modal Overlay -->
  {#if showUpdateModal && availableUpdate}
    <div class="modal-backdrop" onclick={(e) => { if (e.target === e.currentTarget && updateState !== 'downloading') showUpdateModal = false; }} role="presentation">
      <div class="modal-card update-modal-card" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()} role="dialog" aria-modal="true" tabindex="0">
        <div class="modal-header">
          <h2>UPDATE AVAILABLE - V{availableUpdate.version}</h2>
          {#if updateState !== 'downloading'}
            <button class="btn-close-modal" onclick={() => showUpdateModal = false}>X</button>
          {/if}
        </div>
        <div class="modal-body">
          <div class="update-version-banner">
            <span class="pro-dot active"></span>
            <span>A new version of cia app is ready to install (current: V{appVersion})</span>
          </div>
          {#if availableUpdate.body}
            <div class="update-notes-box">
              <div class="update-notes-title">RELEASE NOTES</div>
              <div class="update-notes-content">{availableUpdate.body}</div>
            </div>
          {/if}
          {#if updateState === 'downloading'}
            <div class="update-downloading-box">
              <div class="install-progress-header">
                <span class="pro-dot active"></span>
                <span>DOWNLOADING & INSTALLING UPDATE</span>
              </div>
              <div class="pro-progress-row">
                <div class="pro-track">
                  <div class="pro-fill" style="width: {updateContentLength > 0 ? Math.min(100, (updateDownloadedBytes / updateContentLength) * 100) : 0}%"></div>
                </div>
                <span class="pro-percent-readout">
                  {updateContentLength > 0 ? Math.round((updateDownloadedBytes / updateContentLength) * 100) : 0}%
                </span>
              </div>
              <div class="update-bytes-readout">
                {formatBytes(updateDownloadedBytes)} / {updateContentLength > 0 ? formatBytes(updateContentLength) : '...'}
              </div>
            </div>
          {:else if updateState === 'ready'}
            <div class="update-ready-box">
              <span class="pro-dot active"></span>
              <span>UPDATE APPLIED - RESTARTING CIA APP...</span>
            </div>
          {:else if updateState === 'error'}
            <div class="setup-alert" style="margin-top: 14px;">{updateErrorMessage}</div>
          {/if}
        </div>
        <div class="modal-footer">
          {#if updateState === 'downloading' || updateState === 'ready'}
            <span class="update-installing-status">Please wait while the update finishes...</span>
          {:else}
            <button class="btn-pro-secondary" onclick={() => showUpdateModal = false}>LATER</button>
            <button class="btn-primary-modal" onclick={installAppUpdate}>UPDATE & RELAUNCH</button>
          {/if}
        </div>
      </div>
    </div>
  {/if}

  <!-- T17/T18 Effect Details, Previews & Custom Params Modal -->
  {#if showDetailsModal}
    <div
      class="modal-backdrop"
      onclick={(e) => { if (e.target === e.currentTarget) showDetailsModal = false; }}
      role="presentation"
    >
      <div
        class="modal-card details-modal-card"
        onclick={(e) => e.stopPropagation()}
        onkeydown={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        tabindex="0"
      >
        <div class="modal-header">
          <div class="modal-header-titles">
            <h2>EFFECT DETAILS & TOGGLES</h2>
            <span class="modal-subtitle">Configure individual effects for project plans and preview algorithm output</span>
          </div>
          <button class="btn-close-modal" onclick={() => showDetailsModal = false} aria-label="Close details">âœ•</button>
        </div>

        <div class="details-toolbar">
          <div class="details-toolbar-left">
            <span class="effects-count-badge">{availableEffects.filter(e => effectOverrides[e.id]).length} / {availableEffects.length} ACTIVE</span>
          </div>
          <div class="details-toolbar-actions">
            <button class="btn-toolbar" onclick={handleSelectAllEffects} type="button">SELECT ALL</button>
            <button class="btn-toolbar" onclick={handleDeselectAllEffects} type="button">DESELECT ALL</button>
            <button class="btn-toolbar" onclick={handleResetEffectsToPreset} type="button">RESET TO STYLE</button>
          </div>
        </div>

        <div class="modal-body details-modal-body">

          <!-- SECTION 1: EFFECT TOGGLES & PREVIEWS -->
          <div class="details-section-header">EFFECT TOGGLES &amp; PREVIEWS</div>
          <div class="effects-grid">
            {#each availableEffects as effect (effect.id)}
              <div
                class="effect-card"
                class:active={effectOverrides[effect.id]}
                onclick={() => effectOverrides[effect.id] = !effectOverrides[effect.id]}
                role="button"
                tabindex="0"
                onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { effectOverrides[effect.id] = !effectOverrides[effect.id]; e.preventDefault(); } }}
              >
                <div
                  class="effect-card-thumb-wrap"
                  role="presentation"
                  onmouseenter={(e) => {
                    hoveredPreview = effect;
                    hoverPos = { x: e.clientX, y: e.clientY };
                  }}
                  onmousemove={(e) => {
                    hoverPos = { x: e.clientX, y: e.clientY };
                  }}
                  onmouseleave={() => {
                    hoveredPreview = null;
                  }}
                >
                  <img src={effect.previewDataUrl} alt={effect.name} class="effect-thumb-img" width="128" height="128" />
                  <span class="hover-hint-badge">HOVER TO ENLARGE</span>
                </div>

                <div class="effect-card-info">
                  <div class="effect-card-top">
                    <div class="effect-card-title-row">
                      <span class="effect-card-name">{effect.name}</span>
                      <span class="effect-category-pill">{effect.category}</span>
                    </div>
                    <label class="effect-toggle-label">
                      <input
                        type="checkbox"
                        checked={effectOverrides[effect.id]}
                        onclick={(e) => e.stopPropagation()}
                        onchange={(e) => effectOverrides[effect.id] = e.target.checked}
                      />
                      <span class="effect-toggle-custom"></span>
                    </label>
                  </div>
                  <p class="effect-card-desc">{effect.description}</p>
                </div>
              </div>
            {/each}
          </div>

          <!-- SECTION 2: CUSTOM PARAMETERS -->
          {#if customParams !== null}
          <div class="custom-params-section">
            <div class="custom-params-header">
              <span class="details-section-header">CUSTOM PARAMETERS</span>
              <button class="btn-toolbar btn-reset-defaults" onclick={handleResetToStyleDefaults} type="button">RESET TO STYLE DEFAULTS</button>
            </div>

            <!-- SHAKES -->
            <div class="cp-group">
              <div class="cp-group-label">SHAKES</div>
              <div class="cp-sliders">
                <GlowSlider id="cp-shake-a0" label="Harmonic Amplitude (a0)" bind:value={customParams.shakeA0} min={0} max={30} step={0.1} precision={2} />
                <GlowSlider id="cp-shake-omega" label="Harmonic Frequency (Ï‰)" bind:value={customParams.shakeOmega} min={0} max={30} step={0.1} precision={2} />
                <GlowSlider id="cp-shake-k" label="Harmonic Decay (k)" bind:value={customParams.shakeK} min={0} max={10} step={0.01} precision={3} />
                <GlowSlider id="cp-bouncy-amp" label="Bouncy Amplitude (px)" bind:value={customParams.bouncyAmplitude} min={0} max={60} step={0.5} precision={1} />
                <GlowSlider id="cp-dissolve-pct" label="Dissolve %" bind:value={customParams.dissolvePct} min={0} max={1} step={0.01} precision={2} />
                <GlowSlider id="cp-skew-s0" label="Skew Degrees" bind:value={customParams.skewS0} min={0} max={30} step={0.5} precision={1} />
                <GlowSlider id="cp-squish-ymin" label="Squish scale_y_min" bind:value={customParams.squishScaleYMin} min={0.5} max={1} step={0.01} precision={2} />
                <GlowSlider id="cp-squish-xmax" label="Squish scale_x_max" bind:value={customParams.squishScaleXMax} min={1} max={1.5} step={0.01} precision={2} />
                <GlowSlider id="cp-optics-k0" label="Optics k0" bind:value={customParams.opticsK0} min={0} max={0.3} step={0.001} precision={3} />
                <GlowSlider id="cp-stretch-scale" label="Warp Stretch Max Scale" bind:value={customParams.stretchScale} min={1} max={2} step={0.01} precision={2} />
              </div>
            </div>

            <!-- ZOOM -->
            <div class="cp-group">
              <div class="cp-group-label">ZOOM</div>
              <div class="cp-sliders">
                <GlowSlider id="cp-zoom-start" label="Scale Start" bind:value={customParams.zoomScaleStart} min={0.8} max={1.5} step={0.01} precision={2} />
                <GlowSlider id="cp-zoom-end" label="Scale End" bind:value={customParams.zoomScaleEnd} min={0.8} max={1.5} step={0.01} precision={2} />
                <GlowSlider id="cp-zoom-beat-offset" label="Beat Offset Frames" bind:value={customParams.zoomBeatOffsetFrames} min={0} max={8} step={1} precision={0} />
              </div>
            </div>

            <!-- AMBIANCE -->
            <div class="cp-group">
              <div class="cp-group-label">AMBIANCE</div>
              <div class="cp-sliders">
                <GlowSlider id="cp-flicker-amp" label="Flicker Amplitude" bind:value={customParams.flickerAmplitude} min={0} max={0.5} step={0.01} precision={2} />
                <GlowSlider id="cp-flicker-freq" label="Flicker Frequency (Hz)" bind:value={customParams.flickerFrequencyHz} min={1} max={30} step={0.5} precision={1} />
                <GlowSlider id="cp-exposure-peak" label="Exposure Flash Peak" bind:value={customParams.exposureFlashPeak} min={0} max={1} step={0.01} precision={2} />
                <GlowSlider id="cp-echo-alpha" label="Echo Alpha" bind:value={customParams.echoAlpha} min={0} max={0.9} step={0.01} precision={2} />
                <GlowSlider id="cp-echo-depth" label="Echo Depth (k)" bind:value={customParams.echoKDepth} min={1} max={8} step={1} precision={0} />
                <GlowSlider id="cp-tint-r" label="Tint R Offset" bind:value={customParams.tintROffset} min={-50} max={50} step={1} precision={0} />
                <GlowSlider id="cp-tint-g" label="Tint G Offset" bind:value={customParams.tintGOffset} min={-50} max={50} step={1} precision={0} />
                <GlowSlider id="cp-tint-b" label="Tint B Offset" bind:value={customParams.tintBOffset} min={-50} max={50} step={1} precision={0} />
                <GlowSlider id="cp-vignette" label="Vignette Strength" bind:value={customParams.vignetteStrength} min={0} max={1} step={0.01} precision={2} />
                <GlowSlider id="cp-scanlines" label="Scanlines Opacity" bind:value={customParams.scanlinesOpacity} min={0} max={0.5} step={0.01} precision={2} />
              </div>
            </div>

            <!-- TRANSITIONS -->
            <div class="cp-group">
              <div class="cp-group-label">TRANSITIONS</div>
              <div class="cp-sliders">
                <GlowSlider id="cp-warp-amp" label="Warp Bubble Amplitude" bind:value={customParams.warpBubbleAmplitude} min={0} max={1} step={0.01} precision={2} />
                <GlowSlider id="cp-warp-freq" label="Warp Bubble Frequency" bind:value={customParams.warpBubbleFrequency} min={0.5} max={5} step={0.1} precision={1} />
                <GlowSlider id="cp-wave-height" label="Wave Warp Height (px)" bind:value={customParams.waveWarpHeight} min={0} max={600} step={5} precision={0} />
                <GlowSlider id="cp-wave-speed" label="Wave Warp Speed" bind:value={customParams.waveWarpSpeed} min={0.5} max={4} step={0.1} precision={1} />
                <GlowSlider id="cp-slide-px" label="Slide Shake Pixels" bind:value={customParams.slideShakePixels} min={0} max={200} step={5} precision={0} />
              </div>
            </div>
          </div>
          {/if}

        </div>

        <div class="modal-footer">
          <button class="btn-primary-modal" onclick={() => showDetailsModal = false} type="button">APPLY &amp; CLOSE</button>
        </div>
      </div>
    </div>
  {/if}

  <!-- T17 Floating Large Preview Hover Box -->
  {#if hoveredPreview}
    <div
      class="large-preview-popover"
      style="left: {Math.min(window?.innerWidth ? window.innerWidth - 300 : 800, hoverPos.x + 24)}px; top: {Math.min(window?.innerHeight ? window.innerHeight - 340 : 600, Math.max(20, hoverPos.y - 140))}px;"
    >
      <div class="large-preview-header">
        <span class="large-preview-title">{hoveredPreview.name}</span>
        <span class="large-preview-cat">{hoveredPreview.category}</span>
      </div>
      <img
        src={hoveredPreview.previewDataUrl}
        alt={hoveredPreview.name}
        class="large-preview-img"
        width="256"
        height="256"
      />
      <div class="large-preview-footer">256 Ã— 256 GENERIC PATTERN PREVIEW</div>
    </div>
  {/if}

  <!-- Toast Notification Overlay -->
  {#if toast.show}
    <div class="toast" class:success={toast.type === 'success'} class:error={toast.type === 'error'}>
      {toast.message}
    </div>
  {/if}
</div>

<style>
  /* REFINED INDUSTRIAL DARK SLATE DESIGN SYSTEM */
  *, *::before, *::after {
    box-sizing: border-box;
    margin: 0;
    padding: 0;
    font-family: 'IBM Plex Sans Variable', 'IBM Plex Sans', -apple-system, sans-serif;
    -webkit-font-smoothing: antialiased;
  }

  :global(html), :global(body), :global(#app) {
    margin: 0;
    height: 100%;
    background: #050507;
    color: #e4e4e7;
    overflow: hidden;
    user-select: none;
  }

  .app-root {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: #050507;
    border: 1px solid #1c1c20;
  }

  /* Titlebar */
  .titlebar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    height: 34px;
    padding: 0 12px;
    background: #08080a;
    border-bottom: 1px solid #1c1c20;
  }

  .titlebar-brand { display: flex; align-items: center; }
  .titlebar-text { font-size: 11px; font-weight: 700; letter-spacing: 0.06em; color: #71717a; }
  .titlebar-controls { display: flex; gap: 2px; }

  .titlebar-btn {
    width: 32px;
    height: 24px;
    border: none;
    background: transparent;
    color: #71717a;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 11px;
    border-radius: 4px;
    transition: all 0.15s ease;
  }

  .titlebar-btn:hover { background: #1c1c20; color: #ffffff; }
  .titlebar-btn.close:hover { background: #dc2626; color: #ffffff; }

  /* Navigation Tabs */
  .tab-bar {
    display: flex;
    gap: 4px;
    padding: 8px 12px 0;
    background: #08080a;
    border-bottom: 1px solid #1c1c20;
    min-height: 40px;
  }

  .tab-bar button {
    position: relative;
    z-index: 0;
    min-height: 32px;
    padding: 8px 20px;
    background: #0d0d10;
    border: 1px solid #1c1c20;
    border-bottom-color: #1c1c20;
    border-radius: 6px 6px 0 0;
    color: #71717a;
    cursor: pointer;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.05em;
    transition: color 140ms ease, background-color 140ms ease, border-color 140ms ease;
  }

  .tab-bar button:hover { color: #e4e4e7; background: #16161a; }
  .tab-bar button.active {
    z-index: 1;
    color: #ffffff;
    background: #121215;
    border-color: rgba(255, 255, 255, 0.25);
    border-bottom: 1px solid #121215;
  }
  .tab-bar button:focus-visible {
    z-index: 2;
    outline: none;
    box-shadow: inset 0 0 0 1px #ffffff;
  }

  /* Main Content Area */
  .content-area {
    flex: 1;
    min-height: 0;
    overflow: hidden;
    padding: 12px 16px;
    background: #050507;
    display: flex;
    flex-direction: column;
  }

  .page-stage {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    justify-content: center;
    animation: page-enter 190ms cubic-bezier(0.16, 1, 0.3, 1) both;
  }

  @keyframes page-enter {
    from { opacity: 0; transform: translateY(5px); }
    to { opacity: 1; transform: translateY(0); }
  }

  @media (prefers-reduced-motion: reduce) {
    .page-stage { animation: none; }
  }

  /* Time Remap Page (3 Drop Zones) */
  .remap-page {
    width: min(100%, 920px);
    margin: auto;
    display: flex;
    flex-direction: column;
    gap: 14px;
    height: 100%;
    justify-content: center;
  }

  .remap-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 12px;
    flex: 1;
    max-height: 380px;
  }

  .remap-drop-zone {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    min-height: 320px;
    padding: 20px 14px;
    border: 1px solid rgba(255, 255, 255, 0.16);
    border-radius: 8px;
    background: #09090c;
    cursor: pointer;
    transition: all 0.2s cubic-bezier(0.16, 1, 0.3, 1);
    position: relative;
    text-align: center;
  }

  .remap-drop-zone:hover,
  .remap-drop-zone.hovering {
    border-color: rgba(255, 255, 255, 0.4);
    background: #111116;
    box-shadow: inset 0 0 20px rgba(255, 255, 255, 0.02);
  }

  .remap-drop-zone.has-error {
    border-color: rgba(239, 68, 68, 0.5);
    background: #0c0707;
  }

  .remap-drop-zone.filled {
    border-color: rgba(255, 255, 255, 0.28);
    background: #0d0d11;
    cursor: default;
  }

  .zone-empty-content {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    width: 100%;
  }

  .zone-prompt {
    font-size: 15px;
    font-weight: 700;
    letter-spacing: 0.05em;
    margin: 0;
    color: #e4e4e7;
  }

  .zone-sublabel {
    font-family: 'IBM Plex Mono', monospace;
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.04em;
    color: #71717a;
    line-height: 1.4;
  }

  .zone-error-msg {
    margin-top: 10px;
    padding: 6px 10px;
    background: rgba(127, 29, 29, 0.35);
    border: 1px solid #7f1d1d;
    border-radius: 4px;
    color: #fca5a5;
    font-family: 'IBM Plex Mono', monospace;
    font-size: 9px;
    font-weight: 600;
    line-height: 1.35;
  }

  .zone-filled-content {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: space-between;
    height: 100%;
    width: 100%;
    gap: 12px;
  }

  .zone-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    width: 100%;
  }

  .zone-tag {
    font-family: 'IBM Plex Mono', monospace;
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.06em;
    color: #a1a1aa;
    background: #141417;
    border: 1px solid #27272a;
    border-radius: 4px;
    padding: 2px 6px;
  }

  .zone-title {
    font-size: 14px;
    font-weight: 700;
    letter-spacing: 0.06em;
    color: #ffffff;
  }

  .zone-filename {
    font-size: 11px;
    font-weight: 600;
    color: #f4f4f5;
    background: #050507;
    border: 1px solid #1c1c20;
    border-radius: 6px;
    padding: 10px 12px;
    width: 100%;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    box-sizing: border-box;
  }

  .zone-actions {
    display: flex;
    gap: 8px;
    width: 100%;
  }

  .btn-zone-action {
    flex: 1;
    padding: 7px 10px;
    background: #141417;
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 5px;
    color: #d4d4d8;
    cursor: pointer;
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.04em;
    transition: all 0.15s ease;
  }

  .btn-zone-action:hover {
    border-color: rgba(255, 255, 255, 0.35);
    background: #1c1c20;
    color: #ffffff;
  }

  .btn-zone-action.danger:hover {
    border-color: #7f1d1d;
    background: #450a0a;
    color: #fecaca;
  }

  .continue-row {
    display: flex;
    justify-content: center;
    width: 100%;
    animation: page-enter 160ms ease-out both;
  }

  .btn-continue {
    width: 100%;
    max-width: 400px;
    padding: 12px 24px;
    background: #ffffff;
    color: #000000;
    border: 1px solid #ffffff;
    border-radius: 6px;
    font-weight: 800;
    font-size: 12px;
    letter-spacing: 0.06em;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.2s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .btn-continue:hover:not(:disabled) {
    background: #000000;
    color: #ffffff;
    border-color: #ffffff;
    box-shadow: 0 0 15px rgba(255, 255, 255, 0.15);
  }

  .btn-continue:disabled {
    opacity: 0.8;
    cursor: wait;
  }

  .spinner-inline {
    display: inline-block;
    width: 12px;
    height: 12px;
    border: 2px solid rgba(0, 0, 0, 0.25);
    border-top-color: #000000;
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
    margin-right: 8px;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  /* Full Settings Page (T4) */
  /* Compact Settings Page (T4.5) */
  .settings-page {
    width: min(100%, 880px);
    margin: auto;
    display: flex;
    flex-direction: column;
    height: 100%;
    justify-content: center;
    overflow-y: auto;
    padding: 0;
  }

  .settings-container {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .settings-sources-card,
  .settings-controls-card {
    background: #09090c;
    border: 1px solid #1c1c20;
    border-radius: 8px;
    padding: 8px 12px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .settings-sources-header {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }

  .settings-kicker {
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.08em;
    color: #71717a;
  }

  .settings-sources-header h1 {
    margin: 0;
    font-size: 13px;
    font-weight: 700;
    letter-spacing: 0.04em;
    color: #ffffff;
  }

  .compact-sources-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
    background: #050507;
    border: 1px solid #1c1c20;
    border-radius: 6px;
    padding: 5px 8px;
  }

  .source-row {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 20px;
  }

  .source-tag {
    font-family: 'IBM Plex Mono', monospace;
    font-size: 8px;
    font-weight: 700;
    letter-spacing: 0.06em;
    color: #a1a1aa;
    background: #141417;
    border: 1px solid #27272a;
    border-radius: 3px;
    padding: 1px 4px;
    min-width: 42px;
    text-align: center;
  }

  .source-name {
    font-size: 10px;
    font-weight: 600;
    color: #e4e4e7;
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .meta-pill {
    font-size: 8.5px;
    font-weight: 700;
    color: #4ade80;
    background: rgba(34, 197, 94, 0.1);
    border: 1px solid rgba(34, 197, 94, 0.25);
    border-radius: 3px;
    padding: 0 5px;
    white-space: nowrap;
  }

  /* Control Group Common */
  .control-group {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .group-label {
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.06em;
    color: #71717a;
  }

  /* Style Selector Cards */
  .styles-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 6px;
  }

  .style-card {
    background: #050507;
    border: 1px solid #1c1c20;
    border-radius: 6px;
    padding: 6px 8px;
    cursor: pointer;
    text-align: left;
    transition: all 0.15s ease;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .style-card:hover {
    border-color: rgba(255, 255, 255, 0.25);
    background: #0d0d10;
  }

  .style-card.selected {
    border-color: rgba(255, 255, 255, 0.5);
    background: #111116;
    box-shadow: inset 0 0 10px rgba(255, 255, 255, 0.03);
  }

  .style-card-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .style-name {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.05em;
    color: #ffffff;
  }

  .style-desc {
    font-size: 8.5px;
    line-height: 1.25;
    color: #71717a;
  }

  .style-card.selected .style-desc {
    color: #a1a1aa;
  }

  /* Aspect Ratio */
  .ar-control-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .ar-buttons-row {
    display: flex;
    gap: 6px;
  }

  .btn-ar {
    flex: 1;
    padding: 5px 8px;
    background: #050507;
    border: 1px solid #1c1c20;
    border-radius: 4px;
    color: #71717a;
    font-family: 'IBM Plex Mono', monospace;
    font-size: 9.5px;
    font-weight: 700;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .btn-ar:hover {
    color: #ffffff;
    border-color: rgba(255, 255, 255, 0.25);
  }

  .btn-ar.active {
    background: #121215;
    color: #ffffff;
    border-color: rgba(255, 255, 255, 0.5);
  }

  /* T19 Export Option Buttons */
  .options-buttons-row {
    display: flex;
    gap: 6px;
  }

  .btn-option {
    flex: 1;
    padding: 5px 8px;
    background: #050507;
    border: 1px solid #1c1c20;
    border-radius: 4px;
    color: #71717a;
    font-family: 'IBM Plex Mono', monospace;
    font-size: 9.5px;
    font-weight: 700;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .btn-option:hover {
    color: #ffffff;
    border-color: rgba(255, 255, 255, 0.25);
  }

  .btn-option.active {
    background: #121215;
    color: #ffffff;
    border-color: rgba(255, 255, 255, 0.5);
  }

  .custom-ar-inputs-inline {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .input-field {
    display: flex;
    align-items: center;
    background: #050507;
    border: 1px solid #1c1c20;
    border-radius: 3px;
    padding: 1px 4px;
    width: 60px;
  }

  .input-prefix {
    font-family: 'IBM Plex Mono', monospace;
    font-size: 8.5px;
    font-weight: 700;
    color: #71717a;
    margin-right: 3px;
  }

  .mono-input {
    background: transparent;
    border: none;
    outline: none;
    color: #ffffff;
    font-family: 'IBM Plex Mono', monospace;
    font-size: 9.5px;
    font-weight: 600;
    width: 100%;
  }

  .ar-divider {
    color: #71717a;
    font-size: 9px;
    font-weight: 700;
  }

  .inline-ar-error {
    color: #fca5a5;
    font-size: 8.5px;
    line-height: 1.2;
    margin-top: 1px;
  }

  /* Render Execution Cards (T6) */
  .render-progress-card {
    background: #09090c;
    border: 1px solid rgba(255, 255, 255, 0.35);
    border-radius: 8px;
    padding: 6px 10px;
    display: flex;
    flex-direction: column;
    gap: 5px;
    animation: page-enter 160ms cubic-bezier(0.16, 1, 0.3, 1) both;
  }

  .render-progress-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .phase-badge-row {
    display: flex;
    align-items: center;
    gap: 5px;
  }

  .render-phase-badge {
    font-size: 8px;
    font-weight: 700;
    letter-spacing: 0.05em;
    color: #52525b;
    padding: 2px 5px;
    border-radius: 3px;
    background: #050507;
    border: 1px solid #18181b;
    transition: all 0.2s ease;
  }

  .render-phase-badge.active {
    color: #ffffff;
    background: #18181b;
    border-color: #3f3f46;
  }

  .render-phase-arrow {
    color: #3f3f46;
    font-size: 8px;
    font-weight: 700;
  }

  .render-percent {
    font-size: 9.5px;
    font-weight: 700;
    color: #4ade80;
  }

  .progress-bar-container {
    height: 3px;
    background: #18181b;
    border-radius: 2px;
    overflow: hidden;
  }

  .progress-bar-fill {
    height: 100%;
    background: #ffffff;
    transition: width 0.15s ease;
  }

  .render-progress-footer {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .render-msg {
    font-size: 8px;
    font-weight: 500;
    color: #a1a1aa;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 75%;
  }

  .btn-cancel-render {
    padding: 2px 7px;
    background: #18181b;
    border: 1px solid #27272a;
    border-radius: 3px;
    color: #f87171;
    font-family: 'IBM Plex Mono', monospace;
    font-size: 8px;
    font-weight: 700;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .btn-cancel-render:hover {
    background: #27272a;
    border-color: #f87171;
  }

  /* Render Done Card */
  .render-done-card {
    background: #09090c;
    border: 1px solid rgba(74, 222, 128, 0.4);
    border-radius: 8px;
    padding: 6px 10px;
    display: flex;
    flex-direction: column;
    gap: 5px;
    animation: page-enter 160ms cubic-bezier(0.16, 1, 0.3, 1) both;
  }

  .render-done-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .done-title-row {
    display: flex;
    align-items: center;
    gap: 5px;
  }

  .render-done-title {
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.06em;
    color: #ffffff;
  }

  .done-specs {
    font-size: 8.5px;
    font-weight: 600;
    color: #4ade80;
  }

  .render-stats-grid {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 4px;
    background: #050507;
    border: 1px solid #1c1c20;
    border-radius: 4px;
    padding: 6px 8px;
  }

  .render-stat-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 2px 4px;
  }

  .done-path-box {
    display: flex;
    align-items: center;
    gap: 5px;
    background: #050507;
    border: 1px solid #1c1c20;
    border-radius: 4px;
    padding: 2px 6px;
    overflow: hidden;
  }

  .done-actions-row {
    display: flex;
    gap: 6px;
    margin-top: 1px;
  }

  .btn-open-folder {
    flex: 1;
    padding: 6px 12px;
    background: #ffffff;
    color: #000000;
    border: 1px solid #ffffff;
    border-radius: 4px;
    font-size: 9.5px;
    font-weight: 800;
    letter-spacing: 0.05em;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .btn-open-folder:hover {
    background: #e4e4e7;
  }

  /* Render Error Card */
  .render-error-card {
    background: #09090c;
    border: 1px solid rgba(248, 113, 113, 0.4);
    border-radius: 8px;
    padding: 6px 10px;
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  .render-error-title {
    font-size: 9px;
    font-weight: 700;
    color: #f87171;
  }

  .render-error-msg {
    font-size: 8.5px;
    color: #e4e4e7;
  }

  /* Plan Summary Card (T5) */
  .plan-summary-card {
    background: #09090c;
    border: 1px solid rgba(255, 255, 255, 0.28);
    border-radius: 8px;
    padding: 6px 10px;
    display: flex;
    flex-direction: column;
    gap: 5px;
    animation: page-enter 160ms cubic-bezier(0.16, 1, 0.3, 1) both;
  }

  .plan-summary-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .plan-summary-title {
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.06em;
    color: #ffffff;
  }

  .plan-summary-grid {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 5px;
  }

  .plan-stat {
    display: flex;
    flex-direction: column;
    gap: 1px;
    background: #050507;
    border: 1px solid #1c1c20;
    border-radius: 4px;
    padding: 3px 5px;
  }

  .stat-label {
    font-size: 7.5px;
    font-weight: 700;
    letter-spacing: 0.05em;
    color: #71717a;
  }

  .stat-value {
    font-size: 9.5px;
    font-weight: 700;
    color: #4ade80;
  }

  .plan-saved-path {
    display: flex;
    align-items: center;
    gap: 5px;
    background: #050507;
    border: 1px solid #1c1c20;
    border-radius: 4px;
    padding: 2px 6px;
    overflow: hidden;
  }

  .saved-path-text {
    font-size: 8.5px;
    font-weight: 600;
    color: #e4e4e7;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1;
  }

  /* T11: Echo/Trail toggle row */
  .toggle-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
  }

  .toggle-row-label {
    display: flex;
    flex-direction: column;
    gap: 3px;
    flex: 1;
  }

  .toggle-row-desc {
    font-size: 10px;
    color: #71717a;
    font-family: var(--font-mono);
    line-height: 1.4;
  }

  .toggle-btn {
    padding: 5px 14px;
    background: #18181b;
    color: #52525b;
    border: 1px solid #27272a;
    border-radius: 4px;
    font-size: 10px;
    font-weight: 700;
    font-family: var(--font-mono);
    letter-spacing: 0.08em;
    cursor: pointer;
    transition: background 0.15s, color 0.15s, border-color 0.15s;
    min-width: 48px;
    text-align: center;
    flex-shrink: 0;
  }

  .toggle-btn.active {
    background: #ffffff;
    color: #000000;
    border-color: #ffffff;
  }

  .toggle-btn:hover:not(.active) {
    border-color: #52525b;
    color: #a1a1aa;
  }

  /* T11: Flicker photosensitivity warning badge */
  .flicker-warning {
    background: rgba(220, 38, 38, 0.08);
    border: 1px solid rgba(220, 38, 38, 0.35);
    border-radius: 4px;
    padding: 6px 10px;
  }

  .flicker-badge {
    font-family: var(--font-mono);
    font-size: 10px;
    font-weight: 700;
    color: #ef4444;
    letter-spacing: 0.05em;
    text-transform: uppercase;
  }

  .fx-motion-only {
    color: #f59e0b;
    font-weight: 700;
  }

  /* Footer Actions */
  .settings-actions-footer {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-top: 1px;
  }

  .btn-run-process {
    padding: 9px 24px;
    background: #ffffff;
    color: #000000;
    border: 1px solid #ffffff;
    border-radius: 4px;
    font-size: 11px;
    font-weight: 800;
    letter-spacing: 0.05em;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .btn-run-process:hover {
    background: #000000;
    color: #ffffff;
    box-shadow: 0 0 15px rgba(255, 255, 255, 0.2);
  }

  /* About */
  .about-page {
    width: min(100%, 920px);
    margin: auto;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .about-identity {
    display: flex;
    align-items: center;
    gap: 12px;
    min-height: 88px;
    padding: 14px 16px;
    background: #09090c;
    border: 1px solid #1c1c20;
    border-radius: 8px;
  }

  .about-app-logo {
    width: 52px;
    height: 52px;
    flex: 0 0 auto;
    border-radius: 12px;
  }

  .about-app-copy { min-width: 0; }
  .about-app-copy h1 {
    margin: 4px 0;
    color: #fff;
    font-size: 17px;
    letter-spacing: 0.04em;
  }
  .about-app-copy h1 span {
    margin-left: 6px;
    color: #71717a;
    font-family: 'IBM Plex Mono', monospace;
    font-size: 10px;
    font-weight: 400;
    letter-spacing: 0.02em;
    vertical-align: middle;
  }
  .about-app-copy p {
    color: #a1a1aa;
    font-size: 11px;
    line-height: 1.35;
  }

  .about-contacts {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    margin-left: auto;
  }
  .discord-contact,
  .github-contact {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    margin-left: 0;
    padding: 8px 10px;
    border: 1px solid #27272a;
    border-radius: 5px;
    background: #0d0d10;
    color: #d4d4d8;
    cursor: pointer;
    font-family: 'IBM Plex Mono', monospace;
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.03em;
    transition: background-color 150ms ease, border-color 150ms ease, color 150ms ease;
  }
  .discord-contact svg,
  .github-contact svg {
    width: 17px;
    height: 17px;
    fill: currentColor;
  }
  .discord-contact:hover,
  .discord-contact:focus-visible,
  .github-contact:hover,
  .github-contact:focus-visible {
    border-color: rgba(255, 255, 255, 0.42);
    background: #16161a;
    color: #fff;
    outline: none;
  }
  .github-contact {
    justify-content: center;
    width: 37px;
    padding-inline: 0;
  }

  .about-link-card {
    background: #09090c;
    border: 1px solid #1c1c20;
    border-radius: 8px;
  }

  .about-link-card p {
    color: #a1a1aa;
    font-size: 11px;
    line-height: 1.35;
  }
  .about-link-card h2 {
    color: #e4e4e7;
    font-size: 11px;
    letter-spacing: 0.06em;
  }
  .about-grid {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 8px;
  }
  .about-link-card {
    min-height: 84px;
    padding: 12px;
    display: flex;
    align-items: center;
    text-align: left;
    color: inherit;
    cursor: pointer;
    transition: border-color 0.15s ease, background 0.15s ease, transform 0.15s ease;
    gap: 10px;
  }
  .about-link-card:hover,
  .about-link-card:focus-visible {
    background: #121215;
    border-color: rgba(255, 255, 255, 0.35);
    outline: none;
    transform: translateY(-1px);
  }
  .about-link-copy { min-width: 0; }
  .about-link-card h2 { margin-bottom: 4px; }
  .about-link-arrow { margin-left: auto; color: #71717a; font-size: 15px; }

  @media (max-width: 760px) {
    .about-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); }
    .page-stage { justify-content: flex-start; overflow-y: auto; }
    .about-identity { align-items: flex-start; }
    .about-contacts { align-self: center; }
  }

  /* Dots & Progress */
  .mono {
    font-family: 'IBM Plex Mono', monospace;
  }

  .pro-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #71717a;
  }
  .pro-dot.active { background: #ffffff; box-shadow: 0 0 6px rgba(255, 255, 255, 0.4); }

  .pro-progress-row {
    display: flex;
    align-items: center;
    gap: 14px;
  }

  .pro-track {
    flex: 1;
    height: 6px;
    background: #050507;
    border: 1px solid #27272a;
    border-radius: 4px;
    overflow: hidden;
  }

  .pro-fill {
    height: 100%;
    background: #ffffff;
    border-radius: 4px;
    transition: width 0.15s linear;
  }

  .pro-percent-readout {
    font-family: 'IBM Plex Mono', monospace;
    font-size: 14px;
    font-weight: 800;
    color: #ffffff;
    min-width: 48px;
    text-align: right;
  }

  /* Buttons */
  .btn-pro-secondary {
    background: #141417;
    color: #ffffff;
    border: 1px solid rgba(255, 255, 255, 0.2);
    border-radius: 4px;
    padding: 8px 16px;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.04em;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .btn-pro-secondary:hover {
    background: #1c1c20;
    border-color: rgba(255, 255, 255, 0.4);
  }

  /* Modal Overlay */
  .modal-backdrop {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.75);
    backdrop-filter: blur(4px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 3000;
  }

  .modal-card {
    background: #09090c;
    border: 1px solid rgba(255, 255, 255, 0.25);
    border-radius: 12px;
    width: 540px;
    max-width: 90vw;
    max-height: 85vh;
    display: flex;
    flex-direction: column;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.8);
    overflow: hidden;
  }

  .modal-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 14px 18px;
    background: #08080a;
    border-bottom: 1px solid #1c1c20;
  }

  .modal-header h2 {
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.05em;
    color: #ffffff;
  }

  .btn-close-modal {
    background: transparent;
    border: none;
    color: #71717a;
    cursor: pointer;
    font-size: 12px;
  }

  .btn-close-modal:hover { color: #ffffff; }

  .modal-body {
    padding: 18px;
    overflow-y: auto;
    flex: 1;
  }

  .modal-footer {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 14px 18px;
    background: #08080a;
    border-top: 1px solid #1c1c20;
  }

  .btn-primary-modal {
    background: #ffffff;
    color: #000000;
    border: 1px solid #ffffff;
    border-radius: 6px;
    padding: 8px 18px;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.04em;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .btn-primary-modal:hover {
    background: #000000;
    color: #ffffff;
    border-color: #ffffff;
  }

  /* Toast Overlay */
  .toast {
    position: fixed;
    bottom: 46px;
    right: 14px;
    padding: 8px 14px;
    background: #121215;
    border: 1px solid rgba(255, 255, 255, 0.25);
    border-radius: 6px;
    color: #ffffff;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.05em;
    z-index: 5000;
  }

  .setup-alert {
    padding: 12px;
    border: 1px solid #7f1d1d;
    border-radius: 6px;
    color: #fecaca;
    font-family: 'IBM Plex Mono', monospace;
    font-size: 11px;
  }

  /* Auto-Updater Styling */
  .titlebar-btn.update-badge {
    width: auto;
    padding: 0 8px;
    background: rgba(34, 197, 94, 0.15);
    color: #4ade80;
    border: 1px solid rgba(34, 197, 94, 0.3);
    font-family: 'IBM Plex Mono', monospace;
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.05em;
    display: flex;
    align-items: center;
    gap: 5px;
  }
  .titlebar-btn.update-badge:hover {
    background: rgba(34, 197, 94, 0.25);
    border-color: #4ade80;
    color: #ffffff;
  }
  .update-badge-dot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: #4ade80;
    box-shadow: 0 0 6px #4ade80;
  }

  .about-update-btn {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    padding: 8px 10px;
    border: 1px solid #27272a;
    border-radius: 5px;
    background: #0d0d10;
    color: #d4d4d8;
    cursor: pointer;
    font-family: 'IBM Plex Mono', monospace;
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.03em;
    transition: background-color 150ms ease, border-color 150ms ease, color 150ms ease;
  }
  .about-update-btn:hover:not(:disabled) {
    background: #18181b;
    border-color: #52525b;
    color: #ffffff;
  }
  .about-update-btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
  .update-ready-text {
    display: flex;
    align-items: center;
    gap: 6px;
    color: #4ade80;
  }

  .update-modal-card {
    width: 500px;
  }
  .update-version-banner {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 12px;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid #27272a;
    border-radius: 6px;
    font-family: 'IBM Plex Mono', monospace;
    font-size: 11px;
    color: #f4f4f5;
    margin-bottom: 14px;
  }
  .update-notes-box {
    margin-bottom: 16px;
  }
  .update-notes-title {
    font-family: 'IBM Plex Mono', monospace;
    font-size: 10px;
    font-weight: 700;
    color: #71717a;
    letter-spacing: 0.05em;
    margin-bottom: 6px;
  }
  .update-notes-content {
    background: #050507;
    border: 1px solid #1c1c20;
    border-radius: 6px;
    padding: 10px 12px;
    font-family: 'IBM Plex Mono', monospace;
    font-size: 11px;
    color: #a1a1aa;
    line-height: 1.5;
    max-height: 120px;
    overflow-y: auto;
    white-space: pre-wrap;
  }
  .update-downloading-box {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 14px;
    background: #050507;
    border: 1px solid #1c1c20;
    border-radius: 6px;
  }
  .update-bytes-readout {
    font-family: 'IBM Plex Mono', monospace;
    font-size: 10px;
    color: #71717a;
    text-align: right;
  }
  .update-ready-box {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px;
    background: rgba(34, 197, 94, 0.1);
    border: 1px solid rgba(34, 197, 94, 0.3);
    border-radius: 6px;
    color: #4ade80;
    font-family: 'IBM Plex Mono', monospace;
    font-size: 11px;
    font-weight: 700;
  }
  .update-installing-status {
    font-family: 'IBM Plex Mono', monospace;
    font-size: 10px;
    color: #71717a;
  }

  /* â”€â”€â”€ T17 EFFECT DETAILS & PREVIEWS â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€ */
  .toggle-actions-group {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .btn-details-fx {
    padding: 6px 12px;
    background: #121216;
    border: 1px solid #27272a;
    border-radius: 4px;
    color: #a1a1aa;
    font-family: 'IBM Plex Mono', monospace;
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.05em;
    cursor: pointer;
    transition: background-color 150ms ease, border-color 150ms ease, color 150ms ease;
  }
  .btn-details-fx:hover {
    background: #1f1f26;
    border-color: #52525b;
    color: #ffffff;
  }

  .details-modal-card {
    width: 820px;
    max-width: 94vw;
    max-height: 88vh;
    display: flex;
    flex-direction: column;
  }

  .modal-header-titles {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .modal-subtitle {
    font-size: 11px;
    color: #71717a;
    font-family: 'IBM Plex Sans', sans-serif;
  }

  .details-toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 18px;
    background: #09090c;
    border-bottom: 1px solid #1c1c22;
  }
  .effects-count-badge {
    font-family: 'IBM Plex Mono', monospace;
    font-size: 11px;
    font-weight: 700;
    color: #e4e4e7;
    background: #18181c;
    padding: 3px 8px;
    border-radius: 4px;
    border: 1px solid #27272a;
  }
  .details-toolbar-actions {
    display: flex;
    gap: 6px;
  }
  .btn-toolbar {
    padding: 4px 9px;
    background: #121216;
    border: 1px solid #27272a;
    border-radius: 4px;
    color: #a1a1aa;
    font-family: 'IBM Plex Mono', monospace;
    font-size: 9px;
    font-weight: 700;
    cursor: pointer;
    transition: background-color 150ms ease, border-color 150ms ease, color 150ms ease;
  }
  .btn-toolbar:hover {
    background: #1c1c24;
    border-color: #52525b;
    color: #ffffff;
  }

  .details-modal-body {
    flex: 1;
    overflow-y: auto;
    padding: 14px 18px;
    background: #09090b;
  }

  .effects-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(360px, 1fr));
    gap: 12px;
  }

  .effect-card {
    display: flex;
    gap: 12px;
    padding: 10px;
    background: #0d0d12;
    border: 1px solid #1c1c22;
    border-radius: 6px;
    cursor: pointer;
    transition: border-color 150ms ease, background-color 150ms ease;
    user-select: none;
  }
  .effect-card:hover {
    border-color: #3f3f46;
    background: #121218;
  }
  .effect-card.active {
    border-color: #52525b;
    background: #111117;
  }

  .effect-card-thumb-wrap {
    position: relative;
    width: 80px;
    height: 80px;
    flex-shrink: 0;
    border-radius: 4px;
    overflow: hidden;
    border: 1px solid #27272a;
    background: #000000;
  }
  .effect-thumb-img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .hover-hint-badge {
    position: absolute;
    bottom: 0;
    left: 0;
    right: 0;
    background: rgba(0, 0, 0, 0.85);
    color: #71717a;
    font-size: 7px;
    font-family: 'IBM Plex Mono', monospace;
    font-weight: 700;
    text-align: center;
    padding: 2px 0;
    letter-spacing: 0.02em;
    opacity: 0;
    transition: opacity 150ms ease;
  }
  .effect-card-thumb-wrap:hover .hover-hint-badge {
    opacity: 1;
    color: #e4e4e7;
  }

  .effect-card-info {
    flex: 1;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    min-width: 0;
  }
  .effect-card-top {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 8px;
  }
  .effect-card-title-row {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .effect-card-name {
    font-size: 12px;
    font-weight: 700;
    color: #f4f4f5;
    font-family: 'IBM Plex Sans', sans-serif;
  }
  .effect-category-pill {
    font-family: 'IBM Plex Mono', monospace;
    font-size: 8px;
    font-weight: 700;
    color: #a1a1aa;
    background: #181820;
    border: 1px solid #272730;
    border-radius: 3px;
    padding: 1px 5px;
    width: fit-content;
    letter-spacing: 0.04em;
  }
  .effect-card-desc {
    font-size: 10px;
    color: #71717a;
    line-height: 1.35;
    margin-top: 4px;
  }

  .effect-toggle-label {
    position: relative;
    display: inline-flex;
    align-items: center;
    cursor: pointer;
  }
  .effect-toggle-label input {
    position: absolute;
    opacity: 0;
    width: 0;
    height: 0;
  }
  .effect-toggle-custom {
    width: 32px;
    height: 18px;
    background: #1c1c24;
    border: 1px solid #3f3f46;
    border-radius: 9px;
    position: relative;
    transition: background-color 150ms ease, border-color 150ms ease;
  }
  .effect-toggle-custom::after {
    content: '';
    position: absolute;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: #71717a;
    top: 2px;
    left: 2px;
    transition: transform 150ms ease, background-color 150ms ease;
  }
  .effect-toggle-label input:checked + .effect-toggle-custom {
    background: #e4e4e7;
    border-color: #ffffff;
  }
  .effect-toggle-label input:checked + .effect-toggle-custom::after {
    transform: translateX(14px);
    background: #09090b;
  }

  /* â”€â”€â”€ T17 LARGE HOVER PREVIEW POPOVER â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€ */
  .large-preview-popover {
    position: fixed;
    z-index: 99999;
    pointer-events: none;
    background: #0a0a0f;
    border: 1px solid #3f3f46;
    border-radius: 8px;
    box-shadow: 0 16px 40px rgba(0, 0, 0, 0.85), 0 0 1px rgba(255, 255, 255, 0.2);
    overflow: hidden;
    width: 272px;
    display: flex;
    flex-direction: column;
    animation: popoverFadeIn 120ms ease-out;
  }
  @keyframes popoverFadeIn {
    from { opacity: 0; transform: scale(0.96); }
    to { opacity: 1; transform: scale(1); }
  }
  .large-preview-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 7px 10px;
    background: #121218;
    border-bottom: 1px solid #27272a;
  }
  .large-preview-title {
    font-size: 11px;
    font-weight: 700;
    color: #f4f4f5;
    font-family: 'IBM Plex Sans', sans-serif;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 180px;
  }
  .large-preview-cat {
    font-family: 'IBM Plex Mono', monospace;
    font-size: 8px;
    font-weight: 700;
    color: #a1a1aa;
    background: #1c1c24;
    padding: 1px 4px;
    border-radius: 3px;
  }
  .large-preview-img {
    width: 256px;
    height: 256px;
    margin: 8px auto;
    border-radius: 4px;
    border: 1px solid #27272a;
    display: block;
    object-fit: cover;
  }
  .large-preview-footer {
    padding: 5px 8px;
    background: #08080c;
    border-top: 1px solid #18181c;
    font-family: 'IBM Plex Mono', monospace;
    font-size: 8px;
    color: #52525b;
    text-align: center;
    letter-spacing: 0.05em;
  }
  /* â”€â”€â”€ T18 CUSTOM PARAMETERS â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€ */
  .details-section-header {
    font-family: 'IBM Plex Mono', monospace;
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.08em;
    color: #71717a;
    text-transform: uppercase;
    padding: 8px 0 4px;
    display: block;
  }

  .custom-params-section {
    margin-top: 20px;
    padding-top: 16px;
    border-top: 1px solid #1c1c22;
  }

  .custom-params-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 12px;
  }

  .btn-reset-defaults {
    font-size: 8px;
    padding: 4px 10px;
    border-color: #3f3f46;
    color: #e4e4e7;
  }
  .btn-reset-defaults:hover {
    background: #1c1c24;
    border-color: #71717a;
    color: #ffffff;
  }

  .cp-group {
    margin-bottom: 16px;
  }

  .cp-group-label {
    font-family: 'IBM Plex Mono', monospace;
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.1em;
    color: #52525b;
    text-transform: uppercase;
    margin-bottom: 8px;
    padding-bottom: 4px;
    border-bottom: 1px solid #18181c;
  }

  .cp-sliders {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
    gap: 8px 20px;
  }

  /* ─── DUMPER PAGE ───────────────────────────────────────────────────────── */
  .dumper-page {
    width: min(100%, 780px);
    margin: auto;
    display: flex;
    flex-direction: column;
    gap: 16px;
    height: 100%;
    justify-content: center;
  }

  .dumper-container {
    display: flex;
    flex-direction: column;
    gap: 16px;
    width: 100%;
  }

  .dumper-drop-zone {
    min-height: 160px;
    max-height: 200px;
    width: 100%;
  }

  .dumper-actions-panel {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .dumper-run-btn {
    width: 100%;
    padding: 12px;
    font-size: 11px;
    letter-spacing: 0.08em;
  }

  .dumper-progress-card {
    padding: 12px 16px;
    gap: 10px;
  }

  .dumper-result-card {
    background: #09090d;
    border: 1px solid rgba(255, 255, 255, 0.2);
    border-radius: 8px;
    padding: 16px 20px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    animation: page-enter 160ms cubic-bezier(0.16, 1, 0.3, 1) both;
  }

  .result-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    border-bottom: 1px solid #1c1c22;
    padding-bottom: 8px;
  }

  .result-title-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .result-timestamp {
    font-size: 9px;
    color: #71717a;
    letter-spacing: 0.05em;
  }

  .dumper-stats-grid {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 12px;
  }

  .stat-box {
    background: #0d0d12;
    border: 1px solid #1c1c24;
    border-radius: 6px;
    padding: 10px 12px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .stat-label {
    font-family: 'IBM Plex Mono', monospace;
    font-size: 8.5px;
    font-weight: 700;
    color: #71717a;
    letter-spacing: 0.05em;
  }

  .stat-val {
    font-size: 12px;
    font-weight: 700;
    color: #ffffff;
  }

  .highlight-sync {
    color: #4ade80;
  }

  /* ─── DUMPER SECTION BOXES ─────────────────────────────────────────────── */
  .dumper-section-box {
    background: #0b0b10;
    border: 1px solid #1c1c24;
    border-radius: 6px;
    padding: 12px 14px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .section-box-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .section-box-title {
    font-family: 'IBM Plex Mono', monospace;
    font-size: 9.5px;
    font-weight: 700;
    color: #a1a1aa;
    letter-spacing: 0.06em;
  }

  .badge-accent {
    font-size: 8.5px;
    font-weight: 700;
    color: #4ade80;
    background: rgba(74, 222, 128, 0.1);
    border: 1px solid rgba(74, 222, 128, 0.3);
    padding: 2px 6px;
    border-radius: 4px;
  }

  .justification-list {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 10px;
    color: #e4e4e7;
  }

  .bullet-dot {
    color: #4ade80;
    font-weight: 700;
  }

  /* ─── SEGMENTS DATA TABLE ───────────────────────────────────────────────── */
  .table-container {
    max-height: 200px;
    overflow-y: auto;
    border: 1px solid #18181e;
    border-radius: 4px;
  }

  .dumper-data-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 9px;
    text-align: left;
  }

  .dumper-data-table th {
    background: #121218;
    color: #71717a;
    padding: 6px 8px;
    font-weight: 700;
    border-bottom: 1px solid #27272a;
    position: sticky;
    top: 0;
    z-index: 1;
    white-space: nowrap;
  }

  .dumper-data-table td {
    padding: 5px 8px;
    border-bottom: 1px solid #14141a;
    color: #d4d4d8;
    white-space: nowrap;
  }

  .dumper-data-table tr:hover td {
    background: #14141c;
  }

  .dumper-data-table .col-idx {
    color: #71717a;
    font-weight: 700;
  }

  .badge-hint {
    font-size: 8px;
    font-weight: 700;
    padding: 1px 5px;
    border-radius: 3px;
    text-transform: uppercase;
  }
  .badge-hint.snap {
    background: rgba(239, 68, 68, 0.15);
    color: #f87171;
    border: 1px solid rgba(239, 68, 68, 0.3);
  }
  .badge-hint.fast {
    background: rgba(245, 158, 11, 0.15);
    color: #fbbf24;
    border: 1px solid rgba(245, 158, 11, 0.3);
  }
  .badge-hint.slow {
    background: rgba(59, 130, 246, 0.15);
    color: #60a5fa;
    border: 1px solid rgba(59, 130, 246, 0.3);
  }
  .badge-hint.normal {
    background: rgba(161, 161, 170, 0.12);
    color: #d4d4d8;
    border: 1px solid rgba(161, 161, 170, 0.25);
  }

  /* ─── TWO-COLUMN GRID ───────────────────────────────────────────────────── */
  .dumper-two-col-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
  }

  .pills-scroll-container {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    max-height: 80px;
    overflow-y: auto;
    padding: 2px 0;
  }

  .framer-pill {
    font-size: 8.5px;
    background: #14141c;
    border: 1px solid #27272a;
    color: #e4e4e7;
    padding: 2px 5px;
    border-radius: 3px;
  }

  .section-empty-hint {
    font-size: 9px;
    color: #52525b;
    margin: 0;
  }

  .motion-summary-content {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 9.5px;
  }

  .motion-row {
    display: flex;
    justify-content: space-between;
    color: #d4d4d8;
  }

  .motion-label {
    color: #71717a;
  }

  .motion-val {
    color: #ffffff;
    font-weight: 700;
  }

  .motion-note {
    font-size: 8.5px;
    color: #71717a;
    margin-top: 4px;
    padding-top: 4px;
    border-top: 1px dashed #1c1c24;
    line-height: 1.3;
  }

  .note-tag {
    font-weight: 700;
    color: #a1a1aa;
  }

  /* ─── REUSABLE VS DESCRIPTIVE ───────────────────────────────────────────── */
  .reusable-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
    font-size: 9.5px;
  }

  .reusable-column {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .column-title {
    font-size: 8.5px;
    font-weight: 700;
    letter-spacing: 0.05em;
  }
  .column-title.success {
    color: #4ade80;
  }
  .column-title.muted {
    color: #71717a;
  }

  .feature-checklist {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
    color: #d4d4d8;
  }
  .feature-checklist.muted {
    color: #71717a;
  }

  /* ─── FILE OUTPUTS ──────────────────────────────────────────────────────── */
  .dumper-files-container {
    display: flex;
    flex-direction: column;
    gap: 6px;
    background: #0d0d12;
    border: 1px solid #18181e;
    border-radius: 6px;
    padding: 8px 10px;
  }

  .file-path-row {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 9px;
    overflow: hidden;
  }

  .file-badge {
    background: #181822;
    border: 1px solid #272738;
    color: #a1a1aa;
    font-size: 8px;
    font-weight: 700;
    padding: 2px 6px;
    border-radius: 3px;
    white-space: nowrap;
  }

  .file-path-text {
    color: #71717a;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* ─── FOOTER ACTIONS ────────────────────────────────────────────────────── */
  .result-footer-actions {
    display: flex;
    align-items: center;
    gap: 10px;
    padding-top: 10px;
    border-top: 1px solid #18181e;
  }

  .btn-apply-project {
    background: #15803d;
    color: #ffffff;
    border: 1px solid #22c55e;
    padding: 8px 16px;
    font-size: 10.5px;
    font-weight: 700;
    border-radius: 4px;
    cursor: pointer;
    letter-spacing: 0.05em;
    transition: all 120ms ease;
    box-shadow: 0 0 10px rgba(34, 197, 94, 0.2);
  }
  .btn-apply-project:hover {
    background: #16a34a;
    box-shadow: 0 0 14px rgba(34, 197, 94, 0.4);
    transform: translateY(-1px);
  }
  .btn-apply-project:active {
    transform: translateY(0);
  }

  /* ─── COMPOSITION PAGE STYLES ───────────────────────────────────────────── */
  .composition-page {
    width: min(100%, 780px);
    margin: auto;
    display: flex;
    flex-direction: column;
    gap: 16px;
    height: 100%;
    justify-content: center;
  }

  .composition-container {
    display: flex;
    flex-direction: column;
    gap: 16px;
    width: 100%;
  }

  .composition-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 14px;
  }

  .comp-drop-zone {
    min-height: 150px;
    max-height: 180px;
  }

  .composition-actions-panel {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .comp-error-card {
    background: rgba(239, 68, 68, 0.08);
    border: 1px solid rgba(239, 68, 68, 0.3);
    border-radius: 6px;
    padding: 12px 14px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .comp-error-header {
    display: flex;
    align-items: center;
  }

  .comp-error-msg {
    font-size: 10px;
    color: #fca5a5;
    margin: 0;
    line-height: 1.4;
  }

  .comp-error-help {
    font-size: 9px;
    color: #a1a1aa;
    margin-top: 4px;
    border-top: 1px dashed rgba(239, 68, 68, 0.2);
    padding-top: 6px;
  }

  .code-box {
    display: block;
    background: #0d0d12;
    border: 1px solid #27272a;
    padding: 6px 10px;
    border-radius: 4px;
    color: #e4e4e7;
    margin-top: 4px;
  }

  .comp-stack-card {
    margin-top: 4px;
  }

  .layers-stack-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
    max-height: 260px;
    overflow-y: auto;
    padding-right: 4px;
  }

  .layer-card {
    display: flex;
    align-items: center;
    gap: 12px;
    background: #0d0d12;
    border: 1px solid #1c1c24;
    border-radius: 6px;
    padding: 6px 10px;
    transition: all 120ms ease;
  }
  .layer-card:hover {
    background: #14141c;
    border-color: #272738;
  }

  .layer-thumb-box {
    width: 44px;
    height: 44px;
    border-radius: 4px;
    background: #181822;
    border: 1px solid #27272a;
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
    flex-shrink: 0;
  }

  .layer-thumb {
    width: 100%;
    height: 100%;
    object-fit: contain;
  }

  .layer-thumb-placeholder {
    font-size: 8px;
    color: #52525b;
    font-weight: 700;
  }

  .layer-info {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
    overflow: hidden;
  }

  .layer-name {
    font-size: 10.5px;
    font-weight: 700;
    color: #ffffff;
    letter-spacing: 0.04em;
  }

  .layer-filename {
    font-size: 8.5px;
    color: #71717a;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .layer-meta {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .z-order-badge {
    background: #181824;
    border: 1px solid #272738;
    color: #a1a1aa;
    font-size: 8.5px;
    font-weight: 700;
    padding: 2px 6px;
    border-radius: 3px;
  }

  .layer-status-pill {
    font-size: 8px;
    font-weight: 700;
    padding: 2px 5px;
    border-radius: 3px;
    background: rgba(113, 113, 122, 0.15);
    color: #71717a;
  }
  .layer-status-pill.active {
    background: rgba(74, 222, 128, 0.12);
    color: #4ade80;
    border: 1px solid rgba(74, 222, 128, 0.3);
  }

  /* ─── LAYERED COMPOSITOR OPS & CONTROLS ─────────────────────────────────── */
  .comp-ops-box {
    margin-top: 2px;
  }

  .comp-ops-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }

  .comp-op-item {
    background: #0d0d12;
    border: 1px solid #1c1c24;
    border-radius: 6px;
    padding: 8px 10px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    transition: all 120ms ease;
  }
  .comp-op-item.disabled {
    opacity: 0.45;
    background: #08080a;
    border-color: #141418;
  }

  .op-top-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .op-label-toggle {
    display: flex;
    align-items: center;
    gap: 6px;
    cursor: pointer;
  }

  .op-checkbox {
    accent-color: #3b82f6;
    cursor: pointer;
  }

  .op-title {
    font-size: 10px;
    font-weight: 700;
    color: #ffffff;
    letter-spacing: 0.04em;
  }

  .op-blend-badge {
    background: #181824;
    border: 1px solid #272738;
    color: #93c5fd;
    font-size: 8px;
    font-weight: 700;
    padding: 2px 6px;
    border-radius: 3px;
  }

  .op-control-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .op-pct-label {
    font-size: 8.5px;
    color: #a1a1aa;
    white-space: nowrap;
    min-width: 80px;
  }

  .op-range-slider {
    flex: 1;
    accent-color: #3b82f6;
    height: 4px;
    cursor: pointer;
  }

  .op-desc-row {
    font-size: 8px;
    color: #71717a;
    line-height: 1.3;
  }

  .comp-actions-row {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .comp-actions-row .btn-render {
    flex: 2;
  }

  .comp-sidecar-btn {
    flex: 1;
    padding: 10px 12px;
    font-size: 9.5px;
    font-weight: 700;
    border-radius: 6px;
    background: #121218;
    border: 1px solid #272738;
    color: #a1a1aa;
    cursor: pointer;
    transition: all 120ms ease;
  }
  .comp-sidecar-btn:hover:not(:disabled) {
    background: #181822;
    color: #ffffff;
    border-color: #3b82f6;
  }
  .comp-sidecar-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .comp-render-card {
    margin-top: 4px;
  }
</style>

