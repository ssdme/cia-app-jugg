<script>
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { onMount, onDestroy } from 'svelte';
  import GlowSlider from './GlowSlider.svelte';
  import ColorPickerPopup from './ColorPickerPopup.svelte';
  import BlockInspector from './BlockInspector.svelte';

  let {
    onToast = (msg, type) => {}
  } = $props();

  // Audio File & Playback State
  let audioPath = $state('');
  let audioBlobUrl = $state('');
  let audioElement = $state(null);
  let isPlaying = $state(false);
  let currentTime = $state(0);
  let duration = $state(0);
  let animFrameId = null;

  // Analysis / Whisper State
  let isAnalyzing = $state(false);
  let analyzeProgress = $state(0);
  let analyzeMessage = $state('INITIALIZING WHISPER ENGINE...');
  let isDraggingFile = $state(false);
  let transcriptData = $state(null);
  let transcriptText = $state('');

  // Blocks & Layout State
  let blocks = $state([]);
  let currentBlockIndex = $state(0);

  // Style & Visual Parameters
  let selectedStyle = $state('basic_effort'); // 'basic_effort' | 'low_effort'
  let glowEnabled = $state(true);
  let glowIntensity = $state(0.85);
  let textColor = $state('#FFFFFF');
  let rapidWordEnabled = $state(true);

  // Render & Modal State
  let isRendering = $state(false);
  let renderProgress = $state(0);
  let renderLog = $state('');
  let renderedVideoPath = $state('');
  let showRenderDoneModal = $state(false);

  function getFileName(p) {
    if (!p) return '';
    const parts = p.split(/[\\/]/);
    return parts[parts.length - 1];
  }

  // Load audio file into Web Audio / HTML5 Blob URL for seamless local playback
  async function loadAudioMedia(path) {
    try {
      const bytes = await invoke('read_media_file_bytes', { path });
      if (bytes && bytes.length > 0) {
        const lower = path.toLowerCase();
        const mime = lower.endsWith('.wav') ? 'audio/wav' :
                     lower.endsWith('.flac') ? 'audio/flac' :
                     lower.endsWith('.ogg') ? 'audio/ogg' :
                     lower.endsWith('.m4a') || lower.endsWith('.aac') ? 'audio/mp4' :
                     'audio/mpeg';
        const blob = new Blob([new Uint8Array(bytes)], { type: mime });
        if (audioBlobUrl) URL.revokeObjectURL(audioBlobUrl);
        audioBlobUrl = URL.createObjectURL(blob);
        if (audioElement) {
          audioElement.src = audioBlobUrl;
          audioElement.load();
        }
      }
    } catch (e) {
      console.warn('Direct media bytes load failed, fallback to native URI:', e);
    }
  }

  // Native File Picker Dialog
  async function handlePickAudio() {
    try {
      const selected = await invoke('pick_file', { kind: 'audio' });
      if (selected && typeof selected === 'string') {
        audioPath = selected;
        loadAudioMedia(selected);
        startRealVocalAnalysis(selected);
      }
    } catch (e) {
      console.warn('Native picker error:', e);
    }
  }

  // Drag and Drop Handlers
  function handleDragOver(e) {
    e.preventDefault();
    isDraggingFile = true;
  }

  function handleDragLeave() {
    isDraggingFile = false;
  }

  function handleDrop(e) {
    e.preventDefault();
    isDraggingFile = false;
    // Note: Do not extract files from e.dataTransfer because Windows WebView2 lacks
    // absolute file system paths. Tauri's native 'tauri://drag-drop' event handler handles
    // the drop with full filesystem paths.
  }

  // Speech-to-Text with Faster-Whisper via Rust IPC
  async function startRealVocalAnalysis(path) {
    if (!path || isAnalyzing) return;
    isAnalyzing = true;
    analyzeProgress = 10;
    analyzeMessage = 'INITIALIZING WHISPER SPEECH MODEL (LARGE-V3-TURBO)...';

    try {
      const res = await invoke('transcribe_audio', { audioPath: path });
      if (res && (res.segments || res.text)) {
        transcriptData = res;
        transcriptText = res.text ? res.text.trim() : '';
        if (res.duration && res.duration > 0) {
          duration = res.duration;
        }
        analyzeProgress = 100;
        try {
          generateBlocksFromWhisper(res);
        } catch (genErr) {
          console.error('Error parsing Whisper segments into blocks:', genErr);
          generateFallbackBlocks(res.duration || duration || 15.0);
        }
        onToast(`Transcribed ${res.duration ? `${res.duration.toFixed(1)}s` : ''} vocal speech successfully`, 'success');
      } else {
        throw new Error('No transcript data returned from model');
      }
    } catch (err) {
      console.error('Whisper transcription error:', err);
      onToast(`Transcription notice: ${err?.message || err}. Generating rhythmic fallback...`, 'info');
      // Graceful fallback from audio duration
      generateFallbackBlocks(duration || 15.0);
    } finally {
      isAnalyzing = false;
    }
  }

  // Exact Text Measurement via Canvas 2D
  let measureCtx = null;
  function getMeasuredWidth(text, size, isHero = false) {
    if (typeof document !== 'undefined') {
      if (!measureCtx) {
        const c = document.createElement('canvas');
        measureCtx = c.getContext('2d');
      }
      const fontFam = isHero ? "'Bahnschrift', 'Roboto Condensed', sans-serif" : "'Arial', sans-serif";
      measureCtx.font = `700 ${size}px ${fontFam}`;
      const m = measureCtx.measureText(text);
      return Math.round(m.width);
    }
    return Math.round(text.length * size * (isHero ? 0.58 : 0.60));
  }

  // High-Fidelity Bounding Box Puzzle Composer for Basic Effort
  function buildPuzzleElementsForWords(wordsList, variant = 0, wordsMeta = []) {
    if (!wordsList || wordsList.length === 0) return [];
    const upperWords = wordsList.map(w => w.toUpperCase());
    const n = upperWords.length;

    let rowsDef = [];
    if (n <= 3) {
      rowsDef = [
        { words: upperWords.slice(0, Math.min(2, n)), meta: wordsMeta.slice(0, Math.min(2, n)), type: 'hero' },
        { words: upperWords.slice(Math.min(2, n)), meta: wordsMeta.slice(Math.min(2, n)), type: 'sub' }
      ];
    } else if (n <= 5) {
      if (variant % 2 === 0) {
        rowsDef = [
          { words: upperWords.slice(0, 2), meta: wordsMeta.slice(0, 2), type: 'hero' },
          { words: upperWords.slice(2, 4), meta: wordsMeta.slice(2, 4), type: 'sub' },
          { words: upperWords.slice(4), meta: wordsMeta.slice(4), type: 'punch' }
        ];
      } else {
        rowsDef = [
          { words: upperWords.slice(0, 2), meta: wordsMeta.slice(0, 2), type: 'sub' },
          { words: upperWords.slice(2, 4), meta: wordsMeta.slice(2, 4), type: 'hero' },
          { words: upperWords.slice(4), meta: wordsMeta.slice(4), type: 'punch' }
        ];
      }
    } else if (n <= 7) {
      if (variant % 3 === 0) {
        rowsDef = [
          { words: upperWords.slice(0, 2), meta: wordsMeta.slice(0, 2), type: 'hero' },
          { words: upperWords.slice(2, 4), meta: wordsMeta.slice(2, 4), type: 'sub' },
          { wordsLeft: upperWords.slice(4, 6), metaLeft: wordsMeta.slice(4, 6), wordsRight: upperWords.slice(6), metaRight: wordsMeta.slice(6), type: 'split' }
        ];
      } else if (variant % 3 === 1) {
        rowsDef = [
          { words: upperWords.slice(0, 3), meta: wordsMeta.slice(0, 3), type: 'sub' },
          { words: upperWords.slice(3, 5), meta: wordsMeta.slice(3, 5), type: 'hero' },
          { wordsLeft: upperWords.slice(5, 6), metaLeft: wordsMeta.slice(5, 6), wordsRight: upperWords.slice(6), metaRight: wordsMeta.slice(6), type: 'split' }
        ];
      } else {
        rowsDef = [
          { wordsLeft: upperWords.slice(0, 2), metaLeft: wordsMeta.slice(0, 2), wordsRight: upperWords.slice(2, 3), metaRight: wordsMeta.slice(2, 3), type: 'splitHeroRight' },
          { words: upperWords.slice(3, 5), meta: wordsMeta.slice(3, 5), type: 'sub' },
          { words: upperWords.slice(5), meta: wordsMeta.slice(5), type: 'hero' }
        ];
      }
    } else {
      if (variant % 3 === 0) {
        rowsDef = [
          { words: upperWords.slice(0, 2), meta: wordsMeta.slice(0, 2), type: 'hero' },
          { words: upperWords.slice(2, 5), meta: wordsMeta.slice(2, 5), type: 'sub' },
          { wordsLeft: upperWords.slice(5, 7), metaLeft: wordsMeta.slice(5, 7), wordsRight: upperWords.slice(7, 8), metaRight: wordsMeta.slice(7, 8), type: 'split' },
          { words: upperWords.slice(8), meta: wordsMeta.slice(8), type: 'punch' }
        ];
      } else if (variant % 3 === 1) {
        rowsDef = [
          { words: upperWords.slice(0, 3), meta: wordsMeta.slice(0, 3), type: 'sub' },
          { words: upperWords.slice(3, 5), meta: wordsMeta.slice(3, 5), type: 'hero' },
          { wordsLeft: upperWords.slice(5, 7), metaLeft: wordsMeta.slice(5, 7), wordsRight: upperWords.slice(7), metaRight: wordsMeta.slice(7), type: 'split' }
        ];
      } else {
        rowsDef = [
          { words: upperWords.slice(0, 2), meta: wordsMeta.slice(0, 2), type: 'hero' },
          { words: upperWords.slice(2, 4), meta: wordsMeta.slice(2, 4), type: 'sub' },
          { wordsLeft: upperWords.slice(4, 6), metaLeft: wordsMeta.slice(4, 6), wordsRight: upperWords.slice(6, 8), metaRight: wordsMeta.slice(6, 8), type: 'split' },
          { words: upperWords.slice(8), meta: wordsMeta.slice(8), type: 'hero' }
        ];
      }
    }

    rowsDef = rowsDef.filter(r => r && (r.words?.length > 0 || r.wordsLeft?.length > 0));

    const MAX_ROW_WIDTH = 860;
    const rawRows = [];

    for (const r of rowsDef) {
      if (r.type === 'hero' || r.type === 'sub' || r.type === 'punch') {
        const txt = r.words.join(' ');
        let sz = r.type === 'hero' ? 120 : (r.type === 'punch' ? 65 : 55);
        let w = getMeasuredWidth(txt, sz, r.type === 'hero');
        if (w > MAX_ROW_WIDTH) {
          sz = Math.floor(sz * (MAX_ROW_WIDTH / w));
          w = getMeasuredWidth(txt, sz, r.type === 'hero');
        }
        const wStart = r.meta && r.meta[0] ? r.meta[0].start : null;
        const wEnd = r.meta && r.meta[r.meta.length - 1] ? r.meta[r.meta.length - 1].end : null;

        rawRows.push([{
          text: txt,
          key: r.type === 'hero' ? 'hero' : (r.type === 'punch' ? 'regular' : 'bold'),
          size: sz,
          w,
          h: sz * 1.05,
          start: wStart,
          end: wEnd
        }]);
      } else if (r.type === 'split' || r.type === 'splitHeroRight') {
        const tLeft = r.wordsLeft.join(' ');
        const tRight = r.wordsRight?.join(' ') || '';
        let szL = r.type === 'split' ? 42 : 48;
        let szR = r.type === 'split' ? 95 : 110;
        let wL = getMeasuredWidth(tLeft, szL, false);
        let wR = tRight ? getMeasuredWidth(tRight, szR, true) : 0;

        if (wL + wR + 30 > MAX_ROW_WIDTH) {
          const ratio = MAX_ROW_WIDTH / (wL + wR + 30);
          szR = Math.floor(szR * ratio);
          szL = Math.floor(szL * ratio);
          wL = getMeasuredWidth(tLeft, szL, false);
          wR = tRight ? getMeasuredWidth(tRight, szR, true) : 0;
        }
        const rowH = Math.max(szL * 1.05, szR * 1.05);

        const lStart = r.metaLeft && r.metaLeft[0] ? r.metaLeft[0].start : null;
        const lEnd = r.metaLeft && r.metaLeft[r.metaLeft.length - 1] ? r.metaLeft[r.metaLeft.length - 1].end : null;
        const rStart = r.metaRight && r.metaRight[0] ? r.metaRight[0].start : null;
        const rEnd = r.metaRight && r.metaRight[r.metaRight.length - 1] ? r.metaRight[r.metaRight.length - 1].end : null;

        if (tRight) {
          rawRows.push([
            { text: tLeft, key: r.type === 'split' ? 'light' : 'bold', size: szL, w: wL, h: rowH, start: lStart, end: lEnd },
            { text: tRight, key: 'hero', size: szR, w: wR, h: rowH, start: rStart, end: rEnd }
          ]);
        } else {
          rawRows.push([{ text: tLeft, key: 'bold', size: szL, w: wL, h: rowH, start: lStart, end: lEnd }]);
        }
      }
    }

    const ROW_GAP = 14;
    const totalHeight = rawRows.reduce((acc, row) => acc + Math.max(...row.map(el => el.h)), 0) + ROW_GAP * (rawRows.length - 1);
    let currentY = Math.floor((1080 - totalHeight) / 2);
    const elements = [];

    for (const row of rawRows) {
      const rowH = Math.max(...row.map(el => el.h));
      if (row.length === 1) {
        const el = row[0];
        const x = Math.floor((1080 - el.w) / 2);
        elements.push({
          text: el.text,
          key: el.key,
          size: el.size,
          w: el.w,
          h: el.h,
          x: Math.max(80, Math.min(980 - el.w, x)),
          y: currentY,
          start: el.start,
          end: el.end
        });
      } else if (row.length === 2) {
        const elL = row[0];
        const elR = row[1];
        const totalW = elL.w + 25 + elR.w;
        const startX = Math.floor((1080 - totalW) / 2);

        const yL = currentY + Math.floor(rowH - elL.h);
        const yR = currentY + Math.floor(rowH - elR.h);

        elements.push({
          text: elL.text,
          key: elL.key,
          size: elL.size,
          w: elL.w,
          h: elL.h,
          x: Math.max(80, startX),
          y: yL,
          start: elL.start,
          end: elL.end
        });
        elements.push({
          text: elR.text,
          key: elR.key,
          size: elR.size,
          w: elR.w,
          h: elR.h,
          x: Math.max(80, startX + Math.floor(elL.w) + 25),
          y: yR,
          start: elR.start,
          end: elR.end
        });
      }
      currentY += Math.floor(rowH + ROW_GAP);
    }

    return elements;
  }

  // Group Whisper Word Timestamps into Rhythmic Blocks
  function generateBlocksFromWhisper(transcriptRes) {
    const allWords = [];
    if (transcriptRes.segments && transcriptRes.segments.length > 0) {
      for (const seg of transcriptRes.segments) {
        if (seg.words && seg.words.length > 0) {
          for (const w of seg.words) {
            const clean = w.word.trim();
            if (clean) {
              allWords.push({
                word: clean,
                start: parseFloat(w.start),
                end: parseFloat(w.end),
                probability: w.probability ?? 1.0
              });
            }
          }
        } else if (seg.text) {
          const words = seg.text.trim().split(/\s+/);
          const dur = Math.max(0.2, seg.end - seg.start);
          const wDur = dur / Math.max(1, words.length);
          words.forEach((w, i) => {
            allWords.push({
              word: w,
              start: parseFloat((seg.start + i * wDur).toFixed(3)),
              end: parseFloat((seg.start + (i + 1) * wDur).toFixed(3)),
              probability: 0.95
            });
          });
        }
      }
    }

    if (allWords.length === 0) {
      generateFallbackBlocks(duration || 10.0);
      return;
    }

    const grouped = [];
    let curGroup = [];

    for (let i = 0; i < allWords.length; i++) {
      const w = allWords[i];
      curGroup.push(w);

      const isLast = i === allWords.length - 1;
      const next = !isLast ? allWords[i + 1] : null;
      const pauseAfter = next ? (next.start - w.end) > 0.35 : false;
      const hasPunctuation = /[.!?]$/.test(w.word);
      const reachedMax = curGroup.length >= 7;
      const reachedMinWithBreak = curGroup.length >= 3 && (pauseAfter || hasPunctuation);

      if (isLast || reachedMax || reachedMinWithBreak) {
        grouped.push([...curGroup]);
        curGroup = [];
      }
    }
    if (curGroup.length > 0) {
      grouped.push(curGroup);
    }

    const generatedBlocks = [];
    grouped.forEach((wordGroup, bIdx) => {
      const bStart = wordGroup[0].start;
      const bEnd = wordGroup[wordGroup.length - 1].end + 0.15;
      const wordsTextList = wordGroup.map(w => w.word);

      const elements = buildPuzzleElementsForWords(wordsTextList, bIdx, wordGroup);
      // Ensure each element has a valid start time relative to block start
      elements.forEach((el, eIdx) => {
        if (el.start == null) {
          el.start = bStart + eIdx * 0.15;
        }
      });

      generatedBlocks.push({
        id: `b${bIdx + 1}`,
        start: parseFloat(bStart.toFixed(2)),
        end: parseFloat(bEnd.toFixed(2)),
        isValidated: bIdx === 0,
        layoutVariant: bIdx % 4,
        words: wordGroup,
        wordsRef: wordsTextList,
        elements
      });
    });

    blocks = generatedBlocks;
    currentBlockIndex = 0;
  }

  function generateFallbackBlocks(totalDur) {
    const text = transcriptText || "FREESTYLE VOCALS DEMO TRACK";
    const words = text.split(/\s+/).filter(w => w.length > 0);
    const chunkSize = 5;
    const generated = [];
    const blockCount = Math.max(1, Math.ceil(words.length / chunkSize));
    const blockDur = totalDur / blockCount;

    for (let i = 0; i < blockCount; i++) {
      const bWords = words.slice(i * chunkSize, (i + 1) * chunkSize);
      const bStart = parseFloat((i * blockDur).toFixed(2));
      const bEnd = parseFloat(((i + 1) * blockDur).toFixed(2));
      const wordObjs = bWords.map((w, idx) => ({
        word: w,
        start: bStart + idx * (blockDur / bWords.length),
        end: bStart + (idx + 1) * (blockDur / bWords.length)
      }));

      const elements = buildPuzzleElementsForWords(bWords, i, wordObjs);
      elements.forEach((el, eIdx) => {
        if (el.start == null) el.start = bStart + eIdx * 0.15;
      });

      generated.push({
        id: `b${i + 1}`,
        start: bStart,
        end: bEnd,
        isValidated: i === 0,
        layoutVariant: i % 4,
        words: wordObjs,
        wordsRef: bWords,
        elements
      });
    }

    blocks = generated;
    currentBlockIndex = 0;
  }

  // Audio Playback & Animation Loop
  function togglePlay() {
    if (!audioElement) return;
    if (isPlaying) {
      audioElement.pause();
      isPlaying = false;
    } else {
      audioElement.play().then(() => {
        isPlaying = true;
        startAnimationLoop();
      }).catch(err => {
        console.warn('Audio play error:', err);
      });
    }
  }

  function seekTo(targetTime) {
    currentTime = Math.max(0, Math.min(duration || 10, targetTime));
    if (audioElement) {
      audioElement.currentTime = currentTime;
    }
    // Update active block index based on seek target
    const bIdx = blocks.findIndex(b => currentTime >= b.start && currentTime <= b.end);
    if (bIdx !== -1) {
      currentBlockIndex = bIdx;
    }
  }

  function startAnimationLoop() {
    if (animFrameId) cancelAnimationFrame(animFrameId);
    function loop() {
      if (audioElement && !audioElement.paused) {
        currentTime = audioElement.currentTime;
        animFrameId = requestAnimationFrame(loop);
      } else {
        isPlaying = false;
      }
    }
    animFrameId = requestAnimationFrame(loop);
  }

  function handleRegenerateBlock(idx) {
    if (!blocks[idx]) return;
    const b = blocks[idx];
    b.layoutVariant = (b.layoutVariant || 0) + 1;
    b.elements = buildPuzzleElementsForWords(b.wordsRef, b.layoutVariant, b.words || []);
    onToast(`Block #${idx + 1} layout refreshed`, 'info');
  }

  function handleRegenerateAllBlocks() {
    blocks.forEach((b, idx) => {
      b.layoutVariant = (b.layoutVariant || 0) + 1;
      b.elements = buildPuzzleElementsForWords(b.wordsRef, b.layoutVariant, b.words || []);
    });
    onToast(`Regenerated all ${blocks.length} block layouts`, 'success');
  }

  // Style change handler
  function chooseStyle(style) {
    selectedStyle = style;
    if (style === 'low_effort') {
      textColor = '#FF0C14'; // Saturated Red for Low Effort
    } else {
      textColor = '#FFFFFF'; // Crisp White for Basic Effort
    }
  }

  // Keyboard Shortcuts (Spacebar: Play/Pause, Left/Right: Seek)
  function handleWindowKeyDown(e) {
    if (e.target && (e.target.tagName === 'INPUT' || e.target.tagName === 'TEXTAREA')) {
      return;
    }
    if (e.key === ' ' || e.code === 'Space') {
      e.preventDefault();
      togglePlay();
    } else if (e.key === 'ArrowLeft') {
      e.preventDefault();
      seekTo(currentTime - 1.0);
    } else if (e.key === 'ArrowRight') {
      e.preventDefault();
      seekTo(currentTime + 1.0);
    }
  }

  // Render Execution Pipeline
  async function handleStartRender() {
    if (isPlaying && audioElement) {
      audioElement.pause();
      isPlaying = false;
    }

    isRendering = true;
    renderProgress = 0;
    renderLog = 'Launching Python typographic render pipeline...';

    try {
      const resultPath = await invoke('render_text_video', {
        spec: {
          audioPath: audioPath || '',
          styleMethod: selectedStyle,
          textColor: textColor,
          glowEnabled: glowEnabled,
          glowIntensity: glowIntensity,
          rapidWordEnabled: rapidWordEnabled,
          blocks: blocks
        }
      });

      renderedVideoPath = resultPath;
      isRendering = false;
      renderProgress = 100;
      showRenderDoneModal = true;
      onToast('Video rendered and exported successfully!', 'success');
    } catch (err) {
      console.error('Render error:', err);
      isRendering = false;
      onToast(`Render error: ${err?.message || err}`, 'error');
    }
  }

  async function handleOpenTargetFolder() {
    try {
      await invoke('open_target_folder', { path: renderedVideoPath });
    } catch (e) {
      console.warn('Open folder error:', e);
      onToast(`Saved file: ${renderedVideoPath}`, 'info');
    }
  }

  function handleResetVocals() {
    if (isPlaying && audioElement) {
      audioElement.pause();
      isPlaying = false;
    }
    audioPath = '';
    blocks = [];
    transcriptText = '';
    transcriptData = null;
    currentTime = 0;
    duration = 0;
  }

  onMount(() => {
    let unlistenDrag = null;
    let unlistenRender = null;
    let unlistenTranscribe = null;

    listen('tauri://drag-drop', (event) => {
      const paths = event.payload?.paths;
      if (paths && paths.length > 0) {
        audioPath = paths[0];
        loadAudioMedia(paths[0]);
        startRealVocalAnalysis(paths[0]);
      }
    }).then(u => { unlistenDrag = u; });

    listen('transcribe-progress', (event) => {
      if (event.payload) {
        analyzeProgress = event.payload.percent || analyzeProgress;
        analyzeMessage = event.payload.message || analyzeMessage;
      }
    }).then(u => { unlistenTranscribe = u; });

    listen('render-text-progress', (event) => {
      if (event.payload) {
        renderProgress = event.payload.percent || 0;
        renderLog = event.payload.message || `Rendering frame ${event.payload.currentFrame}/${event.payload.totalFrames}`;
      }
    }).then(u => { unlistenRender = u; });

    window.addEventListener('keydown', handleWindowKeyDown);

    return () => {
      if (unlistenDrag) unlistenDrag();
      if (unlistenTranscribe) unlistenTranscribe();
      if (unlistenRender) unlistenRender();
      if (animFrameId) cancelAnimationFrame(animFrameId);
      window.removeEventListener('keydown', handleWindowKeyDown);
    };
  });

  onDestroy(() => {
    if (audioBlobUrl) URL.revokeObjectURL(audioBlobUrl);
  });
</script>

<div class="text-studio-root">
  <!-- Hidden HTML5 Audio Element for Real-Time Sync -->
  <audio
    bind:this={audioElement}
    onloadedmetadata={() => {
      if (audioElement && audioElement.duration) {
        duration = audioElement.duration;
      }
    }}
    onended={() => {
      isPlaying = false;
      currentTime = duration;
    }}
  ></audio>

  <!-- STATE 1: VOCAL AUDIO INGESTION / DROP ZONE -->
  {#if !audioPath || isAnalyzing}
    <div class="studio-ingest-container">
      <div
        class="vocal-drop-zone"
        class:analyzing={isAnalyzing}
        class:dragging={isDraggingFile}
        onclick={handlePickAudio}
        ondragover={handleDragOver}
        ondragleave={handleDragLeave}
        ondrop={handleDrop}
        role="button"
        tabindex="0"
        onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && handlePickAudio()}
      >
        {#if isAnalyzing}
          <div class="analyzing-content">
            <div class="spinner-ring"></div>
            <div class="analyzing-title">ANALYZING SPEECH WITH WHISPER (LARGE-V3-TURBO)...</div>
            <div class="analyzing-sub mono">{analyzeProgress}% — {analyzeMessage}</div>
            <div class="progress-bar-wrap">
              <div class="progress-bar-fill" style="width: {analyzeProgress}%;"></div>
            </div>
          </div>
        {:else}
          <div class="empty-drop-content">
            <div class="drop-icon-box">
              <svg viewBox="0 0 24 24" width="40" height="40" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
                <path d="M12 2a3 3 0 0 0-3 3v7a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3Z"></path>
                <path d="M19 10v2a7 7 0 0 1-14 0v-2"></path>
                <line x1="12" y1="19" x2="12" y2="22"></line>
              </svg>
            </div>
            <div class="drop-title">DRAG VOCAL AUDIO HERE</div>
            <div class="drop-sub mono">OR CLICK TO BROWSE (MP3, WAV, FLAC, M4A, AAC, OGG)</div>
            <div class="drop-features-badges">
              <span class="feat-badge mono">WHISPER ONSET SYNC</span>
              <span class="feat-badge mono">1:1 PUZZLE LAYOUT</span>
              <span class="feat-badge mono">RAPID-WORD STRETCH</span>
              <span class="feat-badge mono">PRO-MIST GLOW</span>
            </div>
          </div>
        {/if}
      </div>
    </div>

  <!-- STATE 2: UNIFIED STUDIO COCKPIT -->
  {:else}
    <!-- Top Studio Context Bar -->
    <header class="studio-top-bar">
      <div class="file-identity-group">
        <span class="pro-dot active"></span>
        <span class="studio-title-badge">TEXT STUDIO</span>
        <span class="audio-filename mono" title={audioPath}>{getFileName(audioPath)}</span>
        <span class="audio-duration-pill mono">{(duration ?? 0).toFixed(2)}s</span>
      </div>

      <div class="top-bar-actions">
        <button class="btn-pro-secondary" onclick={handleResetVocals}>
          &lt; CHANGE VOCALS
        </button>
      </div>
    </header>

    <!-- 3-Column Studio Grid -->
    <div class="studio-workspace-grid">
      <!-- COLUMN 1: VOCALS & BLOCKS LIST -->
      <aside class="studio-col vocal-blocks-col">
        <div class="col-card-header">
          <div class="col-title-group">
            <span class="col-title">VOCAL BLOCKS</span>
            <span class="blocks-count-pill mono">{blocks.length}</span>
          </div>
          <button
            class="btn-regen-all"
            onclick={handleRegenerateAllBlocks}
            title="Refresh layout for all blocks"
            type="button"
          >
            ↻ RE-LAYOUT ALL
          </button>
        </div>

        <div class="blocks-scroll-list">
          {#each blocks as b, idx}
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <div
              class="block-card"
              class:selected={currentBlockIndex === idx}
              class:playing-active={currentTime >= b.start && currentTime <= b.end}
              onclick={() => {
                currentBlockIndex = idx;
                seekTo(b.start);
              }}
              role="button"
              tabindex="0"
            >
              <div class="block-card-top">
                <span class="block-id-badge mono">#{idx + 1}</span>
                <span class="block-tc-text mono">{(b.start ?? 0).toFixed(2)}s &rarr; {(b.end ?? 0).toFixed(2)}s</span>
                <span class="block-status-dot" class:valid={b.isValidated}></span>
              </div>
              <div class="block-lyrics-snippet mono">
                {b.wordsRef ? b.wordsRef.join(' ') : (b.elements ? b.elements.map(e => e.text).join(' ') : '')}
              </div>
            </div>
          {/each}
        </div>
      </aside>

      <!-- COLUMN 2: CENTER LIVE CANVAS & TRANSPORT PLAYER -->
      <main class="studio-col canvas-center-col">
        <BlockInspector
          bind:blocks={blocks}
          bind:currentBlockIndex={currentBlockIndex}
          bind:currentTime={currentTime}
          bind:isPlaying={isPlaying}
          {duration}
          {textColor}
          {glowIntensity}
          {glowEnabled}
          styleMethod={selectedStyle}
          {rapidWordEnabled}
          onTogglePlay={togglePlay}
          onSeek={seekTo}
          onRegenerateBlock={handleRegenerateBlock}
        />
      </main>

      <!-- COLUMN 3: STYLE & EFFECTS CONTROLS -->
      <aside class="studio-col style-controls-col">
        <div class="col-card-header">
          <span class="col-title">STYLE &amp; EFFECTS</span>
          <span class="pro-dot active"></span>
        </div>

        <div class="style-controls-scroll">
          <!-- 1. Style Selection Cards -->
          <div class="control-group">
            <span class="group-label">TYPOGRAPHY PRESET</span>
            <div class="styles-grid-pair">
              <button
                class="style-card-btn"
                class:selected={selectedStyle === 'basic_effort'}
                onclick={() => chooseStyle('basic_effort')}
                type="button"
              >
                <div class="style-card-row">
                  <span class="style-name">BASIC EFFORT</span>
                  <span class="pro-dot" class:active={selectedStyle === 'basic_effort'}></span>
                </div>
                <p class="style-desc">1:1 square puzzle interlocking, 4-frame rapid-word stretch, Pro-Mist optical glow, 35mm grain.</p>
              </button>

              <button
                class="style-card-btn"
                class:selected={selectedStyle === 'low_effort'}
                onclick={() => chooseStyle('low_effort')}
                type="button"
              >
                <div class="style-card-row">
                  <span class="style-name">LOW EFFORT</span>
                  <span class="pro-dot" class:active={selectedStyle === 'low_effort'}></span>
                </div>
                <p class="style-desc">Top-right perspective tilt, 3.2x vertical stretch, 2-line sequential typewriter reveal.</p>
              </button>
            </div>
          </div>

          <!-- 2. Optical Glow -->
          <div class="control-group">
            <div class="toggle-row">
              <span class="group-label">OPTICAL GLOW</span>
              <button
                class="toggle-btn"
                class:active={glowEnabled}
                onclick={() => glowEnabled = !glowEnabled}
                type="button"
              >
                {glowEnabled ? 'ON' : 'OFF'}
              </button>
            </div>
            {#if glowEnabled}
              <div class="slider-embed">
                <GlowSlider
                  label="GLOW INTENSITY"
                  bind:value={glowIntensity}
                  min={0.0}
                  max={1.0}
                  step={0.05}
                  precision={2}
                />
              </div>
            {/if}
          </div>

          <!-- 3. Font Color Picker -->
          <div class="control-group">
            <ColorPickerPopup bind:color={textColor} label="TEXT COLOR" />
          </div>

          <!-- 4. Rapid-Word Stretch Toggle (Basic Effort) -->
          {#if selectedStyle === 'basic_effort'}
            <div class="control-group">
              <div class="toggle-row">
                <div class="toggle-copy">
                  <span class="group-label">RAPID-WORD STRETCH</span>
                  <span class="toggle-sub">4-frame kinetic scaling (x8 &rarr; x6 &rarr; rest)</span>
                </div>
                <button
                  class="toggle-btn"
                  class:active={rapidWordEnabled}
                  onclick={() => rapidWordEnabled = !rapidWordEnabled}
                  type="button"
                >
                  {rapidWordEnabled ? 'ON' : 'OFF'}
                </button>
              </div>
            </div>
          {/if}

          <!-- 5. Export Plan Summary Card -->
          <div class="plan-summary-card">
            <div class="plan-summary-header">
              <span class="plan-summary-title">RENDER SPEC</span>
              <span class="pro-dot active"></span>
            </div>
            <div class="plan-summary-grid">
              <div class="plan-stat">
                <span class="stat-label">STYLE</span>
                <span class="stat-value mono">{selectedStyle.toUpperCase().replace('_', ' ')}</span>
              </div>
              <div class="plan-stat">
                <span class="stat-label">BLOCKS / DURATION</span>
                <span class="stat-value mono">{blocks.length} cuts • {(duration ?? 0).toFixed(2)}s</span>
              </div>
              <div class="plan-stat">
                <span class="stat-label">FRAMERATE</span>
                <span class="stat-value mono">60 FPS</span>
              </div>
              <div class="plan-stat">
                <span class="stat-label">GLOW</span>
                <span class="stat-value mono">{glowEnabled ? `${(glowIntensity * 100).toFixed(0)}%` : 'DISABLED'}</span>
              </div>
            </div>
          </div>

          <!-- 6. Run Export Action -->
          <div class="export-action-block">
            <button
              class="btn-run-export"
              onclick={handleStartRender}
              disabled={isRendering}
              type="button"
            >
              {#if isRendering}
                <span class="spinner-inline"></span>
                <span>RENDERING ({renderProgress}%)...</span>
              {:else}
                <span>RUN PROCESS EXPORT &gt;</span>
              {/if}
            </button>
          </div>
        </div>
      </aside>
    </div>
  {/if}

  <!-- Render Done Modal -->
  {#if showRenderDoneModal}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div class="modal-backdrop" onclick={() => showRenderDoneModal = false} role="presentation">
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <div class="modal-dialog-box" onclick={(e) => e.stopPropagation()} role="dialog" aria-modal="true" tabindex="-1">
        <div class="modal-icon-badge">✓</div>
        <div class="modal-title">RENDER COMPLETED SUCCESSFULLY</div>
        <div class="modal-path-box mono" title={renderedVideoPath}>{renderedVideoPath}</div>
        <div class="modal-actions">
          <button class="btn-pro-secondary" onclick={() => showRenderDoneModal = false}>CLOSE</button>
          <button class="btn-primary-action" onclick={handleOpenTargetFolder}>OPEN FOLDER</button>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .text-studio-root {
    display: flex;
    flex-direction: column;
    height: 100%;
    width: 100%;
    background: #050507;
    overflow: hidden;
  }

  /* Ingestion State */
  .studio-ingest-container {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 24px;
  }

  .vocal-drop-zone {
    width: 100%;
    max-width: 680px;
    height: 380px;
    background: #08080a;
    border: 2px dashed #27272a;
    border-radius: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition: border-color 0.15s ease, background 0.15s ease;
    user-select: none;
  }

  .vocal-drop-zone:hover,
  .vocal-drop-zone.dragging {
    border-color: #ffffff;
    background: #0d0d10;
  }

  .vocal-drop-zone.analyzing {
    border-style: solid;
    border-color: #3f3f46;
    cursor: wait;
  }

  .empty-drop-content {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    text-align: center;
  }

  .drop-icon-box {
    color: #a1a1aa;
    margin-bottom: 4px;
  }

  .drop-title {
    font-size: 16px;
    font-weight: 800;
    letter-spacing: 0.08em;
    color: #ffffff;
  }

  .drop-sub {
    font-size: 11px;
    color: #71717a;
  }

  .drop-features-badges {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 8px;
    margin-top: 12px;
  }

  .feat-badge {
    font-size: 10px;
    color: #a1a1aa;
    background: #111116;
    padding: 4px 8px;
    border-radius: 4px;
    border: 1px solid #1c1c20;
  }

  /* Analyzing State */
  .analyzing-content {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 14px;
    text-align: center;
    width: 80%;
  }

  .spinner-ring {
    width: 38px;
    height: 38px;
    border: 3px solid #27272a;
    border-top-color: #ffffff;
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .analyzing-title {
    font-size: 13px;
    font-weight: 700;
    color: #ffffff;
    letter-spacing: 0.05em;
  }

  .analyzing-sub {
    font-size: 11px;
    color: #a1a1aa;
  }

  .progress-bar-wrap {
    width: 100%;
    height: 4px;
    background: #1c1c20;
    border-radius: 2px;
    overflow: hidden;
  }

  .progress-bar-fill {
    height: 100%;
    background: #ffffff;
    transition: width 0.2s ease;
  }

  /* Top Studio Bar */
  .studio-top-bar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 8px 16px;
    background: #08080a;
    border-bottom: 1px solid #1c1c20;
    flex-shrink: 0;
  }

  .file-identity-group {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .studio-title-badge {
    font-size: 11px;
    font-weight: 800;
    color: #ffffff;
    letter-spacing: 0.08em;
  }

  .audio-filename {
    font-size: 11px;
    color: #a1a1aa;
    background: #111116;
    padding: 3px 8px;
    border-radius: 4px;
    border: 1px solid #1c1c20;
    max-width: 280px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .audio-duration-pill {
    font-size: 11px;
    color: #71717a;
  }

  /* 3-Column Studio Workspace */
  .studio-workspace-grid {
    flex: 1;
    display: grid;
    grid-template-columns: 280px 1fr 310px;
    gap: 12px;
    padding: 12px;
    overflow: hidden;
    min-height: 0;
  }

  .studio-col {
    background: #08080a;
    border: 1px solid #1c1c20;
    border-radius: 8px;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .col-card-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 10px 14px;
    background: #0d0d10;
    border-bottom: 1px solid #1c1c20;
    flex-shrink: 0;
  }

  .col-title-group {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .col-title {
    font-size: 11px;
    font-weight: 800;
    color: #a1a1aa;
    letter-spacing: 0.08em;
  }

  .blocks-count-pill {
    font-size: 10px;
    font-weight: 700;
    color: #ffffff;
    background: #1c1c20;
    padding: 2px 6px;
    border-radius: 4px;
  }

  .btn-regen-all {
    background: transparent;
    border: 1px solid #27272a;
    color: #a1a1aa;
    font-size: 10px;
    font-weight: 700;
    padding: 3px 8px;
    border-radius: 4px;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .btn-regen-all:hover {
    color: #ffffff;
    border-color: #ffffff;
  }

  /* Blocks List (Col 1) */
  .blocks-scroll-list {
    flex: 1;
    overflow-y: auto;
    padding: 10px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .block-card {
    background: #0d0d10;
    border: 1px solid #1c1c20;
    border-radius: 6px;
    padding: 10px;
    cursor: pointer;
    transition: border-color 0.15s ease, background 0.15s ease;
  }

  .block-card:hover {
    border-color: #3f3f46;
    background: #111116;
  }

  .block-card.selected {
    border-color: #ffffff;
    background: #121215;
  }

  .block-card.playing-active {
    border-color: #22c55e;
    box-shadow: inset 0 0 0 1px #22c55e;
  }

  .block-card-top {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 6px;
  }

  .block-id-badge {
    font-size: 10px;
    font-weight: 700;
    color: #a1a1aa;
  }

  .block-tc-text {
    font-size: 10px;
    color: #71717a;
  }

  .block-status-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #3f3f46;
  }

  .block-status-dot.valid {
    background: #22c55e;
    box-shadow: 0 0 6px #22c55e;
  }

  .block-lyrics-snippet {
    font-size: 11px;
    line-height: 1.4;
    color: #e4e4e7;
    white-space: pre-wrap;
    word-break: break-word;
  }

  /* Center Stage Col */
  .canvas-center-col {
    background: #050507;
    border: none;
  }

  /* Style Controls (Col 3) */
  .style-controls-scroll {
    flex: 1;
    overflow-y: auto;
    padding: 14px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .control-group {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .group-label {
    font-size: 11px;
    font-weight: 800;
    color: #a1a1aa;
    letter-spacing: 0.05em;
  }

  .styles-grid-pair {
    display: grid;
    grid-template-columns: 1fr;
    gap: 8px;
  }

  .style-card-btn {
    background: #0d0d10;
    border: 1px solid #1c1c20;
    border-radius: 6px;
    padding: 10px;
    text-align: left;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .style-card-btn:hover {
    border-color: #3f3f46;
    background: #111116;
  }

  .style-card-btn.selected {
    border-color: #ffffff;
    background: #121215;
  }

  .style-card-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 4px;
  }

  .style-name {
    font-size: 12px;
    font-weight: 800;
    color: #ffffff;
  }

  .style-desc {
    font-size: 10px;
    color: #71717a;
    line-height: 1.35;
    margin: 0;
  }

  .toggle-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .toggle-copy {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .toggle-sub {
    font-size: 10px;
    color: #71717a;
  }

  .toggle-btn {
    background: #111116;
    border: 1px solid #27272a;
    color: #71717a;
    font-size: 11px;
    font-weight: 700;
    padding: 4px 12px;
    border-radius: 4px;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .toggle-btn.active {
    background: #ffffff;
    color: #000000;
    border-color: #ffffff;
  }

  .slider-embed {
    margin-top: 4px;
  }

  /* Plan Summary Card */
  .plan-summary-card {
    background: #0d0d10;
    border: 1px solid #1c1c20;
    border-radius: 6px;
    padding: 12px;
  }

  .plan-summary-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 10px;
  }

  .plan-summary-title {
    font-size: 10px;
    font-weight: 800;
    color: #a1a1aa;
    letter-spacing: 0.08em;
  }

  .plan-summary-grid {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .plan-stat {
    display: flex;
    justify-content: space-between;
    font-size: 11px;
  }

  .stat-label {
    color: #71717a;
    font-weight: 600;
  }

  .stat-value {
    color: #e4e4e7;
  }

  /* Run Export Action */
  .export-action-block {
    margin-top: auto;
    padding-top: 8px;
  }

  .btn-run-export {
    width: 100%;
    background: #ffffff;
    color: #000000;
    font-size: 12px;
    font-weight: 800;
    letter-spacing: 0.08em;
    padding: 12px 16px;
    border: none;
    border-radius: 6px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    transition: background 0.15s ease, transform 0.1s ease;
  }

  .btn-run-export:hover:not(:disabled) {
    background: #e4e4e7;
    transform: translateY(-1px);
  }

  .btn-run-export:disabled {
    opacity: 0.6;
    cursor: wait;
  }

  .spinner-inline {
    width: 14px;
    height: 14px;
    border: 2px solid #71717a;
    border-top-color: #000000;
    border-radius: 50%;
    animation: spin 0.6s linear infinite;
  }

  /* Modal */
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.75);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
    backdrop-filter: blur(4px);
  }

  .modal-dialog-box {
    background: #09090c;
    border: 1px solid #27272a;
    border-radius: 8px;
    padding: 24px;
    max-width: 520px;
    width: 90%;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 14px;
    box-shadow: 0 20px 48px rgba(0, 0, 0, 0.95);
    text-align: center;
  }

  .modal-icon-badge {
    width: 44px;
    height: 44px;
    border-radius: 50%;
    background: #052e16;
    color: #22c55e;
    border: 1px solid #22c55e;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 20px;
    font-weight: 800;
  }

  .modal-title {
    font-size: 14px;
    font-weight: 800;
    color: #ffffff;
    letter-spacing: 0.05em;
  }

  .modal-path-box {
    font-size: 11px;
    color: #a1a1aa;
    background: #111116;
    padding: 8px 12px;
    border-radius: 4px;
    border: 1px solid #1c1c20;
    max-width: 100%;
    word-break: break-all;
  }

  .modal-actions {
    display: flex;
    gap: 12px;
    margin-top: 8px;
  }

  /* Shared App Button Styles */
  .btn-pro-secondary {
    background: #111116;
    border: 1px solid #27272a;
    color: #a1a1aa;
    font-size: 11px;
    font-weight: 700;
    padding: 6px 14px;
    border-radius: 4px;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .btn-pro-secondary:hover {
    background: #1c1c20;
    color: #ffffff;
    border-color: #ffffff;
  }

  .btn-primary-action {
    background: #ffffff;
    color: #000000;
    font-size: 11px;
    font-weight: 700;
    padding: 6px 14px;
    border: none;
    border-radius: 4px;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .btn-primary-action:hover {
    background: #e4e4e7;
  }

  .pro-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #3f3f46;
    transition: all 0.15s ease;
  }

  .pro-dot.active {
    background: #22c55e;
    box-shadow: 0 0 6px #22c55e;
  }
</style>
