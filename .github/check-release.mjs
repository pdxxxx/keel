import { readFileSync } from 'node:fs';

const pkg = JSON.parse(readFileSync('package.json','utf8'));
const cargo = readFileSync('Cargo.toml','utf8').split(/^\[package\]\s*$/m)[1]?.split(/^\[/m)[0];
const rust = /^version\s*=\s*"([^"]+)"\s*$/m.exec(cargo)?.[1];
const semver = /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/;
if (!semver.test(pkg.version) || process.env.TAG !== `v${pkg.version}` || pkg.version !== rust) {
  throw new Error(`Release version mismatch: tag=${process.env.TAG}, Cargo=${rust}, npm=${pkg.version}`);
}
