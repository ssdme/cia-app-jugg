<script>
  export let kind;
</script>

<svg class="project-mark" viewBox="0 0 48 48" aria-hidden="true" focusable="false">
  {#if kind === 'beat'}
    <path d="M8 24h6l4-12 6 24 6-18 4 12 4-6h6" />
  {:else if kind === 'onnx'}
    <circle cx="14" cy="14" r="4" />
    <circle cx="34" cy="14" r="4" />
    <circle cx="24" cy="34" r="4" />
    <path d="M17 16l4 15M31 16l-4 15M18 14h12" />
  {:else if kind === 'symphonia'}
    <path d="M16 35a4 4 0 1 0 0-8 4 4 0 0 0-4 4v-16l18-4v16a4 4 0 1 0 0-8 4 4 0 0 0-4 4" />
    <path d="M12 15l18-4" />
  {:else if kind === 'mp4'}
    <rect x="8" y="10" width="32" height="28" rx="4" />
    <path d="m20 18 12 6-12 6z" fill="currentColor" stroke="none" />
  {:else if kind === 'ffmpeg'}
    <path d="M8 15h32M8 24h32M8 33h21" />
    <path d="m31 27 9-3-9-3z" fill="currentColor" stroke="none" />
  {:else if kind === 'tauri'}
    <circle cx="24" cy="24" r="8" />
    <path d="M7 24c3-10 11-16 17-16s14 6 17 16c-3 10-11 16-17 16S10 34 7 24Z" />
    <circle cx="9" cy="24" r="2" fill="currentColor" stroke="none" />
    <circle cx="39" cy="24" r="2" fill="currentColor" stroke="none" />
  {:else if kind === 'svelte'}
    <path d="M34 12c-3-3-7-4-12-4-7 0-12 4-12 10 0 12 24 5 24 15 0 4-4 7-10 7-5 0-9-2-12-5" />
  {:else if kind === 'plex'}
    <path d="M12 39V9h13c7 0 11 4 11 10s-4 10-11 10H12" />
    <path d="M25 29 38 39" />
  {/if}
</svg>

<style>
  .project-mark {
    width: 30px;
    height: 30px;
    flex: 0 0 30px;
    color: #e4e4e7;
    fill: none;
    stroke: currentColor;
    stroke-width: 2.1;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
</style>
