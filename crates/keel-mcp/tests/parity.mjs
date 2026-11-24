import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const crate = fileURLToPath(new URL('../', import.meta.url));
const cli = path.resolve(crate, '../../bin/keel.js');
const bin = path.resolve(process.argv[2] ?? path.join(crate, 'target/release', process.platform === 'win32' ? 'keel-mcp.exe' : 'keel-mcp'));
const sandbox = fs.mkdtempSync(path.join(os.tmpdir(), 'keel-native-parity-'));
const root = path.join(sandbox, 'repo');
const home = path.join(sandbox, 'home');
const extra = path.join(sandbox, 'extra');
const env = { ...process.env, HOME: home, USERPROFILE: home, KEEL_MEM_DIRS: [extra, path.join(root, 'custom')].join(path.delimiter) };
const rust = [bin, '--root', root];
const node = [process.execPath, cli, 'mcp'];
let count = 0;

function put(file, contents, time = 1700000000) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, contents);
  fs.utimesSync(file, time, time);
}

function exchange([program, ...args], requests) {
  const input = requests.map((request, index) => typeof request === 'string' ? request : JSON.stringify({ jsonrpc: '2.0', id: index + 1, ...request })).join('\n') + '\n';
  const child = spawnSync(program, args, { cwd: root, env, input, encoding: 'utf8', timeout: 30000, maxBuffer: 16 * 1024 * 1024 });
  assert.ifError(child.error);
  assert.equal(child.status, 0, child.stderr);
  return child.stdout.trimEnd().split('\n').map(line => JSON.parse(line));
}

function call(name, args = {}) { return { method: 'tools/call', params: { name, arguments: args } }; }

function comparable(frame, request) {
  if (request.method === 'initialize') {
    assert.equal(frame.result.serverInfo.version, '1.0.0');
    assert.ok(['keel', 'keel-mcp'].includes(frame.result.serverInfo.name));
    return { ...frame, result: { ...frame.result, serverInfo: { version: '1.0.0' } } };
  }
  if (request.params?.name === 'keel_verify') {
    const text = frame.result.content[0].text;
    if (text.startsWith('{')) {
      const report = JSON.parse(text);
      for (const gate of report.gates) { assert.ok(gate.durationMs >= 0); gate.durationMs = 0; }
      return { ...frame, result: { ...frame.result, content: [{ type: 'text', text: JSON.stringify(report, null, 2) }] } };
    }
  }
  return frame;
}

function compare(requests) {
  const expected = exchange(node, requests);
  const actual = exchange(rust, requests);
  const answered = requests.filter(request => !(typeof request === 'string' && (request === '' || request.includes('notifications/'))));
  assert.equal(expected.length, answered.length);
  assert.equal(actual.length, answered.length);
  for (const [index, request] of answered.entries()) {
    assert.deepEqual(comparable(actual[index], request), comparable(expected[index], request), JSON.stringify(request));
    count++;
  }
  return actual;
}

function create(program) {
  const [frame] = exchange(program, [call('keel_task_create', { title: ' Verify interchange ', slug: 'Task files' })]);
  assert.equal(frame.result.isError, undefined);
  const record = JSON.parse(frame.result.content[0].text);
  const spec = fs.readFileSync(path.join(root, record.specFile), 'utf8');
  const meta = fs.readFileSync(path.join(root, record.dir, 'meta.json'), 'utf8');
  assert.equal(JSON.parse(meta).createdAt, record.createdAt);
  assert.equal(record.createdAt, record.updatedAt);
  assert.match(record.createdAt, /^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d\.\d{3}Z$/);
  return { record, spec, meta };
}

try {
  put(path.join(root, '.keel/rules/index.md'), '# Rule directory');
  put(path.join(root, '.keel/rules/abc.md'), '---\nid: scoped\nsummary: Same shell-independent patterns\npaths: ["lib/**/?.rs", "lib/{a,b}[x]"]\nseverity: invariant\n---\n# Rule\n');
  put(path.join(root, '.keel/rules/nested/global.md'), '---\r\nid: global\r\nsummary: Global\r\nseverity: adr\r\n---\r\n');
  put(path.join(root, '.keel/rules/dash-key.md'), '---\r\nid: dash-key\r\n---foo: harmless\r\nsummary: after key\r\n---\r\n# Body');
  put(path.join(root, '.keel/rules/dash-block.md'), '---\nid: dash-block\nsummary: |-\n  before\n  ---foo\n  after\n---');
  put(path.join(root, '.keel/tasks/archive/old/meta.json'), JSON.stringify({ id: 'old', title: 'Existing', status: 'done', createdAt: 2 }));
  put(path.join(root, '.keel/tasks/archive/old/spec.md'), '# Prior decision\nsignal signal');
  put(path.join(root, 'custom/note.md'), '# Notes\nsignal other SIGNAL');
  put(path.join(root, '.keel/config.yaml'), 'verify:\n  gates:\n    - name: out\n      command: echo ok\n    - name: broken\n      command: ' + JSON.stringify(process.platform === 'win32' ? 'echo bad 1>&2&exit /b 3' : 'echo bad >&2; exit 3') + '\nmemory: {sources: [custom, ./custom, not-found]}\n');
  put(path.join(home, '.claude/projects/project/文档.jsonl'), [
    JSON.stringify({ type: 'assistant', message: { content: [{ text: 'α🚀 SIGNAL' }, { content: 'other' }] }, timestamp: 'time' }),
    'malformed JSON', JSON.stringify({ type: 'user', message: { content: '  Ask SIGNAL\ncontinued' } }),
    JSON.stringify({ message: { role: 'tool', content: 'ignored' } }),
  ].join('\n'), 1700000001.7508);
  put(path.join(home, '.codex/sessions/2026/11/session.jsonl'), JSON.stringify({ payload: { role: 'human', content: ['Signal signal', 'other'] }, created_at: 'earlier' }), 1700000002);
  put(path.join(home, '.local/share/opencode/x.jsonl'), JSON.stringify({ role: 'model', text: 'Signal other' }), 1700000002);
  put(path.join(extra, 'backup.jsonl'), JSON.stringify({ role: 'user', text: 'signal' }), 1700000003);
  put(path.join(extra, 'nested/窗口.jsonl'), [
    JSON.stringify({ role: 'assistant', text: 'x'.repeat(79) + '🚀 title boundary' }),
    JSON.stringify({ role: 'assistant', text: 'marker ' + 'x'.repeat(112) + '🌟 clipped' }),
    JSON.stringify({ role: 'assistant', text: 'lead 🌟' + 'x'.repeat(119) + 'marker' }),
  ].join('\n'), 1700000003);

  const [ruleList, ruleCheck] = compare([call('keel_rules_list'), call('keel_rules_check')]);
  const summaries = new Map(JSON.parse(ruleList.result.content[0].text).map(rule => [rule.id, rule.summary]));
  assert.equal(summaries.get('dash-key'), 'after key');
  assert.equal(summaries.get('dash-block'), 'before\n---foo\nafter');
  assert.deepEqual(JSON.parse(ruleCheck.result.content[0].text), { valid: true, checked: 4, issues: [] });

  const archiveIds = ['archive/old', './archive//old/', 'archive\\old'];
  const archived = compare(archiveIds.map(id => call('keel_task_show', { id })));
  for (const frame of archived) {
    assert.equal(frame.result.isError, undefined);
    const task = JSON.parse(frame.result.content[0].text);
    assert.equal(task.dir, '.keel/tasks/archive/old');
    assert.equal(task.spec, '# Prior decision\nsignal signal');
  }
  const invalidIds = ['archive/../archive/old', 'archive\\..\\archive\\old', '/archive/old', '\\archive\\old',
    path.join(root, '.keel/tasks/archive/old'), 'C:/archive/old', 'C:old', '\\\\host\\share\\old'];
  const rejected = compare(invalidIds.map(id => call('keel_task_show', { id })));
  for (const [index, frame] of rejected.entries()) {
    assert.deepEqual(frame.result, { isError: true, content: [{ type: 'text', text: `Task not found: ${invalidIds[index]}` }] });
  }

  compare([{ method: 'initialize' }, '', '{"jsonrpc":"2.0","method":"notifications/initialized"}', { method: 'ping' }, { method: 'tools/list' }, { method: 'unknown' }, { method: 'tools/call' }, 'not-json',
    call('keel_rules_list'), call('keel_rules_check'), call('keel_rules_match', { paths: ['.\\lib\\文.rs', 1], git: true }),
    call('keel_rules_match', { paths: ['lib/🚀.rs'], includeGlobal: false }), call('keel_rules_match', { paths: ['lib/{a,b}[x]'] }),
    call('keel_task_list'), call('keel_task_list', { includeArchived: true, status: 'done' }), call('keel_task_show', { id: 'archive/old' }),
    call('keel_task_create', { title: '   ' }), call('keel_task_show', { id: 'unknown-id' }), call('keel_task_show'),
    call('keel_verify', { gates: ['out'] }), call('keel_verify', { failFast: true }), call('keel_verify', { gates: ['missing'] }),
    call('keel_mem_sessions'), call('keel_mem_sessions', { source: 'custom', limit: 2 }), call('keel_mem_sessions', { source: 'keel', limit: -1 }),
    call('keel_mem_search', { query: ' SIGNAL other ' }), call('keel_mem_search', { query: 'SIGNAL other unused', limit: 2.9 }),
    call('keel_mem_search', { query: 'marker', source: 'custom' }), call('keel_mem_search', { query: 'Ask', source: 'claude' }),
    call('keel_mem_search', { query: 'nonsense' }), call('keel_mem_search', { query: ' ' }), call('unknown-tool'),
  ]);

  put(path.join(root, '.keel/rules/nested/fallback.md'), '\n## Fallback summary');
  put(path.join(root, '.keel/rules/z.md'), '---\nid: global\nsummary: false\nseverity: null\npaths: 72\n---\n# Loose read');
  compare([call('keel_rules_list'), call('keel_rules_check')]);

  const nativeTask = create(rust);
  compare([call('keel_task_show', { id: nativeTask.record.id }), call('keel_task_list', { includeArchived: true })]);
  const taskDir = path.resolve(root, nativeTask.record.dir);
  assert.equal(path.dirname(taskDir), path.join(root, '.keel/tasks'));
  fs.renameSync(taskDir, path.join(sandbox, 'saved-native-task'));
  const nodeTask = create(node);
  compare([call('keel_task_show', { id: nodeTask.record.id }), call('keel_task_list')]);
  assert.deepEqual({ ...nativeTask.record, createdAt: nodeTask.record.createdAt, updatedAt: nodeTask.record.updatedAt }, nodeTask.record);
  assert.equal(nativeTask.spec, nodeTask.spec);
  assert.equal(nativeTask.meta.replaceAll(nativeTask.record.createdAt, nodeTask.record.createdAt), nodeTask.meta);
  console.log(`Passed ${count} stdio response comparisons and bidirectional task-file interchange (Keel CLI 1.0.0).`);
} finally {
  assert.equal(path.dirname(sandbox), path.resolve(os.tmpdir()));
  assert.ok(path.basename(sandbox).startsWith('keel-native-parity-'));
  fs.rmSync(sandbox, { recursive: true, force: true });
}
