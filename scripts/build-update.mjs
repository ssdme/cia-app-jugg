import { execFileSync } from 'node:child_process';
import { verifyUpdate } from './verify-update.mjs';
import fs from 'fs';
import path from 'path';
import os from 'os';

const tauriConf = JSON.parse(fs.readFileSync(path.join('src-tauri', 'tauri.conf.json'), 'utf8'));
const version = tauriConf.version;
const keyPath = path.join(os.homedir(), '.tauri', 'cia-app.key');
const bundleDir = path.join('src-tauri', 'target', 'release', 'bundle', 'nsis');
const candidates = [
  path.join(bundleDir, `${tauriConf.productName}_${version}_x64-setup.exe`),
  path.join(bundleDir, `cia jugg_${version}_x64-setup.exe`),
  path.join(bundleDir, `cia app_${version}_x64-setup.exe`),
  path.join(bundleDir, `cia.jugg_${version}_x64-setup.exe`),
  path.join(bundleDir, `cia.app_${version}_x64-setup.exe`)
];

const installerPath = candidates.find(p => fs.existsSync(p));

if (!installerPath) {
  console.error(`Installer not found in ${bundleDir}. Checked:`, candidates);
  process.exit(1);
}

console.log(`Using installer at ${installerPath}`);
const sigPath = `${installerPath}.sig`;

console.log(`Signing installer for v${version}...`);
execFileSync(process.execPath, [path.resolve('node_modules/@tauri-apps/cli/tauri.js'), 'signer', 'sign', '--private-key-path', keyPath, '--app-version', version, installerPath], {
  encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'], windowsHide: true,
  env: { ...process.env, CI: 'true' }
});

if (!fs.existsSync(sigPath)) {
  console.error(`Signature file not found at ${sigPath}`);
  process.exit(1);
}

const signature = fs.readFileSync(sigPath, 'utf8').trim();
await verifyUpdate(installerPath, signature, tauriConf.plugins.updater.pubkey);
console.log('Verified installer signature against the configured updater public key.');

// Create sanitized alias copy without spaces for direct links
const dotJuggPath = path.join(bundleDir, `cia.jugg_${version}_x64-setup.exe`);
if (installerPath !== dotJuggPath) {
  fs.copyFileSync(installerPath, dotJuggPath);
}
if (sigPath !== `${dotJuggPath}.sig`) {
  fs.copyFileSync(sigPath, `${dotJuggPath}.sig`);
}

const manifest = {
  version: version,
  notes: `cia jugg v${version}\n- Default 3:4 output at 1080 x 1440\n- Smart Scene Flow balances fresh footage with controlled returns\n- Six independent creative effects in Details, enabled by default\n- Time Slice, Ink Pulse, Prism Drift, Focus Pull, Relief Light and Paper Gate`,
  pub_date: new Date().toISOString(),
  platforms: {
    'windows-x86_64': {
      signature: signature,
      url: `https://github.com/ssdme/cia-app-jugg/releases/download/v${version}/cia.jugg_${version}_x64-setup.exe`
    }
  }
};

const manifestPath = path.join('dist', 'latest.json');
fs.mkdirSync('dist', { recursive: true });
fs.writeFileSync(manifestPath, JSON.stringify(manifest, null, 2), 'utf8');
console.log(`\nUpdate manifest successfully written to ${manifestPath}:`);
console.log(JSON.stringify(manifest, null, 2));
