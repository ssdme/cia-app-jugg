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

  const VIDEO_EXTENSIONS = ['mp4', 'mkv', 'webm', 'mov', 'avi'];
  const AUDIO_EXTENSIONS = ['mp3', 'wav', 'flac', 'm4a', 'ogg'];

  let allZonesFilled = $derived(Boolean(scenePath && drumsPath && audioPath));

  const ABOUT_LINKS = [
    { name: 'Practical-RIFE', detail: 'Frame interpolation', mark: 'rife', url: 'https://github.com/hzwer/Practical-RIFE' },
    { name: 'smoothie-rs', detail: 'Frame blending', mark: 'smoothie', url: 'https://github.com/couleur-tweak-tips/smoothie-rs' },
    { name: 'VapourSynth', detail: 'Video processing', mark: 'vapoursynth', url: 'https://github.com/vapoursynth/vapoursynth' },
    { name: 'FFmpeg', detail: 'Media tooling', mark: 'ffmpeg', url: 'https://github.com/FFmpeg/FFmpeg' },
    { name: 'Tauri', detail: 'Desktop runtime', mark: 'tauri', url: 'https://github.com/tauri-apps/tauri' },
    { name: 'Svelte', detail: 'Interface framework', mark: 'svelte', url: 'https://github.com/sveltejs/svelte' },
    { name: 'IBM Plex', detail: 'Interface typography', mark: 'plex', url: 'https://github.com/IBM/plex' },
    { name: 'Flowframes', detail: 'Workflow reference', mark: 'flowframes', url: 'https://github.com/n00mkrad/flowframes' }
  ];
  const PROJECT_REPOSITORY_URL = 'https://github.com/cia213/cia-app';

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
    }
  }

  async function handlePickFile(zone, event) {
    if (event) event.stopPropagation();
    try {
      const kind = zone === 'scene' ? 'video' : 'audio';
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

  function handleCustomWidthInput(e) {
    customWidth = parseInt(e.target.value, 10) || 0;
    validateCustomDimensions();
  }

  function handleCustomHeightInput(e) {
    customHeight = parseInt(e.target.value, 10) || 0;
    validateCustomDimensions();
  }

  function handleRunProcess() {
    if (selectedAspectRatio === 'CUSTOM') {
      validateCustomDimensions();
      if (customArError) {
        showToast('Please specify valid custom dimensions before running', 'error');
        return;
      }
    }
    showToast('RUN — implemented in task T5', 'info');
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

  onMount(async () => {
    try {
      appVersion = await invoke('get_app_version');
    } catch (e) {
      console.error('Failed to retrieve app version:', e);
    }
    checkForAppUpdates(false);
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
                        {sceneInfo.duration.toFixed(2)}s · {sceneInfo.width}x{sceneInfo.height} · {sceneInfo.fps.toFixed(0)}fps
                      </span>
                    {/if}
                  </div>

                  <!-- DRUMS -->
                  <div class="source-row">
                    <span class="source-tag">DRUMS</span>
                    <span class="source-name mono" title={drumsPath}>{getFileName(drumsPath)}</span>
                    {#if drumsInfo}
                      <span class="meta-pill mono">
                        {drumsInfo.duration.toFixed(2)}s · {drumsInfo.audioSampleRate}Hz · {bpm ? bpm.toFixed(1) : '—'} BPM · {beats ? beats.length : 0} beats ({downbeats ? downbeats.length : 0} downbeats)
                      </span>
                    {/if}
                  </div>

                  <!-- AUDIO -->
                  <div class="source-row">
                    <span class="source-tag">AUDIO</span>
                    <span class="source-name mono" title={audioPath}>{getFileName(audioPath)}</span>
                    {#if audioInfo}
                      <span class="meta-pill mono">
                        {audioInfo.duration.toFixed(2)}s · {audioInfo.audioSampleRate}Hz · {audioInfo.audioChannels}ch
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
                        onclick={() => selectedStyle = style.id}
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
              </div>

              <!-- Footer Actions -->
              <div class="settings-actions-footer">
                <button class="btn-pro-secondary" onclick={() => navigateTo('remap')}>
                  &lt; BACK TO SOURCES
                </button>
                <button class="btn-run-process" onclick={handleRunProcess}>
                  RUN PROCESS &gt;
                </button>
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

  .about-kicker {
    display: block;
    color: #71717a;
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.12em;
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
</style>
