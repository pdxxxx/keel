import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { once } from 'node:events';
import { copyFile, mkdtemp, mkdir, readFile, readdir, rm, stat, writeFile } from 'node:fs/promises';
import http from 'node:http';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

const installer = new URL('./install.js', import.meta.url);
const wrapper = fileURLToPath(new URL('./bin/keel.js', import.meta.url));
const payload = Buffer.from([0, 1, 255, 9, 0]);
const digest = createHash('sha256').update(payload).digest('hex');
const file = ({'win32/x64':'keel-windows-x86_64.exe','darwin/arm64':'keel-macos-arm64','darwin/x64':'keel-macos-x86_64','linux/x64':'keel-linux-x86_64','linux/arm64':'keel-linux-aarch64'})[`${process.platform}/${process.arch}`];

async function run(args, env, input = '') {
  const child = spawn(process.execPath, args, { env: { ...process.env, ...env }, stdio: ['pipe','pipe','pipe'] });
  const stdout = [], stderr = [];
  child.stdout.on('data', chunk => stdout.push(chunk)); child.stderr.on('data', chunk => stderr.push(chunk));
  const closed = once(child, 'close');
  child.stdin.end(input);
  const [code] = await closed;
  return { code, stdout: Buffer.concat(stdout).toString(), stderr: Buffer.concat(stderr).toString() };
}

async function fixture(context, options = {}) {
  const root = await mkdtemp(path.join(tmpdir(), 'keel-installer-'));
  context.after(() => rm(root, { recursive: true, force: true }));
  const npm = path.join(root,'npm');
  await mkdir(npm); await copyFile(installer, path.join(npm,'install.js'));
  await copyFile(new URL('../package.json', import.meta.url), path.join(root,'package.json'));
  const server = http.createServer((req, res) => {
    if (req.url === '/SHA256SUMS.txt') res.end(options.manifest ?? `${digest}  ${file}\n`);
    else if (req.url === `/${file}`) { res.writeHead(302, {Location:'/blob'}); res.end(); }
    else if (req.url === '/blob') {res.statusCode = options.status ?? 200; res.end(payload);}
    else {res.statusCode = 404; res.end();}
  });
  server.listen(0,'127.0.0.1'); await once(server,'listening');
  context.after(() => new Promise(resolve => server.close(resolve)));
  const dest = path.join(npm, 'native', process.platform === 'win32' ? 'keel.exe' : 'keel');
  return {dest, install: (env = {}) => run([path.join(npm,'install.js')], { KEEL_SKIP_DOWNLOAD:'0', KEEL_DOWNLOAD_BASE_URL:`http://127.0.0.1:${server.address().port}`, ...env })};
}

test('installs the redirected release asset only after checking its checksum', async context => {
  const {dest, install} = await fixture(context);
  assert.deepEqual(await install(), {code:0,stdout:'',stderr:''});
  assert.deepEqual(await readFile(dest), payload);
  if (process.platform !== 'win32') assert.equal((await stat(dest)).mode & 0o777, 0o755);
});

test('bad SHA-256 leaves an existing binary intact with no temp files', async context => {
  const {dest, install} = await fixture(context, {manifest:`${'f'.repeat(64)} *${file}\n`});
  await mkdir(path.dirname(dest)); await writeFile(dest,'existing');
  const result = await install();
  assert.equal(result.code,1); assert.match(result.stderr,/SHA-256 mismatch/);
  assert.equal(await readFile(dest,'utf8'),'existing'); assert.deepEqual(await readdir(path.dirname(dest)), [path.basename(dest)]);
});

test('rejects duplicate checksums, and handles HTTP errors', async context => {
  const duplicate = await fixture(context, {manifest:`${digest}  ${file}\n`.repeat(2)});
  assert.match((await duplicate.install()).stderr,/exactly one/);
  const missing = await fixture(context, {status:404});
  const result = await missing.install(); assert.equal(result.code,1); assert.match(result.stderr,/HTTP 404/);
});

test('offline skip does not contact a mirror or write a binary', async context => {
  const {dest, install} = await fixture(context);
  assert.equal((await install({KEEL_SKIP_DOWNLOAD:'1', KEEL_DOWNLOAD_BASE_URL:'invalid'})).code,0);
  await assert.rejects(stat(dest),{code:'ENOENT'});
});

test('wrapper passes input, output, errors, argv and the child exit status verbatim', async () => {
  const result = await run([wrapper,'-e',"process.stdin.on('data', x=>process.stdout.write(x)); process.stderr.write(process.argv[1]); process.exitCode=23",'two words 文'], {KEEL_BINARY:process.execPath}, '{"id":1}\n');
  assert.deepEqual(result, {code:23,stdout:'{"id":1}\n',stderr:'two words 文'});
});

test('missing binary is actionable and does not contaminate stdout', async context => {
  const {dest} = await fixture(context);
  const result=await run([wrapper,'mcp'],{KEEL_BINARY:dest});
  assert.equal(result.code,1); assert.equal(result.stdout,''); assert.match(result.stderr,/npm rebuild.*cargo install/);
});
