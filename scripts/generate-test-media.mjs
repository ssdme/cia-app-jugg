import { execFileSync } from 'node:child_process';
import { mkdirSync } from 'node:fs';
import { resolve } from 'node:path';
import { generateSceneFixtures } from './generate-scene-fixtures.mjs';
export const fixtureDirectory = resolve('tests/generated');
export const ffmpeg = resolve('src-tauri/binaries/ffmpeg.exe');
mkdirSync(fixtureDirectory, { recursive: true });
const run = (args) => execFileSync(ffmpeg, ['-nostdin', '-hide_banner', '-loglevel', 'error', '-y', ...args], { stdio: 'inherit', windowsHide: true });
for (const [index, color, fps, duration] of [[0, 'red', 24, 3], [1, 'lime', 30, 4], [2, 'blue', 60, 5]]) {
  run(['-f', 'lavfi', '-i', `color=c=${color}:s=160x90:r=${fps}:d=${duration}`, '-c:v', 'libx264', '-pix_fmt', 'yuv420p', resolve(fixtureDirectory, `source ${index} été.mp4`)]);
}
run(['-f', 'lavfi', '-i', 'testsrc2=s=160x90:r=30:d=2', '-c:v', 'libx264', '-pix_fmt', 'yuv420p', resolve(fixtureDirectory, 'pattern.mp4')]);
run(['-f', 'lavfi', '-i', 'sine=frequency=440:sample_rate=44100:duration=2', '-c:a', 'pcm_s16le', resolve(fixtureDirectory, 'audio.wav')]);
generateSceneFixtures();
console.log(`Generated deterministic media in ${fixtureDirectory}`);
