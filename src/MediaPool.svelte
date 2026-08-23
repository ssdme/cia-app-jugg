<script>
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';

  let {
    onSelectAsScene = null,
    onSelectAsDrums = null,
    onSelectAsAudio = null,
    onDirectOneClick = null,
  } = $props();

  let assets = $state([]);
  let isImporting = $state(false);
  let isDraggingOver = $state(false);

  async function loadMediaPool() {
    try {
      const list = await invoke('get_media_pool');
      if (Array.isArray(list)) {
        assets = list;
      }
    } catch (e) {
      console.warn('Failed to load media pool:', e);
    }
  }

  async function handleImportPaths(paths) {
    if (!paths || paths.length === 0) return;
    isImporting = true;
    try {
      const imported = await invoke('import_media_to_pool', { paths });
      await loadMediaPool();
    } catch (e) {
      console.error('Import error:', e);
    } finally {
      isImporting = false;
    }
  }

  async function handleManualImport() {
    const p = prompt('Enter video/audio file path to import into Media Pool:', 'C:/Videos/footage.mp4');
    if (p) {
      await handleImportPaths([p]);
    }
  }

  async function handleRemove(hash) {
    try {
      await invoke('remove_media_from_pool', { hash });
      await loadMediaPool();
    } catch (e) {
      console.error('Remove error:', e);
    }
  }

  function handleDrop(e) {
    e.preventDefault();
    isDraggingOver = false;
    if (e.dataTransfer && e.dataTransfer.files) {
      const paths = [];
      for (let i = 0; i < e.dataTransfer.files.length; i++) {
        const file = e.dataTransfer.files[i];
        if (file.path) {
          paths.push(file.path);
        } else if (file.name) {
          paths.push(file.name);
        }
      }
      if (paths.length > 0) {
        handleImportPaths(paths);
      }
    }
  }

  function handleDragOver(e) {
    e.preventDefault();
    isDraggingOver = true;
  }

  function handleDragLeave(e) {
    e.preventDefault();
    isDraggingOver = false;
  }

  function formatTime(secs) {
    const s = Math.max(0, secs || 0);
    const m = Math.floor(s / 60);
    const rs = Math.floor(s % 60);
    return `${m}:${String(rs).padStart(2, '0')}`;
  }

  onMount(() => {
    loadMediaPool();
  });
</script>

<div
  class="media-pool-container card-cyber"
  class:dragging-over={isDraggingOver}
  ondrop={handleDrop}
  ondragover={handleDragOver}
  ondragleave={handleDragLeave}
  role="region"
  aria-label="Media Pool Assets"
>
  <div class="media-pool-header">
    <div class="media-pool-title-row">
      <span class="group-label">🗄️ MEDIA POOL & SMART CACHE ({assets.length} ASSETS)</span>
      <span class="pool-drop-hint mono">Drag & Drop files here from Explorer</span>
    </div>
    <div class="media-pool-actions">
      <button class="btn-param-action" onclick={loadMediaPool} type="button">🔄 REFRESH</button>
      <button class="btn-param-action" onclick={handleManualImport} type="button" disabled={isImporting}>
        {isImporting ? '⏳ IMPORTING...' : '➕ IMPORT ASSET'}
      </button>
    </div>
  </div>

  {#if assets.length === 0}
    <div class="media-pool-empty mono">
      <span>No assets in Media Pool yet. Drag & drop files or click "IMPORT ASSET".</span>
    </div>
  {:else}
    <div class="media-pool-grid">
      {#each assets as asset}
        <div class="media-asset-card">
          <div class="asset-top-row">
            <span class="asset-filename mono" title={asset.absolutePath}>{asset.metadata.fileName}</span>
            <button class="btn-remove-asset" onclick={() => handleRemove(asset.quickHash)} title="Remove from pool" type="button">×</button>
          </div>

          <div class="asset-meta-row mono">
            <span class="asset-pill">{formatTime(asset.metadata.duration)}</span>
            <span class="asset-pill">{asset.metadata.width}x{asset.metadata.height}</span>
            <span class="asset-pill">{asset.metadata.fps.toFixed(0)}fps</span>
            {#if asset.analysis}
              <span class="asset-pill bpm-pill">⚡ {asset.analysis.beats?.bpm?.toFixed(0) || '120'} BPM</span>
              <span class="asset-pill style-pill">{asset.analysis.detectedStyle?.styleName?.toUpperCase() || 'JUGG'}</span>
            {:else}
              <span class="asset-pill uncached-pill">NOT ANALYZED</span>
            {/if}
          </div>

          <div class="asset-actions-row">
            {#if onSelectAsScene}
              <button class="btn-asset-action" onclick={() => onSelectAsScene(asset.absolutePath)} type="button">🎬 SCENE</button>
            {/if}
            {#if onSelectAsDrums}
              <button class="btn-asset-action" onclick={() => onSelectAsDrums(asset.absolutePath)} type="button">🥁 DRUMS</button>
            {/if}
            {#if onSelectAsAudio}
              <button class="btn-asset-action" onclick={() => onSelectAsAudio(asset.absolutePath)} type="button">🎵 AUDIO</button>
            {/if}
            {#if onDirectOneClick}
              <button class="btn-asset-action one-click-action" onclick={() => onDirectOneClick(asset.absolutePath)} type="button">⚡ ONE-CLICK</button>
            {/if}
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .media-pool-container {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 14px;
    background: #0b0b12;
    border: 1px solid #1e1e2d;
    border-radius: 8px;
    width: 100%;
    box-sizing: border-box;
    transition: all 150ms ease;
  }
  .media-pool-container.dragging-over {
    border-color: #3b82f6;
    background: #0f172a;
  }

  .media-pool-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .media-pool-title-row {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .pool-drop-hint {
    font-size: 10px;
    color: #71717a;
  }

  .media-pool-actions {
    display: flex;
    gap: 8px;
  }

  .media-pool-empty {
    padding: 24px;
    text-align: center;
    color: #52525b;
    font-size: 11px;
    background: #08080d;
    border: 1px dashed #232336;
    border-radius: 6px;
  }

  .media-pool-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: 10px;
  }

  .media-asset-card {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px 12px;
    background: #11111c;
    border: 1px solid #1c1c2e;
    border-radius: 6px;
    transition: border-color 120ms ease;
  }
  .media-asset-card:hover {
    border-color: #2e2e48;
  }

  .asset-top-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .asset-filename {
    font-size: 11px;
    font-weight: 700;
    color: #e4e4e7;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .btn-remove-asset {
    background: transparent;
    border: none;
    color: #71717a;
    font-size: 14px;
    cursor: pointer;
    padding: 0 4px;
  }
  .btn-remove-asset:hover {
    color: #f87171;
  }

  .asset-meta-row {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .asset-pill {
    font-size: 9.5px;
    font-weight: 700;
    padding: 2px 6px;
    background: #181826;
    border: 1px solid #272738;
    border-radius: 4px;
    color: #a1a1aa;
  }
  .asset-pill.bpm-pill {
    color: #38bdf8;
    border-color: #0369a1;
  }
  .asset-pill.style-pill {
    color: #34d399;
    border-color: #065f46;
  }
  .asset-pill.uncached-pill {
    color: #fbbf24;
    border-color: #78350f;
  }

  .asset-actions-row {
    display: flex;
    gap: 6px;
    margin-top: 2px;
  }
  .btn-asset-action {
    flex: 1;
    background: #181826;
    border: 1px solid #2e2e42;
    color: #cbd5e1;
    padding: 4px 6px;
    border-radius: 4px;
    font-size: 9.5px;
    font-weight: 700;
    cursor: pointer;
    transition: all 120ms ease;
  }
  .btn-asset-action:hover {
    background: #2563eb;
    border-color: #3b82f6;
    color: #ffffff;
  }
  .btn-asset-action.one-click-action {
    background: #1e1b4b;
    border-color: #6366f1;
    color: #a5b4fc;
  }
  .btn-asset-action.one-click-action:hover {
    background: #4f46e5;
    color: #ffffff;
  }
</style>
