# crap-complexity-gates

## Metadata

- Plan Status: IN PROGRESS
- Plan Format: manual-planning v2.0.0
- Template: milestoned
- Tracking: tracked (by user request)

## Status Legend

- Plan Status values: DRAFT, QUESTIONS PENDING, READY FOR APPROVAL, APPROVED, IN PROGRESS, COMPLETED, BLOCKED
- Task/Milestone Status values: TO BE DONE, IN PROGRESS, COMPLETED, BLOCKED, SKIPPED

## Context For A Clean Session

- Repository: `C:\Users\Francisco\.herdr\worktrees\mini-diarium\worktree-silver-meadow-f82f` (Mini Diarium worktree), branch `worktree/silver-meadow-f82f`. The census used `da8c19b`; implementation started from the inherited `5a82c70` commit. Current execution state is below; commit IDs in evidence refer to the state at that check.
- Originating design document (read first): `docs/explorations/2026-10-10-crap-cyclomatic-complexity-integration.md` — contains the verified findings, the measured baseline census, the coding agent's agree-with-changes second-opinion review, and the agreed scope revisions this plan implements.
- Stack: Rust 1.95 (`rust-toolchain.toml:2`) Cargo workspace — `src-tauri/` app crate + `crates/mini-diarium-core` + `crates/mini-diarium-crypto`; SolidJS/TypeScript frontend (Node v24.21.0, bun 1.4.2, Vitest 5.0.3 with v8 coverage); CI: GitHub Actions (`.github/workflows/ci.yml`); Codecov + SonarCloud (SonarCloud analyzes TS only).
- Key tools: `cargo-crap` **pinned 0.6.1** (installed and verified locally on Windows: `cargo install cargo-crap --locked`, 2m19s compile); `cargo-llvm-cov` 0.9.0; `cargo-nextest` 0.9.138; Python 3.13.4 (plan scripts only).
- Exact commands (this shell is Windows — route bun/vitest via `cmd.exe /c` per root `CLAUDE.md`; bare `cargo` is fine):
  - Frontend tests+coverage: `cmd.exe /c bun run test:coverage` → writes `coverage/lcov.info`
  - Backend tests+coverage: `cargo llvm-cov nextest --workspace --no-fail-fast --lcov --output-path lcov.info` run from `src-tauri/` → writes `src-tauri/lcov.info` (workspace-wide results land there)
  - CRAP report: `cargo crap --workspace --lcov src-tauri/lcov.info --format json --sort file --missing pessimistic --epsilon 0.01 --output <file>` from repo root
  - Lint: `cmd.exe /c bun run lint`; type-check: `cmd.exe /c bun run type-check`; frontend tests: `cmd.exe /c bun run test:run`
  - Backend: `cargo test --workspace`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo fmt --all --check`
  - Coverage gate: `cmd.exe /c bun run coverage:diff`

### Current execution boundary (2026-10-11)

- The primary branch contains advisory reporting/normalization, workspace Clippy and frontend ESLint settings, Vitest failure reporting, the equivalent auth cleanup helper/tests, and the separately approved inherited Git-selection fix.
- Merged PR #327 contains only the advisory reporting/normalization work and inherited runner infrastructure with its approved Git-selection fix. It deliberately excludes the independent Clippy, ESLint, Vitest, and auth commits. Local full-gate results validate the primary branch; PR CI results validate that narrower PR, not the independent changes.
- Tasks 1.8 and 1.4 are COMPLETED. The normalizer boundary fix is primary `d7b8dfc` and advisory `926d30f`; replacement CI run 38097918038 and every PR check passed on exact head `926d30f9a7ae5234a0e8ebf6bbdf736d3a2eeb99`. PR #327 merged under DEC-017 as `8a345d70abe8fb5c3aba842150f5b0c2af5a66d9`. Master CI run 38099433982 passed all seven jobs, and its probe-free Linux report and production provenance are verified.
- Merge `4a88bd7` includes master `f1bb84f73f44dd40028299682d0b6ef2f2326ca7` (DEC-018), whose master CI run 38103562280 passed all seven jobs. Task 1.5's dedicated commit `189bb55` contains only the canonical 624-entry Linux baseline and producer-record sidecar. DEC-019 authorizes M2. Task 2.1 is complete after full Linux acceptance; Task 2.2 wiring is committed and awaits three live controls; Task 2.3's tested local advisory integration is complete. Current execution evidence and dependencies are below. This primary-worktree plan and companion decisions file remain the current execution ledger; older execution notes are dated evidence, not current status.

### Initial repository facts (before implementation)

| Fact | Value | How it was verified |
| --- | --- | --- |
| Backend lcov already produced in CI | `.github/workflows/ci.yml:199` (`cargo llvm-cov nextest --workspace ... --output-path lcov.info`) | grep of ci.yml anchors |
| Frontend coverage produced in CI | `.github/workflows/ci.yml:187` (`bun run test:coverage`) | same |
| Codecov uploads | `.github/workflows/ci.yml:204–215` (two `codecov-action` steps: `./coverage/lcov.info`, `./src-tauri/lcov.info`) | same |
| Binary-cache pattern to copy | `.github/workflows/ci.yml:175–184` (caches `~/.cargo/bin/cargo-llvm-cov`, key `llvm-cov-Linux-v0.6`) | same |
| Pre-commit backend coverage = step 7 | `scripts/pre-commit.js:156–177` (skip-with-warning pattern at 176–177 when `cargo llvm-cov` absent — model for Task 2.3) | grep |
| Pre-commit diff-coverage gate = step 11 | `scripts/pre-commit.js:238` (`node scripts/check-diff-coverage.mjs --working-tree`) | grep |
| Policy-script style model | `scripts/check-diff-coverage.mjs` — parses only `SF:`/`DA:` lcov records, has `--self-test` wired as `package.json:23` `coverage:self-test` | earlier exploration |
| Vitest coverage config | `vitest.config.ts:17–36` — provider `v8`, reporters `text/json/html/lcov`, thresholds 76/62/80/77; **no `reportOnFailure`** (default false → lcov silently skipped when any test fails) | file read; empirically confirmed (failed run produced no lcov) |
| ESLint config | `eslint.config.js` — flat config; `@eslint/js` + `typescript-eslint` recommended + `eslint-plugin-solid`; **no `complexity` rule** | file read |
| Clippy complexity config dormant | `src-tauri/.clippy.toml:1–2` — `msrv = "1.75"`, `cognitive-complexity-threshold = 30`; not inherited by `crates/*`; `cognitive_complexity` is a nursery lint that nothing enables | file read |
| No workspace lint tables | no `[workspace.lints]` in root `Cargo.toml`; no `[lints]` in any of the three member crates | grep of all four Cargo.toml |
| Clippy blast radius measured | `cargo clippy --workspace --all-targets -- -A clippy::all -W clippy::cognitive_complexity` → exactly 1 warning: `crates/mini-diarium-core/src/import/jrnl.rs:342` (26/25 at default threshold); **0 violations at threshold 30** | local run 2026-10-10 |
| cargo-crap baseline census (Windows) | 630 functions → 18 above CRAP 30 (app 14, core 4, crypto 0); worst `build_menu` 600.0 (CC 24, 0% cov, `src-tauri/src/menu.rs:27`); path matching 118/122 (misses: `config/tests.rs`, `commands/fonts/tests.rs`, `build.rs`, `webview_security/macos.rs` — all benign) | local run 2026-10-10 |
| lcov files carry function records | backend 4,607 `FN`/`FNDA` pairs; frontend 2,581 across 131 files | local runs |
| cargo-crap 0.6.1 CLI contract | `--workspace`, `--lcov`, `--baseline <json>`, `--fail-regression`, `--fail-above`, `--missing pessimistic|optimistic|skip`, `--epsilon` (default 0.01), `--format human|json|github|markdown|pr-comment|sarif|shields`, `--sort crap|file`, `--output`, config `.cargo-crap.toml`; default-excludes `tests/**`, `benches/**`, `examples/**` | local `cargo crap --help` |
| cargo-crap gate semantics | `--fail-regression` does NOT trip on brand-new functions; `--fail-above` checks ALL functions incl. legacy → native flags cannot express "new ≤ 30, existing no-regress" → custom delta-JSON policy script required | coding-agent source reading of 0.6.1, recorded in proposal doc |
| cargo-crap scoring blind spots | closures not scored (matters for the `with_unlocked_db(..., |db| ...)` command pattern), macro bodies not expanded, function spans with no instrumented lines in a matched file show 100% coverage (cfg-dead code looks healthy) | coding-agent source reading of 0.6.1, recorded in proposal doc |
| Baseline JSON path shape | measured Windows output contains absolute `C:\Users\...` paths → normalization to repo-relative forward-slash form is mandatory before committing | local run |
| SonarCloud ruled out for CRAP | API-verified: no CRAP metric among 165; Rust not analyzed (`q=.rs` → total 0); project `coverage` metric absent | live API queries, recorded in proposal doc |
| CHANGELOG exists at root | `CHANGELOG.md` with `## [X.Y.Z] - dd-mm-YYYY/Unreleased` convention | file read |
| Skill mirrors are generated | canonical `.agents/skills/` only; `.claude/skills/` + `.pi/skills/` must never be edited or committed (root `CLAUDE.md` gotcha #3; DRIFT failure in sync-skills) | root `CLAUDE.md` |
| lcov files are gitignored | generated `coverage/lcov.info` and `src-tauri/lcov.info` do not appear in `git status` | `git status --porcelain` after generation |
| `docs/plans/` is tracked | existing committed plan `docs/plans/2026-10-10-core-write-contract-follow-ups-w-03-to-w-06-plan.md` | `git ls-files docs/plans` |

### Hard constraints

1. **Never block Codecov uploads on a CRAP failure** — the CRAP gate step goes after `ci.yml:204–215` and report artifacts upload with `if: always()`; violating this hides the coverage state the existing patch ≥ 80% gate depends on.
2. **Canonical baseline is Linux-CI-produced with normalized repo-relative forward-slash paths** — a Windows-produced baseline bakes in absolute paths and different cfg-compiled code; gating Linux PRs with it produces phantom regressions.
3. **Baseline updates only in dedicated reviewed commits** — a feature PR that raises its own baseline defeats the gate entirely.
4. **Scratch acceptance probes never merge to master and never enter the canonical baseline** — probes contain deliberately pathological functions; merging would pollute both codebase and gate.
5. **Edit only `.agents/skills/ci-gate-diagnosis/`** — `.claude/skills/` and `.pi/skills/` are generated mirrors; direct edits cause `sync-skills` DRIFT failures.
6. **`cargo clippy --workspace --all-targets -- -D warnings` must stay green** — it is the enforcement backbone of Task 3.2; breaking it breaks every PR.
7. **The policy script must not compute metrics** — cargo-crap stays the single metrics source; the script only interprets its delta JSON. A second metrics implementation would drift.
8. **Route `bun`/`vitest`/`vite` through `cmd.exe /c` from this shell** — bare invocations from WSL silently misbehave in this repo (root `CLAUDE.md`, Execution Environment).
9. **The local Windows CRAP check is advisory-only** — it must print as non-blocking and must not be documented as a blocking CI mirror until cross-platform acceptance (Task 1.2) establishes that Windows-vs-Linux-baseline comparisons are reliable.

## Goal

CI computes per-function CRAP scores for the whole Rust workspace from the existing `cargo llvm-cov` lcov and gates them against a reviewed Linux baseline: new functions must score ≤ 30; existing functions must not regress beyond epsilon 0.01 (whole-function, repo-wide — explicitly accepted as stricter than patch-scoped). TypeScript gains a McCabe cyclomatic-complexity lint (warn, max 15, test files exempt from that rule only) and Rust gains clippy `cognitive_complexity` enabled workspace-wide (threshold 30; measured 0 violations today). The gate's blind spots (closures, macros, cfg-dead code) are documented wherever the gate is documented.

## Scope

- **M1 (Phase 0, advisory)**: pinned cargo-crap 0.6.1 in `ci.yml` with binary cache keyed on exact version+OS+arch; non-blocking JSON report step AFTER Codecov uploads with elapsed-time echo, producer provenance, and best-effort artifact upload; acceptance probes on a draft scratch PR (closure scoring, cfg-dead code, Linux path matching, repeatability across two freshly regenerated coverage runs, test-helper exclusion decision); `scripts/normalize-crap-baseline.mjs` (standalone, self-tested: Windows+Linux roots, slash conversion, outside-repo rejection in baseline mode, duplicate function names; preserves parsed non-path values; no rounding); reviewed advisory-only merge checkpoint (Task 1.4), including CI self-test wiring and any required analyzer exclusions; canonical post-merge master Linux baseline committed at `ci/crap-baseline.json` with durable production-time provenance in `ci/crap-baseline.provenance.md` (Task 1.5); explicit user approval recorded before any M2 task starts (Task 1.6).
- **M2 (Phase 1, gating)**: `scripts/check-crap-gate.mjs` (Node .mjs styled after `check-diff-coverage.mjs`; distinct exit codes pass=0 / policy-fail=1 / input-or-tool-fail=2; missing or invalid input FAILS in CI; emits GitHub annotations from the same delta JSON; `--self-test` covering new functions above and exactly at 30, regressions above and exactly at epsilon, improvements, moved/renamed/ambiguous entries, missing/malformed/incompatible reports, built on captured real 0.6.1 delta shapes); gated CI step placed after Codecov uploads; pre-commit step requiring fresh same-run lcov with explicit "CRAP check skipped" notice when cargo-crap is absent (no auto-install; advisory-only on Windows); baseline-maintenance policy + whole-function repo-wide wording + blind-spot disclosure in `docs/best-practices/CI_BEST_PRACTICES.md`, `.agents/skills/ci-gate-diagnosis/`, and short root `CLAUDE.md` pointers.
- **M3 (Phase 2, lints)**: root `clippy.toml` with `cognitive-complexity-threshold = 30` and **no `msrv` key** (documented as removing an obsolete setting, not declaring an MSRV); delete `src-tauri/.clippy.toml`; `[workspace.lints.clippy]` `cognitive_complexity = "warn"` in root `Cargo.toml` (blocking in CI via the existing `-D warnings`) plus `[lints] workspace = true` in all three member crates; `eslint.config.js` `complexity: ["warn", { max: 15, variant: "classic" }]` with a per-file override disabling only that rule for `*.test.ts(x)`/`*.spec.ts(x)` and durable documentation of the temporary non-blocking exception in `docs/best-practices/FRONTEND_BEST_PRACTICES.md`; `vitest.config.ts` `coverage.reportOnFailure: true` in its own scoped commit; CHANGELOG entry under Unreleased / Internal.
- **M4**: cleanup and final verification.

## Non-Goals

- TypeScript CRAP (no verified mature tool; ESLint CC + Codecov patch LINE coverage are independent controls, not an equivalent per-function CRAP check — per agreed scope).
- SonarCloud lcov import (separate work; requires scanner access to lcov files, not just a property).
- Lizard, SARIF upload lane, `--duplicates` analysis.
- Refactoring the 18 legacy offenders (grandfathered via the baseline; metrics identify review candidates, not proven defects — per scope agreement).
- Per-offender TODO entries. Optionally ONE separate follow-up TODO ("command-layer testability and coverage") may be proposed via todo-manager **after user approval** — not a task of this plan.
- The `prosemirror-dedup.test.ts` timeout fix (removed from this plan per scope agreement; handle separately after confirming the cause).
- Any change to Codecov or SonarCloud gate configuration.

## Assumptions

- `cargo install cargo-crap --version 0.6.1 --locked` works on the Ubuntu CI runner (verified on Windows: 2m19s from source; CI binary cache makes it once-per-version).
- cargo-crap 0.6.1's delta-JSON shape is stable while the version is pinned; policy-script self-tests use fixtures captured from real 0.6.1 runs (Task 2.1).
- The Windows-measured 18 legacy offenders carry over to Linux in similar magnitude; cfg-gated platform code will shift some entries — the Task 1.5 Linux baseline is authoritative and the Windows census is planning-grade only.
- ESLint `warn` without `--max-warnings 0` is non-blocking in `bun run lint` — accepted temporary exception until promotion to `error` (documented in Task 3.3).
- Plan and baseline files are committed per repo convention (`docs/plans/` tracked; `ci/` to be created as tracked).

## Open Questions

- None. Tasks 1.7, 1.8, the approved advisory merge, canonical baseline review, and Task 1.6 approval are complete. DEC-019 approves the planned Linux blocking policy, not finding dismissal or changes to other gates.

## Milestones

### Milestone 1: Phase 0 — advisory CRAP report + canonical Linux baseline

- Status: COMPLETED
- Purpose: prove the toolchain end-to-end on Linux CI without gating anything, and produce the reviewed baseline M2 gates against.
- Exit Criteria: the M1 advisory PR (Task 1.4) is merged and its master CI run produces the report artifact; acceptance probes document closure, cfg-dead-code, path-matching, and numeric repeatability behavior; `ci/crap-baseline.json` + `ci/crap-baseline.provenance.md` committed — Linux-produced, normalized (`--for-baseline` rejects outside-repo paths), probe-free, with the test-helper exclusion list applied; **Task 1.6 approval gate passed**: user reviewed the Linux results (unexplained coverage mismatches, runtime cost, repeatability) and the explicit approval is recorded in the Decision Log before any M2 task starts.

#### Task 1.1: Pin cargo-crap + advisory CI step

- Status: COMPLETED
- Depends On: none
- Objective: CI `test` job produces a best-effort cargo-crap JSON report artifact that can never fail the job or interfere with Codecov uploads, with elapsed time and producer-root provenance echoed.
- Steps:
  1. In `.github/workflows/ci.yml` test job, add a binary-cache step modeled on lines 175–184: path `~/.cargo/bin/cargo-crap`, key including exact version + OS + arch (e.g. `cargo-crap-Linux-x86_64-v0.6.1`); install step `cargo install cargo-crap --version 0.6.1 --locked` guarded on cache miss. BOTH steps carry `continue-on-error: true` — advisory means non-blocking for ALL failure modes (install, analysis, metric), not just "no fail flags".
  2. AFTER the Codecov uploads (line 215+), add a step with `if: always()` and `continue-on-error: true` that times (`date +%s` start/end echo) and runs `cargo crap --workspace --lcov src-tauri/lcov.info --format json --sort file --missing pessimistic --epsilon 0.01 --output crap-report.json`; NO `--fail-above` / `--fail-regression` flags. Placement after Codecov means even a hard step crash cannot affect the uploads.
   3. Add `actions/upload-artifact` with `if: always()` AND `if-no-files-found: ignore` for `crap-report.json` (`always()` cannot upload a report that was never generated — e.g. an earlier test failure skipped the llvm-cov step — so the upload must tolerate absence). In the best-effort report step, write `crap-report.provenance.txt` AT PRODUCTION TIME with the producer root (`$GITHUB_WORKSPACE`), analyzed source commit SHA (including a PR merge SHA when applicable), CI run ID/URL, runner OS/architecture, actual `rustc --version`, cargo-crap/cargo-llvm-cov/nextest versions, exact coverage command/configuration and analyzer flags/exclusions, and UTC date/time. Include the provenance file in the artifact; Tasks 1.3 and 1.5 consume it without guessing runner facts later.
  4. Add narrow `.gitignore` entries for `crap-report.json`, `crap-delta.json`, `crap-report.provenance.txt` (NOT currently ignored — local runs would dirty `git status`).
- Validation: a workflow run shows the new steps green with a downloadable artifact containing per-function entries for all three crates, the elapsed-time echo, and the provenance file. On a scratch PR with a deliberately failing test: Codecov uploads still occur; the CRAP step is skipped-or-errored without changing the job conclusion beyond the test failure; the artifact step completes via `if-no-files-found: ignore` (absence of the report is acceptable when coverage never ran — this validation asserts job flow and uploads, NOT artifact presence, which is impossible to guarantee on the failure path).
- Notes: affects `.github/workflows/ci.yml` and `.gitignore`; keep step names greppable (e.g. "CRAP report (advisory)").

#### Task 1.2: Acceptance probes on a scratch branch

- Status: COMPLETED
- Depends On: 1.1
- Objective: empirically document cargo-crap's behavior for the three known blind spots plus score repeatability, on Linux, before any gate exists.
- Steps:
  1. Create a scratch branch and open it as a DRAFT PR targeting master (NEVER merged): `ci.yml` triggers on `push`/`pull_request` for master — ordinary branch pushes to other refs do not run the workflow, so probes need a PR to execute on Linux CI. Add probe functions — (a) a low-complexity function whose real logic lives in a closure (mimicking `with_unlocked_db(|db| ...)`), (b) a function under `#[cfg(target_os = "macos")]` containing branches (dead on Linux), (c) a mixed covered/uncovered function as sanity control.
   2. Let the draft PR's CI run the advisory step; record: does the closure body affect the enclosing function's CC/CRAP? Does the cfg-dead function report 100% coverage or go missing? Do probe files path-match in the Linux lcov? Also record whether confirmed test-helper sources that cargo-crap's default excludes miss (e.g. `config/tests.rs`-style files — the Windows census showed two such files among the 4 unmatched) appear in the Linux report; if so, define the consistent `--exclude`/`--allow` list HERE, before baseline production. Apply it to the analyzer commands in Task 1.4 before merging and verify that scope in Task 1.5; use the same list in the Task 2.2 CI gate and local report commands.
  3. Repeatability with a NUMERIC criterion: regenerate coverage TWICE (two fresh runs — comparing the same lcov twice only proves parsing determinism), run cargo-crap on each, parse both JSON reports and compute max |CRAP_a(f) − CRAP_b(f)| over all shared function identities; requirement: max delta ≤ 0.01 AND identical function sets. A textual diff alone does not establish epsilon compliance.
  4. Record all findings in this plan's `## Execution Notes` and mirror them into the proposal doc's Phase-0 section (findings only — no probe data into the baseline).
- Validation: written findings answer all probe questions including the test-helper exclusion decision; repeatability reports a numeric max-delta ≤ 0.01 with identical function sets (otherwise documented and the plan goes BLOCKED); probe isolation proven via `git merge-base --is-ancestor <each-probe-commit> origin/master` exiting NON-ZERO for every probe commit (exit 1 = not an ancestor = PASS; any other non-zero code is a command error, not a pass — read the specific exit code, never branch on "non-zero").
- Notes: probe code is disposable; the draft PR is closed, never merged. If repeatability fails, M2's gate design is invalid — mark BLOCKED and write a Decision Log entry immediately.

#### Task 1.3: `scripts/normalize-crap-baseline.mjs`

- Status: COMPLETED
- Depends On: 1.1
- Objective: standalone Node script converting cargo-crap JSON report paths to repo-relative forward-slash form, preserving all parsed non-path values exactly.
- Steps:
  1. Write `scripts/normalize-crap-baseline.mjs`: input = cargo-crap `--format json` report + `--root <producer-workspace-root>` (the runner root recorded in Task 1.1's `crap-report.provenance.txt` — when normalizing on Windows, the script cannot guess the Linux producer's root, so it MUST be supplied explicitly); output = same JSON schema/version with all path fields (function locations AND retained diagnostics) repo-relative with forward slashes; scores, thresholds, and function identities untouched; NO rounding of any numeric field (gate inputs must not be lossy).
  2. Handle: absolute paths under the producer root (strip root + slash conversion), already-relative paths (slash-normalize only), repeated function names within one file (preserve order/identity — no dedup), and paths OUTSIDE the producer root: in `--for-baseline` mode REJECT them (non-zero exit listing offenders) — the all-relative baseline is a hard constraint, so outside-repo paths must fail baseline production rather than persist as absolute; in plain report mode keep them absolute with a stderr warning.
  3. Add `--self-test` following the `scripts/check-diff-coverage.mjs` convention (`package.json:23`) with inline fixtures covering every case in step 2 (Windows roots, Linux roots, outside-root rejection, duplicates, already-relative); add a `package.json` script entry following the `coverage:self-test` naming precedent. Scripts-only `package.json` edits add no dependencies: verify `bun.lock` and `package-lock.json` remain byte-identical (no sync-lockfiles run needed) and `nix/package.nix` `npmDepsHash` untouched (it changes only when `package-lock.json` does).
- Validation: `node scripts/normalize-crap-baseline.mjs --self-test` exits 0; round-tripping a real Task 1.1 report (supplying its provenance root) yields identical parsed scores, thresholds, and function counts (parsed-JSON comparison excluding path fields); a seeded outside-root path makes `--for-baseline` exit non-zero with the offender listed.
- Notes: pure path/syntax transformation — must not filter, sort, or re-score entries.

#### Task 1.4: M1 merge checkpoint (advisory-only PR to master)

- Status: COMPLETED
- Depends On: 1.1, 1.2, 1.3, 1.7, 1.8
- Objective: the advisory infrastructure is reviewed and merged to master, because the canonical baseline can only come from a master CI run — master does not have the advisory step until this merge lands.
- Steps:
   1. Open the M1 PR containing: advisory CI step + binary cache and production-time provenance capture (Task 1.1), normalizer script + self-test + `package.json` entry (Task 1.3), `.gitignore` additions, and a lint-job CI step running `node scripts/normalize-crap-baseline.mjs --self-test` (script self-tests are outside ESLint's `src` scope and Vitest excludes `scripts/**` — without explicit wiring the normalizer could rot silently). Apply any test-helper analyzer exclusions decided in Task 1.2 to the report command before merging; record the exact exclusion list with the report provenance so Task 1.5 receives an already correctly scoped report.
  2. User review; merge to master. This merge IS the checkpoint — no canonical master artifact can exist before it.
- Validation: the M1 PR is green (advisory step + normalizer self-test visible in the run) and merged; the subsequent master CI run produces the report artifact.
- Notes: the CRAP changes are advisory-only. The PR inherited the earlier runner-infrastructure commit `5a82c70` and its separately approved Git-selection fix. Task 1.8 and all replacement checks passed at `926d30f`; PR #327 merged as `8a345d7` under DEC-017. Post-merge master CI run 38099433982 passed, and its report/provenance are verified, completing Task 1.4. This is separate from Task 1.6 approval; no finding was dismissed.

#### Task 1.5: Canonical Linux baseline `ci/crap-baseline.json`

- Status: COMPLETED
- Depends On: 1.4
- Objective: reviewed, normalized, Linux-produced baseline committed with durable provenance.
- Steps:
  1. From the post-merge **master** CI run (never the probe branch), download `crap-report.json` and `crap-report.provenance.txt`.
   2. Verify that the downloaded report was generated with the test-helper analyzer exclusion list decided in Task 1.2 and applied in Task 1.4 (if any). Run the normalizer with `--root <producer-root-from-provenance>` and `--for-baseline`. The normalizer must NOT apply analyzer exclusions or filter entries. If provenance is incomplete or the analyzer scope differs, obtain a new correctly scoped master CI report; do not regenerate the canonical report on Windows or infer missing producer facts.
  3. Review: function count (≈630 expected), crappy count, zero probe entries, spot-check worst offenders against the Windows census (platform-code differences acceptable, unexplained mismatches are not).
   4. Commit `ci/crap-baseline.json` in a DEDICATED commit/PR together with the durable provenance sidecar `ci/crap-baseline.provenance.md`. Copy the producer facts captured AT PRODUCTION TIME in Task 1.1: source commit SHA, CI run ID/URL, runner OS/architecture and workspace root, actual `rustc --version` (CI selects stable — do NOT assume the local 1.95 toolchain), cargo-llvm-cov + nextest versions, exact coverage command/config, cargo-crap version + flags/exclusions, and UTC date/time. Also record the normalization command and review outcome. Provenance is committed with the baseline here, not deferred to M2 docs.
- Validation: `node -e "JSON.parse(require('fs').readFileSync('ci/crap-baseline.json','utf8'))"` succeeds; grep of the baseline for backslashes and absolute-root patterns (`/home/`, drive letters) finds nothing (grep exiting 1 on no-match is the PASS signal here — state that explicitly when running it); no Task 1.2 probe function name appears; the sidecar contains every required producer fact and agrees with the downloaded provenance, including the runner's actual rustc version and exclusion list.
- Notes: Hard constraint 3 applies from day one: the baseline changes only via dedicated reviewed commits.

#### Task 1.6: M1→M2 approval gate

- Status: COMPLETED
- Depends On: 1.5
- Objective: explicit user go/no-go for enabling blocking checks, based on actual Linux results.
- Steps:
  1. Present to the user: Task 1.2 probe findings (closure scoring, cfg-dead code, path matching, numeric repeatability), the Task 1.5 baseline review summary (counts, worst offenders, any unexplained mismatches), CI runtime cost measured in Task 1.1, and the exclusion list decided in 1.2.
  2. Ask for explicit approval to enable the blocking gate; record the decision as a `## Decision Log` entry (approval, or plan BLOCKED with reason).
- Validation: a dated Decision Log entry names this task and records the user's explicit approval to enable blocking checks. Only affirmative approval completes Task 1.6; refusal or pending approval leaves M2 blocked. Task 2.1 depends on completion of Task 1.6, and no M2 task may start before approval.
- Notes: this task exists because milestone prose alone does not constrain the dependency graph — Task 2.1 hard-depends on it.

#### Task 1.7: Resolve inherited prerequisite quality gate

- Status: COMPLETED
- Depends On: none
- Objective: advisory PR #327 has an agreed prerequisite scope and no unresolved inherited SonarCloud security gate failure.
- Steps:
  1. Present the source evidence: pre-existing commit `5a82c70` appears in `git log origin/master..ci/crap-phase0-advisory`; SonarCloud issue `AaEm5TptZUoGI0Yrksmb` flags `javascript:S4036` at `scripts/check-test-runners.mjs:299`, not the new CRAP normalizer.
  2. Obtain the user's decision: authorize a separate prerequisite fix, wait for that prerequisite to be resolved and merged separately, or authorize rebuilding the advisory PR without it. Do not modify earlier work, rewrite the published PR branch, or dismiss the finding before this decision.
  3. Follow the selected approach in a separate scoped change and run its required tests, changelog, and CI validation. Record the decision and results before resuming Task 1.4.
- Validation: the user's decision is recorded; PR #327 has the agreed ancestry/scope; the public SonarCloud project_status API reports a passing security condition for that scope; required checks pass. No CRAP or coverage threshold is weakened.
- Notes: discovered outside the original CRAP implementation scope. The user selected “Fix separately (Recommended)”. The separate fix, all 62 local script tests, SonarCloud, and every updated PR check passed. Task 1.7 is complete. This is not Task 1.6 approval.

#### Task 1.8: User-requested full implementation self-check

- Status: COMPLETED
- Depends On: 1.1, 1.2, 1.3, 1.7, 3.2, 3.3, 3.4
- Objective: verify correctness, completeness, and accuracy of all currently implemented changes before advisory merge review.
- Steps:
  1. Compare the primary branch and advisory PR against the plan and recorded decisions; distinguish implemented work from approval-dependent pending work.
  2. Review workflow failure paths, analyzer scope/provenance, normalizer boundary cases, lint inheritance/exemptions, coverage reporting, and the auth cleanup tests. Check the explicit Git-selection fix and inherited runner integration.
  3. Run the comprehensive local gate with fresh coverage, script self-tests, and an independent real Linux artifact round-trip. Verify retained Linux probe and current PR evidence. Fix small in-scope defects with maintained tests; record any other issue as blocked work.
  4. Update inaccurate current documentation and status claims. Record results and remaining limits without merging or starting M2.
- Validation: full local gate exits 0 with unchanged coverage threshold; negative controls prove malformed paths fail and metrics/identities remain unchanged on real reports; advisory CI checks and acceptance artifacts agree with recorded findings; plan checker passes.
- Notes: The user first requested this self-check instead of approving the merge on 2026-10-11, then explicitly approved merging PR #327 when the self-check finishes. Task 1.6 approval was not given.

### Milestone 2: Phase 1 — CRAP policy gate

- Status: IN PROGRESS
- Purpose: convert the advisory report into a blocking, correctly-scoped gate with a local advisory mirror and full documentation.
- Exit Criteria: Linux CI fails on a seeded new function with CRAP > 30 and on a seeded regression of an existing function, and the policy script exits 0 on unchanged code in that canonical CI environment; artifacts still upload when the gate fails; Codecov uploads unaffected; pre-commit prints a clearly labeled advisory CRAP verdict locally (explicit skip notice when the tool is missing or has the wrong version), without requiring local policy exit 0 against the Linux baseline; docs state the gate mechanics, the whole-function repo-wide policy, the baseline-update rules, and the blind spots; M1→M2 user approval recorded in the Decision Log.

#### Task 2.1: `scripts/check-crap-gate.mjs` + self-tests

- Status: COMPLETED
- Depends On: 1.6
- Objective: small policy interpreter over cargo-crap delta JSON: fail on new functions with CRAP > 30 or existing functions regressed beyond epsilon 0.01; never computes metrics itself.
- Steps:
   1. After Task 1.6 approval, add a mutually exclusive `--for-comparison` mode to the existing path-only normalizer. Read the committed relative baseline, require an explicit current producer root and contained relative input paths, and write `crap-baseline-input.json` with entry paths under that current root. Preserve all parsed non-path values and duplicate identities exactly. Never overwrite the committed baseline. Add Windows/Linux path, outside-root rejection, duplicate-name, and value-preservation controls to its self-tests. Then write `scripts/check-crap-gate.mjs` styled after `check-diff-coverage.mjs`: args `--report <delta-json>` (from `cargo crap ... --baseline crap-baseline-input.json --format json`), `--threshold 30`, `--epsilon 0.01`, `--self-test`. This mode is not implemented during M1; DEC-013 records the verified requirement.
  2. Exit codes: 0 = pass; 1 = policy violation; 2 = input/tool failure (missing file, malformed JSON, schema/version incompatibility). In CI, missing/invalid input MUST exit 2 — never silently pass. Local optional-skip logic lives in the caller (Task 2.3), not here.
  3. Emit GitHub Actions annotations (`::error`/`::warning` with file/line from the delta JSON) for every offender so gate verdict and annotations come from the SAME data; print each offender as `file:line function CRAP old→new` on stdout.
   4. Capture real 0.6.1 delta-JSON fixtures (Task 1.5 baseline + seeded changes) and build `--self-test` coverage: new function above AND exactly at 30; regression above AND exactly at epsilon; improvements; moved/renamed/ambiguous entries; missing, malformed, and version-incompatible reports.
  5. Add a `crap:gate-self-test` entry to `package.json` following the `coverage:self-test` precedent.
- Validation: `node scripts/check-crap-gate.mjs --self-test` exits 0 on every supported local platform; against an unchanged tree with a fresh delta report the policy script exits 0 on Linux CI; against a deliberately seeded offender in that same environment it exits 1 with an accurate stdout list. Windows-versus-Linux baseline comparisons are advisory and are not required to exit 0.
- Notes: Hard constraint 7 — no metrics computation. Policy wording in all output: whole-function, repo-wide — explicitly stricter than patch-scoped, by decision.

#### Task 2.2: CI gating step

- Status: COMPLETED
- Depends On: 2.1
- Objective: `ci.yml` runs cargo-crap against the committed baseline and gates via the policy script, without endangering Codecov uploads or artifacts.
- Steps:
    1. Add a step AFTER the Codecov uploads (line 215+): first run `node scripts/normalize-crap-baseline.mjs --input ci/crap-baseline.json --root "$GITHUB_WORKSPACE" --for-comparison --output crap-baseline-input.json`, then `cargo crap --workspace --lcov src-tauri/lcov.info --baseline crap-baseline-input.json --format json --sort file --missing pessimistic --epsilon 0.01 --output crap-delta.json` (plus the confirmed analyzer exclusions recorded with the canonical baseline), then `node scripts/check-crap-gate.mjs --report crap-delta.json`. Add a narrow ignore entry for the temporary input file; never stage it or overwrite the committed baseline.
  2. Add a SEPARATE artifact-upload step for `crap-delta.json` placed AFTER delta generation and gate evaluation, with `if: always()` and `if-no-files-found: ignore` — extending the Task 1.1 upload step would execute before the delta file exists. Reports must survive gate failure.
  3. Wire `node scripts/check-crap-gate.mjs --self-test` into the ci.yml lint job (next to the normalizer self-test from Task 1.4) and into `scripts/pre-commit.js` as part of the comprehensive local check — otherwise the policy interpreter can regress without any test failure (ESLint targets `src` only; Vitest excludes `scripts/**`).
  4. Verify by seeded scratch PRs: gate failure does not prevent Codecov uploads (ordering) and both artifacts upload.
- Validation: scratch PR with a seeded new crappy function (> 30) fails the gate step with exit 1 and visible annotations; scratch PR with a seeded regression of an existing function fails; unchanged code passes with exit 0 **on Linux CI**; artifacts and Codecov uploads present in all three runs; the lint job visibly runs both script self-tests.
- Notes: do NOT add a second cargo-crap `--format github` pass unless the script's annotations prove insufficient; if added, its output must be visibly distinct from the gate verdict (scope agreement).

#### Task 2.3: Pre-commit advisory step

- Status: COMPLETED
- Depends On: 2.1
- Objective: `bun run pre-commit` includes a CRAP check that requires fresh coverage, degrades loudly, and is advisory-only on Windows.
- Steps:
  1. In `scripts/pre-commit.js` after step 7 (lines 156–177): if `cargo crap --version` unavailable → skip with explicit warning "CRAP check skipped (install cargo-crap 0.6.1 for the local advisory check)" plus a `results.warnings` entry, mirroring the step-7 pattern at lines 176–177. No auto-install. If installed but the version is NOT 0.6.1 → also skip with an explicit version-mismatch notice (baseline comparability requires the pinned version; never silently gate on a different scorer).
  2. Freshness: run only when step 7 regenerated `src-tauri/lcov.info` in the SAME run (track the step result; a pre-existing file on disk is not fresh input).
   3. Prepare `crap-baseline-input.json` from the committed relative baseline using the normalizer's future `--for-comparison` mode and the current absolute repo root. Run cargo-crap with `--baseline crap-baseline-input.json` plus the policy script in ADVISORY mode: print the verdict, never fail the pre-commit run. This applies on Windows AND on non-Windows local machines for now — the baseline is Linux-CI-produced and cross-platform score comparability is only established for the CI environment itself (Task 1.2 findings); promoting any local platform to blocking is a documented follow-up decision, not part of this plan. Output states that the blocking comparison is Linux CI.
- Validation: `cmd.exe /c bun run pre-commit` on an unchanged local tree prints a clearly labeled advisory CRAP verdict and does not fail because the policy script returns 1 against the Linux baseline; policy exit 0 is NOT required locally. With cargo-crap removed from PATH the exact skip notice appears and the run continues; when step 7 did not regenerate lcov, the CRAP check refuses with a stale-input notice.
- Notes: Hard constraint 9 — never document this as a blocking mirror of CI.

#### Task 2.4: Policy + docs

- Status: COMPLETED
- Depends On: 2.2, 2.3
- Objective: the gate, its whole-function policy, baseline-maintenance rules, and blind spots are documented where devs and agents look.
- Steps:
  1. `docs/best-practices/CI_BEST_PRACTICES.md`: new "CRAP gate" section — mechanics; policy (new functions ≤ 30; existing functions no regression beyond epsilon 0.01; whole-function repo-wide, explicitly stricter than patch-scoped); baseline-update rules (dedicated reviewed commits; reviewed improvements and identity updates permitted; unexplained score increases prohibited; policy-only enforcement acceptable initially); provenance requirements; blind-spots paragraph (closures unscored — incl. `with_unlocked_db(|db| ...)`; macros unexpanded; cfg-dead spans may show 100%; `--missing pessimistic` mandatory in CI, `skip` for diagnosis only).
  2. `.agents/skills/ci-gate-diagnosis/SKILL.md`: add failure recipes (gate exit 1 → read stdout offender list / delta JSON artifact; exit 2 → input/tool problem; all-0% table → path mismatch, see cargo-crap troubleshooting). Edit ONLY `.agents/` (Hard constraint 5), then `cmd.exe /c bun run sync-skills`.
  3. Root `CLAUDE.md`: short pointers only (Verification Commands: `node scripts/check-crap-gate.mjs --self-test`; one gotcha-style line pointing at CI_BEST_PRACTICES) per `docs/best-practices/CONTEXT_FILES_BEST_PRACTICES.md`.
- Validation: the three files contain the new sections (grep anchors); `cmd.exe /c bun run sync-skills` exits 0 without DRIFT; the CLAUDE.md diff is pointers only — no copied tables.
- Notes: no `website/docs-src/` change — this is a dev/CI gate, not a user-facing feature (scope agreement).

### Milestone 3: Phase 2 — complexity lints + adjacent fix

- Status: IN PROGRESS
- Purpose: cyclomatic/cognitive complexity lints active in both languages with measured-zero legacy debt, plus the coverage-reporting fix discovered during verification.
- Exit Criteria: `cargo clippy --workspace --all-targets -- -D warnings` green with `cognitive_complexity` enabled and a positive control proving the wiring live; `bun run lint` emits complexity warnings without failing; test files exempt from the complexity rule only; a failing test run still writes lcov; CHANGELOG updated.

#### Task 3.1: Root `clippy.toml`, delete `src-tauri/.clippy.toml`

- Status: COMPLETED
- Depends On: none
- Objective: one workspace-level clippy config with the cognitive threshold; obsolete crate-local file removed entirely.
- Steps:
  1. Create root `clippy.toml` containing `cognitive-complexity-threshold = 30`. NO `msrv` key — write a Decision Log entry recording that this removes an obsolete setting (`src-tauri/.clippy.toml` had `msrv = "1.75"` matching no declared policy) and does NOT declare an MSRV.
  2. Delete `src-tauri/.clippy.toml` (no pointer file — a leftover could silently take precedence over the root config for the app crate).
- Validation: the clippy invocation itself completes successfully (cargo exit 0 — establish this BEFORE interpreting absent warnings; a crashed run also produces no warnings) and its output contains no `jrnl.rs:342` warning (26 ≤ 30; the previous 26/25 warning is gone — this check passes by producing no matching output, so grep exiting 1 on no-match is the PASS signal; read it explicitly, never branch on bare non-zero); `git ls-files` filtered for `*clippy.toml` lists exactly one entry, the root `clippy.toml` (scoped to tracked project files — a recursive filesystem search would also hit `target/` and dependency directories).
- Notes: clippy reads config from the linted package directory upward — root placement covers all three crates (this is precisely why the old `src-tauri/` file never applied to `crates/*`).

#### Task 3.2: Enable `cognitive_complexity` workspace-wide

- Status: COMPLETED
- Depends On: 3.1
- Objective: the nursery lint is genuinely enabled for all three crates and enforced by existing CI via `-D warnings`.
- Steps:
  1. Root `Cargo.toml`: add `[workspace.lints.clippy]` with `cognitive_complexity = { level = "warn", priority = -1 }` (nursery lints need explicit per-lint opt-in — verify the mechanism enables the lint without pulling in the rest of the nursery group; if per-lint nursery opt-in is rejected by the toolchain, record the alternative in the Decision Log before proceeding).
  2. Add `[lints]` + `workspace = true` to ALL THREE members: `src-tauri/Cargo.toml`, `crates/mini-diarium-core/Cargo.toml`, `crates/mini-diarium-crypto/Cargo.toml` (without this the workspace table is inert — scope agreement).
  3. Confirm no pre-existing `[lints]` tables conflict (exploration verified none exist).
- Validation: `cargo clippy --workspace --all-targets -- -D warnings` exits 0 (measured 0 violations at threshold 30); POSITIVE CONTROL: temporarily set threshold 25 in root `clippy.toml` → the `jrnl.rs:342` warning appears → wiring proven live → restore 30 and re-verify green.
- Notes: `warn` level is deliberate — `-D warnings` in CI (ci.yml:72) and pre-commit step 8 (scripts/pre-commit.js:195) already make it blocking (scope agreement Q4).

#### Task 3.3: ESLint `complexity` rule

- Status: COMPLETED
- Depends On: none
- Objective: TS/JSX functions linted for McCabe cyclomatic complexity; test files exempt from that rule only.
- Steps:
  1. `eslint.config.js`: add `complexity: ["warn", { max: 15, variant: "classic" }]` to the base rules block.
  2. Add a config override for `**/*.test.ts`, `**/*.test.tsx`, `**/*.spec.ts`, `**/*.spec.tsx` setting ONLY `complexity: "off"` — all other rules stay active for tests.
   3. Write a Decision Log entry: warn is intentionally non-blocking today (`lint` script has no `--max-warnings 0`); promotion to `error` is a follow-up once the warning backlog burns down — documented temporary exception to CI guidance (scope agreement).
   4. Update `docs/best-practices/FRONTEND_BEST_PRACTICES.md` with durable guidance for this temporary exception: classic complexity max 15 runs at `warn`, `bun run lint` has no `--max-warnings 0`, and complexity warnings therefore do not block CI or the comprehensive local check. Test files are exempt from ONLY the complexity rule; other lint errors remain blocking. Promotion to `error` requires a separate decision after reviewing the measured warning backlog. Link to the CI guidance rather than duplicating its general lint policy.
- Validation: `cmd.exe /c bun run lint` exits 0 while printing complexity warnings; the warning count and top files are captured in `## Execution Notes` (expect hot spots `DiaryEditor.tsx`, `EditorToolbar.tsx`, `BackupsPanel.tsx` per SonarCloud data — but THIS run is the calibration measurement, not Sonar's file totals); a scratch test file containing a high-CC function produces NO complexity warning while still producing other lint errors when seeded; the `FRONTEND_BEST_PRACTICES.md` diff states the temporary non-blocking exception, test-only rule exemption, and separate promotion decision.
- Notes: exclude tests from the complexity rule ONLY (scope agreement Q6/5).

#### Task 3.4: `coverage.reportOnFailure: true` (own scoped commit)

- Status: COMPLETED
- Depends On: none
- Objective: Vitest writes a fresh `coverage/lcov.info` even when tests fail; this reporting fix does not replace the caller's freshness checks.
- Steps:
  1. `vitest.config.ts` coverage block (lines 17–36): add `reportOnFailure: true`.
  2. Commit as its own scoped commit (scope agreement: included, but not mixed into feature commits).
- Validation: create a disposable scratch test with a failing assertion (a missing `--testNamePattern` is NOT a failing test). Delete ONLY the generated `coverage/lcov.info` before the forced-failure run, verify it is absent, then record the run-start UTC timestamp. Run `cmd.exe /c bun run test:coverage` and verify a non-zero test exit with the scratch assertion shown as failed. Require that `coverage/lcov.info` was recreated by this run: it exists and its UTC last-write timestamp is NEWER than the recorded run-start timestamp. Mere file existence does not pass. Remove only the disposable scratch test, then run normal coverage again; require exit 0 and a newly written lcov. Do not revert unrelated source or user changes.
- Notes: complements — does not replace — Task 2.3's freshness requirement. The `prosemirror-dedup` timeout fix is explicitly OUT of this plan.

#### Task 3.5: CHANGELOG entry

- Status: IN PROGRESS
- Depends On: 2.2, 2.4, 3.2, 3.3, 3.4
- Objective: the new gates/lints are recorded per repo convention.
- Steps:
   1. Add entries under `### Internal` in the Unreleased section of `CHANGELOG.md` following the existing template: CI CRAP gate for Rust (advisory→gated), clippy `cognitive_complexity` enabled workspace-wide, ESLint `complexity` rule, Vitest `reportOnFailure` fix. These changes are developer-only; do not place them under user-facing Added/Fixed/Changed sections.
- Validation: the `CHANGELOG.md` diff shows the entries under Unreleased / Internal in the established format.
- Notes: none.

### Milestone 4: Cleanup And Final Verification

- Status: IN PROGRESS
- Purpose: ensure the repository contains only intentional final artifacts and the complete change is verified.
- Exit Criteria: intermediate artifacts removed, every `## Pre-flight Checks` item passes, and Task 4.2 final verification passes (including Linux CI policy exit 0 and the successful local advisory analysis). After these criteria pass and task/milestone statuses are reconciled, set Plan Status to COMPLETED; that status change is the consequence of verification, not a prerequisite.

#### Task 4.1: Cleanup Intermediate Artifacts

- Status: TO BE DONE
- Depends On: 2.2, 2.3, 3.5
- Objective: remove artifacts created only to support implementation.
- Steps:
   1. Inspect the worktree: scratch probe branch deleted (never merged); stray local `crap-report.json`/`crap-delta.json`/`crap-baseline-input.json` files removed (gitignored, but confirm none got staged); no leftover temp fixtures beyond the scripts' inline self-tests; no debug logging added to `pre-commit.js`.
    2. Keep: `ci/crap-baseline.json`, `ci/crap-baseline.provenance.md`, the two new scripts and their self-tests, docs updates, this plan file and its companion decisions file, the proposal doc.
- Validation: worktree diff contains only intended final changes; `git branch --list` shows no scratch branch; `git status --porcelain` shows no unintended `??` entries.
- Notes: do not remove user-provided files or unrelated worktree changes.

#### Task 4.2: Final Verification

- Status: TO BE DONE
- Depends On: 4.1, 4.3
- Objective: validate the integrated change after cleanup.
- Steps:
   1. Regenerate both lcov files using the frontend/backend coverage commands in `## Context For A Clean Session`, requiring successful test runs, then run every item in `## Pre-flight Checks` against those fresh inputs.
    2. Local advisory dry run from repo root: prepare the ignored path-aligned `crap-baseline-input.json` using the normalizer's `--for-comparison` mode and current absolute repo root. Run `cargo crap --workspace --lcov src-tauri/lcov.info --baseline crap-baseline-input.json --format json --sort file --missing pessimistic --epsilon 0.01 --output crap-delta.json` (plus the analyzer exclusions recorded with the canonical baseline), then `node scripts/check-crap-gate.mjs --report crap-delta.json`. Require cargo-crap analysis exit 0 and a valid fresh delta report. Capture and clearly label the policy verdict as ADVISORY: script exit 0 (no violations) OR exit 1 (reported violations) is acceptable locally; exit 2 is an input/tool failure and must be diagnosed, not counted as a successful analysis. In particular, Windows comparison against the Linux baseline does NOT require policy exit 0; platform differences are legitimate under Hard constraint 9. Run all three platform-independent self-tests locally (`coverage:self-test`, `crap:gate-self-test`, normalizer `--self-test`), requiring exit 0 for each.
   3. Confirm one green Linux CI run for a PR targeting master with the blocking gate active, on code without seeded offenders. Require the policy script to exit 0 in that canonical CI environment, both script self-tests to pass, and report artifacts plus Codecov uploads to be present. This is the REQUIRED blocking unchanged-code check; the local Windows advisory dry run cannot replace it.
- Validation: successful fresh coverage generation and all pre-flight commands exit 0; local analysis exits 0 and prints a clearly labeled advisory policy verdict (policy exit 0 or 1, never an unhandled input/tool failure); all three local self-tests exit 0; the Linux CI run is green with the blocking policy script exiting 0 and the new steps verified.
- Notes: known limitation — the gate does not see closure bodies or macro expansions (documented per Task 2.4). Do not promote the local advisory comparison to a blocking check as part of final verification.

#### Task 4.3: Resolve local frontend timeout validation

- Status: COMPLETED
- Depends On: none
- Objective: obtain a successful fresh full frontend coverage run without weakening tests, and record the changing timeout failures accurately.
- Steps:
  1. Retain both failed-run logs and the unchanged targeted controls. Do not report either failed full run as green.
  2. Verify the installed Vitest worker-limit option, then run `cmd.exe /c bun run test:coverage --maxWorkers 4` as a local runner-load diagnostic with every test and unchanged timeouts/thresholds.
  3. If the full diagnostic fails again, ask for separate authorization before any unrelated test/source fix. If it passes, record the validation result and its explicit concurrency setting, without claiming that the default-concurrency run passed or that the root cause is proven.
- Validation: full diagnostic exits 0 with all 1277 frontend tests passing and fresh coverage; both timeout suites pass unchanged targeted controls; no source test, timeout, coverage threshold, or CI setting changed.
- Notes: temporary validation blocker discovered while running Task 2.1's comprehensive local checks (DEC-020/DEC-021). The existing dedup source fix remains a Non-Goal. This task records and verifies local validation, not a new feature or permission for unrelated fixes.

## Project Gates

- `cmd.exe /c bun run type-check`
- `cmd.exe /c bun run lint`
- `cmd.exe /c bun run format:check` and `cargo fmt --all --check`
- `cmd.exe /c bun run test:run`
- `cargo test --workspace` (all three crates — `--manifest-path src-tauri/...` alone silently skips core/crypto)
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cmd.exe /c bun run coverage:diff` (after generating both lcov files — mirrors Codecov patch ≥ 80%)
- `cmd.exe /c bun run validate:locales`
- CHANGELOG entry required (root `CHANGELOG.md` exists)
- Scoped commits: one logical change per commit — dedicated commits at minimum for the baseline and provenance sidecar (Task 1.5) and `reportOnFailure` (Task 3.4); root `CLAUDE.md` Agent Workflow Rule 6
- Skills: edit only `.agents/skills/`, then `cmd.exe /c bun run sync-skills`
- Per-task completion checklist per `docs/best-practices/POST_TASK_BEST_PRACTICES.md`

## Pre-flight Checks

- [ ] `cmd.exe /c bun run type-check`
- [ ] `cmd.exe /c bun run lint`
- [ ] `cmd.exe /c bun run format:check`
- [ ] `cargo fmt --all --check`
- [ ] `cmd.exe /c bun run test:run`
- [ ] `cargo test --workspace`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cmd.exe /c bun run coverage:diff` (with both lcov files freshly generated)
- [ ] `node scripts/check-crap-gate.mjs --self-test`
- [ ] `node scripts/normalize-crap-baseline.mjs --self-test`
- [ ] `cmd.exe /c bun run sync-skills` (no DRIFT)
- [ ] `cmd.exe /c bun run validate:locales`

## Decision Log

Entries live in [2026-10-10-crap-complexity-gates-decisions.md](2026-10-10-crap-complexity-gates-decisions.md) (22 entries). The timing rule is unchanged: write an entry before moving to the next task.

## Final Verification

Task 4.2: successful fresh coverage generation, the full pre-flight list, all three platform-independent local script self-tests (exit 0), and a successful local CRAP analysis with a clearly reported ADVISORY policy verdict. Local policy exit 0 is NOT required: exit 1 may reflect legitimate platform differences against the Linux baseline; exit 2 is an input/tool failure to diagnose. The REQUIRED blocking unchanged-code check is a green Linux CI PR run with the gating step active and the policy script exiting 0. Complete verification before setting Plan Status to COMPLETED.

## Approval Gate

Approved by the user on 2026-10-10: “Do an exploration on the plan … Then implement it.” The additional M1→M2 gate was approved on 2026-10-11: after the acceptance review and explicit Linux blocking-gate approval question, the user replied “ok we need to continue” (DEC-019). Local checks remain advisory.

## Plan Self-Check

```
$ python "C:\Users\Francisco\.config\opencode\skills\manual-planning\scripts\check-plan.py" "docs/plans/2026-10-10-crap-complexity-gates-plan.md"
0 error(s), 0 warning(s)
```

Run: 2026-10-11 (continuation check after canonical baseline review)

Checker exit code: 0.

The earlier W001 warning is resolved: the plan is now tracked in commit `4f480f0`.

## Execution Notes

- Task 2.4 is COMPLETED: CI best practices now state the whole-function/repository-wide Linux policy, 0/1/2 failure contract, coverage/artifact ordering, all-local-platform advisory behavior, path alignment, reviewed baseline update/provenance rules, and closure/macro/cfg-dead limits. Canonical `ci-gate-diagnosis` includes artifact-first recipes, all-0% path checks, and the same policy/limits. Root `CLAUDE.md` adds only a self-test command and a short gotcha with pointers. All new relative links resolve, required anchors are present, `sync-skills` exits 0 without DRIFT, and no generated mirror is edited or staged. No website docs change applies to this developer-only check.
- Task 2.2 is COMPLETED: unchanged-source run 38142680713 passed all seven jobs on `3e296fdd5e1ede64f2be77a4964734ecffcefd8e`, including the production gate, both lint self-tests, Windows/macOS/Linux builds, Flatpak, and clean/stateful Linux E2E. Both Codecov uploads and both CRAP artifacts succeeded. Independently parsed delta reports policy exit 0, 625 functions, zero violations, no probes, and unchanged `build_menu` at CRAP 600. Report/provenance/delta evidence is retained in `C:\Users\Francisco\AppData\Local\Temp\opencode\crap-gate-unchanged-evidence-38142680713`. Together with the two expected exit-1 negative controls below, this proves blocking behavior and upload preservation. All watchers have finished. Task 2.4 can now start; final post-cleanup verification remains Task 4.2.
- Task 2.2 new-offender negative control passed its acceptance condition in run 38142680404 on `761fa966440e1a1358cb2429014af5cdc0fb70db`: only the production gate failed, exit 1, with the accurate `crap_gate_new_offender` annotation. Independently parsed delta has 626 functions and exactly one offender: New, CC 6, coverage 0%, CRAP 42, null baseline/delta. Frontend/script/backend/print tests, both lint self-tests, both Codecov uploads, and both report/delta uploads succeeded; dependent builds correctly skipped. Report, provenance, and delta are retained in `C:\Users\Francisco\AppData\Local\Temp\opencode\crap-gate-new-evidence-38142680404`; failed-step log is `crap-gate-new-failed-step-38142680404.log` in the parent approved temp directory. Both negative controls now pass; the unchanged-source watcher still owns the remaining Task 2.2 result.
- Task 2.2 existing-regression negative control passed its acceptance condition in run 38142680872 on `d53c75e69d82697e2c5e75d915f8478f219e5535`: only the real CRAP gate failed, with policy exit 1 and the accurate `build_menu` annotation. Downloaded delta independently evaluates to exactly one violation: existing `build_menu`, line 27, CC 25, coverage 0%, CRAP 600→650, delta 50, status regressed; 625 functions. Frontend, script, backend, print tests, both lint self-tests, both Codecov upload attempts, and both CRAP artifact uploads succeeded. Dependent build/E2E jobs correctly skipped due to the blocking gate. Report, provenance, delta, and failed-step log are retained in `C:\Users\Francisco\AppData\Local\Temp\opencode\crap-gate-regression-evidence-38142680872`. This expected workflow failure is not a production defect. Task 2.2 still requires the unchanged and new-offender controls; their watchers remain active.
- Task 2.3 is COMPLETED in `3e296fd`: the maintained advisory caller uses only successful fresh same-run backend coverage, checks pinned tool version, creates an ignored root-aligned input, refuses stale deltas, and reports local policy exits without blocking. Seven new maintained controls cover stale lcov, missing/wrong-version tools, canonical preservation, analyzer and policy failures, invalid baseline, real empty-PATH skip, and caller freshness/warning wiring. Fresh comprehensive run `sh_12b1a40e0001MuUWSnZ7pTRdfX` passed all 15 checks, 1277 frontend tests in 116 files, 950 backend tests, and 82 script tests; ownership covers 133 tracked files. The explicit local `VITEST_MAX_WORKERS=4` control is supported by installed Vitest `index.DpLw24bj.js:14592`; no source/timeouts/thresholds/CI concurrency changed. Both lcov files and the advisory delta were freshly written at 13:16:26/13:18:39/13:18:40 UTC. The policy returned 1 on the same four Windows differences (625 functions), while the comprehensive run returned 0. Fresh patch coverage remains 46/48 (95.8%) at 80%. Log: `C:\Users\Francisco\AppData\Local\Temp\opencode\crap-gated-local-advisory-20261011.log`. This does not claim default-concurrency success or a Linux verdict.
- Task 2.2 controls use isolated NEVER-MERGE draft PRs: unchanged [#332](https://github.com/fjrevoredo/mini-diarium/pull/332) at `3e296fd`, run 38142680713; new-offender [#331](https://github.com/fjrevoredo/mini-diarium/pull/331) at `761fa96`, run 38142680404; existing-regression [#333](https://github.com/fjrevoredo/mini-diarium/pull/333) at `d53c75e`, run 38142680872. The negative branches change only workflow seeding before fresh backend coverage; no production source or baseline is committed. The real production gate remains unchanged and is expected to fail with exit 1 in each negative run; Codecov uploads and both artifacts must still succeed. The existing control adds one branch inside untested `build_menu`, preserving its signature/location identity. Watchers `sh_12b1e17560013Pk1AgmIkeAwAC`, `sh_12b1e1775001YLLSmHj5gJEZKt`, and `sh_12b1e17a0001RuFig3UWZZ1T6X` are active; await completion notifications, do not poll or push over them.
- Task 2.2 production wiring is committed as `7942411`: gate follows both Codecov uploads and the independent advisory artifact upload; missing same-run coverage, missing/wrong-version analyzer, and analyzer failure fail as input/tool errors; policy exit 1 remains blocking; a separate `if: always()` delta upload follows the gate. Lint and pre-commit run both self-tests. All 75 script tests, ownership for 132 tracked files, actionlint, targeted formatting, and diff checks passed. Live unchanged/new/regression gate controls are still required before Task 2.2 completes. Task 2.3 will use the maintained caller helper under DEC-022; the canonical baseline remains unchanged.
- CI watcher `sh_12af6249d001RY8KWniuI67XZM` completed successfully: all seven jobs in run 38139778634 passed, including Windows build and clean/stateful Linux E2E. The error annotation for `crap_policy_new_offender` is the expected seeded negative control, not a job failure. Task 2.1 is COMPLETED with independently parsed artifact evidence and unchanged canonical metrics. Task 2.2 can now begin; PR #330 remains disposable and must never merge.
- Task 2.1 Linux controls passed in CI run 38139778634 on scratch head `fce630cce0d4cf937847f00d950b8aa94f6b1d50`: the Test job, frontend/backend/script tests, unchanged-source policy control, seeded-new control, and acceptance artifact upload all succeeded. Downloaded deltas independently revalidated: 625 unchanged-source functions pass policy exit 0; the seeded report has 626 functions and exactly one violation, `crap_policy_new_offender` (CC 6, coverage 0%, CRAP 42, status New, null baseline/delta), with policy exit 1. The probe is absent from the unchanged report. Evidence is retained in `C:\Users\Francisco\AppData\Local\Temp\opencode\crap-policy-acceptance-evidence-38139778634`. Lint, Linux/macOS builds, and Flatpak also passed; Windows build and E2E were still running at the user-requested status check, so the whole workflow is not yet claimed green. These completed Test-job controls satisfy Task 2.1; later full-pipeline and regression checks remain Task 2.2/4.2. Do not push over the active scratch run or merge PR #330.
- Task 4.3 is COMPLETED: the unchanged four-worker full-coverage control passed all 1277 frontend tests in all 116 files, exit 0, duration 92.60 seconds. Run start was `2026-10-11T12:47:46Z`; fresh `coverage/lcov.info` was written at `2026-10-11T12:49:21.1283026Z`. Both previously timed-out suites also passed unchanged targeted controls. The fresh patch gate passed again at 46/48 (95.8%), with the unchanged 80% threshold. No source, test, timeout, threshold, or production CI concurrency setting changed. The result supports runner-load sensitivity but does not prove root cause, erase either failed default-concurrency run, or make the earlier comprehensive run green. Final whole-plan verification remains pending; if local default concurrency again causes timeouts, use an explicitly recorded bounded-worker diagnostic instead of weakening assertions or timeout limits. Task 2.1 still awaits Linux acceptance; the active CI watcher must not be polled.
- The full frontend rerun `sh_12afd8bc6001gaIeGRD1nh8zNv` also exited 1: the dedup test passed, but `PreferencesOverlay.integration.test.tsx`'s Security persistence test timed out at 5672 ms against 5000 ms; 1276 other tests passed. Its unchanged targeted suite passed 2/2, with total duration 39.44 seconds dominated by transforms/imports and test time below one second. Task 4.3 records this validation blocker under DEC-021 and is IN PROGRESS during its reduced-worker control. No app/test/timeout configuration was changed. `bun run test:coverage --maxWorkers 4` is running as `sh_12b01429e0018miMbXy3KeKl6k`, log `C:\Users\Francisco\AppData\Local\Temp\opencode\crap-policy-frontend-workers4-20261011.log`; await its notification. This control is not evidence of a passing default-concurrency run.
- Fresh local gate `sh_12af5fee60018GD0MJRbUGaJOW` completed with exit 1, not a pass: only `src/test/prosemirror-dedup.test.ts` timed out (7382 ms against the unchanged 5000 ms limit). Other frontend tests passed (1276); backend passed 950/950; scripts passed 74/74; the other 13 gate checks passed; patch coverage remains 46/48 (95.8%) at 80%. The known dedup timeout fix is a Non-Goal. DEC-020 records unchanged targeted/full frontend controls without weakening the guard. The targeted `bun run test:run src/test/prosemirror-dedup.test.ts` control passed 1/1 with total duration 2.34 seconds. Fresh full frontend coverage is running as `sh_12afd8bc6001gaIeGRD1nh8zNv`, log `C:\Users\Francisco\AppData\Local\Temp\opencode\crap-policy-frontend-recheck-20261011.log`; await its notification, not a poll. The Linux watcher remains active; this local failure does not establish a CI failure.
- Task 2.1 Linux acceptance is running in NEVER-MERGE draft PR [#330](https://github.com/fjrevoredo/mini-diarium/pull/330), branch `ci/crap-policy-acceptance`, external worktree `C:\Users\Francisco\AppData\Local\Temp\opencode\crap-policy-acceptance-20261011`. Primary implementation commit is `00fa12a`; disposable workflow-only commit is `fce630cce0d4cf937847f00d950b8aa94f6b1d50`. CI run [38139778634](https://github.com/fjrevoredo/mini-diarium/actions/runs/38139778634) first compares unchanged source after fresh workspace coverage, then injects a runner-only new offender, regenerates coverage, and requires the real analyzer/policy to report it as New above 30 with exit 1. Runner source is restored on exit; both deltas upload as disposable evidence. Actionlint and diff checks passed. No probe enters primary source or the canonical baseline. All 132 tracked test files belong to gated runners. A real-baseline round-trip also passed for Linux and Windows roots, preserving every parsed value/order and the canonical SHA-256. Task 2.1 stays IN PROGRESS pending Linux results; production CI remains advisory. Background CI watcher: `sh_12af6249d001RY8KWniuI67XZM`, log `C:\Users\Francisco\AppData\Local\Temp\opencode\crap-policy-ci-watch-38139778634.log`. Fresh comprehensive local check: `sh_12af5fee60018GD0MJRbUGaJOW`, log `C:\Users\Francisco\AppData\Local\Temp\opencode\crap-policy-local-gate-20261011.log`. Await notifications in this session; do not poll either watcher.
- Task 2.1 local implementation controls passed: comparison-mode normalization restores the explicit current root without changing baseline metrics, diagnostics, duplicate locations, or input data; both normalizer modes are mutually exclusive and refuse input/canonical baseline overwrite. The policy interpreter accepts only pinned 0.6.1 delta-v2 reports with matched LCOV data, validates locations and delta status, returns 0/1/2, and emits annotations from its offender list. The real Windows workspace delta supplied maintained fixture shapes, not canonical metrics. Local analysis exited 0; the advisory policy verdict exited 1 on four entries: Windows screen-lock `init`, `screen_lock_subclass_proc`, `trigger_auto_lock`, and backup-store `dir_state` (increase 0.20391442876650068). This is not a Linux gate verdict and does not change the baseline. Both inline self-tests and all 74 script tests passed. Dependency lockfiles and Nix metadata are unchanged. Linux unchanged-code and seeded-offender checks remain required before Task 2.1 completes; no Task 2.2/2.3 implementation started.
- Task 1.6 is COMPLETED under DEC-019. The user approved M2 after the acceptance findings and explicit approval question. Milestone 1 now meets all exit criteria. No policy code was written before that approval.
- Task 1.5 is COMPLETED in dedicated baseline commit `189bb55`, containing only `ci/crap-baseline.json` and `ci/crap-baseline.provenance.md`. Continuation verified master CI run 38103562280 completed successfully on exact source `f1bb84f73f44dd40028299682d0b6ef2f2326ca7`: all seven jobs passed, with final E2E completion at `2026-10-11T02:27:26Z`. The sidecar now records that result. Revalidation passed: JSON parsing, baseline-mode path-only equality with the downloaded report, unchanged parsed non-path values and entry order, exact producer text, all three recorded SHA-256 checksums, no absolute/backslash paths or probes, 624 unique source locations, 17 above 30, and crate counts 247/346/31. All four probe ancestry controls again returned exactly 1 against the producer source. Excluded test helpers remain only in coverage diagnostics, not scored entries; an initially overbroad validation searched diagnostics as well, then the corrected scored-entry check passed. Normalizer self-test and plan checker passed with zero errors/warnings; staged diff checks passed. No source, dependency, threshold, or workflow changed. Task 1.6 is IN PROGRESS; explicit approval is still required before M2. Baseline provenance is covered by the existing Internal advisory-report changelog entry; the final gate entry remains Task 3.5. No originating TODO or rendered-output change applies.
- Fresh integrated checks after master sync passed all 14 `bun run pre-commit` checks: 1277 frontend tests in 116 files, 950 backend tests, 67 script tests, 131 tracked test files owned by a gated runner, Clippy, formatting, and patch coverage 46/48 (95.8%) at the unchanged 80% threshold against merge-base `f1bb84f73f`. The latest Linux artifact has 624 entries (app 247, core 346, crypto 31), 17 above 30, zero scored probes/helpers, all non-path values/order preserved, and five expected production helper names added since `8a345d7`. The baseline and sidecar were prepared while master CI finished; both are now committed as recorded above. A fresh local diagnostic has 625 entries; 16 of the Linux top 17 exactly match, and the only difference is the independent `create_diary_auto_inner` extraction absent from master. Its report remains outside the repo. The source/coverage mismatch diagnostics are fully explained in the sidecar.
- Latest-master sync completed as merge `4a88bd7`: origin/master `f1bb84f73f44dd40028299682d0b6ef2f2326ca7` is an ancestor of HEAD. The only conflict was `CHANGELOG.md`; resolution retains master's 233-byte legacy filename description, one advisory-report entry, and this branch's independent lint/reporting/diagnosis entries. Staged formatting and diff checks passed. At that point Task 1.5 awaited master CI run 38103562280 and fresh local `bun run pre-commit`; both subsequently passed. No M2 work or gate approval is implied (DEC-018).
- Task 1.4 is COMPLETED: post-merge master CI run 38099433982 passed Test, Lint, Windows/Linux/macOS builds, Flatpak, and E2E on exact merge SHA `8a345d70abe8fb5c3aba842150f5b0c2af5a66d9`. The downloaded `crap-report` artifact has production timestamp `2026-10-11T00:52:22Z`, event push, ref refs/heads/master, correct source SHA and run ID, Linux/X64, rustc 1.95.0, cargo-crap 0.6.1, cargo-llvm-cov 0.8.7, nextest 0.9.130, successful workspace coverage, and the confirmed exclusions. Its report covers all three crates: 619 functions, 17 above CRAP 30, zero probes/helper entries. In-memory baseline-mode normalization accepts every path and preserves all non-path values and order. This verifies the required master artifact only; no canonical baseline was written or committed. Task 1.5 and separate Task 1.6 approval remain pending; no M2 work started.
- PR #327 merged at `2026-10-11T00:44:08Z` with merge commit `8a345d70abe8fb5c3aba842150f5b0c2af5a66d9`. It was marked ready only after Task 1.8 completed and every check passed. The merge used `--match-head-commit 926d30f9a7ae5234a0e8ebf6bbdf736d3a2eeb99`, without admin/auto-merge or any check bypass, preserving the scoped commits. Freshly fetched origin/master contains the merge, and all four disposable probe commits still return exactly 1 from ancestor checks. Post-merge master CI [38099433982](https://github.com/fjrevoredo/mini-diarium/actions/runs/38099433982) is active on the exact merge SHA. Task 1.4 remains IN PROGRESS until its report artifact is verified; no canonical baseline or M2 work started.
- Task 1.8 is COMPLETED: CI run 38097918038 passed all seven jobs, including Windows/Linux/macOS builds, Flatpak, and E2E. Every PR check also passed, including SonarCloud, CodeQL, and Codecov patch coverage. The checked PR head is exactly `926d30f9a7ae5234a0e8ebf6bbdf736d3a2eeb99`, still draft/open and mergeable at the final check. Together with the recorded code audit, fresh local checks, maintained regression tests, and production artifact validation, this satisfies the full self-check. Task 1.4 may now merge under DEC-017 without any check bypass; master CI validation remains required afterward.
- The user approved merging PR #327 once Task 1.8 finishes (DEC-017). Updated Test, Lint, Linux/macOS builds, SonarCloud, CodeQL, and Codecov checks pass; Windows build, Flatpak, and E2E remain active. The downloaded run-38097918038 Linux artifact again has 619 functions, 17 above 30, no probes/helpers, complete production provenance, and unchanged non-path values through normalization. No master baseline was created from this PR artifact. Merge will target the explicitly checked head and will not bypass any check; Task 1.4 will remain open until post-merge master CI produces its report.
- Task 1.8 review found one source defect: baseline mode accepted the drive-relative path `C:outside.rs` with Linux and Windows producer roots. Primary commit `d7b8dfc`, copied as advisory `926d30f`, refuses those ambiguous paths in both modes and malformed producer roots. A separate normalizer-only script suite covers CLI score/order preservation, outside-root and diagnostic rejection, malformed/version/delta/non-finite reports, unchanged existing output on failure, and warning-only plain report mode. The primary ESLint suite no longer duplicates the normalizer fixture entry point. No scorer, threshold, gate configuration, auth behavior, or dependency changed.
- Task 1.8 full local check passed all 14 checks in `bun run pre-commit`: frontend coverage 1272/1272 in 116 files; script tests 67/67; fresh backend coverage 937/937, including both named auto-key tests; TypeScript, ESLint (seven advisory warnings), source Prettier, locale and repo guards, Clippy, rustfmt, and fresh patch coverage 46/48 (95.8%) at the unchanged 80% threshold. A final script run again passed 67/67, and ownership now covers 131 tracked test files. Targeted script Prettier, normalizer self-test, coverage-parser self-test, actionlint, frontend production build, and skill sync passed. The build retains its existing large-chunk warning; no bundling configuration changed.
- Task 1.8 independently rechecked real Linux evidence: normalization of the 619-entry PR report from run 38086188720 preserves all parsed non-path values, entry order, and identities after paths are restored for comparison. All normalized paths are portable; zero probe/helper entries remain. Re-parsing the two freshly generated Linux reports from 38074856678 confirms 622 identical unique identities, max CRAP delta 0.0007656249999996589, and 622 unchanged/zero removed in the aligned-path delta. PR #328 remains closed and unmerged; each of its four probe commits returns exactly 1 against freshly fetched origin/master. This is evidence review, not new baseline production.
- Task 1.8 repeated the Clippy positive control: lowering only the root threshold to 25 made the workspace check exit 101 on `jrnl.rs:342` at 26/25. Restoring 30 made `cargo clippy --workspace --all-targets -- -D warnings` and workspace rustfmt pass again. All three manifest inheritance tables remain present, exactly one Clippy config is tracked, and every lockfile plus `nix/package.nix` is unchanged.
- Task 1.8 advisory-worktree validation first failed because frontend dependencies had never been installed there (`typescript` missing), not because of the source fix. A frozen-lockfile install followed by rerunning the complete suite passed 62/62 and ownership for 130 tracked test files, with a clean worktree and unchanged lockfiles. The expected changelog cherry-pick conflict was resolved with only the advisory entry; independent lint, Vitest, and auth entries were not copied onto the narrower branch. The source fix is pushed; replacement CI run 38097918038 is active. The PR body now states the resolved inherited finding, self-check correction, primary-versus-PR validation boundary, and pending merge/baseline/gate approvals accurately.
- Task 1.8 documentation review corrected the original snapshot label, local-versus-CI tool versions, stale exploration next steps, stale canceled-run/blocker wording, and the current execution boundary. Direct CLI tests now run independently of ESLint controls on the advisory PR. Production `cargo build --workspace` and plain `cargo test --workspace --quiet` passed (app 304, core 591, crypto 42; total 937), including doc-test targets. Relative documentation links resolve and the plan checker reports zero errors/warnings. Prior local Windows E2E and forced-failure Vitest evidence still apply: auth sources have no diff since `b42c3da`, and Vitest config has no diff since `f7b5239`. Installed Vitest still checks `reportOnFailure` before dropping a failed-run report. No rendered-output change was made in this self-check. No advisory merge or blocking-gate approval was given.
- Task 1.8 fresh Windows analysis also passed with the confirmed exclusions: 620 functions, 17 above 30, 115/117 matched source files. The two unmatched sources are build.rs and the macOS WebView source; four excluded helper sources and import/mod.rs explain all five coverage-only files. A parsed-value comparison after normalizing this fresh report preserved every non-path value and entry order. This local primary-branch diagnostic is not the narrower Linux PR report or a canonical baseline; no local policy verdict is possible before M2.
- Task 3.2 coverage follow-up: extracted only `create_diary_auto_inner(&DiaryState)` and added named tests `test_create_diary_auto_keeps_key_after_success` and `test_create_diary_auto_clears_key_and_preserves_database_error`. Both passed. The success test proves that the saved key opens the journal; the failure test blocks the database parent with a file and checks exact error preservation, no open connection, and removal of the auto key. No keys are printed. Clippy and rustfmt passed after the extraction. Fresh full backend tests and coverage each passed 937/937 (app 304, core 591, crypto 42). `bun run coverage:diff --working-tree` passed at 46/48 lines, 95.8%, with the unchanged 80% threshold. Local Windows E2E rebuilt the release app and passed all six spec files on WebView2 154.0.4258.62, ending at `2026-10-10T17:56:07Z`. Task 3.2 is COMPLETED.
- Task 1.1 failure-path preparation: scratch commit `a33438a` added only a deliberate failing frontend assertion. Its push was held until the first successful acceptance run finished, to avoid canceling that run under CI's concurrency rule. Validation passed in run 38074581034.
- Task 1.2 Linux evidence saved: run [38072720599](https://github.com/fjrevoredo/mini-diarium/actions/runs/38072720599) completed successfully. Two fresh coverage runs produced identical sets of 645 functions and maximum absolute CRAP delta 0. Closure probe: CC 1, coverage 57.14285714285714%, CRAP 1.0787172011661808; closure branches do not increase CC, but covered closure lines contribute to coverage. cfg-dead probe: CC 3, coverage 100%, CRAP 3 despite not compiling on Linux. Mixed control: CC 4, coverage 55.55555555555556%, CRAP 5.404663923182442. Paths matched 119/124 analyzed files; the five unmatched sources are the two test-only files, build.rs, and macOS/Windows WebView sources. One coverage-only import/mod.rs is expected because it contains no scored production functions. Absolute-versus-relative baseline control found 638 unchanged and seven false new repeated-name identities. Analysis elapsed 0 whole seconds; cold cargo-crap install took 52 seconds. Both Codecov uploads and both report artifacts succeeded. Evidence and production-time provenance are retained outside the repo, never as a baseline.
- Task 1.1 failed-test control passed its acceptance condition: run [38074581034](https://github.com/fjrevoredo/mini-diarium/actions/runs/38074581034) failed only the deliberate frontend assertion (116 normal test files passed). Backend coverage skipped. Both Codecov upload steps ran and completed; their logs explicitly report no coverage files, as expected for this advisory-only branch without reportOnFailure. CRAP printed its same-run-coverage skip message and completed without a report. The report artifact upload succeeded with provenance only; the disposable evidence upload tolerated no files. No additional step failed. Together with the first successful Linux report and both successful uploads, this completes Task 1.1.
- Scope update: primary commit `7323092` adds the two test-only exclusion patterns to every advisory analyzer command and its provenance, and records the findings in CI best practices. Local analysis removed exactly 23 helper entries (643 to 620); every retained production entry remained exactly equal. Workflow syntax passed actionlint. Advisory PR #327 contains the same isolated change at `f3e8ade`; run [38074726733](https://github.com/fjrevoredo/mini-diarium/actions/runs/38074726733) completed successfully: Test, Lint, Windows/Linux/macOS builds, Flatpak, and E2E all passed. Its probe-free report contains 619 functions, 17 above 30, and zero probe entries. This is a PR report, not the canonical master baseline. The inherited SonarCloud security condition still blocks merge; the PR description now states the evidence and inherited scope.
- Final Task 1.2 control passed in [38074856678](https://github.com/fjrevoredo/mini-diarium/actions/runs/38074856678), using the confirmed test-only exclusions. Two fresh coverage runs retained identical sets of 622 functions; maximum absolute CRAP delta was 0.0007656249999996589, below epsilon 0.01. Only `backup::create_snapshot` changed (CRAP 7 to 7.000765625, coverage 100% to 97.5%, uncovered line 166 is its manifest-save warning). The unchanged-source path-aligned control reports all 622 functions unchanged and zero removed; the direct relative-baseline control still falsely reports seven new entries. No test-helper entries remained. Scope: 115/118 source files matched; unmatched sources are build.rs and the macOS/Windows WebView sources. The four excluded helper files plus import/mod.rs explain all five coverage-only files. Both Codecov uploads, both report uploads, and normalizer self-tests passed. This completes Task 1.2; no gate or canonical baseline was produced.
- Probe cleanup completed after all artifacts and failure-path logs were saved. PR #328 is CLOSED with `mergedAt: null`. All four scratch commits (`dac0351`, `a33438a`, `53b0c50`, `e9f259e`) were checked against freshly fetched `origin/master` immediately before cleanup; every ancestry command returned exactly 1. The clean disposable worktree and its local/remote scratch branch were removed without force-removing worktree contents. Downloaded evidence remains outside the repo. The advisory worktree is retained for the review checkpoint.
- Task 1.4 was blocked by inherited SonarCloud issue `AaEm5TptZUoGI0Yrksmb` (`javascript:S4036`, scripts/check-test-runners.mjs:299). At discovery, only the security rating failed (2 vs required 1); duplication was 0.0%, and no issue targeted the normalizer. The earlier user commit `5a82c70` is not yet on origin/master and appears in advisory PR #327. The user then authorized Task 1.7's separate fix, which passed updated CI. Advisory merge now awaits Task 1.8 and user review.
- Task 1.7 implementation: primary commit `d7dcfda`, copied to the advisory branch as `92488d0`, selects Git from the standard absolute OS installation or an explicit trusted absolute `MINI_DIARIUM_GIT_EXECUTABLE` override. It never discovers Git through PATH, refuses relative/drive-relative/empty/NUL input, and fails if the selected executable cannot run. Six maintained controls cover Windows/Linux/macOS selection, invalid paths, the real CLI with empty PATH, and missing-executable failure without fallback. `test:scripts` passed 62/62, runner ownership passed for 130 tracked test files, targeted Prettier passed, and patch coverage remained 46/48 (95.8%) at the unchanged 80% threshold. No dependencies or gate configuration changed. Full updated CI run [38086188720](https://github.com/fjrevoredo/mini-diarium/actions/runs/38086188720) passed Test, Lint, Windows/Linux/macOS builds, Flatpak, and E2E. All other PR checks passed too, including SonarCloud, CodeQL, and Codecov patch coverage. Its current report has 619 functions (app 243, core 345, crypto 31), 17 above 30, and no probe or test-helper entries. The report and production-time provenance were downloaded for review only, not committed as a baseline.
- Task 1.7 SonarCloud validation passed on PR head `92488d05bdd319c81bbc87a9f78034df014b9fff`. The check completed successfully at `2026-10-10T21:04:35Z`; the public API reports overall OK, new security rating 1 (A), duplication 0.0%, and zero unresolved vulnerabilities. No issue was dismissed or gate threshold changed. Task 1.7 is COMPLETED; Task 1.4 is IN PROGRESS and awaits user review before merge. No canonical master baseline or blocking gate exists yet.
- Task 1.7 final local checks: `bun run check` passed TypeScript, ESLint, source Prettier, locale validation, UI-error sanitization, build-path guard, donation-address guard, and runner ownership. The separate diagnostic documentation improvement is committed as `88e052a`; it was not mixed into the source fix or pushed onto the advisory branch while its CI ran. Backend/frontend suites and Windows E2E were not repeated locally for this scripts-only prerequisite fix; their earlier validated results remain recorded, and the updated full CI pipeline passed before Task 1.7 completed.
- One session-reflection cycle: the public API and isolated success/failure controls worked well; inherited PR ancestry was not checked early enough, causing a late unrelated blocker; the skill lacked a security-rating recipe. The narrow recipe and inherited-commit check are now in canonical `.agents/skills/ci-gate-diagnosis/SKILL.md`. Skill sync passed with no DRIFT or generated-mirror edits. Existing plan/status, security, and E2E instructions needed no changes; the earlier E2E exemption was an execution mistake, not a missing rule. Future M2 commands now include the verified temporary path-aligned input requirement (DEC-013); no comparison mode or gate code was implemented.
- The first advisory run 38072527514 completed with Test, Lint, Linux/macOS/Windows builds, and Flatpak all successful. Its E2E job was canceled by the newer scope-update push, so the full workflow conclusion is canceled, not a code failure. Replacement runs 38074726733 and 38086188720 later passed all jobs; each new source update still requires a passing replacement run before merge review.
- Task 1.2 local path control: the fresh Windows report contains 643 functions, 17 above 30. `create_diary_auto_inner` scores 8.003251536859219 at 96.29629629629629% coverage; the thin wrapper scores 2 at 0%. Normalize the report, then restore each entry's current absolute workspace root in a disposable input copy: cargo-crap reports all 643 identities unchanged and zero removed. Parsed entries remain exactly equal apart from slash spelling. This validates a possible path-alignment approach locally, not Linux acceptance or a blocking gate. All diagnostic reports remain outside the repo under the approved temporary directory.
- Auth validation follow-up is committed separately as `b42c3da` with its Internal changelog refinement. Local E2E passed and completes Task 3.2. Linux acceptance is complete, but advisory merge, canonical baseline, and Task 1.6 approval remain pending; this does not authorize M2.
- Local diagnostic comparison against the earlier normalized Windows report produced seven false `new` identities for unchanged repeated names (`SecretBytes::eq`, platform `init`, and `trigger_auto_lock`). This is not a canonical score comparison because coverage inputs differ. It confirms the identity risk in DEC-003: the baseline paths must be aligned before cargo-crap computes the delta. Linux acceptance includes the exact unchanged-source control.
- Verification completed: `bun run test:run` passed 1272/1272 in 116 files. `bun run sync-skills` passed with no DRIFT and no canonical skill edits. Local E2E is required for the auth helper extraction (DEC-009). No website screenshot is required because rendered output did not change.
- The first `gh run watch` for the scratch PR stopped because of a GitHub API connection timeout, not a CI failure. The run was still active; its lint job, normalizer self-test, frontend tests, and script tests had passed. Restarted the watcher without changing CI.

- Task 1.2 preparation used isolated probe commit `dac0351` in external worktree `C:\Users\Francisco\AppData\Local\Temp\opencode\crap-acceptance-20261010` and NEVER-MERGE draft PR [#328](https://github.com/fjrevoredo/mini-diarium/pull/328). Probe binaries were not built. Original Linux run: [38072720599](https://github.com/fjrevoredo/mini-diarium/actions/runs/38072720599). The final validation and probe cleanup are complete as recorded above.
- Task 1.1 Linux workflow validation originally ran on advisory PR #327 as [38072527514](https://github.com/fjrevoredo/mini-diarium/actions/runs/38072527514); see the scoped replacement run and cancellation evidence above. Artifact production, failed-test flow, and final scoped Linux acceptance are proven. Task 1.7's inherited finding is resolved; advisory merge remains pending under Tasks 1.4 and 1.8.
- Independent implementation commits: `f7b5239` (reportOnFailure plus Internal changelog), `96753a7` (equivalent inspect_err cleanup), `0f277cd` (root Clippy config and three-crate lint inheritance), `f0bd106` (ESLint warning rule, durable exception docs, and maintained controls). Full backend tests passed: app 302, core 591, crypto 42, total 935. Final script tests passed 56/56; runner ownership passed for all 130 tracked test files. The tracked project has exactly one Clippy config: `clippy.toml`.
- Changelog records implemented advisory reports, coverage reporting, and the two lint changes under Unreleased/Internal. Task 3.5 remains pending because the CRAP blocking gate and its docs are not approved or implemented yet; do not describe the gate as shipped.

- Task 3.2: the full workspace Clippy check passed after the small `inspect_err` change. Temporarily lowering the root threshold to 25 made CI-style `-D warnings` fail on `jrnl.rs:342` with cognitive complexity 26/25. Restoring 30 made the full check pass again. No other nursery lints were enabled.
- Task 3.4: forced assertion run started at `2026-10-10T17:33:48.5000153Z`, failed only the named probe (1 failed, 1272 passed), and recreated lcov at `2026-10-10T17:34:54.9494531Z`. Removed the disposable test. Normal coverage started at `2026-10-10T17:35:20.7617682Z`, passed 1272/1272 tests in 116 files, and rewrote lcov at `2026-10-10T17:36:58.0894401Z`. `reportOnFailure` behavior was checked in installed Vitest source before the edit. No timeout settings were changed.
- Local checks passed: TypeScript, ESLint (7 advisory warnings), source Prettier, workspace rustfmt, locale validation, normalizer self-test, diff-coverage parser self-test, and production frontend build. Workflow syntax passed `actionlint -shellcheck= -pyflakes= .github/workflows/ci.yml` (no output, exit 0).
- Task 1.4 preparation: opened advisory-only draft PR [#327](https://github.com/fjrevoredo/mini-diarium/pull/327) on `ci/crap-phase0-advisory` at `9c3ad8c`. Independent lint and coverage-report commits remain separate from that advisory branch. No merge or baseline production has occurred.

- Task 3.3: `bun run lint` exited 0 with seven warnings and no errors. Functions: `mapTauriError` (32), EditorToolbar callback (25), `getFirstStrongDir` (20), useMultiEntryNav callback (20), BackupsPanel callback (16), Calendar callback (16), `isAnyOverlayOpen` (16). DiaryEditor has no warning at this threshold. All four test-name overrides retain the unused-variable error. Script suite including these maintained controls passed (56/56). Type-check passed.

- Task 3.1: a workspace Clippy run with explicit cognitive lint opt-in exited 0 at root threshold 30, with no cognitive-complexity warnings. The root config replaces the crate-local config.

- Initial exploration rechecked `.github/workflows/ci.yml`, `package.json`, `eslint.config.js`, `vitest.config.ts`, all four Cargo manifests, the proposal, the second-brain project page, and installed cargo-crap/ESLint sources. Machine identity resolved to `machine.main-desktop`. Current starting commit is `5a82c70`, not the plan's earlier census commit.
- Task 1.3: inline self-tests passed. Normalizing a real Windows 0.6.1 report preserved every parsed non-path value and all 642 function entries. This report is diagnostic only and uses existing local coverage, not a fresh canonical run. `bun.lock`, `package-lock.json`, and `nix/package.nix` have no diff. Existing script tests passed (50/50); runner ownership passed (129 tracked files).

- Update milestone and task status before starting and after validation (`plan-status.py <plan> set <id> "<STATUS>"`).
- Task numbering is not execution order; follow `Depends On` (e.g. Tasks 3.1/3.3/3.4 can start before Milestone 1 finishes).
- Record here as they arise: Task 1.2 probe findings and repeatability diffs; Task 3.3 ESLint warning counts and top files; any CI runtime measurements from Task 1.1.
- Write `## Decision Log` entries **before starting the next task** whenever execution diverges, an unplanned problem is found, or a validation is deferred — never retrospectively.
