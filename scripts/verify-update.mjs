import { createHash, createPublicKey, verify } from 'node:crypto';
import { createReadStream } from 'node:fs';

/** Verify the prehashed Minisign format used by the Tauri updater, including
 * its signed trusted comment. Never accept a signature from another key ID. */
export async function verifyUpdate(file, encodedSignature, encodedPublicKey) {
  const decode = (value) => value.startsWith('untrusted comment:') ? value : Buffer.from(value, 'base64').toString('utf8');
  const publicLines = decode(encodedPublicKey.trim()).trim().split(/\r?\n/);
  const signatureLines = decode(encodedSignature.trim()).trim().split(/\r?\n/);
  const publicBytes = Buffer.from(publicLines[1] || '', 'base64');
  const signatureBytes = Buffer.from(signatureLines[1] || '', 'base64');
  const global = Buffer.from(signatureLines[3] || '', 'base64');
  if (publicBytes.length !== 42 || signatureBytes.length !== 74 || global.length !== 64
      || signatureBytes.subarray(0, 2).toString() !== 'ED'
      || !signatureBytes.subarray(2, 10).equals(publicBytes.subarray(2, 10))
      || !signatureLines[2]?.startsWith('trusted comment: ')) {
    throw new Error('Invalid updater signature or signing key mismatch');
  }
  const key = createPublicKey({ key: Buffer.concat([Buffer.from('302a300506032b6570032100', 'hex'), publicBytes.subarray(10)]), format: 'der', type: 'spki' });
  const hash = createHash('blake2b512');
  for await (const chunk of createReadStream(file)) hash.update(chunk);
  const signature = signatureBytes.subarray(10);
  const comment = signatureLines[2].slice('trusted comment: '.length);
  if (!verify(null, hash.digest(), key, signature)
      || !verify(null, Buffer.concat([signature, Buffer.from(comment)]), key, global)) {
    throw new Error('Updater artifact signature verification failed');
  }
  return comment;
}
