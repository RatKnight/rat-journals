// Optional local caller only. Metric and policy logic stay in the pinned analyzer
// and check-crap-gate.mjs. The comprehensive check owns coverage freshness.
import { readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { normalizeReport } from './normalize-crap-baseline.mjs';

export function runCrapAdvisory({ root, freshCoverage, run }) {
  const skipped = (message) => ({ kind: 'skipped', message });
  const problem = (message) => ({ kind: 'tool-error', message });
  if (!freshCoverage) {
    return skipped(
      'CRAP check skipped (no successful same-run backend coverage; existing lcov is stale input)',
    );
  }
  const version = run('cargo crap --version', 'Checking cargo-crap', { cwd: root, silent: true });
  if (!version.success) {
    return skipped('CRAP check skipped (install cargo-crap 0.6.1 for the local advisory check)');
  }
  if (version.output.trim() !== 'cargo-crap 0.6.1') {
    return skipped(
      `CRAP check skipped (version mismatch: expected cargo-crap 0.6.1, found ${version.output.trim()})`,
    );
  }
  try {
    const baseline = JSON.parse(readFileSync(join(root, 'ci/crap-baseline.json'), 'utf8'));
    const input = normalizeReport(baseline, { root, forComparison: true });
    writeFileSync(join(root, 'crap-baseline-input.json'), JSON.stringify(input, null, 2) + '\n');
    // A previous diagnostic must not become this run's policy input.
    rmSync(join(root, 'crap-delta.json'), { force: true });
    const analyzer = run(
      'cargo crap --workspace --lcov src-tauri/lcov.info --baseline crap-baseline-input.json --format json --sort file --missing pessimistic --epsilon 0.01 --exclude "**/tests.rs" --exclude "**/test_support.rs" --output crap-delta.json',
      'CRAP analysis (ADVISORY)',
      { cwd: root },
    );
    if (!analyzer.success) return problem('CRAP advisory input/tool failure: analyzer failed');
    const policy = run(
      'node scripts/check-crap-gate.mjs --report crap-delta.json',
      'CRAP policy (ADVISORY)',
      { cwd: root },
    );
    if (policy.success)
      return {
        kind: 'pass',
        message: 'CRAP advisory verdict: PASS (blocking comparison is Linux CI)',
      };
    if (policy.error?.status === 1) {
      return {
        kind: 'violations',
        message:
          'CRAP advisory verdict: VIOLATIONS (non-blocking locally; blocking comparison is Linux CI)',
      };
    }
    return problem(
      'CRAP advisory input/tool failure: missing or invalid delta report (non-blocking locally)',
    );
  } catch (error) {
    return problem(`CRAP advisory input/tool failure: ${error.message} (non-blocking locally)`);
  }
}
