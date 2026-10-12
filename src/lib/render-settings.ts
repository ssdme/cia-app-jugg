import type { EffectOverrides } from './contracts';

/** Gallery IDs are presentation identifiers; only these DTO keys cross IPC. */
const galleryKeys = {
  shakes: 'shakes', zoom: 'zoom', flicker: 'flicker', one_framers: 'oneFramers',
  transitions: 'transitions', tint: 'tint', vignette: 'vignette', scanlines: 'scanlines',
  echo_trail: 'echoTrail', exposure_flash: 'exposureFlash', bouncy_shake: 'bouncyShake',
  dissolve_shake: 'dissolveShake', skew_shake: 'skewShake', squish_pop: 'squishPop',
  optics_bounce: 'opticsBounce', buildup_chain: 'buildupChain', warp_stretch: 'warpStretch',
  zoom_beat_offset: 'zoomBeatOffset', cc_deep_dark: 'ccDeepDark', anti_flash: 'antiFlash',
} satisfies Record<string, keyof EffectOverrides>;

export function effectKey(id: string): keyof EffectOverrides {
  if (!Object.hasOwn(galleryKeys, id)) throw new Error(`Unknown gallery effect: ${id}`);
  return galleryKeys[id as keyof typeof galleryKeys];
}

export function withPersistentOptions<T extends { ccDeepDark: boolean; antiFlash: boolean }>(
  preset: T, current: Pick<T, 'ccDeepDark' | 'antiFlash'>,
): T {
  return { ...preset, ccDeepDark: current.ccDeepDark, antiFlash: current.antiFlash };
}

/** Native metadata fingerprints also invalidate replacements at the same path. */
export const analysisKey = (fingerprints: string[]) => JSON.stringify(['analysis-v2', ...fingerprints]);
