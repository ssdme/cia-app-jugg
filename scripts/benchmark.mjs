import { execFileSync } from 'node:child_process';
import { mkdirSync } from 'node:fs';
import { resolve } from 'node:path';
import './generate-test-media.mjs';
const directory = resolve('tests/generated/benchmark'); mkdirSync(directory,{recursive:true});
for (const [name,size,duration] of [['1080p','1920x1080',2],['long','1920x1080',12],['4k','3840x2160',2]]) {
  execFileSync(resolve('src-tauri/binaries/ffmpeg.exe'),['-nostdin','-hide_banner','-loglevel','error','-y','-f','lavfi','-i',`testsrc2=s=${size}:r=30:d=${duration}`,'-c:v','libx264','-preset','ultrafast','-pix_fmt','yuv420p',resolve(directory,`${name}.mp4`)],{stdio:'inherit',windowsHide:true});
}
execFileSync('cargo',['run','--release','--locked','--manifest-path','src-tauri/Cargo.toml','--example','benchmark_render'],{stdio:'inherit',windowsHide:true});
