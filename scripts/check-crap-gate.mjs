#!/usr/bin/env node
// Policy only: cargo-crap computes metrics, identities, and deltas.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { pathToFileURL } from 'node:url';

const SCHEMA = 'https://raw.githubusercontent.com/minikin/cargo-crap/main/schemas/delta-v2.json';
const finite = (value) => typeof value === 'number' && Number.isFinite(value);
const text = (value) => typeof value === 'string' && value.length > 0 && !/[\x00-\x1f]/.test(value);
const score = (value) => finite(value) && value >= 0;

export function evaluateReport(report, { threshold = 30, epsilon = 0.01 } = {}) {
  if (!score(threshold) || !score(epsilon))
    throw new Error('Threshold and epsilon must be finite non-negative numbers');
  if (
    !report ||
    report.version !== '0.6.1' ||
    report.$schema !== SCHEMA ||
    !Array.isArray(report.entries) ||
    !report.entries.length ||
    !Array.isArray(report.removed)
  ) {
    throw new Error('Expected a non-empty cargo-crap 0.6.1 delta-v2 report');
  }
  if (report.try_weight != null && report.try_weight !== 1)
    throw new Error('Incompatible try_weight; the baseline uses 1');
  const diagnostics = report.diagnostics;
  if (
    !diagnostics ||
    !Number.isSafeInteger(diagnostics.matched_files) ||
    diagnostics.matched_files <= 0
  ) {
    throw new Error('Missing matched LCOV diagnostics; verify coverage and producer paths');
  }
  const identities = new Set();
  const offenders = [];
  for (const entry of report.entries) {
    if (
      !entry ||
      !text(entry.file) ||
      !text(entry.function) ||
      !Number.isSafeInteger(entry.line) ||
      entry.line < 1 ||
      !score(entry.crap) ||
      !score(entry.cyclomatic) ||
      !(
        entry.coverage === null ||
        (finite(entry.coverage) && entry.coverage >= 0 && entry.coverage <= 100)
      )
    ) {
      throw new Error('Malformed delta entry location or metric');
    }
    const identity = JSON.stringify([entry.file.replaceAll('\\', '/'), entry.function, entry.line]);
    if (identities.has(identity))
      throw new Error(`Duplicate source identity: ${entry.file}:${entry.line} ${entry.function}`);
    identities.add(identity);
    if (entry.status === 'new') {
      if (entry.baseline_crap !== null || entry.delta !== null || entry.previous_file != null) {
        throw new Error('New entry must have null baseline_crap/delta and no previous_file');
      }
      if (entry.crap > threshold) offenders.push(entry);
    } else {
      if (
        !score(entry.baseline_crap) ||
        !finite(entry.delta) ||
        !['unchanged', 'improved', 'regressed', 'moved'].includes(entry.status) ||
        (entry.previous_file != null && !text(entry.previous_file))
      ) {
        throw new Error('Malformed existing-function delta');
      }
      const expected =
        entry.delta > epsilon ? 'regressed' : entry.delta < -epsilon ? 'improved' : 'unchanged';
      if (!(
        entry.status === expected ||
        (entry.status === 'moved' && expected === 'unchanged' && text(entry.previous_file))
      )) {
        throw new Error('Delta status disagrees with the requested epsilon');
      }
      if (entry.delta > epsilon) offenders.push(entry);
    }
  }
  for (const entry of report.removed) {
    if (!entry || !text(entry.file) || !text(entry.function) || !score(entry.baseline_crap)) {
      throw new Error('Malformed removed-function entry');
    }
  }
  return { code: offenders.length ? 1 : 0, offenders, count: report.entries.length };
}

const escapeData = (value) =>
  String(value).replaceAll('%', '%25').replaceAll('\r', '%0D').replaceAll('\n', '%0A');
const escapeProperty = (value) => escapeData(value).replaceAll(':', '%3A').replaceAll(',', '%2C');

export function entryMessage(entry) {
  return `${entry.function} CRAP ${entry.baseline_crap ?? 'new'} -> ${entry.crap} (${entry.status})`;
}

export function annotation(entry, root = process.env.GITHUB_WORKSPACE || process.cwd()) {
  let file = entry.file.replaceAll('\\', '/');
  const prefix = root.replaceAll('\\', '/').replace(/\/+$/, '') + '/';
  if (file.startsWith(prefix)) file = file.slice(prefix.length);
  return `::error file=${escapeProperty(file)},line=${entry.line},title=CRAP policy::${escapeData(entryMessage(entry))}`;
}

export function selfTest() {
  // Shape captured from the real 0.6.1 workspace delta on 2026-10-11.
  // Boundary values are policy inputs, not a second metrics implementation.
  const entry = {
    file: 'crates/mini-diarium-core/src/auth/mod.rs',
    function: 'add_keypair_slot',
    line: 54,
    cyclomatic: 2,
    coverage: 100,
    crap: 2,
    crate: 'mini-diarium-core',
    baseline_crap: 2,
    delta: 0,
    status: 'unchanged',
  };
  const report = (overrides = {}, removed = []) => ({
    $schema: SCHEMA,
    version: '0.6.1',
    entries: [{ ...entry, ...overrides }],
    removed,
    diagnostics: { matched_files: 115 },
  });
  const check = (overrides, code) => assert.equal(evaluateReport(report(overrides)).code, code);
  check({}, 0);
  check({ status: 'new', baseline_crap: null, delta: null, crap: 30 }, 0);
  check({ status: 'new', baseline_crap: null, delta: null, crap: 30.001 }, 1);
  check({ status: 'unchanged', baseline_crap: 0, delta: 0.01, crap: 0.01 }, 0);
  check({ status: 'regressed', baseline_crap: 0, delta: 0.010001, crap: 0.010001 }, 1);
  check({ status: 'improved', baseline_crap: 72, delta: -70, crap: 2 }, 0);
  check({ status: 'unchanged', baseline_crap: 600, delta: 0, crap: 600 }, 0);
  check({ status: 'moved', previous_file: 'src/old.rs' }, 0);
  check({ status: 'regressed', previous_file: 'src/old.rs', delta: 1, crap: 3 }, 1);
  // Renamed and ambiguous identities remain New; never pair them ourselves.
  for (const name of ['renamed', 'ambiguous'])
    check({ function: name, status: 'new', baseline_crap: null, delta: null, crap: 31 }, 1);
  assert.equal(
    evaluateReport(report({}, [{ file: 'src/old.rs', function: 'deleted', baseline_crap: 600 }]))
      .code,
    0,
  );
  const repeated = report();
  repeated.entries.push({ ...entry, line: 55 });
  assert.equal(evaluateReport(repeated).count, 2, 'same names at different locations survive');
  repeated.entries.push({ ...entry });
  assert.throws(() => evaluateReport(repeated), /Duplicate/);
  for (const invalid of [
    null,
    {},
    { ...report(), version: '0.7.0' },
    { ...report(), $schema: 'wrong' },
    { ...report(), entries: [] },
    { ...report(), removed: undefined },
    { ...report(), try_weight: 0.5 },
    { ...report(), diagnostics: undefined },
    { ...report(), diagnostics: { matched_files: 0 } },
  ]) {
    assert.throws(() => evaluateReport(invalid));
  }
  for (const invalid of [
    { crap: Infinity },
    { coverage: NaN },
    { line: 0 },
    { status: 'other' },
    { delta: null },
    { delta: 0.1 },
    { status: 'new' },
    { baseline_crap: undefined },
    { status: 'moved' },
    { file: 'x\n::error::bad' },
  ]) {
    assert.throws(() => evaluateReport(report(invalid)));
  }
  assert.throws(() => evaluateReport(report(), { epsilon: -1 }));
  assert.throws(() => evaluateReport(report(), { threshold: NaN }));
  assert.match(
    annotation({ ...entry, file: '/repo/src/a,b%file.rs', function: 'x%y' }, '/repo'),
    /file=src\/a%2Cb%25file.rs,line=54/,
  );
  console.log(
    'CRAP gate self-test passed (threshold, epsilon, legacy debt, moves, identities, invalid inputs, annotations)',
  );
}

export function main(args) {
  if (args.length === 1 && args[0] === '--self-test') {
    selfTest();
    return 0;
  }
  if (args.length === 1 && args[0] === '--help') {
    console.log(
      'Usage: node scripts/check-crap-gate.mjs --report <delta.json> [--threshold 30] [--epsilon 0.01]',
    );
    return 0;
  }
  const options = {};
  for (let i = 0; i < args.length; i++) {
    const key = args[i].slice(2);
    if (
      !['--report', '--threshold', '--epsilon'].includes(args[i]) ||
      Object.hasOwn(options, key) ||
      !args[i + 1] ||
      args[i + 1].startsWith('--')
    ) {
      throw new Error(`Unknown, repeated, or incomplete argument: ${args[i]}`);
    }
    options[key] = key === 'report' ? args[++i] : Number(args[++i]);
  }
  if (!options.report) throw new Error('--report is required');
  const result = evaluateReport(JSON.parse(readFileSync(options.report, 'utf8')), options);
  console.log(
    `CRAP policy: whole-function, repository-wide; new <= ${options.threshold ?? 30}, existing increase <= ${options.epsilon ?? 0.01}`,
  );
  for (const entry of result.offenders) {
    console.log(`${entry.file}:${entry.line} ${entryMessage(entry)}`);
    if (process.env.GITHUB_ACTIONS === 'true') console.log(annotation(entry));
  }
  console.log(
    `CRAP gate ${result.code ? 'FAIL' : 'PASS'}: ${result.offenders.length} violation(s), ${result.count} functions`,
  );
  return result.code;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    process.exitCode = main(process.argv.slice(2));
  } catch (error) {
    console.error(`CRAP gate input/tool failure: ${error.message}`);
    if (process.env.GITHUB_ACTIONS === 'true')
      console.log(`::error title=CRAP input/tool failure::${escapeData(error.message)}`);
    process.exitCode = 2;
  }
}
