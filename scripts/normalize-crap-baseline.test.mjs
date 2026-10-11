import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { selfTest } from './normalize-crap-baseline.mjs';

test('CRAP normalizer inline acceptance fixtures', selfTest);

// Captured 0.6.1 entry shape. Scores are input data, not computed in this suite.
const report = () => ({
  $schema: 'https://raw.githubusercontent.com/minikin/cargo-crap/main/schemas/report-v1.json',
  version: '0.6.1',
  entries: [50, 51].map((line) => ({
    file: '/repo/src/a.rs',
    function: 'SnapshotCredential::fmt',
    line,
    cyclomatic: 4,
    coverage: 85.71428571428571,
    crap: 4.0466472303206995,
    crate: 'mini-diarium-core',
    uncovered: [{ start: 53, end: 53 }],
  })),
  diagnostics: {
    analyzed_files: 1,
    lcov_files: 1,
    matched_files: 1,
    source_only: { count: 0, examples: [] },
    lcov_only: { count: 0, examples: [] },
  },
});

function fixture(t) {
  const base = process.platform === 'win32' ? join(tmpdir(), 'opencode') : tmpdir();
  mkdirSync(base, { recursive: true });
  const dir = mkdtempSync(join(base, 'crap-normalizer-'));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const input = join(dir, 'input.json');
  const output = join(dir, 'output.json');
  return {
    input,
    output,
    run: (text, args = []) => {
      writeFileSync(input, text);
      return spawnSync(
        process.execPath,
        [
          fileURLToPath(new URL('normalize-crap-baseline.mjs', import.meta.url)),
          '--input',
          input,
          '--root',
          '/repo',
          '--output',
          output,
          ...args,
        ],
        { encoding: 'utf8' },
      );
    },
  };
}

test('normalizer CLI preserves scores and duplicate identities in a baseline report', (t) => {
  const files = fixture(t);
  const input = report();
  const result = files.run(JSON.stringify(input), ['--for-baseline']);
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stderr, '');
  const normalized = JSON.parse(readFileSync(files.output, 'utf8'));
  const expected = structuredClone(input);
  for (const entry of expected.entries) entry.file = 'src/a.rs';
  assert.deepEqual(normalized, expected);
});

test('normalizer CLI rejects outside-root and drive-relative paths without overwriting output', (t) => {
  const files = fixture(t);
  for (const path of ['/outside/a.rs', '../outside.rs', 'C:outside.rs']) {
    const input = report();
    input.entries[0].file = path;
    writeFileSync(files.output, 'keep existing output');
    const result = files.run(JSON.stringify(input), ['--for-baseline']);
    assert.equal(result.status, 2, result.stderr);
    assert.ok(result.stderr.includes(path), result.stderr);
    assert.equal(readFileSync(files.output, 'utf8'), 'keep existing output');
  }
});

test('normalizer CLI rejects outside-root diagnostic paths too', (t) => {
  const files = fixture(t);
  const input = report();
  input.diagnostics.lcov_only = { count: 1, examples: ['/outside/lcov.rs'] };
  const result = files.run(JSON.stringify(input), ['--for-baseline']);
  assert.equal(result.status, 2);
  assert.match(result.stderr, /outside\/lcov.rs/);
});

test('normalizer CLI refuses malformed, incompatible, delta, and non-finite reports', (t) => {
  const files = fixture(t);
  for (const text of [
    '{invalid',
    JSON.stringify({ ...report(), version: '0.7.0' }),
    JSON.stringify({ ...report(), removed: [] }),
    JSON.stringify(report()).replace('4.0466472303206995', '1e400'),
  ]) {
    writeFileSync(files.output, 'keep existing output');
    const result = files.run(text, ['--for-baseline']);
    assert.equal(result.status, 2);
    assert.match(result.stderr, /CRAP normalization failed:/);
    assert.equal(readFileSync(files.output, 'utf8'), 'keep existing output');
  }
});

test('normalizer report mode warns and retains an outside-root absolute path', (t) => {
  const files = fixture(t);
  const input = report();
  input.entries[0].file = '/outside/a.rs';
  const result = files.run(JSON.stringify(input));
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stderr, /Warning: Paths outside producer root/);
  assert.equal(JSON.parse(readFileSync(files.output, 'utf8')).entries[0].file, '/outside/a.rs');
});
