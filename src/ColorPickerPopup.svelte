<script>
  let {
    color = $bindable('#FFFFFF'),
    label = 'FONT COLOR'
  } = $props();

  let isOpen = $state(false);

  // Preset palette
  const presetColors = [
    '#FFFFFF', '#000000',
    '#4A72FF', '#002699',
    '#34D399', '#059669',
    '#38BDF8', '#0284C7',
    '#F87171', '#DC2626',
    '#F472B6', '#C026D3',
    '#FDE047', '#EA580C',
    '#E4E4E7', '#71717A'
  ];

  // HSV State for 2D picker
  let hue = $state(0);
  let sat = $state(0);
  let val = $state(1);
  let alpha = $state(1);
  let isDragging = false;
  let svBoxEl = $state(null);

  function hsvToHex(h, s, v, a = 1) {
    let f = (n, k = (n + h / 60) % 6) => v - v * s * Math.max(Math.min(k, 4 - k, 1), 0);
    let r = Math.round(f(5) * 255);
    let g = Math.round(f(3) * 255);
    let b = Math.round(f(1) * 255);
    let rHex = r.toString(16).padStart(2, '0').toUpperCase();
    let gHex = g.toString(16).padStart(2, '0').toUpperCase();
    let bHex = b.toString(16).padStart(2, '0').toUpperCase();
    if (a < 0.999) {
      let aHex = Math.round(a * 255).toString(16).padStart(2, '0').toUpperCase();
      return `#${aHex}${rHex}${gHex}${bHex}`;
    }
    return `#${rHex}${gHex}${bHex}`;
  }

  function hexToHsv(hexStr) {
    let hex = (hexStr || '').replace('#', '');
    let a = 1;
    if (hex.length === 8) {
      a = parseInt(hex.substring(0, 2), 16) / 255;
      hex = hex.substring(2);
    }
    if (hex.length === 3) {
      hex = hex.split('').map(c => c + c).join('');
    }
    if (hex.length !== 6) return;
    let r = parseInt(hex.substring(0, 2), 16) / 255;
    let g = parseInt(hex.substring(2, 4), 16) / 255;
    let b = parseInt(hex.substring(4, 6), 16) / 255;

    let max = Math.max(r, g, b), min = Math.min(r, g, b);
    let d = max - min;
    let h = 0;
    let s = max === 0 ? 0 : d / max;
    let v = max;

    if (max !== min) {
      switch (max) {
        case r: h = (g - b) / d + (g < b ? 6 : 0); break;
        case g: h = (b - r) / d + 2; break;
        case b: h = (r - g) / d + 4; break;
      }
      h /= 6;
    }
    hue = Math.round(h * 360);
    sat = s;
    val = v;
    alpha = a;
  }

  $effect(() => {
    if (color && !isDragging) {
      hexToHsv(color);
    }
  });

  function handleSvMove(e) {
    if (!svBoxEl) return;
    const rect = svBoxEl.getBoundingClientRect();
    const x = Math.max(0, Math.min(rect.width, e.clientX - rect.left));
    const y = Math.max(0, Math.min(rect.height, e.clientY - rect.top));
    sat = x / rect.width;
    val = 1 - (y / rect.height);
    color = hsvToHex(hue, sat, val, alpha);
  }

  function handleHueMove(e, el) {
    const rect = el.getBoundingClientRect();
    const y = Math.max(0, Math.min(rect.height, e.clientY - rect.top));
    hue = Math.round((y / rect.height) * 360);
    color = hsvToHex(hue, sat, val, alpha);
  }

  function handleAlphaMove(e, el) {
    const rect = el.getBoundingClientRect();
    const y = Math.max(0, Math.min(rect.height, e.clientY - rect.top));
    alpha = Math.max(0, Math.min(1, 1 - (y / rect.height)));
    color = hsvToHex(hue, sat, val, alpha);
  }

  function selectPreset(presetHex) {
    color = presetHex;
    hexToHsv(presetHex);
  }
</script>

<div class="color-picker-root">
  <!-- Clean Color Input Bar -->
  <div class="color-input-bar">
    <span class="color-label">{label}</span>
    <button
      class="color-swatch-trigger"
      style="background: {color};"
      onclick={() => isOpen = !isOpen}
      aria-label="Toggle color palette"
      type="button"
    ></button>
    <input
      type="text"
      class="color-hex-input mono"
      bind:value={color}
      onchange={(e) => hexToHsv(e.target.value)}
      maxlength="9"
    />
  </div>

  <!-- Flyout Palette -->
  {#if isOpen}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div class="color-picker-backdrop" onclick={() => isOpen = false} role="presentation"></div>
    <div class="color-popup-panel" role="dialog" aria-modal="true">
      <div class="picker-body">
        <!-- 2D Saturation / Value Gradient Box -->
        <div
          class="sv-box"
          bind:this={svBoxEl}
          style="background-color: hsl({hue}, 100%, 50%);"
          onmousedown={(e) => {
            isDragging = true;
            handleSvMove(e);
            const onMouseMove = (me) => handleSvMove(me);
            const onMouseUp = () => {
              isDragging = false;
              window.removeEventListener('mousemove', onMouseMove);
              window.removeEventListener('mouseup', onMouseUp);
            };
            window.addEventListener('mousemove', onMouseMove);
            window.addEventListener('mouseup', onMouseUp);
          }}
          role="presentation"
        >
          <div class="sv-white-overlay"></div>
          <div class="sv-black-overlay"></div>
          <div
            class="sv-cursor"
            style="left: {sat * 100}%; top: {(1 - val) * 100}%;"
          ></div>
        </div>

        <!-- Vertical Hue Spectrum Slider -->
        <div
          class="hue-slider-bar"
          onmousedown={(e) => {
            handleHueMove(e, e.currentTarget);
            const el = e.currentTarget;
            const onMouseMove = (me) => handleHueMove(me, el);
            const onMouseUp = () => {
              window.removeEventListener('mousemove', onMouseMove);
              window.removeEventListener('mouseup', onMouseUp);
            };
            window.addEventListener('mousemove', onMouseMove);
            window.addEventListener('mouseup', onMouseUp);
          }}
          role="presentation"
        >
          <div class="hue-thumb" style="top: {(hue / 360) * 100}%;"></div>
        </div>

        <!-- Vertical Alpha Slider (Checkerboard + Gradient) -->
        <div
          class="alpha-slider-bar"
          onmousedown={(e) => {
            handleAlphaMove(e, e.currentTarget);
            const el = e.currentTarget;
            const onMouseMove = (me) => handleAlphaMove(me, el);
            const onMouseUp = () => {
              window.removeEventListener('mousemove', onMouseMove);
              window.removeEventListener('mouseup', onMouseUp);
            };
            window.addEventListener('mousemove', onMouseMove);
            window.addEventListener('mouseup', onMouseUp);
          }}
          role="presentation"
        >
          <div class="alpha-track-fill" style="background: linear-gradient(to bottom, {hsvToHex(hue, sat, val, 1)}, transparent);"></div>
          <div class="alpha-thumb" style="top: {(1 - alpha) * 100}%;"></div>
        </div>

        <!-- 2x8 Preset Swatches Grid -->
        <div class="preset-grid">
          {#each presetColors as pHex}
            <button
              class="preset-swatch"
              class:selected={color.toUpperCase() === pHex.toUpperCase()}
              style="background: {pHex};"
              onclick={() => selectPreset(pHex)}
              aria-label={pHex}
              type="button"
            ></button>
          {/each}
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .color-picker-root {
    position: relative;
    width: 100%;
    user-select: none;
  }

  .color-input-bar {
    display: flex;
    align-items: center;
    gap: 12px;
    background: #08080a;
    padding: 8px 12px;
    border: 1px solid #1c1c20;
    border-radius: 4px;
  }

  .color-label {
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.05em;
    color: #a1a1aa;
    text-transform: uppercase;
    flex-grow: 1;
  }

  .color-swatch-trigger {
    width: 20px;
    height: 20px;
    border: 1px solid #3f3f46;
    border-radius: 3px;
    cursor: pointer;
    box-shadow: 0 0 8px rgba(0, 0, 0, 0.5);
    transition: transform 0.15s ease, border-color 0.15s ease;
  }

  .color-swatch-trigger:hover {
    transform: scale(1.1);
    border-color: #ffffff;
  }

  .color-hex-input {
    width: 90px;
    background: transparent;
    border: none;
    border-bottom: 1px solid #3f3f46;
    color: #ffffff;
    font-size: 12px;
    font-weight: 600;
    padding: 2px 4px;
    outline: none;
    text-align: right;
  }

  .color-hex-input:focus {
    border-bottom-color: #ffffff;
  }

  .color-picker-backdrop {
    position: fixed;
    inset: 0;
    z-index: 90;
  }

  .color-popup-panel {
    position: relative;
    z-index: 91;
    margin-top: 6px;
    background: #09090c;
    border: 1px solid #27272a;
    border-radius: 6px;
    padding: 12px;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.85);
  }

  .picker-body {
    display: flex;
    gap: 10px;
    height: 140px;
  }

  /* 2D SV Box */
  .sv-box {
    position: relative;
    flex-grow: 1;
    height: 100%;
    border-radius: 4px;
    cursor: crosshair;
    overflow: hidden;
    border: 1px solid #1c1c20;
  }

  .sv-white-overlay {
    position: absolute;
    inset: 0;
    background: linear-gradient(to right, #ffffff, transparent);
  }

  .sv-black-overlay {
    position: absolute;
    inset: 0;
    background: linear-gradient(to top, #000000, transparent);
  }

  .sv-cursor {
    position: absolute;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    border: 2px solid #ffffff;
    box-shadow: 0 0 2px #000000;
    transform: translate(-50%, -50%);
    pointer-events: none;
  }

  /* Vertical Hue Bar */
  .hue-slider-bar {
    position: relative;
    width: 14px;
    height: 100%;
    border-radius: 3px;
    background: linear-gradient(to bottom, 
      #ff0000 0%, #ffff00 17%, #00ff00 33%, 
      #00ffff 50%, #0000ff 67%, #ff00ff 83%, #ff0000 100%);
    cursor: pointer;
    border: 1px solid #1c1c20;
  }

  .hue-thumb {
    position: absolute;
    left: -2px;
    right: -2px;
    height: 4px;
    background: #ffffff;
    border: 1px solid #000000;
    border-radius: 1px;
    transform: translateY(-50%);
    pointer-events: none;
  }

  /* Vertical Alpha Bar */
  .alpha-slider-bar {
    position: relative;
    width: 14px;
    height: 100%;
    border-radius: 3px;
    background-image: linear-gradient(45deg, #1c1c20 25%, transparent 25%),
      linear-gradient(-45deg, #1c1c20 25%, transparent 25%),
      linear-gradient(45deg, transparent 75%, #1c1c20 75%),
      linear-gradient(-45deg, transparent 75%, #1c1c20 75%);
    background-size: 8px 8px;
    background-position: 0 0, 0 4px, 4px -4px, -4px 0px;
    cursor: pointer;
    border: 1px solid #1c1c20;
    overflow: hidden;
  }

  .alpha-track-fill {
    position: absolute;
    inset: 0;
  }

  .alpha-thumb {
    position: absolute;
    left: -2px;
    right: -2px;
    height: 4px;
    background: #ffffff;
    border: 1px solid #000000;
    border-radius: 1px;
    transform: translateY(-50%);
    pointer-events: none;
  }

  /* Preset Matrix */
  .preset-grid {
    display: grid;
    grid-template-columns: repeat(2, 16px);
    grid-template-rows: repeat(8, 14px);
    gap: 3px;
  }

  .preset-swatch {
    width: 16px;
    height: 14px;
    border: 1px solid #27272a;
    border-radius: 2px;
    cursor: pointer;
    padding: 0;
    transition: transform 0.1s ease, border-color 0.1s ease;
  }

  .preset-swatch:hover {
    transform: scale(1.15);
    border-color: #ffffff;
    z-index: 2;
  }

  .preset-swatch.selected {
    border: 1px solid #ffffff;
    box-shadow: 0 0 4px rgba(255, 255, 255, 0.6);
  }
</style>
