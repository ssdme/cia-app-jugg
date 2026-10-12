import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createHash, generateKeyPairSync, sign } from 'node:crypto';
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { verifyUpdate } from '../../scripts/verify-update.mjs';

test('updater verifies artifact, key ID and trusted comment before publication', async () => {
  const directory = mkdtempSync(join(tmpdir(), 'cia-signature-test-'));
  try {
    const file = join(directory, 'installer.bin');
    const payload = Buffer.from('deterministic test installer'); writeFileSync(file, payload);
    const { publicKey, privateKey } = generateKeyPairSync('ed25519');
    const publicBytes = publicKey.export({ type: 'spki', format: 'der' }).subarray(-32);
    const id = Buffer.from('12345678');
    const pub = `untrusted comment: test key\n${Buffer.concat([Buffer.from('Ed'), id, publicBytes]).toString('base64')}`;
    const signature = sign(null, createHash('blake2b512').update(payload).digest(), privateKey);
    const comment = 'timestamp:123\tfile:installer.bin\tversion:1.0.4';
    const global = sign(null, Buffer.concat([signature, Buffer.from(comment)]), privateKey);
    const sig = `untrusted comment: test signature\n${Buffer.concat([Buffer.from('ED'), id, signature]).toString('base64')}\ntrusted comment: ${comment}\n${global.toString('base64')}`;
    const encoded = Buffer.from(sig).toString('base64');
    assert.equal(await verifyUpdate(file, encoded, Buffer.from(pub).toString('base64')), comment);
    await assert.rejects(verifyUpdate(file, sig.replace('version:1.0.4', 'version:2.0.0'), pub));
    writeFileSync(file, 'modified installer'); await assert.rejects(verifyUpdate(file, sig, pub));
  } finally { rmSync(directory, { recursive: true, force: true }); }
});
