<script>
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { check } from '@tauri-apps/plugin-updater';
  import { relaunch } from '@tauri-apps/plugin-process';
  import { onMount } from 'svelte';
  import GlowSlider from './GlowSlider.svelte';
  import ProjectMark from './ProjectMark.svelte';
  import TextStudio from './TextStudio.svelte';
  import appLogo from '../src-tauri/icons/128x128@2x.png';

  const appWindow =
    typeof window !== 'undefined' && window.__TAURI_INTERNALS__
      ? getCurrentWindow()
      : null;

  let activePage = $state('remap'); // 'remap' | 'settings' | 'text' | 'about'
  let lastRemapPage = $state('remap'); // remembers 'remap' (dropzones) or 'settings'
  let historyStack = $state(['remap']);
  let historyIndex = $state(0);
  let canGoBack = $derived(historyIndex > 0);
  let canGoForward = $derived(historyIndex < historyStack.length - 1);

  function pushNavigation(page) {
    if (page === activePage) return;
    historyStack = [...historyStack.slice(0, historyIndex + 1), page];
    historyIndex = historyStack.length - 1;
    activePage = page;
    if (page === 'remap' || page === 'settings') {
      lastRemapPage = page;
    }
  }

  function handleGoBack() {
    if (!canGoBack) return;
    historyIndex -= 1;
    activePage = historyStack[historyIndex];
    if (activePage === 'remap' || activePage === 'settings') {
      lastRemapPage = activePage;
    }
  }

  function handleGoForward() {
    if (!canGoForward) return;
    historyIndex += 1;
    activePage = historyStack[historyIndex];
    if (activePage === 'remap' || activePage === 'settings') {
      lastRemapPage = activePage;
    }
  }

  // Beta features toggle (false for public release distribution)
  const SHOW_BETA_TEXT_STUDIO = false;
  let toast = $state({ show: false, message: '', type: 'info' });
  let appVersion = $state('1.0.2');
  let discordCopyFeedback = $state(false);
  let isMaximized = $state(false);
  const buildDate = typeof __BUILD_DATE__ !== 'undefined' ? __BUILD_DATE__ : '';
  const buildTime = typeof __BUILD_TIME__ !== 'undefined' ? __BUILD_TIME__ : '';

  async function handleToggleMaximize() {
    if (!appWindow) return;
    try {
      await appWindow.toggleMaximize();
      isMaximized = await appWindow.isMaximized();
    } catch (e) {
      console.error('Failed to toggle maximize:', e);
    }
  }

  // Auto-Updater State
  let updateState = $state('idle'); // 'idle' | 'checking' | 'available' | 'downloading' | 'ready' | 'error' | 'up-to-date'
  let availableUpdate = $state(null);
  let updateDownloadedBytes = $state(0);
  let updateContentLength = $state(0);
  let updateErrorMessage = $state('');
  let showUpdateModal = $state(false);

  // Time Remap State (3 drop zones)
  let scenePath = $state('');
  let scenePaths = $state([]);
  let sceneError = $state('');
  let drumsPath = $state('');
  let drumsError = $state('');
  let audioPath = $state('');
  let audioError = $state('');
  let hoveredZone = $state(null);

  // Probe & Beat Detection State & Cache
  let probedScenePath = $state('');
  let probedDrumsPath = $state('');
  let probedAudioPath = $state('');
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
  let fullFxEnabled = $state(true);    // T13 full fx, default ON
  let renderStats = $state(null);      // T16 render logs & stats

  // T19 Export Options State
  let selectedCodec = $state('H.264'); // 'H.264' | 'H.265' | 'VP9'
  let bitrateValue = $state(12); // min 5, max 50, step 1, default 12
  let selectedFormat = $state('MP4'); // 'MP4' | 'MKV' | 'WEBM'

  // T17 Generic Effect Preview and Toggleable Overrides
  let showDetailsModal = $state(false);
  let availableEffects = $state([]);
  let hoveredPreview = $state(null);
  let hoverPos = $state({ x: 0, y: 0 });

  function getDefaultOverrides(style, fullFx) {
    return {
      shakes: true,
      zoom: true,
      flicker: true,
      oneFramers: true,
      one_framers: true,
      transitions: false,
      tint: true,
      vignette: true,
      scanlines: false,
      exposureFlash: true,
      exposure_flash: true,
      bouncyShake: true,
      bouncy_shake: true,
      dissolveShake: true,
      dissolve_shake: true,
      skewShake: true,
      skew_shake: true,
      squishPop: true,
      squish_pop: true,
      opticsBounce: true,
      optics_bounce: true,
      buildupChain: true,
      buildup_chain: true,
      warpStretch: true,
      warp_stretch: true,
      zoomBeatOffset: true,
      zoom_beat_offset: true,
      ccDeepDark: false,
      cc_deep_dark: false,
      antiFlash: false,
      anti_flash: false,
    };
  }

  let showLongVideoModal = $state(false);
  let longVideoMode = $state('no_change'); // 'no_change' | 'scenepack' | 'long_clip'
  let detectedScenes = $state([]);
  let isDetectingScenes = $state(false);
  let sceneDetectProgress = $state('');

  // ScenePack Gallery & Clip Filtering State
  let showScenePackGallery = $state(false);
  let sceneClipsList = $state([]);
  let selectedClipIndices = $state(new Set());
  let hoveredSceneClip = $state(null);
  let hoverScenePos = $state({ x: 0, y: 0 });
  let scenePackRhythm = $state('fast'); // 'fast' | 'mid' | 'slow'

  function formatTimecode(sec) {
    if (isNaN(sec) || sec == null) return '0:00.0';
    const m = Math.floor(sec / 60);
    const s = Math.floor(sec % 60);
    const ms = Math.floor((sec % 1) * 10);
    return `${m}:${s < 10 ? '0' : ''}${s}.${ms}`;
  }

  function toggleClipSelection(idx) {
    const next = new Set(selectedClipIndices);
    if (next.has(idx)) {
      next.delete(idx);
    } else {
      next.add(idx);
    }
    selectedClipIndices = next;
  }

  function handleSelectAllClips() {
    selectedClipIndices = new Set(sceneClipsList.map((_, i) => i));
  }

  function handleDeselectAllClips() {
    selectedClipIndices = new Set();
  }

  function handleApplyScenePack() {
    if (selectedClipIndices.size === 0) {
      showToast('Please select at least 1 clip for rendering', 'error');
      return;
    }
    const chosen = sceneClipsList.filter((_, i) => selectedClipIndices.has(i));
    detectedScenes = chosen.flatMap((c) => [c.startTime, c.endTime]);
    showScenePackGallery = false;
    navigateTo('settings');
    showToast(`${chosen.length} clip(s) selected for ScenePack render`, 'success');
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
    const prevCc = Boolean(effectOverrides.ccDeepDark || effectOverrides.cc_deep_dark);
    const prevAntiFlash = Boolean(effectOverrides.antiFlash || effectOverrides.anti_flash);
    fullFxEnabled = !fullFxEnabled;
    effectOverrides = {
      ...getDefaultOverrides(selectedStyle, fullFxEnabled),
      ccDeepDark: prevCc,
      cc_deep_dark: prevCc,
      antiFlash: prevAntiFlash,
      anti_flash: prevAntiFlash,
    };
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
    { name: 'beat_this (CP-JKU)', detail: 'Beat/downbeat tracking', mark: 'beat', url: 'https://github.com/CP-JKU/beat_this' },
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

  function validateAndSetFiles(zone, paths) {
    if (!paths || paths.length === 0) return;
    if (zone === 'scene') {
      const valid = paths.filter((p) => VIDEO_EXTENSIONS.includes(getFileExtension(p)));
      if (valid.length > 0) {
        scenePaths = valid;
        scenePath = valid[0];
        sceneError = '';
        sceneInfo = null;
        probedScenePath = '';
        sceneClipsList = [];
        selectedClipIndices = new Set();
        detectedScenes = [];
      } else {
        sceneError = 'Expected: video - mp4/mkv/webm/mov/avi';
      }
    } else if (zone === 'drums') {
      validateAndSetFile('drums', paths[0]);
    } else if (zone === 'audio') {
      validateAndSetFile('audio', paths[0]);
    }
  }

  function validateAndSetFile(zone, path) {
    if (!path) return;
    if (zone === 'scene') {
      validateAndSetFiles('scene', [path]);
      return;
    }
    const ext = getFileExtension(path);
    if (zone === 'drums') {
      if (AUDIO_EXTENSIONS.includes(ext)) {
        if (drumsPath !== path) {
          drumsPath = path;
          drumsError = '';
          drumsInfo = null;
          probedDrumsPath = '';
          beats = null;
          downbeats = null;
          bpm = null;
        }
      } else {
        drumsError = 'Expected: audio - mp3/wav/flac/m4a/ogg';
      }
    } else if (zone === 'audio') {
      if (AUDIO_EXTENSIONS.includes(ext)) {
        if (audioPath !== path) {
          audioPath = path;
          audioError = '';
          audioInfo = null;
          probedAudioPath = '';
        }
      } else {
        audioError = 'Expected: audio - mp3/wav/flac/m4a/ogg';
      }
    }
  }

  function clearZone(zone, event) {
    if (event) event.stopPropagation();
    if (zone === 'scene') {
      scenePaths = [];
      scenePath = '';
      sceneError = '';
      sceneInfo = null;
      probedScenePath = '';
      sceneClipsList = [];
      selectedClipIndices = new Set();
      detectedScenes = [];
    } else if (zone === 'drums') {
      drumsPath = '';
      drumsError = '';
      drumsInfo = null;
      probedDrumsPath = '';
      beats = null;
      downbeats = null;
      bpm = null;
    } else if (zone === 'audio') {
      audioPath = '';
      audioError = '';
      audioInfo = null;
      probedAudioPath = '';
    }
  }

  async function handlePickFile(zone, event) {
    if (event) event.stopPropagation();
    try {
      if (zone === 'scene') {
        const picked = await invoke('pick_files', { kind: 'video' });
        if (picked && picked.length > 0) {
          validateAndSetFiles('scene', picked);
        }
      } else {
        const kind = 'audio';
        const picked = await invoke('pick_file', { kind });
        if (picked) {
          validateAndSetFile(zone, picked);
        }
      }
    } catch (e) {
      showToast(`Selection cancelled or error: ${e}`, 'error');
    }
  }

  async function selectLongVideoMode(mode) {
    longVideoMode = mode;
    if (mode === 'scenepack') {
      isDetectingScenes = true;
      sceneDetectProgress = 'Analyzing scene cuts and generating clip previews...';
      try {
        const clips = await invoke('get_scene_clips', {
          videoPath: scenePath,
          videoDuration: sceneInfo.duration,
        });
        sceneClipsList = clips || [];
        selectedClipIndices = new Set(sceneClipsList.map((_, i) => i));
        showLongVideoModal = false;
        showScenePackGallery = true;
      } catch (err) {
        console.warn('Scene detection error:', err);
        detectedScenes = [0.0, sceneInfo.duration];
        showToast('Scene detection fallback to single clip', 'warning');
        showLongVideoModal = false;
        navigateTo('settings');
      } finally {
        isDetectingScenes = false;
      }
    } else {
      detectedScenes = [];
      showLongVideoModal = false;
      navigateTo('settings');
    }
  }

  async function handleContinue() {
    if (!scenePath || !drumsPath || !audioPath) return;

    const currentSceneKey = scenePaths.length > 1 ? scenePaths.join('|') : scenePath;
    // Fast-path cache hit: if media files haven't changed and beat detection is cached
    const cacheHit = (
      currentSceneKey === probedScenePath &&
      drumsPath === probedDrumsPath &&
      audioPath === probedAudioPath &&
      sceneInfo !== null &&
      drumsInfo !== null &&
      audioInfo !== null &&
      beats !== null
    );

    if (cacheHit) {
      if (scenePaths.length > 1) {
        if (sceneInfo && audioInfo && sceneInfo.duration > audioInfo.duration + 0.5) {
          showScenePackGallery = true;
        } else {
          navigateTo('settings');
        }
      } else if (sceneInfo && audioInfo && sceneInfo.duration > audioInfo.duration + 0.5) {
        if (longVideoMode === 'scenepack' && sceneClipsList.length > 0) {
          showScenePackGallery = true;
        } else {
          showLongVideoModal = true;
        }
      } else {
        longVideoMode = 'no_change';
        detectedScenes = [];
        navigateTo('settings');
      }
      return;
    }

    isAnalyzing = true;
    try {
      if (scenePaths.length > 1) {
        analyzingStep = `Probing ${scenePaths.length} scene videos...`;
        const clips = await invoke('get_multi_scene_clips', { videoPaths: scenePaths });
        sceneClipsList = clips || [];
        selectedClipIndices = new Set(sceneClipsList.map((_, i) => i));
        const totalDur = sceneClipsList.reduce((acc, c) => acc + c.duration, 0);
        sceneInfo = {
          duration: totalDur,
          width: 1920,
          height: 1080,
          fps: 30.0,
          audioChannels: 0,
          audioSampleRate: 0,
        };
        probedScenePath = currentSceneKey;
        detectedScenes = sceneClipsList.flatMap((c) => [c.startTime, c.endTime]);
        longVideoMode = 'scenepack';
      } else {
        analyzingStep = 'Probing scene video...';
        sceneInfo = await invoke('probe_media', { filePath: scenePath });
        probedScenePath = currentSceneKey;
      }

      analyzingStep = 'Probing drums audio...';
      drumsInfo = await invoke('probe_media', { filePath: drumsPath });
      probedDrumsPath = drumsPath;

      analyzingStep = 'Probing target audio...';
      audioInfo = await invoke('probe_media', { filePath: audioPath });
      probedAudioPath = audioPath;

      analyzingStep = 'Detecting beats with ONNX model...';
      const beatResult = await invoke('detect_beats', { audioPath: drumsPath });
      beats = beatResult.beats;
      downbeats = beatResult.downbeats;
      bpm = beatResult.bpm;

      if (scenePaths.length > 1) {
        if (sceneInfo && audioInfo && sceneInfo.duration > audioInfo.duration + 0.5) {
          showScenePackGallery = true;
        } else {
          navigateTo('settings');
        }
      } else if (sceneInfo && audioInfo && sceneInfo.duration > audioInfo.duration + 0.5) {
        showLongVideoModal = true;
      } else {
        longVideoMode = 'no_change';
        detectedScenes = [];
        navigateTo('settings');
      }
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
        effectOverrides: {
          ...effectOverrides,
          ccDeepDark: Boolean(effectOverrides.ccDeepDark || effectOverrides.cc_deep_dark),
          cc_deep_dark: Boolean(effectOverrides.ccDeepDark || effectOverrides.cc_deep_dark),
          antiFlash: Boolean(effectOverrides.antiFlash || effectOverrides.anti_flash),
          anti_flash: Boolean(effectOverrides.antiFlash || effectOverrides.anti_flash),
        },
        customParams: customParams || null,
        exportConfig: {
          codec: selectedCodec,
          bitrateMbps: bitrateValue,
          format: selectedFormat,
        },
        longVideoMode,
        detectedScenes: detectedScenes.length > 0 ? detectedScenes : null,
        scenepackRhythm: scenePackRhythm,
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
        echoTrail: false,
        export: parsed.export || {
          codec: selectedCodec,
          bitrateMbps: bitrateValue,
          format: selectedFormat,
        },
      };

      const activeScenePaths = (scenePaths.length > 1 && selectedClipIndices.size > 0)
        ? sceneClipsList.filter((_, i) => selectedClipIndices.has(i)).map((c) => scenePaths[c.index] || scenePaths[0])
        : (scenePaths.length > 0 ? scenePaths : [scenePath]);

      console.log('[RENDER] Launching 3-pass render pipeline...');
      const renderRes = await invoke('run_render_pipeline', {
        planJson,
        scenePath: activeScenePaths[0] || scenePath,
        audioPath,
        echoTrail: false,
        scenePaths: activeScenePaths,
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

  async function handleCancelRender() {
    try {
      await invoke('cancel_render');
      renderState = 'idle';
      showToast('Render cancelled', 'info');
    } catch (e) {
      console.error('Failed to cancel render:', e);
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
    let target = page;
    if (target === 'text' && !SHOW_BETA_TEXT_STUDIO) {
      target = 'remap';
    }
    if (target === 'remap' && lastRemapPage) {
      target = lastRemapPage;
    }
    pushNavigation(target);
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
        if (targetZone === 'scene') {
          validateAndSetFiles('scene', paths);
        } else {
          validateAndSetFile(targetZone, paths[0]);
        }
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

    if (appWindow) {
      try {
        isMaximized = await appWindow.isMaximized();
      } catch (e) {
        console.warn('Failed to check maximized state:', e);
      }
    }

    checkForAppUpdates(false);

    return () => {
      if (unlistenProgress) unlistenProgress();
    };
  });
</script>

<div class="app-root">
  <!-- Custom Windows Titlebar -->
  <div class="titlebar" data-tauri-drag-region>
    <div class="titlebar-left">
      <div class="titlebar-nav-controls" data-tauri-drag-region="false">
        <button
          class="titlebar-nav-btn"
          onclick={handleGoBack}
          disabled={!canGoBack}
          title="Go back"
          aria-label="Back"
        >
          ←
        </button>
        <button
          class="titlebar-nav-btn"
          onclick={handleGoForward}
          disabled={!canGoForward}
          title="Go forward"
          aria-label="Forward"
        >
          →
        </button>
      </div>
      <div class="titlebar-brand">
        <span class="titlebar-text">cia jugg</span>
        {#if buildDate && buildTime}
          <span class="titlebar-build-badge mono">{buildDate} {buildTime}</span>
        {/if}
      </div>
    </div>
    <div class="titlebar-controls" data-tauri-drag-region="false">
      {#if availableUpdate}
        <button class="titlebar-btn update-badge" onclick={() => showUpdateModal = true} aria-label="Update available">
          <span class="update-badge-dot"></span> UPDATE V{availableUpdate.version}
        </button>
      {/if}
      <button class="titlebar-btn" onclick={() => appWindow?.minimize()} aria-label="Minimize" disabled={!appWindow}>−</button>
      <button class="titlebar-btn" onclick={handleToggleMaximize} aria-label="Maximize / Restore" disabled={!appWindow}>
        {isMaximized ? '❐' : '□'}
      </button>
      <button class="titlebar-btn close" onclick={() => appWindow?.close()} aria-label="Close" disabled={!appWindow}>✕</button>
    </div>
  </div>

  <nav class="tab-bar">
    <button class:active={activePage === 'remap' || activePage === 'settings'} onclick={() => navigateTo('remap')}>TIME REMAP</button>
    {#if SHOW_BETA_TEXT_STUDIO}
      <button class:active={activePage === 'text'} onclick={() => navigateTo('text')}>TEXT</button>
    {/if}
    <button class:active={activePage === 'about'} onclick={() => navigateTo('about')}>ABOUT</button>
  </nav>

  <!-- Main Content Area -->
  <main class="content-area">
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
                    <div class="zone-top-bar">
                      <span class="zone-tag mono">{scenePaths.length > 1 ? `${scenePaths.length} VIDEOS` : 'VIDEO SOURCE'}</span>
                    </div>
                    <div class="zone-filled-body">
                      <div class="zone-filled-icon">
                        <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
                          <rect x="2" y="4" width="20" height="16" rx="2" />
                          <path d="M7 4v16M17 4v16M2 12h20" />
                        </svg>
                      </div>
                      <div class="zone-title">SCENE FOOTAGE</div>
                      <div class="zone-filename mono" title={scenePaths.length > 1 ? scenePaths.map(getFileName).join(', ') : scenePath}>
                        {#if scenePaths.length > 1}
                          {scenePaths.length} videos ({getFileName(scenePaths[0])}, +{scenePaths.length - 1})
                        {:else}
                          {getFileName(scenePath)}
                        {/if}
                      </div>
                    </div>
                    <div class="zone-actions">
                      <button class="btn-zone-action" onclick={(e) => handlePickFile('scene', e)}>REPLACE</button>
                      <button class="btn-zone-action danger" onclick={(e) => clearZone('scene', e)}>REMOVE</button>
                    </div>
                  </div>
                {:else}
                  <div class="zone-empty-content">
                    <div class="zone-center-body">
                      <div class="zone-icon-wrap">
                        <svg width="34" height="34" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round">
                          <rect x="2" y="4" width="20" height="16" rx="2" />
                          <path d="M7 4v16M17 4v16M2 12h20M2 8h5M2 16h5M17 8h5M17 16h5" />
                        </svg>
                      </div>
                      <span class="zone-prompt">DRAG SCENE</span>
                      {#if sceneError}
                        <span class="zone-error-msg">{sceneError}</span>
                      {/if}
                    </div>
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
                    <div class="zone-top-bar">
                      <span class="zone-tag mono">DRUMS STEM</span>
                    </div>
                    <div class="zone-filled-body">
                      <div class="zone-filled-icon">
                        <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
                          <path d="M4 10v4M8 6v12M12 3v18M16 7v10M20 11v2" />
                        </svg>
                      </div>
                      <div class="zone-title">DRUMS STEM</div>
                      <div class="zone-filename mono" title={drumsPath}>{getFileName(drumsPath)}</div>
                    </div>
                    <div class="zone-actions">
                      <button class="btn-zone-action" onclick={(e) => handlePickFile('drums', e)}>REPLACE</button>
                      <button class="btn-zone-action danger" onclick={(e) => clearZone('drums', e)}>REMOVE</button>
                    </div>
                  </div>
                {:else}
                  <div class="zone-empty-content">
                    <div class="zone-center-body">
                      <div class="zone-icon-wrap">
                        <svg width="34" height="34" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round">
                          <path d="M4 10v4M8 6v12M12 3v18M16 7v10M20 11v2" />
                        </svg>
                      </div>
                      <span class="zone-prompt">DRAG DRUMS</span>
                      {#if drumsError}
                        <span class="zone-error-msg">{drumsError}</span>
                      {/if}
                    </div>
                    <div class="zone-bottom-bar">
                      <button
                        class="zone-link mono"
                        onclick={(e) => { e.stopPropagation(); openAboutLink('https://vocalremover.org/splitter-ai'); }}
                        type="button"
                        title="Open vocal remover AI"
                      >
                        https://vocalremover.org/splitter-ai
                      </button>
                    </div>
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
                    <div class="zone-top-bar">
                      <span class="zone-tag mono">MASTER AUDIO</span>
                    </div>
                    <div class="zone-filled-body">
                      <div class="zone-filled-icon">
                        <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
                          <path d="M3 12h2l2-4 3 8 3-6 2 4 2-2h4" />
                        </svg>
                      </div>
                      <div class="zone-title">SOUNDTRACK</div>
                      <div class="zone-filename mono" title={audioPath}>{getFileName(audioPath)}</div>
                    </div>
                    <div class="zone-actions">
                      <button class="btn-zone-action" onclick={(e) => handlePickFile('audio', e)}>REPLACE</button>
                      <button class="btn-zone-action danger" onclick={(e) => clearZone('audio', e)}>REMOVE</button>
                    </div>
                  </div>
                {:else}
                  <div class="zone-empty-content">
                    <div class="zone-center-body">
                      <div class="zone-icon-wrap">
                        <svg width="34" height="34" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round">
                          <path d="M3 12h2l2-4 3 8 3-6 2 4 2-2h4" />
                        </svg>
                      </div>
                      <span class="zone-prompt">DRAG FULL AUDIO</span>
                      {#if audioError}
                        <span class="zone-error-msg">{audioError}</span>
                      {/if}
                    </div>
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
                          const prevCc = Boolean(effectOverrides.ccDeepDark || effectOverrides.cc_deep_dark);
                          const prevAntiFlash = Boolean(effectOverrides.antiFlash || effectOverrides.anti_flash);
                          selectedStyle = style.id;
                          effectOverrides = {
                            ...getDefaultOverrides(selectedStyle, fullFxEnabled),
                            ccDeepDark: prevCc,
                            cc_deep_dark: prevCc,
                            antiFlash: prevAntiFlash,
                            anti_flash: prevAntiFlash,
                          };
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
                <div class="toggle-row">
                  <div class="toggle-row-left" title="All effects — one-framers, transitions, tint, vignette. Default ON.">
                    <span class="toggle-row-title">FULL FX</span>
                    <span class="toggle-info-icon" aria-label="Info">ⓘ</span>
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

                <!-- 5. Color Correction (CC) -->
                <div class="toggle-row">
                  <div class="toggle-row-left" title="Deep tone curve, highlight bloom glow (10px), 19% film grain noise. Grayscale monochrome.">
                    <span class="toggle-row-title">COLOR CORRECTION (CC)</span>
                    <span class="toggle-info-icon" aria-label="Info">ⓘ</span>
                  </div>
                  <button
                    id="toggle-cc-deep-dark"
                    class="toggle-btn"
                    class:active={effectOverrides.ccDeepDark || effectOverrides.cc_deep_dark}
                    onclick={() => {
                      const val = !(effectOverrides.ccDeepDark || effectOverrides.cc_deep_dark);
                      effectOverrides.ccDeepDark = val;
                      effectOverrides.cc_deep_dark = val;
                    }}
                    type="button"
                    aria-pressed={effectOverrides.ccDeepDark || effectOverrides.cc_deep_dark}
                  >
                    {(effectOverrides.ccDeepDark || effectOverrides.cc_deep_dark) ? 'DEEP DARK [ON]' : 'DEEP DARK [OFF]'}
                  </button>
                </div>

                <!-- 6. Anti Flash Mode -->
                <div class="toggle-row">
                  <div class="toggle-row-left" title="Eliminates all white/black flashes, brightness strobes, and color inversions. Safe for photosensitive viewers.">
                    <span class="toggle-row-title">ANTI FLASH</span>
                    <span class="toggle-info-icon" aria-label="Info">ⓘ</span>
                  </div>
                  <button
                    id="toggle-anti-flash"
                    class="toggle-btn"
                    class:active={effectOverrides.antiFlash || effectOverrides.anti_flash}
                    onclick={() => {
                      const val = !(effectOverrides.antiFlash || effectOverrides.anti_flash);
                      effectOverrides.antiFlash = val;
                      effectOverrides.anti_flash = val;
                    }}
                    type="button"
                    aria-pressed={effectOverrides.antiFlash || effectOverrides.anti_flash}
                  >
                    {(effectOverrides.antiFlash || effectOverrides.anti_flash) ? 'ANTI FLASH [ON]' : 'ANTI FLASH [OFF]'}
                  </button>
                </div>

                <!-- 7. ScenePack Controls (visible when longVideoMode === 'scenepack') -->
                {#if longVideoMode === 'scenepack'}
                  <div class="toggle-row">
                    <div class="toggle-row-left" title="Clips currently selected for ScenePack rendering. Click to modify selection.">
                      <span class="toggle-row-title">SCENEPACK CLIPS</span>
                      <span class="toggle-info-icon" aria-label="Info">ⓘ</span>
                    </div>
                    <button
                      class="btn-details-fx"
                      onclick={() => showScenePackGallery = true}
                      type="button"
                    >
                      SELECT CLIPS ({selectedClipIndices.size} / {sceneClipsList.length})
                    </button>
                  </div>

                  <div class="control-group">
                    <div class="scenepack-rhythm-header">
                      <span class="group-label">SCENEPACK RHYTHM</span>
                    </div>
                    <div class="scenepack-rhythm-grid">
                      <button
                        class="btn-rhythm"
                        class:active={scenePackRhythm === 'fast'}
                        onclick={() => scenePackRhythm = 'fast'}
                        type="button"
                      >
                        <div class="rhythm-title">FAST</div>
                        <div class="rhythm-desc">Cuts on almost every beat (~0.4s)</div>
                      </button>

                      <button
                        class="btn-rhythm"
                        class:active={scenePackRhythm === 'mid'}
                        onclick={() => scenePackRhythm = 'mid'}
                        type="button"
                      >
                        <div class="rhythm-title">MID</div>
                        <div class="rhythm-desc">Dynamic cuts (~1.5s - 2s)</div>
                      </button>

                      <button
                        class="btn-rhythm"
                        class:active={scenePackRhythm === 'slow'}
                        onclick={() => scenePackRhythm = 'slow'}
                        type="button"
                      >
                        <div class="rhythm-title">SLOW</div>
                        <div class="rhythm-desc">Long takes (~3.5s - 4.5s)</div>
                      </button>
                    </div>
                  </div>
                {/if}
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
                      <span class="render-done-title">RENDER COMPLETE</span>
                    </div>
                    <span class="done-specs mono">{planSummary?.aspect || '1080x1080'} • {renderStats?.targetFps || fpsValue} FPS</span>
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
              {/if}

              <!-- Footer Actions (only when idle/error) -->
              {#if renderState !== 'running' && renderState !== 'done'}
                <div class="settings-actions-footer">
                  <button class="btn-pro-secondary" onclick={() => pushNavigation('remap')}>
                    &lt; BACK TO SOURCES
                  </button>
                  <button class="btn-run-process" onclick={handleRunProcess}>
                    RUN PROCESS &gt;
                  </button>
                </div>
              {/if}
            </div>
          </section>

        {:else if SHOW_BETA_TEXT_STUDIO && activePage === 'text'}
          <TextStudio onToast={(msg, type) => showToast(msg, type)} />

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

  <!-- Long Video Processing Mode Modal -->
  {#if showLongVideoModal}
    <div
      class="modal-backdrop"
      onclick={(e) => { if (e.target === e.currentTarget && !isDetectingScenes) showLongVideoModal = false; }}
      role="presentation"
    >
      <div
        class="modal-card long-video-modal-card"
        onclick={(e) => e.stopPropagation()}
        onkeydown={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        tabindex="0"
      >
        <div class="modal-header">
          <div class="modal-header-titles">
            <h2>LONG FOOTAGE DETECTED</h2>
            <span class="modal-subtitle">
              Video ({sceneInfo ? sceneInfo.duration.toFixed(1) : 0}s) is longer than target music ({audioInfo ? audioInfo.duration.toFixed(1) : 0}s). Choose how footage should be distributed:
            </span>
          </div>
          {#if !isDetectingScenes}
            <button class="btn-close-modal" onclick={() => showLongVideoModal = false} aria-label="Close">✕</button>
          {/if}
        </div>

        <div class="modal-body long-video-modal-body">
          {#if isDetectingScenes}
            <div class="scene-detect-loading">
              <div class="scene-detect-spinner"></div>
              <span class="scene-detect-text mono">{sceneDetectProgress || 'ANALYZING SCENE CUTS WITH FFMPEG...'}</span>
            </div>
          {:else}
            <div class="long-video-options-grid">
              <!-- Option 1: No Change -->
              <button
                class="long-video-card"
                class:selected={longVideoMode === 'no_change'}
                onclick={() => selectLongVideoMode('no_change')}
                type="button"
              >
                <div class="long-video-card-top">
                  <span class="long-video-card-title">NO CHANGE</span>
                  <span class="long-video-card-tag">DEFAULT</span>
                </div>
                <p class="long-video-card-desc">
                  Sequential playback from 0:00 up to the music duration. Standard sequential timeline behavior.
                </p>
              </button>

              <!-- Option 2: Scenepack -->
              <button
                class="long-video-card"
                class:selected={longVideoMode === 'scenepack'}
                onclick={() => selectLongVideoMode('scenepack')}
                type="button"
              >
                <div class="long-video-card-top">
                  <span class="long-video-card-title">SCENEPACK</span>
                  <span class="long-video-card-tag">SMART CUTS</span>
                </div>
                <p class="long-video-card-desc">
                  Detects individual scene cuts. Ensures a ~3s minimum threshold before switching clips strictly on musical downbeats.
                </p>
              </button>

              <!-- Option 3: Long Clip -->
              <button
                class="long-video-card"
                class:selected={longVideoMode === 'long_clip'}
                onclick={() => selectLongVideoMode('long_clip')}
                type="button"
              >
                <div class="long-video-card-top">
                  <span class="long-video-card-title">LONG CLIP</span>
                  <span class="long-video-card-tag">TIMELINE ANCHORS</span>
                </div>
                <p class="long-video-card-desc">
                  Evenly distributes anchor points across continuous long video (e.g. 2-min freestyle) and condenses them onto beat-synced intervals.
                </p>
              </button>
            </div>
          {/if}
        </div>
      </div>
    </div>
  {/if}

  <!-- ScenePack Gallery Selection Modal -->
  {#if showScenePackGallery}
    <div
      class="modal-backdrop"
      onclick={(e) => { if (e.target === e.currentTarget) showScenePackGallery = false; }}
      role="presentation"
    >
      <div
        class="modal-card scenepack-modal-card"
        onclick={(e) => e.stopPropagation()}
        onkeydown={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        tabindex="0"
      >
        <div class="modal-header">
          <div class="modal-header-titles">
            <h2>SCENEPACK CLIP SELECTION</h2>
            <span class="modal-subtitle">
              Select which detected clips to include in the time remap render. Unchecked clips will be skipped.
            </span>
          </div>
          <button class="btn-close-modal" onclick={() => showScenePackGallery = false} aria-label="Close">✕</button>
        </div>

        <div class="scenepack-toolbar">
          <div class="scenepack-toolbar-left">
            <span class="clips-count-badge mono">{selectedClipIndices.size} / {sceneClipsList.length} CLIPS ACTIVE</span>
          </div>
          <div class="scenepack-toolbar-actions">
            <button class="btn-toolbar" onclick={handleSelectAllClips} type="button">SELECT ALL</button>
            <button class="btn-toolbar" onclick={handleDeselectAllClips} type="button">DESELECT ALL</button>
          </div>
        </div>

        <div class="modal-body scenepack-modal-body">
          <div class="scenepack-grid">
            {#each sceneClipsList as clip (clip.index)}
              <div
                class="scene-card"
                class:active={selectedClipIndices.has(clip.index)}
                onclick={() => toggleClipSelection(clip.index)}
                role="button"
                tabindex="0"
                onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { toggleClipSelection(clip.index); e.preventDefault(); } }}
              >
                <div
                  class="scene-card-thumb-wrap"
                  role="presentation"
                  onmouseenter={(e) => {
                    hoveredSceneClip = clip;
                    hoverScenePos = { x: e.clientX, y: e.clientY };
                  }}
                  onmousemove={(e) => {
                    hoverScenePos = { x: e.clientX, y: e.clientY };
                  }}
                  onmouseleave={() => {
                    hoveredSceneClip = null;
                  }}
                >
                  <img src={clip.thumbnail} alt={`Clip #${clip.index + 1}`} class="scene-thumb-img" width="320" height="180" />
                  <span class="hover-hint-badge">HOVER TO ENLARGE</span>
                </div>

                <div class="scene-card-info">
                  <div class="scene-card-top">
                    <div class="scene-card-title-row">
                      <span class="scene-card-name">CLIP #{clip.index + 1}</span>
                      <span class="scene-duration-pill mono">{clip.duration.toFixed(1)}s</span>
                    </div>
                    <label class="scene-toggle-label">
                      <input
                        type="checkbox"
                        checked={selectedClipIndices.has(clip.index)}
                        onclick={(e) => e.stopPropagation()}
                        onchange={() => toggleClipSelection(clip.index)}
                      />
                      <span class="scene-toggle-custom"></span>
                    </label>
                  </div>
                  <div class="scene-timecode-row mono">
                    {formatTimecode(clip.startTime)} &rarr; {formatTimecode(clip.endTime)}
                  </div>
                </div>
              </div>
            {/each}
          </div>
        </div>

        <div class="modal-footer scenepack-footer">
          <button
            class="btn-pro-secondary"
            onclick={() => { showScenePackGallery = false; showLongVideoModal = true; }}
            type="button"
          >
            &lt; BACK
          </button>
          <button
            class="btn-primary-modal"
            disabled={selectedClipIndices.size === 0}
            onclick={handleApplyScenePack}
            type="button"
          >
            APPLY &amp; CONTINUE ({selectedClipIndices.size} CLIPS)
          </button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Floating Large Preview Card for ScenePack Clip Hover -->
  {#if hoveredSceneClip}
    <div
      class="scene-large-preview-overlay"
      style="left: {Math.min(hoverScenePos.x + 20, window.innerWidth - 360)}px; top: {Math.min(hoverScenePos.y - 120, window.innerHeight - 300)}px;"
    >
      <div class="large-preview-header">
        <span class="large-preview-title">CLIP #{hoveredSceneClip.index + 1}</span>
        <span class="large-preview-cat mono">{hoveredSceneClip.duration.toFixed(1)}s</span>
      </div>
      <img src={hoveredSceneClip.thumbnail} alt={`Clip #${hoveredSceneClip.index + 1}`} class="scene-large-preview-img" width="320" height="180" />
      <div class="large-preview-footer mono">
        {formatTimecode(hoveredSceneClip.startTime)} - {formatTimecode(hoveredSceneClip.endTime)} • {selectedClipIndices.has(hoveredSceneClip.index) ? 'ACTIVE IN RENDER' : 'EXCLUDED'}
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
          <button class="btn-close-modal" onclick={() => showDetailsModal = false} aria-label="Close details">✕</button>
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
  /* CUSTOM MONOCHROME STUDIO SCROLLBARS */
  ::-webkit-scrollbar {
    width: 6px;
    height: 6px;
  }
  ::-webkit-scrollbar-track {
    background: transparent;
  }
  ::-webkit-scrollbar-thumb {
    background: #27272a;
    border-radius: 999px;
  }
  ::-webkit-scrollbar-thumb:hover {
    background: #71717a;
  }
  * {
    scrollbar-width: thin;
    scrollbar-color: #27272a transparent;
  }

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

  .titlebar-left { display: flex; align-items: center; gap: 12px; }
  .titlebar-nav-controls { display: flex; align-items: center; gap: 4px; }
  .titlebar-nav-btn {
    width: 24px;
    height: 22px;
    border: 1px solid transparent;
    background: transparent;
    color: #a1a1aa;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 13px;
    border-radius: 4px;
    transition: all 0.15s ease;
  }
  .titlebar-nav-btn:hover:not(:disabled) {
    background: #1c1c20;
    border-color: #27272a;
    color: #ffffff;
  }
  .titlebar-nav-btn:disabled {
    color: #3f3f46;
    cursor: not-allowed;
    opacity: 0.35;
  }
  .titlebar-brand { display: flex; align-items: center; gap: 8px; }
  .titlebar-text { font-size: 11px; font-weight: 700; letter-spacing: 0.06em; color: #a1a1aa; text-transform: uppercase; }
  .titlebar-build-badge {
    font-family: 'IBM Plex Mono', monospace;
    font-size: 9px;
    color: #71717a;
    background: #111116;
    padding: 2px 6px;
    border-radius: 4px;
    border: 1px solid #27272a;
    letter-spacing: 0.04em;
    user-select: text;
  }
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
    overflow-y: auto;
    overflow-x: hidden;
    padding: 16px 20px 32px;
    background: #050507;
    display: flex;
    flex-direction: column;
  }

  .page-stage {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    justify-content: flex-start;
  }

  /* Time Remap Page (3 Drop Zones) */
  .remap-page {
    width: min(100%, 1240px);
    margin: 0 auto;
    display: flex;
    flex-direction: column;
    gap: 16px;
    height: 100%;
    flex: 1;
    min-height: 0;
    justify-content: flex-start;
  }

  .remap-grid {
    display: grid;
    grid-template-columns: 2fr 1fr 1fr;
    gap: 14px;
    width: 100%;
    flex: 1;
    min-height: 0;
  }

  @media (max-width: 860px) {
    .remap-grid {
      grid-template-columns: 1fr;
    }
  }

  .remap-drop-zone {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 320px;
    padding: 16px;
    border: 1px solid #1c1c24;
    border-radius: 8px;
    background: #07070a;
    cursor: pointer;
    transition: background 0.15s ease, border-color 0.15s ease, box-shadow 0.15s ease;
    position: relative;
    box-sizing: border-box;
  }

  @media (max-width: 860px) {
    .remap-drop-zone {
      min-height: 180px;
      padding: 14px 12px;
    }
  }

  .remap-drop-zone:hover {
    background: #0b0b10;
    border-color: #2e2e3c;
  }

  .remap-drop-zone.hovering {
    background: #0e0e16;
    border-color: #ffffff;
    box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.25);
  }

  .remap-drop-zone.has-error {
    border-color: rgba(239, 68, 68, 0.45);
    background: #0e0707;
  }

  .remap-drop-zone.filled {
    border-color: #272733;
    background: #08080c;
    cursor: default;
  }

  /* Empty Zone Layout */
  .zone-empty-content {
    display: flex;
    flex-direction: column;
    justify-content: center;
    align-items: center;
    height: 100%;
    width: 100%;
    position: relative;
    pointer-events: none;
  }

  .zone-top-bar {
    display: flex;
    justify-content: flex-end;
    align-items: center;
    width: 100%;
  }

  .zone-center-body {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    text-align: center;
    width: 100%;
    flex: 1;
    padding: 12px 0;
  }

  .zone-icon-wrap {
    color: #3f3f46;
    transition: color 0.15s ease, transform 0.15s ease;
    margin-bottom: 4px;
  }

  .remap-drop-zone:hover .zone-icon-wrap {
    color: #a1a1aa;
    transform: translateY(-2px);
  }

  .remap-drop-zone.hovering .zone-icon-wrap {
    color: #ffffff;
    transform: translateY(-4px);
  }

  .zone-prompt {
    font-size: 12.5px;
    font-weight: 700;
    letter-spacing: 0.06em;
    margin: 0;
    color: #e4e4e7;
    text-transform: uppercase;
    transition: color 0.15s ease;
  }

  .remap-drop-zone:hover .zone-prompt {
    color: #ffffff;
  }

  .zone-bottom-bar {
    position: absolute;
    bottom: 0;
    left: 0;
    right: 0;
    display: flex;
    justify-content: center;
    align-items: center;
    width: 100%;
    padding-top: 8px;
    border-top: 1px solid #14141c;
    pointer-events: auto;
  }

  .zone-link {
    background: transparent;
    border: none;
    padding: 3px 6px;
    font-family: 'IBM Plex Mono', monospace;
    font-size: 9.5px;
    font-weight: 500;
    letter-spacing: 0.02em;
    color: #38bdf8;
    cursor: pointer;
    text-decoration: underline;
    text-underline-offset: 3px;
    pointer-events: auto;
    transition: all 0.15s ease;
    border-radius: 4px;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .zone-link:hover {
    color: #7dd3fc;
    background: rgba(56, 189, 248, 0.1);
  }

  .zone-error-msg {
    margin-top: 8px;
    padding: 5px 8px;
    background: rgba(127, 29, 29, 0.35);
    border: 1px solid #7f1d1d;
    border-radius: 4px;
    color: #fca5a5;
    font-family: 'IBM Plex Mono', monospace;
    font-size: 9px;
    font-weight: 600;
    line-height: 1.35;
  }

  /* Filled Zone Layout */
  .zone-filled-content {
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    height: 100%;
    width: 100%;
  }

  .zone-tag {
    font-family: 'IBM Plex Mono', monospace;
    font-size: 8.5px;
    font-weight: 700;
    letter-spacing: 0.06em;
    color: #a1a1aa;
    background: #121217;
    border: 1px solid #24242e;
    border-radius: 4px;
    padding: 2px 6px;
  }

  .zone-filled-body {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    width: 100%;
    text-align: center;
    padding: 20px 0;
  }

  .zone-filled-icon {
    color: #a1a1aa;
    margin-bottom: 2px;
  }

  .zone-title {
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.06em;
    color: #ffffff;
    text-transform: uppercase;
  }

  .zone-filename {
    font-size: 10px;
    font-weight: 600;
    color: #e4e4e7;
    background: #040406;
    border: 1px solid #1c1c24;
    border-radius: 5px;
    padding: 8px 10px;
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
    padding-top: 10px;
    border-top: 1px solid #14141c;
  }

  .btn-zone-action {
    flex: 1;
    padding: 7px 10px;
    background: #111116;
    border: 1px solid #24242e;
    border-radius: 4px;
    font-family: 'IBM Plex Mono', monospace;
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.05em;
    color: #a1a1aa;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .btn-zone-action:hover {
    background: #1c1c24;
    border-color: #3f3f50;
    color: #ffffff;
  }

  .btn-zone-action.danger:hover {
    background: rgba(239, 68, 68, 0.15);
    border-color: #ef4444;
    color: #ef4444;
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

  .settings-controls-card {
    background: #09090c;
    border: 1px solid #1c1c20;
    border-radius: 8px;
    padding: 12px 16px;
    display: flex;
    flex-direction: column;
    gap: 10px;
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
    text-transform: uppercase;
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
    color: #ffffff;
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

  /* Render Done Card - Monochrome Studio */
  .render-done-card {
    background: #0d0d12;
    border: 1px solid #27272a;
    border-radius: 8px;
    padding: 14px 16px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .render-done-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding-bottom: 8px;
    border-bottom: 1px solid #1c1c22;
  }

  .done-title-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .render-done-title {
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.08em;
    color: #ffffff;
  }

  .done-specs {
    font-size: 10px;
    font-weight: 600;
    color: #a1a1aa;
    letter-spacing: 0.04em;
  }

  .render-stats-grid {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 8px;
  }

  @media (max-width: 700px) {
    .render-stats-grid {
      grid-template-columns: repeat(2, 1fr);
    }
  }

  .render-stat-item {
    display: flex;
    flex-direction: column;
    gap: 3px;
    background: #121217;
    border: 1px solid #1f1f26;
    border-radius: 6px;
    padding: 8px 10px;
  }

  .done-path-box {
    display: flex;
    align-items: center;
    gap: 8px;
    background: #121217;
    border: 1px solid #1f1f26;
    border-radius: 6px;
    padding: 8px 10px;
    overflow: hidden;
  }

  .done-actions-row {
    display: flex;
    gap: 8px;
    margin-top: 2px;
  }

  .btn-open-folder {
    flex: 2;
    padding: 8px 16px;
    background: #1c1c24;
    color: #ffffff;
    border: 1px solid #3f3f46;
    border-radius: 6px;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.06em;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .btn-open-folder:hover {
    background: #272732;
    border-color: #71717a;
    color: #ffffff;
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

  /* Plan Stat Labels */

  .stat-label {
    font-size: 7.5px;
    font-weight: 700;
    letter-spacing: 0.05em;
    color: #71717a;
  }

  .stat-value {
    font-size: 9.5px;
    font-weight: 700;
    color: #ffffff;
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

  /* Toggle Rows */
  .toggle-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 7px 12px;
    background: #050508;
    border: 1px solid #1c1c24;
    border-radius: 6px;
    transition: background 0.15s ease, border-color 0.15s ease;
  }

  .toggle-row:hover {
    background: #09090f;
    border-color: #272733;
  }

  .toggle-row-left {
    display: flex;
    align-items: center;
    gap: 8px;
    cursor: help;
  }

  .toggle-row-title {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.05em;
    color: #d4d4d8;
    text-transform: uppercase;
    transition: color 0.15s ease;
  }

  .toggle-row:hover .toggle-row-title {
    color: #ffffff;
  }

  .toggle-info-icon {
    font-size: 11px;
    color: #52525b;
    line-height: 1;
    transition: color 0.15s ease;
    user-select: none;
  }

  .toggle-row:hover .toggle-info-icon {
    color: #a1a1aa;
  }

  .toggle-actions-group {
    display: flex;
    align-items: center;
    gap: 8px;
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
    font-size: 14px;
    line-height: 1;
    width: 28px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 4px;
    transition: all 0.15s ease;
  }

  .btn-close-modal:hover {
    color: #ffffff;
    background: #1c1c24;
  }

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
    background: #1c1c24;
    color: #ffffff;
    border: 1px solid #3f3f46;
    font-family: 'IBM Plex Mono', monospace;
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.05em;
    display: flex;
    align-items: center;
    gap: 5px;
  }
  .titlebar-btn.update-badge:hover {
    background: #272732;
    border-color: #71717a;
    color: #ffffff;
  }
  .update-badge-dot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: #ffffff;
    box-shadow: 0 0 4px #ffffff;
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
    color: #ffffff;
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
    background: #18181b;
    border: 1px solid #3f3f46;
    border-radius: 6px;
    color: #ffffff;
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

  /* ─── LONG VIDEO MODAL ─────────────────────────────────────────────────── */
  .long-video-modal-card {
    width: 680px;
    max-width: 92vw;
  }

  .long-video-modal-body {
    padding: 16px 20px 24px;
  }

  .long-video-options-grid {
    display: grid;
    grid-template-columns: 1fr;
    gap: 12px;
  }

  .long-video-card {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 14px 16px;
    background: #0d0d10;
    border: 1px solid #27272a;
    border-radius: 6px;
    text-align: left;
    cursor: pointer;
    transition: background-color 0.15s, border-color 0.15s, transform 0.15s;
  }

  .long-video-card:hover {
    background: #141418;
    border-color: #52525b;
    transform: translateY(-1px);
  }

  .long-video-card.selected {
    background: #18181f;
    border-color: #ffffff;
  }

  .long-video-card-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .long-video-card-title {
    font-family: 'IBM Plex Mono', monospace;
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.06em;
    color: #ffffff;
  }

  .long-video-card-tag {
    font-family: 'IBM Plex Mono', monospace;
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.05em;
    color: #a1a1aa;
    background: #1f1f26;
    padding: 2px 6px;
    border-radius: 3px;
    border: 1px solid #3f3f46;
  }

  .long-video-card-desc {
    font-family: 'IBM Plex Sans', sans-serif;
    font-size: 11px;
    color: #8e8e93;
    line-height: 1.4;
    margin: 0;
  }

  .scene-detect-loading {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 16px;
    padding: 36px 20px;
  }

  .scene-detect-spinner {
    width: 28px;
    height: 28px;
    border: 2px solid #27272a;
    border-top-color: #ffffff;
    border-radius: 50%;
    animation: spin-cw 0.75s linear infinite;
  }

  .scene-detect-text {
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.08em;
    color: #d4d4d8;
  }

  @keyframes spin-cw {
    to { transform: rotate(360deg); }
  }

  /* ─── SCENEPACK GALLERY MODAL ──────────────────────────────────────────── */
  .scenepack-modal-card {
    width: 900px;
    max-width: 95vw;
    max-height: 88vh;
    display: flex;
    flex-direction: column;
  }

  .scenepack-toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 18px;
    background: #09090c;
    border-bottom: 1px solid #1c1c22;
  }

  .clips-count-badge {
    font-size: 11px;
    font-weight: 700;
    color: #e4e4e7;
    background: #18181c;
    padding: 3px 8px;
    border-radius: 4px;
    border: 1px solid #27272a;
  }

  .scenepack-toolbar-actions {
    display: flex;
    gap: 6px;
  }

  .scenepack-modal-body {
    padding: 16px 18px;
    overflow-y: auto;
    flex: 1;
    min-height: 280px;
  }

  .scenepack-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
    gap: 12px;
  }

  .scene-card {
    background: #0e0e12;
    border: 1px solid #27272a;
    border-radius: 6px;
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    cursor: pointer;
    transition: background-color 0.15s, border-color 0.15s, transform 0.15s;
  }

  .scene-card:hover {
    background: #14141a;
    border-color: #52525b;
    transform: translateY(-1px);
  }

  .scene-card.active {
    background: #181820;
    border-color: #ffffff;
  }

  .scene-card-thumb-wrap {
    position: relative;
    width: 100%;
    aspect-ratio: 16 / 9;
    border-radius: 4px;
    overflow: hidden;
    background: #050507;
    border: 1px solid #1c1c22;
  }

  .scene-thumb-img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .scene-card-info {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .scene-card-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
  }

  .scene-card-title-row {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .scene-card-name {
    font-family: 'IBM Plex Mono', monospace;
    font-size: 11px;
    font-weight: 700;
    color: #ffffff;
  }

  .scene-duration-pill {
    font-size: 9px;
    font-weight: 700;
    color: #a1a1aa;
    background: #1f1f26;
    padding: 1px 5px;
    border-radius: 3px;
    border: 1px solid #3f3f46;
  }

  .scene-timecode-row {
    font-size: 10px;
    color: #71717a;
  }

  .scene-toggle-label {
    position: relative;
    display: inline-flex;
    align-items: center;
    cursor: pointer;
  }

  .scene-toggle-label input {
    opacity: 0;
    width: 0;
    height: 0;
    position: absolute;
  }

  .scene-toggle-custom {
    width: 16px;
    height: 16px;
    border: 1px solid #3f3f46;
    border-radius: 3px;
    background: #09090c;
    transition: all 0.15s ease;
    display: inline-block;
    position: relative;
  }

  .scene-toggle-label input:checked + .scene-toggle-custom {
    background: #ffffff;
    border-color: #ffffff;
  }

  .scene-toggle-label input:checked + .scene-toggle-custom::after {
    content: '';
    position: absolute;
    left: 4px;
    top: 1px;
    width: 4px;
    height: 8px;
    border: solid #000000;
    border-width: 0 2px 2px 0;
    transform: rotate(45deg);
  }

  .scenepack-footer {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 12px 18px;
    border-top: 1px solid #1c1c22;
    background: #09090c;
  }

  .scene-large-preview-overlay {
    position: fixed;
    z-index: 1100;
    pointer-events: none;
    width: 336px;
    background: #0e0e12;
    border: 1px solid #3f3f46;
    border-radius: 6px;
    box-shadow: 0 12px 36px rgba(0, 0, 0, 0.85);
    padding: 8px;
    animation: fadeInPreview 0.12s ease-out;
  }

  .scene-large-preview-img {
    width: 320px;
    height: 180px;
    border-radius: 4px;
    border: 1px solid #27272a;
    display: block;
    object-fit: cover;
    margin: 6px auto;
  }

  .scenepack-rhythm-header {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-bottom: 10px;
  }

  .scenepack-rhythm-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 8px;
  }

  .btn-rhythm {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 4px;
    padding: 10px 8px;
    background: #0d0d10;
    border: 1px solid #27272a;
    border-radius: 6px;
    cursor: pointer;
    text-align: center;
    transition: background-color 0.15s, border-color 0.15s, transform 0.15s;
  }

  .btn-rhythm:hover {
    background: #141418;
    border-color: #52525b;
    transform: translateY(-1px);
  }

  .btn-rhythm.active {
    background: #181820;
    border-color: #ffffff;
  }

  .rhythm-title {
    font-family: 'IBM Plex Mono', monospace;
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.08em;
    color: #ffffff;
  }

  .rhythm-desc {
    font-family: 'IBM Plex Sans', sans-serif;
    font-size: 10px;
    color: #8e8e93;
    line-height: 1.3;
  }
</style>

