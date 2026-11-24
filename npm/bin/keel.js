#!/usr/bin/env node
import { spawn } from 'node:child_process';
import { constants } from 'node:os';
import { fileURLToPath } from 'node:url';

const native = process.platform === 'win32' ? 'keel.exe' : 'keel';
const binary = process.env.KEEL_BINARY || fileURLToPath(new URL(`../native/${native}`, import.meta.url));
const child = spawn(binary, process.argv.slice(2), { stdio: 'inherit' });
const handlers = new Map();
for (const signal of ['SIGINT', 'SIGTERM', 'SIGHUP']) {
  const forward = () => child.kill(signal);
  handlers.set(signal, forward);
  process.on(signal, forward);
}
child.once('error', error => {
  console.error(`keel: Cannot start ${binary}: ${error.message}. Run npm rebuild @pdxxxx/keel to download again, or cargo install --git https://github.com/pdxxxx/keel.`);
  process.exitCode = 1;
});
child.once('close', (code, signal) => {
  for (const [name, handler] of handlers) process.off(name, handler);
  if (signal) {
    process.exitCode = 128 + (constants.signals[signal] || 0);
    if (process.platform !== 'win32') process.kill(process.pid, signal);
  } else if (!process.exitCode) process.exitCode = code ?? 1;
});
