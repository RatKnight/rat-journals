---
name: ci-gate-diagnosis
description: Diagnose a failing SonarCloud, Codecov, or Rust CRAP gate on a Mini Diarium PR, and mirror the Codecov patch gate locally before pushing. Use when CI reports coverage, duplication, or CRAP failures, including missing inputs or tool errors. Triggers: SonarCloud, quality gate, new_duplicated_lines_density, new_coverage, Codecov, patch coverage, coverage:diff, lcov, cargo-llvm-cov, CRAP, cargo-crap, crap-delta, crap-baseline.
---

# CI gate diagnosis (SonarCloud + Codecov + CRAP)

Read the failing check's evidence before changing code or thresholds. SonarCloud has a public API; GitHub artifacts and logs require repository access.

## SonarCloud quality gate — read the API, don't guess

When the "SonarCloud Code Analysis" check fails on a PR, the PR comment gives only a summary. To find which files are responsible, use the public API directly — no login required:

```bash
# Which condition failed and by how much
curl -s "https://sonarcloud.io/api/qualitygates/project_status?projectKey=fjrevoredo_mini-diarium&pullRequest=<PR>" | jq .

# Per-file breakdown (replace metric key as needed: new_duplicated_lines_density, new_coverage, etc.)
curl -s "https://sonarcloud.io/api/measures/component_tree?component=fjrevoredo_mini-diarium&pullRequest=<PR>&metricKeys=new_duplicated_lines_density,new_duplicated_lines&strategy=leaves&ps=50" | jq '.components[] | select(.measures[].value != "0.0") | {name: .name, measures: .measures}'
```

Common failures and their usual causes:

- **`new_duplicated_lines_density` > 3%**: copy-pasted test helpers or fixture objects — extract to a shared constant/function in the same file.
- **`new_coverage` < threshold**: new logic in a file that `generatePdfFromElement`-style functions (html2canvas/jsPDF) can't be tested in JSDOM — mock the module boundary instead.

- **`new_security_rating` worse than A**: query the issue API for the actual rule and file; a project-status summary does not identify the finding:

  ```bash
  curl -s "https://sonarcloud.io/api/issues/search?componentKeys=fjrevoredo_mini-diarium&pullRequest=<PR>&types=VULNERABILITY&resolved=false&ps=100"
  ```

  Check `git log origin/master..HEAD` and the affected file's history before changing it. A PR can inherit earlier unmerged work; agree on its scope before editing that work or dismissing a finding. For `javascript:S4036`, do not hide the warning: select a trusted absolute executable instead of inherited PATH, test failure without fallback, and document any explicit nonstandard-installation choice. This protects executable selection, not a compromised host installation.

Note: `website/**/*.html` is excluded from duplication detection — every new blog post would otherwise fail the gate.

## Codecov patch check — mirror it locally before pushing

CI uploads `coverage/lcov.info` (frontend) and `src-tauri/lcov.info` (backend) to Codecov, which enforces `patch ≥ 80%` (new/changed lines) and `project: auto` (no total regression) per `codecov.yml`.

**The Vitest thresholds in `vitest.config.ts` are a coarse frontend-only global floor and do NOT catch patch/project failures — you can pass locally and still fail Codecov.**

Run the local mirror:

```bash
cmd.exe /c bun run coverage:diff       # gate existing lcov files
cmd.exe /c bun run coverage:check      # generate coverage (--generate), then gate
cmd.exe /c bun run coverage:self-test  # parser self-test
```

`scripts/check-diff-coverage.mjs` consumes the same lcov files plus `git diff origin/master`, fails below 80%, and lists every uncovered new line as `file:line`. This mirrors the **patch** check (the most common CI failure); the **project** total-regression check needs a base-branch coverage baseline and is not replicated locally.

The gate also runs as step 9 of `bun run pre-commit` (via `--working-tree`, so it checks not-yet-committed changes against `origin/master`); that run generates both lcov files by running the frontend/backend tests with coverage.

Generating the lcov files by hand:

- **Frontend**: `bun run test:coverage`
- **Backend**: requires `cargo-llvm-cov` + `cargo-nextest` (`cargo install cargo-llvm-cov cargo-nextest --locked`), then from `src-tauri/`:
  `cargo llvm-cov nextest --workspace --lcov --output-path lcov.info`
  (`--workspace` so the `mini-diarium-core` and `mini-diarium-crypto` crates are covered; lcov still lands at `src-tauri/lcov.info`.)

Flags for `coverage:diff`: `--generate` (run both), `--base <ref>`, `--fail-under <pct>`, `--no-fail`, `--frontend`/`--backend`.

See [CI Best Practices → Coverage Gating](../../../docs/best-practices/CI_BEST_PRACTICES.md#coverage-gating).

## Rust CRAP gate — use the delta, not a second analyzer pass

The blocking comparison runs only in Linux CI. It checks **whole functions across the repository**, not only changed lines: New functions must score ≤30; existing functions must not increase by more than epsilon 0.01. Unchanged legacy debt passes. Local comparisons in `bun run pre-commit` are advisory on every OS, and cannot replace a Linux verdict.

1. Read the `CRAP policy gate (whole-function, repository-wide)` step. Exit **1** means policy violations; stdout and annotations identify the same offenders. Download the `crap-delta` artifact, then run `node scripts/check-crap-gate.mjs --report <downloaded-crap-delta.json>` to interpret that exact input. Do not compute alternative metrics or add another `--format github` pass.
2. For exit **2**, diagnose inputs/tools first: successful same-run backend coverage, non-empty `src-tauri/lcov.info`, exact `cargo-crap 0.6.1`, a valid committed baseline, successful `--for-comparison` normalization, and a valid pinned `delta-v2` report. A missing artifact after analyzer failure is expected; an absent report is never a pass.
3. Download `crap-report` for producer provenance and scope diagnostics. Both Codecov upload attempts occur before CRAP; report and delta uploads run even after gate failure. Check their actual step results rather than assuming uploads succeeded.

```powershell
gh run download <run-id> --name crap-delta --dir <evidence-directory>
gh run download <run-id> --name crap-report --dir <evidence-directory>
node scripts/check-crap-gate.mjs --report <evidence-directory>/crap-delta.json
node scripts/check-crap-gate.mjs --self-test
node scripts/normalize-crap-baseline.mjs --self-test
```

An all-0% table can mean **path mismatch**. Compare LCOV `SF:` paths, the recorded producer root, matched-file diagnostics, and analyzer scope. The committed baseline is relative; comparisons need an ignored root-aligned `crap-baseline-input.json` made with `--for-comparison`. Direct relative-versus-absolute comparison can mark repeated names as New. Preserve `--missing pessimistic`, `--exclude "**/tests.rs" --exclude "**/test_support.rs"`, and the baseline's pinned configuration. `skip` is for diagnosis only; do not use it to pass CI.

Do not raise the baseline in a feature change. Baseline updates require a separate reviewed commit from probe-free master Linux CI, with exact production provenance. Reviewed improvements and identity updates are permitted; unexplained increases are not. See [CI Best Practices → Rust CRAP Gate](../../../docs/best-practices/CI_BEST_PRACTICES.md#rust-crap-gate) for the full policy and commands.

Limits apply even when the gate passes: closure branches, including `with_unlocked_db(|db| ...)`, do not increase scored complexity; macros are not expanded; cfg-dead spans in matched files can show 100% coverage. Do not treat a passing score as proof of command-layer test coverage.
