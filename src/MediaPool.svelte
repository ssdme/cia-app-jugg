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
      await invoke('import_media_to_pool', { paths });
      await loadMediaPool();
    } catch (e) {
      console.error('Import error:', e);
    } finally {
      isImporting = false;
    }
  }

  async function handleManualImport() {
    const p = await invoke('pick_file', { kind: 'media_or_image' });
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
  class="media-pool-card"
  class:dragging-over={isDraggingOver}
  ondrop={handleDrop}
  ondragover={handleDragOver}
  ondragleave={handleDragLeave}
  role="region"
  aria-label="Media Pool Assets"
>
  <div class="media-pool-top">
    <div class="pool-title-group">
      <span class="pool-title">Media Pool & Smart Cache</span>
      <span class="pool-counter-badge">{assets.length} {assets.length === 1 ? 'asset' : 'assets'}</span>
    </div>
    <div class="pool-btn-group">
      <button class="btn-pool-action" onclick={loadMediaPool} type="button">Refresh</button>
      <button class="btn-pool-primary" onclick={handleManualImport} type="button" disabled={isImporting}>
        {isImporting ? 'Importing...' : '+ Import Media'}
      </button>
    </div>
  </div>

  {#if assets.length === 0}
    <div class="media-pool-empty">
      <div class="empty-icon-wrap">
        <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
          <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
          <polyline points="17 8 12 3 7 8"/>
          <line x1="12" y1="3" x2="12" y2="15"/>
        </svg>
      </div>
      <p class="empty-title">Drag & drop media files here from Windows Explorer</p>
      <span class="empty-sub">MP4, MOV, MKV, MP3, WAV — Files are automatically fingerprinted and cached</span>
    </div>
  {:else}
    <div class="media-pool-grid">
      {#each assets as asset}
        <div class="media-item">
          <div class="media-item-header">
            <span class="media-item-name" title={asset.absolutePath}>{asset.metadata.fileName}</span>
            <button class="media-item-del" onclick={() => handleRemove(asset.quickHash)} title="Remove from pool" type="button">×</button>
          </div>

          <div class="media-meta-row">
            <span class="meta-tag">{formatTime(asset.metadata.duration)}</span>
            <span class="meta-tag">{asset.metadata.width}×{asset.metadata.height}</span>
            <span class="meta-tag">{asset.metadata.fps.toFixed(0)} fps</span>
            {#if asset.analysis}
              <span class="meta-tag bpm-tag">{asset.analysis.beats?.bpm?.toFixed(0) || '120'} BPM</span>
              <span class="meta-tag style-tag">{asset.analysis.detectedStyle?.styleName || 'Jugg'}</span>
            {:else}
              <span class="meta-tag uncached-tag">Unanalyzed</span>
            {/if}
          </div>

          <div class="media-actions-row">
            {#if onSelectAsScene}
              <button class="btn-assign" onclick={() => onSelectAsScene(asset.absolutePath)} type="button">Scene</button>
            {/if}
            {#if onSelectAsDrums}
              <button class="btn-assign" onclick={() => onSelectAsDrums(asset.absolutePath)} type="button">Drums</button>
            {/if}
            {#if onSelectAsAudio}
              <button class="btn-assign" onclick={() => onSelectAsAudio(asset.absolutePath)} type="button">Audio</button>
            {/if}
            {#if onDirectOneClick}
              <button class="btn-assign btn-jugg-quick" onclick={() => onDirectOneClick(asset.absolutePath)} type="button">One-Click</button>
            {/if}
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .media-pool-card {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 14px 16px;
    background: #09090c;
    border: 1px solid #1c1c20;
    border-radius: 8px;
    box-sizing: border-box;
    transition: border-color 150ms ease, background-color 150ms ease;
    margin-top: 14px;
  }
  .media-pool-card.dragging-over {
    border-color: #ffffff;
    background: #121215;
  }

  .media-pool-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .pool-title-group {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .pool-title {
    font-size: 12px;
    font-weight: 600;
    color: #ffffff;
    letter-spacing: 0.02em;
  }
  .pool-counter-badge {
    font-family: 'IBM Plex Mono', monospace;
    font-size: 10px;
    font-weight: 500;
    color: #71717a;
    background: #0d0d10;
    padding: 2px 6px;
    border-radius: 4px;
    border: 1px solid #27272a;
  }

  .pool-btn-group {
    display: flex;
    gap: 6px;
  }
  .btn-pool-action {
    background: #0d0d10;
    border: 1px solid #27272a;
    color: #d4d4d8;
    font-size: 11px;
    font-weight: 500;
    padding: 5px 10px;
    border-radius: 5px;
    cursor: pointer;
    transition: all 120ms ease;
  }
  .btn-pool-action:hover {
    background: #16161a;
    border-color: rgba(255, 255, 255, 0.42);
    color: #ffffff;
  }
  .btn-pool-primary {
    background: #ffffff;
    border: 1px solid #ffffff;
    color: #000000;
    font-size: 11px;
    font-weight: 700;
    padding: 5px 12px;
    border-radius: 5px;
    cursor: pointer;
    transition: all 120ms ease;
  }
  .btn-pool-primary:hover:not(:disabled) {
    background: #e4e4e7;
    box-shadow: 0 0 12px rgba(255, 255, 255, 0.2);
  }
  .btn-pool-primary:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .media-pool-empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 24px 16px;
    background: #050507;
    border: 1px dashed #27272a;
    border-radius: 6px;
  }
  .empty-icon-wrap {
    color: #71717a;
    margin-bottom: 2px;
  }
  .empty-title {
    font-size: 12px;
    font-weight: 500;
    color: #e4e4e7;
    margin: 0;
  }
  .empty-sub {
    font-size: 10.5px;
    color: #71717a;
  }

  .media-pool-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
    gap: 8px;
  }

  .media-item {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px 12px;
    background: #0d0d10;
    border: 1px solid #1c1c20;
    border-radius: 6px;
    transition: border-color 120ms ease;
  }
  .media-item:hover {
    border-color: rgba(255, 255, 255, 0.35);
  }

  .media-item-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .media-item-name {
    font-size: 11px;
    font-weight: 600;
    color: #ffffff;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .media-item-del {
    background: transparent;
    border: none;
    color: #71717a;
    font-size: 14px;
    line-height: 1;
    cursor: pointer;
    padding: 0 3px;
  }
  .media-item-del:hover {
    color: #ffffff;
  }

  .media-meta-row {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
  }
  .meta-tag {
    font-family: 'IBM Plex Mono', monospace;
    font-size: 9.5px;
    font-weight: 500;
    padding: 2px 6px;
    background: #050507;
    border: 1px solid #27272a;
    border-radius: 3px;
    color: #a1a1aa;
  }
  .meta-tag.bpm-tag {
    color: #e4e4e7;
    border-color: #3f3f46;
  }
  .meta-tag.style-tag {
    color: #ffffff;
    border-color: rgba(255, 255, 255, 0.25);
    text-transform: uppercase;
  }
  .meta-tag.uncached-tag {
    color: #71717a;
  }

  .media-actions-row {
    display: flex;
    gap: 5px;
    margin-top: 2px;
  }
  .btn-assign {
    flex: 1;
    background: #141418;
    border: 1px solid #27272a;
    color: #d4d4d8;
    padding: 5px 6px;
    border-radius: 4px;
    font-size: 10px;
    font-weight: 600;
    cursor: pointer;
    transition: all 120ms ease;
  }
  .btn-assign:hover {
    background: #1c1c22;
    border-color: rgba(255, 255, 255, 0.42);
    color: #ffffff;
  }
  .btn-assign.btn-jugg-quick {
    background: transparent;
    border-color: rgba(255, 255, 255, 0.6);
    color: #ffffff;
    font-weight: 700;
  }
  .btn-assign.btn-jugg-quick:hover {
    background: #16161a;
    border-color: #ffffff;
  }
</style>
