import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';
import { runCrapAdvisory } from './crap-advisory.mjs';

function fixture(t) {
  const base = process.platform === 'win32' ? join(tmpdir(), 'opencode') : tmpdir();
  mkdirSync(base, { recursive: true });
  const root = mkdtempSync(join(base, 'crap-advisory-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  mkdirSync(join(root, 'ci'));
  writeFileSync(
    join(root, 'ci/crap-baseline.json'),
    JSON.stringify({
      $schema: 'https://raw.githubusercontent.com/minikin/cargo-crap/main/schemas/report-v1.json',
      version: '0.6.1',
      entries: [{ file: 'src/a.rs', function: 'same_name', line: 1, crap: 600 }],
    }),
  );
  const calls = [];
  const run = (command) => {
    calls.push(command);
    return { success: true, output: 'cargo-crap 0.6.1\n' };
  };
  return { root, calls, run, freshCoverage: true };
}

test('advisory refuses an existing lcov without successful same-run coverage', (t) => {
  const options = fixture(t);
  mkdirSync(join(options.root, 'src-tauri'));
  writeFileSync(join(options.root, 'src-tauri/lcov.info'), 'old coverage');
  const result = runCrapAdvisory({ ...options, freshCoverage: false });
  assert.equal(result.kind, 'skipped');
  assert.match(result.message, /stale input/);
  assert.deepEqual(options.calls, []);
});

test('advisory reports missing and wrong-version tools without installing anything', (t) => {
  const options = fixture(t);
  const missing = runCrapAdvisory({ ...options, run: () => ({ success: false }) });
  assert.equal(
    missing.message,
    'CRAP check skipped (install cargo-crap 0.6.1 for the local advisory check)',
  );
  const wrong = runCrapAdvisory({
    ...options,
    run: () => ({ success: true, output: 'cargo-crap 0.7.0\n' }),
  });
  assert.equal(wrong.kind, 'skipped');
  assert.match(wrong.message, /version mismatch/);
  assert.ok(!existsSync(join(options.root, 'crap-baseline-input.json')));
});

test('advisory prepares a temporary path-aligned copy without changing the canonical baseline', (t) => {
  const options = fixture(t);
  const before = readFileSync(join(options.root, 'ci/crap-baseline.json'), 'utf8');
  writeFileSync(join(options.root, 'crap-delta.json'), 'stale delta');
  const result = runCrapAdvisory({
    ...options,
    run: (command) => {
      if (command.startsWith('cargo crap --workspace')) {
        assert.ok(!existsSync(join(options.root, 'crap-delta.json')));
        assert.ok(
          command.includes('--exclude "**/tests.rs" --exclude "**/test_support.rs"'),
          'portable shell quotes',
        );
      }
      return options.run(command);
    },
  });
  assert.equal(result.kind, 'pass');
  assert.equal(options.calls.length, 3);
  assert.equal(readFileSync(join(options.root, 'ci/crap-baseline.json'), 'utf8'), before);
  const input = JSON.parse(readFileSync(join(options.root, 'crap-baseline-input.json'), 'utf8'));
  assert.equal(input.entries[0].file, options.root.replaceAll('\\', '/') + '/src/a.rs');
  assert.equal(input.entries[0].crap, 600);
});

test('advisory labels policy exits 1 and 2, and analyzer failure, without a blocking result', (t) => {
  const options = fixture(t);
  for (const [status, kind] of [
    [1, 'violations'],
    [2, 'tool-error'],
  ]) {
    const result = runCrapAdvisory({
      ...options,
      run: (command) =>
        command.startsWith('node ') ? { success: false, error: { status } } : options.run(command),
    });
    assert.equal(result.kind, kind);
    assert.match(result.message, /non-blocking locally/);
  }
  const failed = runCrapAdvisory({
    ...options,
    run: (command) =>
      command.startsWith('cargo crap --workspace')
        ? { success: false, error: { status: 101 } }
        : options.run(command),
  });
  assert.equal(failed.kind, 'tool-error');
  assert.match(failed.message, /analyzer failed/);
});

test('advisory exposes input failure and refuses to overwrite an invalid baseline', (t) => {
  const options = fixture(t);
  writeFileSync(join(options.root, 'ci/crap-baseline.json'), '{bad');
  const result = runCrapAdvisory(options);
  assert.equal(result.kind, 'tool-error');
  assert.equal(readFileSync(join(options.root, 'ci/crap-baseline.json'), 'utf8'), '{bad');
});

test('real optional-tool caller continues with PATH empty', () => {
  const result = spawnSync(
    process.execPath,
    [
      '--input-type=module',
      '-e',
      `
    import {execSync} from 'node:child_process';
    import {runCrapAdvisory} from './scripts/crap-advisory.mjs';
    const result=runCrapAdvisory({root:process.cwd(),freshCoverage:true,run:command=>{
      try {return {success:true,output:execSync(command,{encoding:'utf8',stdio:'pipe'})};}
      catch(error) {return {success:false,error};}
    }});
    console.log(result.message);
    if(result.kind!=='skipped') process.exitCode=1;
  `,
    ],
    { encoding: 'utf8', env: { ...process.env, PATH: '' } },
  );
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /CRAP check skipped \(install cargo-crap 0\.6\.1/);
});

test('pre-commit records fresh coverage before calling its advisory-only helper', () => {
  const source = readFileSync(new URL('pre-commit.js', import.meta.url), 'utf8');
  assert.match(
    source,
    /freshBackendCoverage = lcov\.size > 0 && lcov\.mtimeMs > coverageStartedAt/,
  );
  const caller = source.slice(source.indexOf('// 7b.'), source.indexOf('// 8.'));
  assert.match(caller, /freshCoverage: freshBackendCoverage/);
  assert.match(caller, /results\.warnings\.push/);
  assert.doesNotMatch(caller, /results\.failed\.push/);
});
