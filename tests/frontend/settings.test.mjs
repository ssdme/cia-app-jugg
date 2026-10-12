import { test } from 'node:test';
import assert from 'node:assert/strict';
import { effectKey, withPersistentOptions, analysisKey } from '../../src/lib/render-settings.ts';

test('every gallery ID addresses exactly one IPC property', () => {
  const ids = ['shakes', 'zoom', 'flicker', 'one_framers', 'transitions', 'tint', 'vignette', 'scanlines',
    'exposure_flash', 'bouncy_shake', 'dissolve_shake', 'skew_shake', 'squish_pop', 'optics_bounce',
    'buildup_chain', 'warp_stretch', 'zoom_beat_offset', 'cc_deep_dark'];
  assert.equal(new Set(ids.map(effectKey)).size, ids.length);
  const dto = { oneFramers: true };
  dto[effectKey('one_framers')] = false;
  assert.deepEqual(dto, { oneFramers: false });
  assert.throws(() => effectKey('unexpected_effect'), /Unknown gallery effect/);
});
test('style changes retain explicit safety and grading options', () => {
  const current = { ccDeepDark: true, antiFlash: true };
  const preset = { ...current, ccDeepDark: false, antiFlash: false, shakes: false };
  assert.deepEqual(withPersistentOptions(preset, current), { ...current, shakes: false });
  assert.equal(preset.antiFlash, false);
});
test('cache keys distinguish replaced files and path list boundaries', () => {
  assert.notEqual(analysisKey(['a:5:10']), analysisKey(['a:5:11']));
  assert.notEqual(analysisKey(['a|b', 'c']), analysisKey(['a', 'b|c']));
});
