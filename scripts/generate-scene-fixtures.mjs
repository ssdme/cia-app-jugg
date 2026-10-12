import { execFileSync } from 'node:child_process';
import { mkdirSync, writeFileSync, unlinkSync } from 'node:fs';
import { resolve } from 'node:path';

export function generateSceneFixtures() {
  const directory = resolve('tests/generated/scenes');
  const ffmpeg = resolve('src-tauri/binaries/ffmpeg.exe');
  mkdirSync(directory, { recursive: true });
  const width = 64, height = 40, frames = 20;
  const pixels = Buffer.alloc(width * height * 6 * frames);
  for (let frame = 0; frame < frames; frame++) {
    for (let y = 0; y < height; y++) for (let x = 0; x < width; x++) {
      let value = ((x + frame * 2) % 8 < 4 ? 30000 : 50000) + y % 3 * 1500;
      if (frame < 2 || frame >= 18) value = 0;
      if (frame === 8 || frame === 9 || frame === 11) value = 65535;
      if (frame === 10) value = 65534;
      if (frame === 12) value = 1;
      if (frame === 11 && x === 63 && y === 39) value = 65534;
      const offset = ((frame * height + y) * width + x) * 6;
      for (let channel = 0; channel < 3; channel++) pixels.writeUInt16LE(value, offset + channel * 2);
    }
  }
  const raw = resolve(directory, 'native.rgb');
  writeFileSync(raw, pixels);
  const run = args => execFileSync(ffmpeg, ['-nostdin', '-hide_banner', '-loglevel', 'error', '-y', ...args], { stdio: 'inherit', windowsHide: true });
  const input = ['-f', 'rawvideo', '-pixel_format', 'rgb48le', '-video_size', `${width}x${height}`, '-framerate', '10', '-i', raw];
  const encode = ['-c:v', 'ffv1', '-pix_fmt', 'gbrp16le', '-fps_mode', 'passthrough'];
  run([...input, ...encode, resolve(directory, 'native gaps.mkv')]);
  run([...input, '-vf', "setpts='if(lt(N,10),N*0.1/TB,(1+(N-10)*0.2)/TB)'", ...encode, resolve(directory, 'variable gaps.mkv')]);
  for (const color of ['black', 'white']) {
    run(['-f', 'lavfi', '-i', `color=${color}:s=64x40:r=10:d=1`, '-c:v', 'ffv1', '-pix_fmt', 'yuv420p', resolve(directory, `${color}.mkv`)]);
  }
  run(['-f', 'lavfi', '-i', 'testsrc2=s=160x90:r=30:d=8', '-c:v', 'libx264', '-pix_fmt', 'yuv420p', resolve(directory, 'motion.mp4')]);
  run(['-f', 'lavfi', '-i', 'sine=frequency=330:sample_rate=44100:duration=8', '-c:a', 'pcm_s16le', resolve(directory, 'audio.wav')]);
  unlinkSync(raw);
}
