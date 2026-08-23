<script>
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';

  let {
    plan = null,
    beats = [],
    downbeats = [],
    currentTime = $bindable(0.0),
    onScrub = null,
  } = $props();

  let canvasRef = $state(null);
  let isDragging = $state(false);
  let isPlaying = $state(false);
  let lastFrameTime = $state(0);
  let animFrameId = null;
  let lastScrubCall = 0;
  let instantaneousVelocity = $state(1.0);
  let currentSourceTime = $state(0.0);

  let duration = $derived(plan?.targetDuration || plan?.target_duration || 5.0);
  let segments = $derived(plan?.segments || []);

  function formatTimecode(secs) {
    const s = Math.max(0, secs || 0);
    const mins = Math.floor(s / 60);
    const remSecs = Math.floor(s % 60);
    const ms = Math.floor((s - Math.floor(s)) * 1000);
    return `${String(mins).padStart(2, '0')}:${String(remSecs).padStart(2, '0')}.${String(ms).padStart(3, '0')}`;
  }

  async function triggerScrub(t) {
    currentTime = Math.max(0, Math.min(duration, t));
    const now = performance.now();
    if (now - lastScrubCall >= 50) { // Throttle ~20 fps
      lastScrubCall = now;
      if (plan) {
        try {
          const res = await invoke('get_scrub_frame', {
            targetTimeMs: currentTime * 1000.0,
            plan: plan
          });
          const parsed = typeof res === 'string' ? JSON.parse(res) : res;
          instantaneousVelocity = parsed.velocity ?? 1.0;
          currentSourceTime = parsed.sourceTime ?? currentTime;
          if (onScrub) onScrub(parsed);
        } catch (e) {
          console.warn('Scrub error:', e);
        }
      }
    }
  }

  function handleMouseDown(e) {
    isDragging = true;
    updateTimeFromPointer(e);
  }

  function handleMouseMove(e) {
    if (isDragging) {
      updateTimeFromPointer(e);
    }
  }

  function handleMouseUp() {
    isDragging = false;
  }

  function updateTimeFromPointer(e) {
    if (!canvasRef) return;
    const rect = canvasRef.getBoundingClientRect();
    const x = Math.max(0, Math.min(rect.width, e.clientX - rect.left));
    const pct = x / rect.width;
    triggerScrub(pct * duration);
  }

  function togglePlayPause() {
    isPlaying = !isPlaying;
    if (isPlaying) {
      lastFrameTime = performance.now();
      playLoop();
    } else {
      if (animFrameId) cancelAnimationFrame(animFrameId);
    }
  }

  function playLoop() {
    if (!isPlaying) return;
    const now = performance.now();
    const dt = (now - lastFrameTime) / 1000.0;
    lastFrameTime = now;

    let nextTime = currentTime + dt;
    if (nextTime >= duration) {
      nextTime = 0.0; // Loop playback
    }
    triggerScrub(nextTime);

    animFrameId = requestAnimationFrame(playLoop);
  }

  function drawTimeline() {
    if (!canvasRef) return;
    const ctx = canvasRef.getContext('2d');
    const w = canvasRef.width;
    const h = canvasRef.height;

    ctx.clearRect(0, 0, w, h);

    // 1. Background grid
    ctx.fillStyle = '#0a0a10';
    ctx.fillRect(0, 0, w, h);

    // 2. Zero-line and Normal Speed (1.0x) line
    const zeroY = h * 0.70;
    const oneY = h * 0.35;

    ctx.strokeStyle = '#1e1e2f';
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(0, zeroY);
    ctx.lineTo(w, zeroY);
    ctx.stroke();

    ctx.strokeStyle = '#27273a';
    ctx.setLineDash([4, 4]);
    ctx.beginPath();
    ctx.moveTo(0, oneY);
    ctx.lineTo(w, oneY);
    ctx.stroke();
    ctx.setLineDash([]);

    // Labels
    ctx.fillStyle = '#71717a';
    ctx.font = '9px monospace';
    ctx.fillText('1.0x (NORMAL)', 8, oneY - 4);
    ctx.fillText('0.0x (FREEZE)', 8, zeroY - 4);

    const dur = Math.max(0.1, duration);

    // 3. Beat Markers (Gray)
    if (Array.isArray(beats)) {
      ctx.strokeStyle = '#3f3f46';
      ctx.lineWidth = 1;
      for (const b of beats) {
        if (b >= 0 && b <= dur) {
          const x = (b / dur) * w;
          ctx.beginPath();
          ctx.moveTo(x, 0);
          ctx.lineTo(x, h);
          ctx.stroke();
        }
      }
    }

    // 4. Downbeat Markers (Neon Red/Pink)
    if (Array.isArray(downbeats)) {
      ctx.strokeStyle = '#ef4444';
      ctx.lineWidth = 1.5;
      for (const db of downbeats) {
        if (db >= 0 && db <= dur) {
          const x = (db / dur) * w;
          ctx.beginPath();
          ctx.moveTo(x, 0);
          ctx.lineTo(x, h);
          ctx.stroke();
        }
      }
    }

    // 5. Segment Cut Markers (White)
    ctx.strokeStyle = '#ffffff';
    ctx.lineWidth = 2;
    for (const seg of segments) {
      if (seg.t0 > 0 && seg.t0 <= dur) {
        const x = (seg.t0 / dur) * w;
        ctx.beginPath();
        ctx.moveTo(x, 0);
        ctx.lineTo(x, h);
        ctx.stroke();
      }
    }

    // 6. Time-Curve Velocity graph
    if (segments.length > 0) {
      ctx.strokeStyle = '#06b6d4';
      ctx.lineWidth = 2.5;
      ctx.shadowColor = '#06b6d4';
      ctx.shadowBlur = 8;
      ctx.beginPath();

      const samples = 200;
      for (let i = 0; i <= samples; i++) {
        const t = (i / samples) * dur;
        const x = (t / dur) * w;

        // Estimate velocity
        let v = 1.0;
        const seg = segments.find(s => t >= s.t0 && t <= s.t1) || segments[0];
        if (seg) {
          const dt = Math.max(1e-6, seg.t1 - seg.t0);
          const ds = seg.s1 - seg.s0;
          if (Math.abs(ds) < 1e-6) {
            v = 0.0;
          } else {
            v = ds / dt;
          }
        }

        // Map v: 0.0 -> zeroY, 1.0 -> oneY
        const y = zeroY - (v * (zeroY - oneY));
        const clampedY = Math.max(6, Math.min(h - 6, y));

        if (i === 0) ctx.moveTo(x, clampedY);
        else ctx.lineTo(x, clampedY);
      }
      ctx.stroke();
      ctx.shadowBlur = 0;
    }

    // 7. Playhead
    const playheadX = (currentTime / dur) * w;
    ctx.strokeStyle = '#f59e0b';
    ctx.lineWidth = 2.5;
    ctx.shadowColor = '#f59e0b';
    ctx.shadowBlur = 10;
    ctx.beginPath();
    ctx.moveTo(playheadX, 0);
    ctx.lineTo(playheadX, h);
    ctx.stroke();
    ctx.shadowBlur = 0;

    // Playhead head
    ctx.fillStyle = '#f59e0b';
    ctx.beginPath();
    ctx.moveTo(playheadX - 6, 0);
    ctx.lineTo(playheadX + 6, 0);
    ctx.lineTo(playheadX, 8);
    ctx.closePath();
    ctx.fill();
  }

  $effect(() => {
    // Redraw whenever inputs change
    currentTime;
    plan;
    beats;
    downbeats;
    drawTimeline();
  });

  onMount(() => {
    const handleResize = () => {
      if (canvasRef) {
        canvasRef.width = canvasRef.parentElement.clientWidth || 800;
        canvasRef.height = 90;
        drawTimeline();
      }
    };
    handleResize();
    window.addEventListener('resize', handleResize);
    window.addEventListener('mouseup', handleMouseUp);
    window.addEventListener('mousemove', handleMouseMove);

    return () => {
      window.removeEventListener('resize', handleResize);
      window.removeEventListener('mouseup', handleMouseUp);
      window.removeEventListener('mousemove', handleMouseMove);
      if (animFrameId) cancelAnimationFrame(animFrameId);
    };
  });
</script>

<div class="timeline-container">
  <div class="timeline-toolbar">
    <div class="toolbar-left">
      <button class="btn-play-pause mono" onclick={togglePlayPause} type="button">
        {isPlaying ? 'PAUSE' : 'PLAY'}
      </button>
      <div class="timecode-display mono">
        <span class="tc-current">{formatTimecode(currentTime)}</span>
        <span class="tc-divider">/</span>
        <span class="tc-total">{formatTimecode(duration)}</span>
      </div>
    </div>

    <div class="toolbar-center">
      <span class="legend-item"><span class="dot white"></span> CUTS</span>
      <span class="legend-item"><span class="dot white-muted"></span> DOWNBEATS</span>
      <span class="legend-item"><span class="dot gray"></span> BEATS</span>
      <span class="legend-item"><span class="dot white-solid"></span> SPEED CURVE</span>
    </div>

    <div class="toolbar-right mono">
      <span class="stat-pill">
        {#if Math.abs(instantaneousVelocity) < 1e-4}
          0.00x (FREEZE)
        {:else if instantaneousVelocity < 0}
          {instantaneousVelocity.toFixed(2)}x (REVERSE)
        {:else}
          {instantaneousVelocity.toFixed(2)}x (SPEED)
        {/if}
      </span>
      <span class="stat-pill source-time">SRC: {formatTimecode(currentSourceTime)}</span>
    </div>
  </div>

  <div class="timeline-canvas-wrapper" onmousedown={handleMouseDown}>
    <canvas bind:this={canvasRef} class="timeline-canvas"></canvas>
  </div>
</div>

<style>
  .timeline-container {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px 14px;
    background: #09090c;
    border: 1px solid #1c1c20;
    border-radius: 8px;
    width: 100%;
    box-sizing: border-box;
  }

  .timeline-toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .toolbar-left,
  .toolbar-center,
  .toolbar-right {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .btn-play-pause {
    background: #ffffff;
    border: 1px solid #ffffff;
    color: #000000;
    padding: 5px 12px;
    border-radius: 4px;
    font-size: 11px;
    font-weight: 700;
    cursor: pointer;
    transition: all 120ms ease;
  }
  .btn-play-pause:hover {
    background: #e4e4e7;
    box-shadow: 0 0 10px rgba(255, 255, 255, 0.2);
  }

  .timecode-display {
    font-size: 11px;
    font-weight: 700;
    background: #0d0d10;
    padding: 4px 8px;
    border-radius: 4px;
    border: 1px solid #27272a;
  }
  .tc-current {
    color: #ffffff;
  }
  .tc-divider {
    color: #52525b;
  }
  .tc-total {
    color: #71717a;
  }

  .legend-item {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 9.5px;
    font-weight: 700;
    color: #71717a;
  }
  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
  }
  .dot.white { background: #ffffff; }
  .dot.white-muted { background: #a1a1aa; }
  .dot.gray { background: #52525b; }
  .dot.white-solid { background: #ffffff; box-shadow: 0 0 4px rgba(255, 255, 255, 0.5); }

  .stat-pill {
    font-size: 10px;
    font-weight: 700;
    padding: 3px 8px;
    border-radius: 4px;
    background: #0d0d10;
    color: #ffffff;
    border: 1px solid #27272a;
  }
  .stat-pill.source-time {
    color: #a1a1aa;
  }

  .timeline-canvas-wrapper {
    width: 100%;
    height: 90px;
    background: #050507;
    border: 1px solid #1c1c20;
    border-radius: 6px;
    overflow: hidden;
    cursor: crosshair;
  }

  .timeline-canvas {
    width: 100%;
    height: 100%;
    display: block;
  }
</style>
