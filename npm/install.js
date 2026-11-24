import { createHash, randomUUID } from 'node:crypto';
import { createWriteStream } from 'node:fs';
import { chmod, mkdir, readFile, rename, rm } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { Readable, Transform } from 'node:stream';
import { pipeline } from 'node:stream/promises';
import { fileURLToPath } from 'node:url';

const assets = {
  'win32/x64': 'keel-windows-x86_64.exe',
  'darwin/arm64': 'keel-macos-arm64',
  'darwin/x64': 'keel-macos-x86_64',
  'linux/x64': 'keel-linux-x86_64',
  'linux/arm64': 'keel-linux-aarch64',
};

async function download(base, name) {
  const url = new URL(name, base);
  const response = await fetch(url, { signal: AbortSignal.timeout(120_000) });
  if (!response.ok || !response.body) {
    await response.body?.cancel();
    throw new Error(`Download failed: ${url} (HTTP ${response.status})`);
  }
  return response;
}

async function install() {
  if (process.env.KEEL_SKIP_DOWNLOAD === '1') return;
  const platform = `${process.platform}/${process.arch}`;
  const asset = assets[platform];
  if (!asset) throw new Error(`No prebuilt binary for ${platform}; build Keel using cargo install --git https://github.com/pdxxxx/keel`);
  const { version } = JSON.parse(await readFile(new URL('../package.json', import.meta.url), 'utf8'));
  const base = (process.env.KEEL_DOWNLOAD_BASE_URL || `https://github.com/pdxxxx/keel/releases/download/v${version}`).replace(/\/*$/, '/');
  const manifest = await (await download(base, 'SHA256SUMS.txt')).text();
  const hashes = manifest.split(/\r?\n/).map(line => /^([\da-f]{64})[ \t]+\*?(\S+)$/i.exec(line)).filter(match => match?.[2] === asset);
  if (hashes.length !== 1) throw new Error(`Expected exactly one SHA-256 checksum for ${asset}`);
  const native = join(dirname(fileURLToPath(import.meta.url)), 'native');
  const dest = join(native, process.platform === 'win32' ? 'keel.exe' : 'keel');
  await mkdir(native, { recursive: true });
  const tmp = join(native, `.${randomUUID()}.tmp`);
  try {
    const binary = await download(base, asset);
    const hash = createHash('sha256');
    const verify = new Transform({ transform(chunk, encoding, callback) { hash.update(chunk); callback(null, chunk); } });
    await pipeline(Readable.fromWeb(binary.body), verify, createWriteStream(tmp, { flags: 'wx', mode: 0o600 }));
    if (hash.digest('hex') !== hashes[0][1].toLowerCase()) throw new Error(`SHA-256 mismatch for ${asset}; refusing to install`);
    if (process.platform !== 'win32') await chmod(tmp, 0o755);
    await rename(tmp, dest);
  } finally {
    await rm(tmp, { force: true });
  }
}

try { await install(); }
catch (error) { console.error(`keel: ${error.message}`); process.exitCode = 1; }
