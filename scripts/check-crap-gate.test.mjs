import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { evaluateReport, selfTest } from './check-crap-gate.mjs';

test('CRAP policy inline acceptance fixtures', selfTest);

// Real cargo-crap 0.6.1 shapes from the root-aligned workspace comparison.
const entry = {
  file: 'crates/mini-diarium-core/src/backup/store.rs',
  function: 'dir_state',
  line: 351,
  cyclomatic: 8,
  coverage: 84.61538461538461,
  crap: 8.233045061447429,
  crate: 'mini-diarium-core',
  uncovered: [
    { start: 366, end: 366 },
    { start: 373, end: 373 },
  ],
  baseline_crap: 8.029130632680928,
  delta: 0.20391442876650068,
  status: 'regressed',
};
const report = (entries = [entry]) => ({
  $schema: 'https://raw.githubusercontent.com/minikin/cargo-crap/main/schemas/delta-v2.json',
  version: '0.6.1',
  entries,
  removed: [],
  diagnostics: { matched_files: 115 },
});

function fixture(t) {
  const base = process.platform === 'win32' ? join(tmpdir(), 'opencode') : tmpdir();
  mkdirSync(base, { recursive: true });
  const dir = mkdtempSync(join(base, 'crap-policy-'));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const input = join(dir, 'report.json');
  return (value, args = []) => {
    if (value !== undefined)
      writeFileSync(input, typeof value === 'string' ? value : JSON.stringify(value));
    return spawnSync(
      process.execPath,
      [fileURLToPath(new URL('check-crap-gate.mjs', import.meta.url)), '--report', input, ...args],
      { encoding: 'utf8', env: { ...process.env, GITHUB_ACTIONS: 'true' } },
    );
  };
}

test('policy CLI exits 1 and emits the same regression in stdout and annotations', (t) => {
  const result = fixture(t)(report());
  assert.equal(result.status, 1, result.stderr);
  assert.match(result.stdout, /store.rs:351 dir_state CRAP 8.029130632680928 -> 8.233045061447429/);
  assert.match(
    result.stdout,
    /::error file=crates\/mini-diarium-core\/src\/backup\/store.rs,line=351/,
  );
  assert.match(result.stdout, /whole-function, repository-wide/);
});

test('policy CLI passes unchanged legacy debt and fails new debt', (t) => {
  const run = fixture(t);
  assert.equal(
    run(report([{ ...entry, status: 'unchanged', baseline_crap: 600, delta: 0, crap: 600 }]))
      .status,
    0,
  );
  assert.equal(
    run(report([{ ...entry, status: 'new', baseline_crap: null, delta: null, crap: 31 }])).status,
    1,
  );
});

test('policy CLI fails closed on missing, malformed, incompatible, and invalid input', (t) => {
  const run = fixture(t);
  const missing = run(undefined);
  assert.equal(missing.status, 2);
  for (const invalid of [
    '{bad',
    { ...report(), version: '0.7.0' },
    { ...report(), removed: undefined },
    { ...report(), $schema: 'report-v1' },
    report([{ ...entry, crap: null }]),
    JSON.stringify(report()).replace('8.233045061447429', '1e400'),
  ]) {
    const result = run(invalid);
    assert.equal(result.status, 2, result.stdout);
    assert.match(result.stderr, /input\/tool failure/);
  }
  assert.equal(run(report(), ['--epsilon', 'NaN']).status, 2);
  assert.equal(run(report(), ['--threshold']).status, 2);
});

test('ambiguous new locations retain the new-function ceiling without identity guessing', () => {
  const entries = [351, 400].map((line) => ({
    ...entry,
    line,
    status: 'new',
    baseline_crap: null,
    delta: null,
    crap: 31,
  }));
  const result = evaluateReport(report(entries));
  assert.equal(result.code, 1);
  assert.equal(result.offenders.length, 2);
});
