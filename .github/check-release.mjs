import { readFileSync } from 'node:fs';

const pkg = JSON.parse(readFileSync('package.json','utf8'));
const cargo = readFileSync('Cargo.toml','utf8').split(/^\[package\]\s*$/m)[1]?.split(/^\[/m)[0];
const rust = /^version\s*=\s*"([^"]+)"\s*$/m.exec(cargo)?.[1];
if (process.env.TAG !== `v${pkg.version}` || pkg.version !== rust || pkg.version !== '2.0.0') {
  throw new Error(`Release version mismatch: tag=${process.env.TAG}, Cargo=${rust}, npm=${pkg.version}, expected 2.0.0`);
}
