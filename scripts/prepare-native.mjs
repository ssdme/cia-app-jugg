import { createHash } from 'node:crypto';
import { createReadStream, createWriteStream, existsSync, readFileSync, mkdirSync, mkdtempSync, copyFileSync, readdirSync } from 'node:fs';
import { pipeline } from 'node:stream/promises';
import { Readable } from 'node:stream';
import { execFileSync } from 'node:child_process';
import { resolve, join } from 'node:path';
import { tmpdir } from 'node:os';

const manifest = JSON.parse(readFileSync(new URL('./native-payload.json', import.meta.url), 'utf8'));
const binaries = resolve('src-tauri/binaries');
async function hash(path) {
  const digest = createHash('sha256');
  for await (const chunk of createReadStream(path)) digest.update(chunk);
  return digest.digest('hex');
}
const missing = manifest.files.filter(({ name }) => !existsSync(join(binaries, name)));
if (missing.some(({ name }) => !['ffmpeg.exe', 'ffprobe.exe'].includes(name))) {
  throw new Error('The tracked beat_this/model/ONNX Runtime payload is incomplete. Restore it from Git.');
}
if (missing.length) {
  if (process.platform !== 'win32') throw new Error('Preparing the Windows runtime requires Windows.');
  const directory = mkdtempSync(join(tmpdir(), 'cia-native-'));
  const archive = join(directory, 'ffmpeg.zip');
  const response = await fetch(manifest.ffmpeg.url);
  if (!response.ok || !response.body) throw new Error(`FFmpeg download failed: HTTP ${response.status}`);
  await pipeline(Readable.fromWeb(response.body), createWriteStream(archive));
  if (await hash(archive) !== manifest.ffmpeg.archiveSha256) throw new Error('FFmpeg archive checksum mismatch');
  const extracted = join(directory, 'extracted');
  execFileSync('powershell.exe', ['-NoProfile', '-Command', 'Expand-Archive -LiteralPath $env:CIA_NATIVE_ARCHIVE -DestinationPath $env:CIA_NATIVE_EXTRACT'], {
    windowsHide: true, stdio: 'inherit', env: { ...process.env, CIA_NATIVE_ARCHIVE: archive, CIA_NATIVE_EXTRACT: extracted },
  });
  const release = readdirSync(extracted).find(name => name.startsWith('ffmpeg-'));
  if (!release) throw new Error('The verified archive has no FFmpeg release directory');
  mkdirSync(binaries, { recursive: true });
  for (const name of ['ffmpeg.exe', 'ffprobe.exe']) copyFileSync(join(extracted, release, 'bin', name), join(binaries, name));
}
for (const { name, sha256 } of manifest.files) {
  if (await hash(join(binaries, name)) !== sha256) throw new Error(`Native payload checksum mismatch: ${name}`);
}
for (const name of ['ffmpeg.exe', 'ffprobe.exe']) {
  const version = execFileSync(join(binaries, name), ['-version'], { encoding: 'utf8', windowsHide: true }).split('\n')[0];
  if (!version.includes(`version ${manifest.ffmpeg.version}`)) throw new Error(`Unexpected ${name} version`);
}
console.log(`Verified ${manifest.files.length} native payload files (FFmpeg ${manifest.ffmpeg.version}).`);
