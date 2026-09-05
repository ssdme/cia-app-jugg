import { execSync } from 'child_process';
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
execSync(`npx tauri signer sign --private-key-path "${keyPath}" "${installerPath}"`, {
  encoding: 'utf8',
  input: '\n',
  env: {
    ...process.env,
    TAURI_SIGNING_PRIVATE_KEY_PASSWORD: '',
    CI: 'true'
  }
});

if (!fs.existsSync(sigPath)) {
  console.error(`Signature file not found at ${sigPath}`);
  process.exit(1);
}

const signature = fs.readFileSync(sigPath, 'utf8').trim();

// Create sanitized alias copies without spaces for direct links
const dotJuggPath = path.join(bundleDir, `cia.jugg_${version}_x64-setup.exe`);
const dotAppPath = path.join(bundleDir, `cia.app_${version}_x64-setup.exe`);
if (installerPath !== dotJuggPath) {
  fs.copyFileSync(installerPath, dotJuggPath);
}
if (installerPath !== dotAppPath) {
  fs.copyFileSync(installerPath, dotAppPath);
}

const manifest = {
  version: version,
  notes: `cia jugg v${version}\n- Multi-video sequential decoding and timeline assembly\n- Streamlined dropzones layout with splitter-ai stem separation integration\n- Pure Rust matrix transform engine (Ambiance, CC Deep Dark, Anti-Flash, Shake, Optical Flow)\n- Standalone zero-dependency offline installer for clean Windows installations`,
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
