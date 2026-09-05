<script>
  import GlowSlider from './GlowSlider.svelte';
  import { onMount } from 'svelte';

  let {
    blocks = $bindable([]),
    currentBlockIndex = $bindable(0),
    textColor = '#FFFFFF',
    glowIntensity = 0.8,
    glowEnabled = true,
    styleMethod = 'basic_effort',
    rapidWordEnabled = true,
    currentTime = $bindable(0),
    duration = 0,
    isPlaying = $bindable(false),
    onTogglePlay = () => {},
    onSeek = (t) => {},
    onRegenerateBlock = () => {},
  } = $props();

  let activeElementIndex = $state(0);
  let isDraggingCanvas = $state(false);
  let isResizingCanvas = $state(false);
  let resizeHandleCorner = $state('');
  let dragStartPos = { x: 0, y: 0 };
  let dragElementInitialPos = { x: 0, y: 0 };
  let resizeInitialSize = 100;

  // Visual snapping guides
  let showSnapH = $state(false);
  let showSnapV = $state(false);

  // Responsive canvas size mapping (1080x1080 coordinate space)
  let canvasContainerEl = $state(null);
  let previewWidth = $state(480);
  const CANVAS_SIZE = 1080;
  let scaleRatio = $derived(previewWidth / CANVAS_SIZE);

  // Identify active block either by currentBlockIndex (when paused) or by currentTime (when playing)
  const activePlayingBlock = $derived.by(() => {
    if (!blocks || blocks.length === 0) return null;
    const found = blocks.find(b => currentTime >= (b.start - 0.05) && currentTime <= (b.end + 0.08));
    return found || null;
  });

  const currentBlock = $derived.by(() => {
    if (isPlaying && activePlayingBlock) {
      return activePlayingBlock;
    }
    return blocks[currentBlockIndex] || null;
  });

  const currentElement = $derived(currentBlock?.elements?.[activeElementIndex] || null);

  // Update preview container dimensions dynamically
  onMount(() => {
    function updateSize() {
      if (canvasContainerEl) {
        const rect = canvasContainerEl.getBoundingClientRect();
        const availableW = Math.max(300, Math.min(rect.width - 32, 540));
        previewWidth = Math.floor(availableW);
      }
    }
    updateSize();
    window.addEventListener('resize', updateSize);
    return () => window.removeEventListener('resize', updateSize);
  });

  function prevBlock() {
    if (currentBlockIndex > 0) {
      currentBlockIndex--;
      activeElementIndex = 0;
      if (blocks[currentBlockIndex]) {
        onSeek(blocks[currentBlockIndex].start);
      }
    }
  }

  function nextBlock() {
    if (currentBlockIndex < blocks.length - 1) {
      currentBlockIndex++;
      activeElementIndex = 0;
      if (blocks[currentBlockIndex]) {
        onSeek(blocks[currentBlockIndex].start);
      }
    }
  }

  function toggleValidateCurrentBlock() {
    if (currentBlock) {
      currentBlock.isValidated = !currentBlock.isValidated;
    }
  }

  function handleRegen() {
    if (onRegenerateBlock && currentBlock) {
      onRegenerateBlock(currentBlockIndex);
    }
  }

  function handleCanvasMouseDown(e, elIndex) {
    if (isPlaying) return; // Disable dragging while video is playing
    if (isResizingCanvas) return;
    activeElementIndex = elIndex;
    isDraggingCanvas = true;
    dragStartPos = { x: e.clientX, y: e.clientY };
    if (currentBlock?.elements?.[elIndex]) {
      dragElementInitialPos = {
        x: currentBlock.elements[elIndex].x,
        y: currentBlock.elements[elIndex].y
      };
    }

    const onMouseMove = (me) => {
      if (!isDraggingCanvas || !currentBlock?.elements?.[activeElementIndex]) return;
      const dx = (me.clientX - dragStartPos.x) / scaleRatio;
      const dy = (me.clientY - dragStartPos.y) / scaleRatio;
      let targetX = Math.round(Math.max(0, Math.min(1080 - 80, dragElementInitialPos.x + dx)));
      let targetY = Math.round(Math.max(0, Math.min(1080 - 60, dragElementInitialPos.y + dy)));

      // Magnetic center snapping (tolerance +/- 14px)
      const elW = currentBlock.elements[activeElementIndex].w || 200;
      const elH = currentBlock.elements[activeElementIndex].h || 80;
      const centerX = targetX + elW / 2;
      const centerY = targetY + elH / 2;

      if (Math.abs(centerX - 540) < 14) {
        targetX = Math.round(540 - elW / 2);
        showSnapV = true;
      } else {
        showSnapV = false;
      }

      if (Math.abs(centerY - 540) < 14) {
        targetY = Math.round(540 - elH / 2);
        showSnapH = true;
      } else {
        showSnapH = false;
      }

      currentBlock.elements[activeElementIndex].x = targetX;
      currentBlock.elements[activeElementIndex].y = targetY;
    };

    const onMouseUp = () => {
      isDraggingCanvas = false;
      showSnapH = false;
      showSnapV = false;
      window.removeEventListener('mousemove', onMouseMove);
      window.removeEventListener('mouseup', onMouseUp);
    };

    window.addEventListener('mousemove', onMouseMove);
    window.addEventListener('mouseup', onMouseUp);
  }

  function handleResizeMouseDown(e, corner) {
    if (isPlaying) return;
    e.stopPropagation();
    isResizingCanvas = true;
    resizeHandleCorner = corner;
    dragStartPos = { x: e.clientX, y: e.clientY };
    if (currentElement) {
      resizeInitialSize = currentElement.size;
    }

    const onMouseMove = (me) => {
      if (!isResizingCanvas || !currentBlock?.elements?.[activeElementIndex]) return;
      const dx = (me.clientX - dragStartPos.x) / scaleRatio;
      const dy = (me.clientY - dragStartPos.y) / scaleRatio;
      
      let delta = 0;
      if (corner === 'bottom-right') {
        delta = (dx + dy) * 0.5;
      } else if (corner === 'top-left') {
        delta = -(dx + dy) * 0.5;
      } else if (corner === 'top-right') {
        delta = (dx - dy) * 0.5;
      } else if (corner === 'bottom-left') {
        delta = (-dx + dy) * 0.5;
      }

      const newSize = Math.round(Math.max(20, Math.min(260, resizeInitialSize + delta)));
      currentBlock.elements[activeElementIndex].size = newSize;
    };

    const onMouseUp = () => {
      isResizingCanvas = false;
      window.removeEventListener('mousemove', onMouseMove);
      window.removeEventListener('mouseup', onMouseUp);
    };

    window.addEventListener('mousemove', onMouseMove);
    window.addEventListener('mouseup', onMouseUp);
  }

  function getElementFontFamily(key) {
    if (key === 'hero') return "'Bahnschrift', 'Roboto Condensed', sans-serif";
    if (key === 'bold') return "'Arial', 'Helvetica Neue', sans-serif";
    if (key === 'light') return "'Segoe UI', 'Helvetica Neue', sans-serif";
    return "'Arial', sans-serif";
  }

  function getElementFontWeight(key) {
    if (key === 'hero' || key === 'bold') return '700';
    if (key === 'light') return '300';
    return '400';
  }

  // 4-frame rapid-word kinetic stretch calculation
  function getRapidWordTransform(el) {
    if (!isPlaying || !rapidWordEnabled) return '';
    const elStart = el.start != null ? el.start : (currentBlock?.start || 0);
    const ageSec = currentTime - elStart;
    if (ageSec < 0) return '';
    const frameIdx = Math.floor(ageSec * 60);
    if (frameIdx === 0) return 'scale(1.0, 8.0)';
    if (frameIdx === 1) return 'scale(8.0, 1.0)';
    if (frameIdx === 2) return 'scale(1.0, 6.0)';
    if (frameIdx === 3) return 'scale(6.0, 1.0)';
    return '';
  }

  function isElementVisible(el) {
    if (!isPlaying) return true;
    const elStart = el.start != null ? el.start : (currentBlock?.start || 0);
    return currentTime >= elStart;
  }

  // Low Effort 2-line formatting
  const lowEffortLines = $derived.by(() => {
    if (!currentBlock) return { line1: '', line2: '' };
    const rawWords = currentBlock.words || [];
    let words = [];
    if (rawWords.length > 0 && typeof rawWords[0] === 'object' && rawWords[0].word) {
      words = rawWords;
    } else {
      const texts = currentBlock.wordsRef || (currentBlock.elements ? currentBlock.elements.map(e => e.text).join(' ').split(/\s+/) : []);
      words = texts.map((w, i) => ({ word: w, start: (currentBlock.start || 0) + i * 0.2 }));
    }

    if (words.length === 0) return { line1: '', line2: '' };
    const mid = Math.max(1, Math.floor(words.length / 2));
    const wLine1 = words.slice(0, mid);
    const wLine2 = words.slice(mid);

    if (!isPlaying) {
      return {
        line1: wLine1.map(w => w.word).join(' '),
        line2: wLine2.map(w => w.word).join(' ')
      };
    }

    const rev1 = wLine1.filter(w => currentTime >= (w.start || 0)).map(w => w.word).join(' ');
    const rev2 = wLine2.filter(w => currentTime >= (w.start || 0)).map(w => w.word).join(' ');
    return { line1: rev1, line2: rev2 };
  });

  function formatTime(s) {
    if (isNaN(s) || s == null) return '0:00.0';
    const m = Math.floor(s / 60);
    const sec = Math.floor(s % 60);
    const ms = Math.floor((s % 1) * 10);
    return `${m}:${sec < 10 ? '0' : ''}${sec}.${ms}`;
  }
</script>

<div class="block-inspector-root">
  <!-- Top Bar: Navigation & Block Context -->
  <div class="inspector-header">
    <div class="block-nav-group">
      <button
        class="btn-icon-nav"
        disabled={currentBlockIndex === 0}
        onclick={prevBlock}
        aria-label="Previous block"
        type="button"
        title="Previous block"
      >
        &lt;
      </button>
      <span class="block-indicator mono">
        BLOCK {currentBlockIndex + 1} / {blocks.length}
      </span>
      <button
        class="btn-icon-nav"
        disabled={currentBlockIndex >= blocks.length - 1}
        onclick={nextBlock}
        aria-label="Next block"
        type="button"
        title="Next block"
      >
        &gt;
      </button>
    </div>

    {#if currentBlock}
      <div class="block-timecode-badge mono">
        {(currentBlock.start ?? 0).toFixed(2)}s &rarr; {(currentBlock.end ?? 0).toFixed(2)}s
        <span class="badge-dur">({((currentBlock.end ?? 0) - (currentBlock.start ?? 0)).toFixed(2)}s)</span>
      </div>
    {/if}

    <div class="header-actions">
      <button
        class="btn-regen-block"
        onclick={handleRegen}
        title="Regenerate puzzle layout for this block"
        type="button"
      >
        <span class="btn-icon">↻</span>
        <span>RE-LAYOUT</span>
      </button>

      {#if currentBlock}
        <button
          class="btn-validate-block"
          class:validated={currentBlock.isValidated}
          onclick={toggleValidateCurrentBlock}
          type="button"
        >
          {currentBlock.isValidated ? '✓ VALIDATED' : 'VALIDATE'}
        </button>
      {/if}
    </div>
  </div>

  <!-- Center Stage: 1:1 Live Canvas -->
  <div class="canvas-workspace" bind:this={canvasContainerEl}>
    <div
      class="canvas-stage"
      style="width: {previewWidth}px; height: {previewWidth}px;"
    >
      <!-- Snap-to-center magnetic lines -->
      {#if showSnapH}
        <div class="snap-line horizontal" style="top: {540 * scaleRatio}px;"></div>
      {/if}
      {#if showSnapV}
        <div class="snap-line vertical" style="left: {540 * scaleRatio}px;"></div>
      {/if}

      <!-- BASIC EFFORT: 1:1 Bounding Box Interlocking -->
      {#if styleMethod === 'basic_effort'}
        {#if currentBlock && currentBlock.elements}
          {#each currentBlock.elements as el, idx}
            {#if isElementVisible(el)}
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <div
                class="canvas-element"
                class:selected={activeElementIndex === idx && !isPlaying}
                class:rapid-stretching={Boolean(getRapidWordTransform(el))}
                style="
                  left: {el.x * scaleRatio}px;
                  top: {el.y * scaleRatio}px;
                  font-size: {el.size * scaleRatio}px;
                  font-family: {getElementFontFamily(el.key)};
                  font-weight: {getElementFontWeight(el.key)};
                  color: {textColor};
                  transform: {getRapidWordTransform(el)};
                  filter: {glowEnabled ? `drop-shadow(0 0 ${(el.size * 0.16 * glowIntensity).toFixed(1)}px ${textColor})` : 'none'};
                  letter-spacing: {el.key === 'hero' ? '0.02em' : 'normal'};
                "
                onmousedown={(e) => handleCanvasMouseDown(e, idx)}
              >
                <span class="element-text">{el.text}</span>

                {#if activeElementIndex === idx && !isPlaying}
                  <div class="element-bounding-box">
                    <!-- svelte-ignore a11y_no_static_element_interactions -->
                    <span
                      class="box-handle top-left"
                      onmousedown={(e) => handleResizeMouseDown(e, 'top-left')}
                      title="Drag to resize font"
                    ></span>
                    <!-- svelte-ignore a11y_no_static_element_interactions -->
                    <span
                      class="box-handle top-right"
                      onmousedown={(e) => handleResizeMouseDown(e, 'top-right')}
                      title="Drag to resize font"
                    ></span>
                    <!-- svelte-ignore a11y_no_static_element_interactions -->
                    <span
                      class="box-handle bottom-left"
                      onmousedown={(e) => handleResizeMouseDown(e, 'bottom-left')}
                      title="Drag to resize font"
                    ></span>
                    <!-- svelte-ignore a11y_no_static_element_interactions -->
                    <span
                      class="box-handle bottom-right"
                      onmousedown={(e) => handleResizeMouseDown(e, 'bottom-right')}
                      title="Drag to resize font"
                    ></span>
                  </div>
                {/if}
              </div>
            {/if}
          {/each}
        {/if}

      <!-- LOW EFFORT: Top-Right Perspective Tilt + 3.2x Vertical Stretch -->
      {:else if styleMethod === 'low_effort'}
        <div
          class="low-effort-perspective-box"
          style="
            filter: {glowEnabled ? `drop-shadow(0 0 16px ${textColor}) drop-shadow(0 0 35px ${textColor})` : 'none'};
            color: {textColor};
          "
        >
          <div class="low-effort-line line-1 mono">
            {lowEffortLines.line1 || '\u00A0'}
          </div>
          <div class="low-effort-line line-2 mono">
            {lowEffortLines.line2 || '\u00A0'}
          </div>
        </div>
      {/if}

      <!-- 35mm Grain Overlay Simulation -->
      <div class="grain-overlay" class:grain-active={isPlaying}></div>
    </div>
  </div>

  <!-- Player Transport Bar -->
  <div class="transport-bar">
    <button
      class="btn-play-pause"
      onclick={onTogglePlay}
      type="button"
      title="Play / Pause (Spacebar)"
      aria-label={isPlaying ? 'Pause' : 'Play'}
    >
      {#if isPlaying}
        <span class="transport-glyph">❚❚</span>
      {:else}
        <span class="transport-glyph play">&gt;</span>
      {/if}
    </button>

    <div class="timecode-display mono">
      <span class="current-tc">{formatTime(currentTime)}</span>
      <span class="tc-sep">/</span>
      <span class="total-tc">{formatTime(duration)}</span>
    </div>

    <!-- Timeline Scrubber with Block Marks -->
    <div class="scrubber-track-container">
      <input
        type="range"
        class="timeline-scrubber"
        min="0"
        max={Math.max(1, duration)}
        step="0.01"
        value={currentTime}
        oninput={(e) => onSeek(parseFloat(e.currentTarget.value))}
      />
      <!-- Visual Block Markers on Scrubber Track -->
      <div class="scrubber-markers">
        {#each (blocks || []) as b}
          <div
            class="block-marker"
            class:marker-active={currentTime >= (b.start ?? 0) && currentTime <= (b.end ?? 0)}
            style="left: {((b.start ?? 0) / Math.max(1, duration || 1)) * 100}%; width: {(((b.end ?? 0) - (b.start ?? 0)) / Math.max(1, duration || 1)) * 100}%;"
            title="#{b.id || ''}: {(b.start ?? 0).toFixed(2)}s - {(b.end ?? 0).toFixed(2)}s"
          ></div>
        {/each}
      </div>
    </div>

    <div class="transport-shortcuts-hint mono">
      SPACE: PLAY/PAUSE
    </div>
  </div>

  <!-- Quick Inspector Sliders (when paused and element selected in Basic Effort) -->
  {#if !isPlaying && styleMethod === 'basic_effort' && currentElement}
    <div class="quick-adjust-bar">
      <div class="adjust-tag mono">
        WORD: "{currentElement.text || ''}" ({(currentElement.key || 'HERO').toUpperCase()})
      </div>
      <div class="adjust-sliders-row">
        <div class="slider-cell">
          <GlowSlider
            label="X"
            bind:value={currentElement.x}
            min={0}
            max={980}
            step={5}
            unit="px"
          />
        </div>
        <div class="slider-cell">
          <GlowSlider
            label="Y"
            bind:value={currentElement.y}
            min={0}
            max={980}
            step={5}
            unit="px"
          />
        </div>
        <div class="slider-cell">
          <GlowSlider
            label="SIZE"
            bind:value={currentElement.size}
            min={20}
            max={240}
            step={2}
            unit="px"
          />
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .block-inspector-root {
    display: flex;
    flex-direction: column;
    height: 100%;
    width: 100%;
    background: #08080a;
    border: 1px solid #1c1c20;
    border-radius: 8px;
    overflow: hidden;
  }

  /* Header */
  .inspector-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 10px 16px;
    background: #0d0d10;
    border-bottom: 1px solid #1c1c20;
    flex-shrink: 0;
  }

  .block-nav-group {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .btn-icon-nav {
    background: #121215;
    border: 1px solid #27272a;
    color: #ffffff;
    width: 26px;
    height: 26px;
    border-radius: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    font-size: 11px;
    font-weight: 700;
    transition: all 0.15s ease;
  }

  .btn-icon-nav:hover:not(:disabled) {
    background: #1c1c20;
    border-color: #ffffff;
  }

  .btn-icon-nav:disabled {
    opacity: 0.3;
    cursor: not-allowed;
  }

  .block-indicator {
    font-size: 11px;
    font-weight: 700;
    color: #ffffff;
    letter-spacing: 0.05em;
  }

  .block-timecode-badge {
    font-size: 11px;
    color: #a1a1aa;
    background: #121215;
    padding: 4px 10px;
    border-radius: 4px;
    border: 1px solid #1c1c20;
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .badge-dur {
    color: #71717a;
  }

  .header-actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .btn-regen-block {
    display: flex;
    align-items: center;
    gap: 5px;
    background: #121215;
    border: 1px solid #27272a;
    color: #a1a1aa;
    font-size: 11px;
    font-weight: 700;
    padding: 4px 10px;
    border-radius: 4px;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .btn-regen-block:hover {
    background: #1c1c20;
    border-color: #ffffff;
    color: #ffffff;
  }

  .btn-validate-block {
    background: #121215;
    border: 1px solid #27272a;
    color: #ffffff;
    font-size: 11px;
    font-weight: 700;
    padding: 4px 12px;
    border-radius: 4px;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .btn-validate-block:hover {
    border-color: #ffffff;
  }

  .btn-validate-block.validated {
    background: #052e16;
    border-color: #22c55e;
    color: #4ade80;
  }

  /* Canvas Workspace */
  .canvas-workspace {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    background: #040406;
    padding: 16px;
    position: relative;
    user-select: none;
    min-height: 320px;
  }

  .canvas-stage {
    position: relative;
    background: #000000;
    border: 1px solid #27272a;
    border-radius: 4px;
    box-shadow: 0 12px 36px rgba(0, 0, 0, 0.9);
    overflow: hidden;
  }

  /* Snapping Guides */
  .snap-line {
    position: absolute;
    z-index: 5;
    pointer-events: none;
  }

  .snap-line.horizontal {
    left: 0;
    right: 0;
    height: 1px;
    background: #38bdf8;
    box-shadow: 0 0 4px #38bdf8;
  }

  .snap-line.vertical {
    top: 0;
    bottom: 0;
    width: 1px;
    background: #38bdf8;
    box-shadow: 0 0 4px #38bdf8;
  }

  /* Elements */
  .canvas-element {
    position: absolute;
    cursor: move;
    line-height: 1.05;
    white-space: nowrap;
    transform-origin: center center;
    transition: transform 0.03s linear;
  }

  .canvas-element.rapid-stretching {
    z-index: 20;
  }

  .canvas-element.selected .element-bounding-box {
    position: absolute;
    inset: -6px -8px;
    border: 1px dashed rgba(255, 255, 255, 0.7);
    pointer-events: none;
  }

  .box-handle {
    position: absolute;
    width: 8px;
    height: 8px;
    background: #ffffff;
    border: 1px solid #000000;
    pointer-events: auto;
  }

  .box-handle.top-left { top: -4px; left: -4px; cursor: nwse-resize; }
  .box-handle.top-right { top: -4px; right: -4px; cursor: nesw-resize; }
  .box-handle.bottom-left { bottom: -4px; left: -4px; cursor: nesw-resize; }
  .box-handle.bottom-right { bottom: -4px; right: -4px; cursor: nwse-resize; }

  /* Low Effort Perspective View */
  .low-effort-perspective-box {
    position: absolute;
    top: 15%;
    right: 5%;
    width: 75%;
    height: 55%;
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 30px;
    transform: perspective(600px) rotateY(-18deg) rotateX(12deg);
    transform-origin: top right;
    pointer-events: none;
  }

  .low-effort-line {
    font-size: 24px;
    font-weight: 700;
    transform: scaleY(3.2) scaleX(0.95);
    transform-origin: left center;
    letter-spacing: -0.02em;
    white-space: pre;
    line-height: 1;
  }

  /* 35mm Grain Overlay */
  .grain-overlay {
    position: absolute;
    inset: 0;
    pointer-events: none;
    opacity: 0.12;
    background-image: radial-gradient(#ffffff 1px, transparent 1px);
    background-size: 3px 3px;
  }

  .grain-overlay.grain-active {
    animation: grainShift 0.2s steps(2) infinite;
  }

  @keyframes grainShift {
    0% { background-position: 0 0; }
    50% { background-position: 1px 2px; }
    100% { background-position: 2px 1px; }
  }

  /* Transport Bar */
  .transport-bar {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 16px;
    background: #0b0b0e;
    border-top: 1px solid #1c1c20;
    flex-shrink: 0;
  }

  .btn-play-pause {
    width: 32px;
    height: 32px;
    border-radius: 4px;
    background: #ffffff;
    color: #000000;
    border: none;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 13px;
    font-weight: 800;
    transition: transform 0.1s ease, background 0.15s ease;
    flex-shrink: 0;
  }

  .btn-play-pause:hover {
    background: #e4e4e7;
    transform: scale(1.05);
  }

  .transport-glyph.play {
    font-size: 16px;
    margin-left: 2px;
  }

  .timecode-display {
    font-size: 11px;
    color: #a1a1aa;
    display: flex;
    align-items: center;
    gap: 4px;
    flex-shrink: 0;
  }

  .current-tc {
    color: #ffffff;
    font-weight: 700;
  }

  .scrubber-track-container {
    position: relative;
    flex: 1;
    display: flex;
    align-items: center;
    height: 24px;
  }

  .timeline-scrubber {
    width: 100%;
    accent-color: #ffffff;
    cursor: pointer;
    z-index: 2;
  }

  .scrubber-markers {
    position: absolute;
    left: 0;
    right: 0;
    height: 4px;
    pointer-events: none;
    display: flex;
  }

  .block-marker {
    position: absolute;
    height: 100%;
    background: rgba(255, 255, 255, 0.15);
    border-right: 1px solid #000000;
    border-radius: 1px;
  }

  .block-marker.marker-active {
    background: rgba(255, 255, 255, 0.5);
  }

  .transport-shortcuts-hint {
    font-size: 10px;
    color: #52525b;
    letter-spacing: 0.05em;
    flex-shrink: 0;
  }

  /* Quick Adjust Bar */
  .quick-adjust-bar {
    padding: 8px 16px;
    background: #0e0e12;
    border-top: 1px solid #1c1c20;
    display: flex;
    flex-direction: column;
    gap: 6px;
    flex-shrink: 0;
  }

  .adjust-tag {
    font-size: 10px;
    color: #a1a1aa;
    font-weight: 700;
    letter-spacing: 0.05em;
  }

  .adjust-sliders-row {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 12px;
  }

  .slider-cell {
    display: flex;
    flex-direction: column;
  }
</style>
