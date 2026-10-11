# Exploration Output: CRAP + Cyclomatic Complexity Integration

> Artifact type:
> [x] Proposal — a proposed feature, project, or change (description only)
> [x] Documentation — explains how something works or why it is the way it is
>
> IMPORTANT: This artifact contains NO implementation code. Only prose, diagrams,
> tables, and references. Implementation happens in a different mode.

## Context

- **Why this exploration started**: User asked how to integrate CRAP (Change Risk Analysis and Predictions) and cyclomatic complexity analysis into Mini Diarium (Rust/Tauri workspace + SolidJS/TypeScript frontend, GitHub Actions CI, SonarCloud + Codecov).
- **Date**: 2026-10-10
- **Sources consulted**:
  - Repo: `.github/workflows/ci.yml`, `codecov.yml`, `.sonarcloud.properties`, `vitest.config.ts`, `package.json`, `scripts/pre-commit.js`, `scripts/check-diff-coverage.mjs`, `eslint.config.js`, `src-tauri/.clippy.toml`, `rust-toolchain.toml`, root/crate `Cargo.toml` files.
  - Web: artima.com Crap4J origin post, NDepend CRAP article, ESLint `complexity` rule docs, clippy lint-configuration reference, crates.io APIs (`cargo-crap`, `crap4rs`, `rust-code-analysis-cli`), cargo-crap design docs, lizard README, SonarSource docs/community.
  - Live SonarCloud API queries against project `fjrevoredo_mini-diarium`.
  - Local verification runs (see "Measured verification results").

## Background: what CRAP needs

```
CRAP(m) = CC(m)² × (1 − cov(m)/100)³ + CC(m)
```

- `CC(m)` = cyclomatic complexity of function/method m (Crap4J/Savoia–Evans formula, verified via artima.com and NDepend).
- `cov(m)` = test coverage of m in percent.
- Threshold convention: flag at **CRAP > 30** (NDepend, Qt; cargo-crap default).
- Consequences: a function with CC > 30 can never score below 30 even at 100% coverage (must be split); CC 10 needs ≈42% coverage to clear 30; CC 25 needs ≈80%.

CRAP therefore needs **two inputs joined per function**: complexity + coverage. Mini Diarium already produces the coverage half (two lcov files in CI and locally); the complexity half and the join are what this proposal adds.

## Findings (verified)

### Initial state of the repo (before implementation)

1. **Coverage infrastructure is already CRAP-ready on the input side.**
   - CI `test` job (`.github/workflows/ci.yml`): frontend `bun run test:coverage` (L187, Vitest v8 provider, reporters text/json/html/lcov → `coverage/lcov.info`); backend `cargo llvm-cov nextest --workspace --no-fail-fast --lcov --output-path lcov.info` (L199, run from `src-tauri/` → `src-tauri/lcov.info`); both uploaded to Codecov (L204–215).
   - Local mirror: `bun run coverage:check|coverage:diff` → `scripts/check-diff-coverage.mjs`; steps 6–7 + 11 of `scripts/pre-commit.js` regenerate both lcov files.
   - `codecov.yml`: patch ≥ 80%, project auto, flags `frontend` (src/) and `backend` (src-tauri/src, crates/*/src).
2. **Complexity analysis is effectively absent.**
   - Rust: no `[lints]`/`[workspace.lints]` anywhere; no crate-level lint attributes. Sole complexity config: `src-tauri/.clippy.toml` with `cognitive-complexity-threshold = 30` — dormant because (a) `clippy::cognitive_complexity` is a nursery lint nothing enables, and (b) the file lives in `src-tauri/`, so `mini-diarium-core` and `mini-diarium-crypto` never inherit it.
   - TypeScript: `eslint.config.js` has only `@eslint/js` recommended + `typescript-eslint` recommended + `eslint-plugin-solid`. No `complexity`, `max-depth`, `max-lines-per-function`, or `sonarjs` rules.
   - No lizard / rust-code-analysis / CRAP tooling anywhere in configs, scripts, or workflows.
3. **SonarCloud cannot be the CRAP provider** (live API queries, 2026-10-10):
   - `api/metrics/search` returns 165 metrics; **none** is CRAP or combines complexity with coverage per function.
   - Project measures: `complexity` = 7397, `cognitive_complexity` = 2679, `functions` = 5099, `ncloc` = 61040, `duplicated_lines_density` = 2.6; **`coverage` metric is absent entirely** (no coverage report imported into Sonar).
   - Component-tree queries for `.rs`, `crates`, `src-tauri` all return `total: 0` → **SonarCloud does not analyze the Rust code at all** (TS/JS only, 368 files indexed).
   - Top-complexity files (useful initial refactor targets): `scripts/check-diff-coverage.mjs` (CC 138 / cognitive 170), `src/components/editor/DiaryEditor.tsx` (121/71), `scripts/generate-website-docs.mjs` (121/72), `src/components/editor/EditorToolbar.tsx` (108/65), `src/components/backups/BackupsPanel.tsx` (89/58), `src/components/layout/JournalPicker.tsx` (74), `src/components/editor/useEntryLifecycle.ts` (72), `Calendar.tsx` (83). (File-level, not per-function; test files excluded from this list.)
4. **Toolchain facts**: `rust-toolchain.toml` pins channel **1.95** (≥ cargo-crap's `rust_version` 1.88 ✓). Local tools are `cargo-llvm-cov` 0.9.0 and `cargo-nextest` 0.9.138. CI selects stable Rust and restores separately cached tools; captured Linux provenance reports rustc 1.95.0, cargo-llvm-cov 0.8.7, and nextest 0.9.130. Do not assume local versions are the CI versions.
5. **`scripts/check-diff-coverage.mjs` parses only `SF:`/`DA:`/`end_of_record`** LCOV records (L68–78) — it ignores `FN`/`FNDA`. This does not block CRAP (see tool mechanics below) but bounds what the existing diff gate can do.

### Tooling landscape (verified maturity as of 2026-10-10)

| Tool | Language | Metric | Status | Notes |
|---|---|---|---|---|
| **cargo-crap** (minikin) | Rust | CRAP (cyclomatic via `syn`) | v0.6.1, published 2026-09-29, MIT, 418★, very active | **CLI verified locally** (`cargo crap --help`): `--workspace` (walks all members via `cargo metadata`, one LCOV), `--lcov`, `--threshold` (default 30), `--fail-above`, `--baseline <json> --fail-regression` (ratchet), `--epsilon`, `--missing pessimistic\|optimistic\|skip` (functions with no coverage data), `--exclude`/`--allow` globs, `.cargo-crap.toml` config file, formats: `human`/`json`/`github` (workflow `::warning` annotations)/`markdown`/`pr-comment`/`sarif` (Code Scanning; incompatible with `--baseline`)/`shields`; `--sort file` for stable committed baselines; default-excludes `tests/**`, `benches/**`, `examples/**`; MSRV 1.88 |
| **crap4rs** (breezy-bays-labs) | Rust | CRAP (cognitive by default) | v0.6.2, published 2026-06-24, MIT/Apache (early versions GPL-3.0), 6★, ~1k downloads | Rich outputs (sarif, github-annotations, markdown), `crap.toml`, baseline/delta gate; weak adoption; workspace support unverified |
| **rust-code-analysis-cli** (Mozilla) | multi | cyclomatic, cognitive, Halstead, … | v0.0.25, last published **2023-01-13** | General metrics tree (per-function spaces), not a CRAP tool; stale releases |
| **ESLint `complexity` rule** | TS/JS | cyclomatic (McCabe "classic") | stable, bundled | `["error"|"warn", { max: N, variant: "classic" }]`; default max 20 |
| **lizard** | multi (incl. Rust, TS/TSX) | CCN, NLOC, params (+`-Ecognitive`) | mature | **Does not compute CRAP** (no coverage join); reporting only |
| **crap4ts** | TS | CRAP | small project, input formats/CI support unverified | Not gate-ready |
| **Clippy** | Rust | `cognitive_complexity` (nursery, knob `cognitive-complexity-threshold`, default 25) | stable | Complexity lint only — never CRAP (no coverage input). No current cyclomatic-complexity lint confirmed in the clippy configuration reference |

**Mechanics note (cargo-crap)**: complexity comes from parsing source with `syn`; the LCOV supplies coverage. Function line-ranges are matched against lcov line-hit data, with path normalization in the merge step (per its design doc). Known failure mode: path mismatch → table full of 0% (documented in its troubleshooting guide). This is why the measured verification run below matters for our three-crate workspace layout where `--workspace` lcov lands in `src-tauri/lcov.info`.

## Options Compared

### Rust CRAP provider

| Option | How it works | Pros | Cons | Fit |
|---|---|---|---|---|
| A. cargo-crap | New CI step after `cargo llvm-cov`; reads existing lcov | Exact Crap4J formula w/ cyclomatic; workspace-aware; regression-baseline gating; active; MSRV satisfied | Young project (0.x); single maintainer; path-matching risk with 3-crate layout | **Recommended** |
| B. crap4rs | Same shape | More output formats incl. SARIF/annotations; config file | Cognitive-by-default (we want cyclomatic); GPL history; ~no adoption; workspace unverified | Fallback |
| C. Clippy-only (enable `cognitive_complexity`) | `[workspace.lints]` + existing clippy CI step | Zero new tools; enforced by `-D warnings` | No coverage input → no CRAP; cognitive ≠ cyclomatic | Complement, not substitute |
| D. SonarCloud | — | — | **Rust not analyzed at all**; no CRAP metric exists in SonarCloud (verified via API) | Not viable |

### TypeScript complexity/CRAP provider

| Option | How it works | Pros | Cons | Fit |
|---|---|---|---|---|
| A. ESLint `complexity` rule | Add rule to `eslint.config.js`; runs in existing `bun run lint` (CI L56, pre-commit step 2) | Zero new deps; McCabe cyclomatic; enforced everywhere lint runs | No CRAP (no coverage join); file-run granularity only | **Recommended** |
| B. crap4ts | Separate CLI | Real TS CRAP | Unverified formats/CI support; tiny project | Defer / spike only |
| C. SonarCloud complexity metrics | Already computed (per-function in Sonar UI) | Free | No CRAP metric; gate only via quality-gate conditions on `new_*` complexity; no local mirror | Keep as-is, informational |
| D. Custom join script (vitest json coverage FN records × ESLint/lizard CCN) | New `scripts/` tool | Full control | Build+maintain a metrics tool ourselves; against incremental-dev principle | Only if TS CRAP becomes a hard requirement |

### Gating strategy

| Option | How it works | Pros | Cons | Fit |
|---|---|---|---|---|
| A. Absolute gate (`--fail-above 30` on everything) | Fail on any function CRAP > 30 | Strong | Blocks on day one for unknown legacy debt; contradicts patch-scoped culture (Codecov patch≥80%, Sonar `new_*`) | Not initially |
| B. **Regression baseline** (`--fail-regression` + saved baseline) | Baseline committed; fail when existing functions regress or (optionally) when new functions exceed threshold | Matches repo's diff-scoped philosophy; ratchet-able | Baseline file needs maintenance | **Recommended** |
| C. Advisory only (JSON artifact, no fail) | Report in CI, never block | Zero risk | No teeth; likely ignored | First 1–2 weeks only |

## Proposed design (phased)

> **Revised after second opinion (2026-10-10).** The phase sketches below are the original design; the binding corrections are in **"Design revisions adopted from the review"** near the end of this document (delta-JSON policy gate instead of raw flags, Linux canonical baseline with normalized paths, Phase 0 acceptance checks, `[lints] workspace = true` wiring, closure blind-spot disclosure). Read both sections together.

### Phase 0 — Advisory baseline (no gating)

- Install `cargo-crap` in CI `test` job (cached like nextest/llvm-cov) and locally. Pin the exact version (`cargo install cargo-crap --version 0.6.1 --locked`).
- Run `cargo crap --workspace --lcov <lcov> --format json --sort file --output crap-baseline.json` against the already-generated `src-tauri/lcov.info`; upload the human/markdown report as a CI artifact; **no failure flags**.
- Sanity-check path matching: a report full of 0% coverage means `SF:` paths did not match walked sources (cargo-crap's documented failure mode) — try `--missing skip` vs `pessimistic` to distinguish "no data" from "untested".
- Commit `crap-baseline.json` (stable ordering via `--sort file`) so Phase 1 can gate regressions against it.
- Exit criteria: coverage join verified, legacy CRAP > 30 count recorded, baseline committed.

#### Phase 0 implementation evidence (2026-10-10)

Disposable Linux acceptance run [38072720599](https://github.com/fjrevoredo/mini-diarium/actions/runs/38072720599), draft PR #328 (NEVER MERGE), passed:

- Two freshly generated coverage inputs yielded identical sets of 645 function identities and maximum absolute CRAP difference **0**, within epsilon 0.01.
- The closure probe scored CC 1 despite four branches inside its closure. Its coverage was 57.14285714285714% and CRAP 1.0787172011661808: closure lines affect coverage, but closure branches do not increase the enclosing function's CC.
- A macOS-only function in the same matched source file scored 100% coverage and CRAP 3 on Linux despite not compiling. The mixed covered/uncovered control scored CC 4, coverage 55.55555555555556%, CRAP 5.404663923182442.
- Coverage paths matched 119 of 124 analyzed source files. Unmatched: two test-only files, build.rs, and macOS/Windows WebView sources. One coverage-only import/mod.rs has no scored production functions. No unexplained all-zero coverage table occurred.
- Six confirmed test-only files contributed helper functions: config/tests.rs, fonts/tests.rs, and the core entries, images, markdown, and rhai_loader test_support.rs files. Their parent module declarations use `#[cfg(test)]`. Add `--exclude '**/tests.rs' --exclude '**/test_support.rs'`, while retaining default exclusions, before canonical baseline production. The final Linux control below validates this scope.
- Direct comparison of absolute current paths to a normalized relative baseline falsely marked seven repeated-name identities as new (two SecretBytes::eq, three platform init, two trigger_auto_lock). Baseline paths must be aligned before cargo-crap computes the delta. A disposable Windows control restored the current root to an input copy and matched all 643 identities unchanged; the final Linux control below confirms the approach. No gate script or canonical baseline exists yet.
- Analysis took 0 whole seconds; the cold pinned install took 52 seconds. Both Codecov uploads and both report-artifact uploads succeeded. The runner used rustc 1.95.0, cargo-crap 0.6.1, cargo-llvm-cov 0.8.7, and nextest 0.9.130, not the locally installed coverage-tool versions.

Failed-test control [38074581034](https://github.com/fjrevoredo/mini-diarium/actions/runs/38074581034) passed its acceptance condition: only the deliberate assertion failed; both Codecov steps attempted uploads and reported absent coverage files; backend coverage skipped; CRAP explicitly skipped analysis; artifact upload succeeded with provenance only and the disposable evidence upload tolerated absent files. The original advisory branch does not include the independent reportOnFailure change, so no reports on this failure path are expected.

Final scoped control [38074856678](https://github.com/fjrevoredo/mini-diarium/actions/runs/38074856678) passed. It retained identical sets of 622 functions across two fresh coverage runs, with maximum absolute CRAP delta **0.0007656249999996589**, below epsilon 0.01. Only create_snapshot varied: CRAP 7 to 7.000765625, coverage 100% to 97.5%; its uncovered line was the manifest-save warning at backup/mod.rs:166. No test-helper entries remained. Paths matched 115/118 sources; build.rs and the inactive macOS/Windows WebView sources explain the three source-only files. Four excluded helper files plus import/mod.rs explain five coverage-only files. An input copy of the relative baseline with the current producer root restored matched all 622 functions unchanged and zero removed. The direct relative-baseline comparison still produced seven false new identities. Both Codecov uploads, both report uploads, and normalizer self-tests passed.

PR #328 is closed without merging, and its clean disposable worktree plus local/remote scratch branch were removed after evidence was saved. Advisory PR #327 inherited a SonarCloud PATH finding from earlier runner-infrastructure commit 5a82c70, not from CRAP code. The user authorized its separate fix (primary d7dcfda, advisory 92488d0): an absolute Git executable with a documented trusted override and no PATH fallback. SonarCloud passed without dismissal or gate changes. The user-requested self-check then fixed ambiguous drive-relative report paths (primary d7b8dfc, advisory 926d30f) and added direct normalizer CLI controls. All primary local checks passed (1272 frontend, 937 backend, 67 script tests; patch coverage 95.8% at the unchanged 80% threshold), and replacement PR CI 38097918038 plus every external check passed. Its probe-free Linux report retains 619 functions, 17 above CRAP 30, and no test-helper entries, with unchanged non-path values through normalization.

The user approved the advisory merge after the self-check. PR #327 merged at 2026-10-11T00:44:08Z as 8a345d70abe8fb5c3aba842150f5b0c2af5a66d9, without a check bypass. All four probe commits still return exactly 1 from ancestor checks against freshly fetched post-merge origin/master. Master CI run 38099433982 is active on that merge SHA. Its report verification, the dedicated canonical baseline change, and the separate M1-to-M2 approval remain pending. Independent lint, Vitest, and auth commits in the primary worktree were not part of this advisory merge.

### Phase 1 — Rust CRAP regression gate

- CI: after the `cargo llvm-cov` step, run `cargo crap --workspace --lcov src-tauri/lcov.info --baseline crap-baseline.json --fail-regression` (add `--format github` for inline PR annotations, or `--format pr-comment` piped into a PR comment step).
- Policy for *new* functions (no baseline entry): verify in Phase 0 whether `--fail-regression` trips on them; if not (web research says new functions are reported as `New` and do not trip the gate), add `--fail-above` only if the team wants new code held to CRAP ≤ 30 immediately — that is the stricter, recommended end-state, introduced after the advisory period.
- Local mirror: new step in `scripts/pre-commit.js` after step 7 (lcov already exists at that point), following the same "skip cleanly if inputs missing" pattern as step 11.
- Baseline updates require an explicit commit — same ratchet pattern as the vitest thresholds comment ("raise them as coverage improves").
- Note: `--format sarif` exists for GitHub Code Scanning but is incompatible with `--baseline`; pick one lane (recommend baseline+annotations first, SARIF later if desired).

### Phase 2 — Cyclomatic complexity lints (both languages)

- **TS**: add `complexity` rule to `eslint.config.js` — start `["warn", { max: 15, variant: "classic" }]`; measure warning count against the SonarCloud top-file list (DiaryEditor.tsx, EditorToolbar.tsx, BackupsPanel.tsx are the expected hot spots); promote to `error` per-file or globally once clean. Consider excluding `**/*.test.tsx` (6 of Sonar's top-10 complexity files are tests — gating tests on CC adds friction with little risk reduction; cargo-crap already excludes test code on the Rust side).
- **Rust**: move clippy config to the **workspace root** (`clippy.toml`) so all three crates inherit `cognitive-complexity-threshold = 30`, and actually enable the lint via `[workspace.lints.clippy]` in the root `Cargo.toml` (nursery lints need explicit opt-in). Existing CI (`cargo clippy --workspace --all-targets -- -D warnings`) then enforces it with no workflow change. Keep `msrv` aligned with `rust-toolchain.toml` (1.95) rather than the stale 1.75.
- Delete or keep `src-tauri/.clippy.toml` as a pointer comment; avoid two competing config files.

### Phase 3 — Optional extensions (decide later)

- TS CRAP spike (crap4ts or custom join of vitest JSON function coverage × ESLint CCN) **only if** the team wants symmetric CRAP gating; current recommendation is no — ESLint CC + Codecov patch coverage already approximate the two CRAP inputs for TS, and Sonar tracks per-function complexity.
- Lizard cross-language complexity heatmap as a reporting-only CI artifact (no gate).
- Import lcov into SonarCloud (`sonar.javascript.lcov.reportPaths` / `sonar.coverageReportPaths`) so the Sonar `coverage` metric stops being absent — orthogonal but cheap once lcov files exist. (Sonar's Rust blindness remains; that would need the sonar-rust plugin/commercial analyzer — out of scope.)

### Docs & conventions to update when this ships

- `docs/best-practices/CI_BEST_PRACTICES.md` (new gate section), `ci-gate-diagnosis` skill (new failure recipes: cargo-crap exit 1, path-mismatch 0% tables), root `CLAUDE.md` gotchas + verification commands, PR template if a new local gate step is added.

## Measured verification results

_(Local verification run of 2026-10-10 in worktree `worktree-silver-meadow-f82f`; backend: cargo-llvm-cov 0.9.0 + nextest, 910/910 tests passed; cargo-crap 0.6.1 installed from source, 2m19s.)_

- **Backend lcov (`src-tauri/lcov.info`)**: **PASS** — contains full function-level records: 119 `SF:` blocks (absolute Windows paths spanning all three crates), 4,607 `FN` + 4,607 `FNDA` records, 22,567 `DA` records, plus `BRF`/`BRDA` branch data. File size ≈ 1.2 MB.
- **cargo-crap path matching & coverage join**: **PASS** — `cargo crap --workspace --lcov src-tauri/lcov.info` matched 118 of 122 analyzed files against 119 LCOV entries. The 4 unmatched sources are benign and expected: `crates/mini-diarium-core/src/config/tests.rs`, `src-tauri/src/commands/fonts/tests.rs` (test-only files), `src-tauri/build.rs`, `src-tauri/src/webview_security/macos.rs` (not compiled on Windows). One LCOV entry (`crates/mini-diarium-core/src/import/mod.rs`) matched no analyzed source. Coverage percentages are non-trivial (e.g. 45–51% on partially tested functions), so the join is real, not the documented 0%-table failure mode. The scope-mismatch warning goes to stderr; exit code stays 0 without fail flags.
- **Baseline CRAP census (threshold 30)**: **630 functions analyzed → 18 crappy (2.9%)**:
  - `mini-diarium` (app crate): 237 functions, **14 crappy**
  - `mini-diarium-core`: 362 functions, **4 crappy**
  - `mini-diarium-crypto`: 31 functions, **0 crappy**
  - Worst offenders (CRAP / CC / coverage): `build_menu` 600/24/0% (src-tauri/src/menu.rs:27), `run_export_plugin` 182/13/0% (commands/plugin.rs:92), `set_require_all_auth` 156/12/0% (commands/auth/auth_policy.rs:13), `perform_unlock` 156/28/45.3% (commands/auth/auth_core.rs:152), `migrate_journal_plugins` 132/11/0% (core plugin/rhai_loader/mod.rs:45), `unlock_diary_auto` 110/10/0%, `generate_debug_dump` 90/9/0%, `change_password` 72/8/0%, `run_import_plugin` 72/8/0%, `remove_auth_method_inner` 56.2/18/51%, `create_diary_auto` 56/7/0%, `screen_lock_subclass_proc` 56/7/0%.
  - Pattern: nearly all top offenders are **Tauri command-layer functions** with 0% coverage — direct confirmation of the known "no Tauri integration tests" debt (root `CLAUDE.md`). An absolute `--fail-above` gate would fail on these 18 legacy functions on day one → the **baseline-ratchet strategy is empirically justified**, not just philosophical.
  - JSON baseline artifact written to `%TEMP%\opencode\crap-baseline.json` (`--format json --sort file`), exit 0 — the committed-baseline mechanics work on Windows.
- **Frontend lcov (`coverage/lcov.info`, Vitest v8 provider)**: **PASS** — contains function-level records: 131 `SF:` blocks (relative paths, e.g. `src\App.tsx`), **2,581 `FN` + 2,581 `FNDA`**, 5,891 `DA`, 2,335 `BRDA` branch records (≈208 KB). Suite totals: statements 86.5%, branches 75.2%, functions 88.45%, lines 87.98%. A future custom TS CRAP join (FN/FNDA × per-function CCN) is therefore mechanically feasible; anonymous functions appear as `(anonymous_N)` names, which is the main wrinkle such a join would need to handle.
  - **Incidental finding (CI gotcha worth fixing regardless)**: when any frontend test fails, `coverage/lcov.info` is silently not written. In CI the job fails anyway, but locally `bun run coverage:diff` then gates against a stale/missing lcov. Setting `coverage.reportOnFailure: true` in `vitest.config.ts` (or passing the flag in the npm script) would make the local mirror honest.
  - Test-suite state on a fresh worktree: 1241/1242 pass; the single failure is `src/test/prosemirror-dedup.test.ts` ("resolves to exactly one installed copy") timing out at 5 s while recursively scanning a freshly installed `node_modules` — environmental (slow first scan on Windows), not a code regression. Candidate fix: raise that test's timeout.
- Toolchain compatibility: **PASS** — `rust-toolchain.toml` channel 1.95 ≥ cargo-crap MSRV 1.88.
- **Clippy `cognitive_complexity` blast radius**: **MEASURED — negligible.** `cargo clippy --workspace --all-targets -- -A clippy::all -W clippy::cognitive_complexity` produced exactly **1 warning**: `crates/mini-diarium-core/src/import/jrnl.rs:342` — "cognitive complexity of (26/25)". The app crate (which does see `src-tauri/.clippy.toml`'s threshold 30) produced none; core/crypto ran at clippy's default 25, which is why the single warning shows `26/25`. **Conclusion**: a workspace-root `clippy.toml` with `cognitive-complexity-threshold = 30` + enabling the lint would pass the existing `-D warnings` CI gate with **zero legacy violations today**. Phase 2 carries no cleanup debt.
- SonarCloud: **CLOSED** — no CRAP metric (165 metrics scanned via `api/metrics/search`), Rust not analyzed (`q=.rs`/`crates`/`src-tauri` → total 0), `coverage` metric absent project-wide; complexity (7397) / cognitive_complexity (2679) available for TS only (368 files). Top TS complexity files: `scripts/check-diff-coverage.mjs` (138/170), `DiaryEditor.tsx` (121/71), `generate-website-docs.mjs` (121/72), `EditorToolbar.tsx` (108/65), `BackupsPanel.tsx` (89/58).

## Risks & Unknowns

- **Risk**: cargo-crap is a 0.x, single-maintainer project. *Mitigation*: advisory phase first; pin exact version in CI; exit-code contract is simple enough to replace with crap4rs or a small script if abandoned.
- **Risk**: lcov path mismatch across the 3-crate workspace (`SF:` paths vs. `crates/*/src` layout) — cargo-crap's documented failure mode. **RESOLVED by measurement**: 118/122 files matched; the 4 misses are benign (test-only files, `build.rs`, `macos.rs` on Windows). Keep the Phase 0 sanity check for CI's Linux paths, which differ in shape from Windows ones.
- **Risk**: enabling `clippy::cognitive_complexity` workspace-wide may produce a burst of warnings in legacy code. **RESOLVED by measurement**: exactly one function exceeds the default 25 (jrnl.rs at 26); none exceeds 30. Start `warn`, promote to `deny` immediately — no debt.
- **Unknown → RESOLVED (reviewer read cargo-crap 0.6.1 source)**: `--fail-regression` does **not** trip on brand-new functions absent from the baseline, and `--fail-above` applies to *all* functions including legacy ones — so native flags cannot express "new ≤ 30, existing no-regress." Consequence: Phase 1 uses a delta-JSON policy check (see "Design revisions adopted from the review").
- **Resolved by measurement**: frontend lcov FN/FNDA presence (PASS, 2,581 records); cargo-crap path matching on the 3-crate workspace (PASS, 118/122); clippy cognitive_complexity blast radius (1 warning at default 25, 0 at threshold 30).
- **Investigation suggested (outside exploration mode)**: none blocking — Phase 0 *is* the investigation for the remaining unknowns.

## Recommended Direction

Adopt **cargo-crap for Rust CRAP** (Phase 0 advisory with acceptance checks → Phase 1 **delta-JSON policy gate**: new functions ≤ 30, existing functions no regression vs. a reviewed, normalized, **Linux-produced** baseline), **ESLint `complexity` for TS cyclomatic** (test files exempt from that rule only) and **workspace-level clippy `cognitive_complexity`** for Rust complexity lints (Phase 2, corrected wiring: root `clippy.toml` + `[workspace.lints.clippy]` + `[lints] workspace = true` per member + delete `src-tauri/.clippy.toml`), and explicitly **reject SonarCloud as a CRAP provider** (verified: no such metric, no Rust analysis). Gating philosophy follows the repo's existing patch-scoped culture: never block on legacy debt, ratchet via reviewed baseline updates only, and disclose the metric's blind spots (closures, macros, inactive code). Coding-agent second opinion: **agree-with-changes**; all changes adopted.

## Next Steps

- [x] Second opinion from coding agent (tab wK:t4) — received, verdict agree-with-changes, all changes adopted (see below)
- [x] Review this proposal; decide Phase 2 thresholds and the Clippy MSRV contract (recorded in the implementation plan and decisions)
- [x] Capture and approve the implementation plan via `manual-planning`
- [x] Implement advisory reporting, normalization, Linux acceptance, and independent complexity lints
- [x] Finish the requested pre-merge self-check and merge advisory PR #327 with user approval
- [ ] Produce and review the canonical baseline from post-merge master Linux CI
- [ ] Obtain separate Task 1.6 approval before any blocking-gate implementation

## Second opinion (coding agent, Herdr tab wK:t4)

**Received 2026-10-10** via `herdr agent prompt` → pane `wK:p4` (opencode agent, cwd `D:\Repos\mini-diarium`, model GPT-6.1 Sol). The reviewer read the proposal, the current CI/local gate files, the measured baseline JSON, **and the installed cargo-crap 0.6.1 source**. Verdict: **agree-with-changes** — advisory phase fits the repo; the blocking gate needs changes before Phase 1. No files modified on their side.

### Reviewer's critique (distilled, numbered as in their reply)

1. **Gate semantics**: baseline comparison is *not* patch-scoped — `--fail-regression` fails on any score increase above epsilon (including sub-30 scores and untouched functions). **Source-verified: new functions do NOT trip `--fail-regression`**, and adding `--fail-above` checks *all* functions (the 18 legacy offenders), so native flags cannot express "new ≤ 30, existing must not regress." Recommends a small policy check over the delta JSON, keeping metric calculation in cargo-crap.
2. **Baseline portability**: the measured JSON contains **absolute Windows worktree paths**; `--sort file` stabilizes order but not paths/line numbers. Windows and Linux also **compile different code** (platform cfg), so a Windows coverage baseline is not a reliable Linux gate baseline. Linux CI must be the canonical baseline producer; normalize committed paths to repo-relative form.
3. **Scoring blind spots (from 0.6.1 source)**: cargo-crap **does not score closures**, does not expand macro bodies, and scans source without platform/feature filtering. Critically for this repo's `with_unlocked_db(..., |db| ...)` command pattern, much real logic lives in closures and would be invisible. Also: a matched file whose function span has no instrumented lines scores **100% coverage** — inactive code can look healthy.
4. **Ops details**: cache the pinned binary keyed on exact version+OS+arch; measure the extra pass's elapsed time in Phase 0 (it parses, doesn't compile — should be cheap); locally require *fresh* coverage generation, not just lcov file existence (stale-lcov risk).
5. **SonarCloud/TS**: agrees with rejecting Sonar as CRAP provider and deferring TS CRAP, but notes ESLint CC + Codecov are *independent controls, not an equivalent* per-function CRAP check; Sonar's file-level totals can't calibrate an ESLint function threshold (measure ESLint violations directly); Sonar coverage import is separate work needing scanner access to the lcov, not just a property.
6. **Repo-policy corrections**: `[workspace.lints.clippy]` alone does nothing — each member crate needs `[lints] workspace = true`; **delete** `src-tauri/.clippy.toml` when moving config (no empty pointer file that could take precedence); clippy's `msrv` means *minimum supported Rust*, not the pinned toolchain — decide that contract deliberately, don't copy 1.95; a committed baseline is a ceiling, not an automatic ratchet — **a PR must never raise its own baseline to pass**; prefer `--format github` annotations/artifacts over PR comments (preserves `contents: read`); detailed rules go in `docs/best-practices/CI_BEST_PRACTICES.md` + `.agents/skills/`, short pointers in `CLAUDE.md`.
7. **Thresholds**: CRAP 30 fine for reporting; keep default try-weight and explicit `--epsilon 0.01`; evaluate run-to-run repeatability before blocking. ESLint classic max 15 warn is a reasonable advisory start, but current `bun run lint` has **no `--max-warnings 0`**, so warnings are non-blocking — document that temporary exception. Exclude `*.test.{ts,tsx}`/`*.spec.{ts,tsx}` from the complexity rule *only* (keep their other lints); cargo-crap's built-in test filtering is incomplete — exclude Rust test-helper files explicitly where needed. Keep `--missing pessimistic` in CI; `skip` is for diagnosis only, never to make CI pass.

**Reviewer's top 3 changes**: (1) define the new-code/regression policy correctly — don't combine native flags as a substitute; (2) produce a normalized, reviewed **Linux** baseline with clear update/freshness rules; (3) add Phase 0 acceptance checks for closures, inactive code, and coverage matching before enabling any blocking failure.

### Research agent's reactions (point-by-point)

- **Accept 1 (gate semantics)** — this is the most important correction. Our Phase 1 flag combination (`--fail-regression` [+ optional `--fail-above`]) cannot express the intended patch-scoped policy. Revised design below adds a small delta-JSON policy check (fits naturally beside `scripts/check-diff-coverage.mjs`'s existing philosophy). Their source-reading also **closes our last open unknown** (new functions don't trip `--fail-regression`).
- **Accept 2 (baseline portability)** — confirmed by our own measurement: the temp baseline JSON holds `C:\Users\Francisco\.herdr\...` absolute paths. Linux CI as canonical producer + path normalization is mandatory, not optional.
- **Accept 3 (closure blind spot)** — significant for this codebase: the Tauri command layer's `with_unlocked_db(|db| ...)` pattern means closure bodies carry real logic that cargo-crap won't score. This doesn't invalidate the gate (functions are still scored, and the 0%-coverage command wrappers dominated our measured top-12), but it bounds what CRAP covers here. Must be stated in the docs so nobody over-trusts a green CRAP gate.
- **Accept 4–7** — all concrete, verifiable, and consistent with repo conventions we documented (patch-scoped culture, `contents: read` CI permissions, docs-maintenance hierarchy). The "PR must never raise its own baseline" rule is adopted verbatim.
- **No pushbacks.** The critique changed the design in three places (Phase 0 acceptance checks, Phase 1 gate mechanism, Phase 2 lint wiring); revisions are folded into the phase descriptions above and summarized here:

### Design revisions adopted from the review

1. **Phase 0 gains explicit acceptance checks** before any blocking gate: (a) verify closure/inactive-code behavior on a scratch function with a closure and a `#[cfg]`-dead branch; (b) confirm Linux CI lcov ↔ source path matching (re-run our Windows validation on the Linux runner); (c) measure added wall-clock; (d) check score repeatability across two identical runs (epsilon sensitivity); (e) Linux CI produces the canonical baseline JSON with **repo-relative paths** (normalization step or post-processing, reviewed before commit).
2. **Phase 1 gate = delta-JSON policy script, not raw flags**: consume cargo-crap `--format json --baseline` output and enforce exactly: *new functions must be ≤ 30; existing functions must not regress beyond epsilon 0.01*. Use `--format github` for annotations + artifact upload (no PR-comment step, preserving `contents: read`). Baseline updates only via reviewed, dedicated commits — never inside a feature PR that needs them to pass. Local pre-commit mirror must require freshly generated lcov in the same run.
3. **Phase 2 wiring corrected**: root `clippy.toml` (threshold 30, deliberate `msrv` decision) + `[workspace.lints.clippy]` in root `Cargo.toml` + **`[lints] workspace = true` in all three member crates** + **delete** `src-tauri/.clippy.toml`. ESLint: `complexity` rule with `**/*.test.{ts,tsx}`/`**/*.spec.{ts,tsx}` overrides turning only that rule off; document that `warn` is intentionally non-blocking until promotion to `error` (no `--max-warnings 0` today).
4. **Honesty note added to docs plan**: CRAP here does not see closure bodies or macro expansions — state this in `CI_BEST_PRACTICES.md` and the `ci-gate-diagnosis` skill so the gate's scope is not over-trusted.

### Scope agreement round (same reviewer, 2026-10-10, second exchange)

The reviewer answered all 8 design questions and returned **scope-needs-changes**; every change was adopted, reaching agreement on the final scope. Resolutions:

1. **Baseline location**: `ci/crap-baseline.json` accepted; keep run reports as artifacts, not tracked files; document Linux runner + tool versions + coverage config as provenance.
2. **Policy script**: Node `.mjs` agreed; small — validates delta JSON and enforces policy, never computes metrics; missing/invalid input must FAIL in CI; local optional skips belong in the caller; distinct exit codes for policy failure vs input/tool failure.
3. **Clippy `msrv`**: drop the key entirely (neither 1.75 nor 1.95 establishes a supported minimum); document as removing an obsolete setting, not declaring an MSRV.
4. **Lint level**: `warn` in workspace lints — existing `-D warnings` already makes it blocking; explicit `deny` adds nothing.
5. **Legacy TODOs**: no per-offender TODOs (metrics flag review candidates, not proven defects); optionally one separate follow-up TODO for command-layer testability/coverage via todo-manager, only after approval; do not prescribe menu refactoring from CRAP 600 alone.
6. **Incidental fixes**: `coverage.reportOnFailure: true` included (own scoped commit); `prosemirror-dedup` timeout fix REMOVED from this plan (separate small fix after confirming cause).
7. **Normalization**: standalone script; self-tested for Windows+Linux roots, slash conversion, outside-repo paths, repeated function names; normalize diagnostics too; preserve schema/version/scores/identities; no rounding of gate inputs.
8. **Missing local cargo-crap**: skip + explicit "CRAP check skipped" notice, no auto-install; Windows-vs-Linux-baseline comparison stays **advisory** until cross-platform acceptance checks prove blocking results reliable.

Reviewer additions (all adopted): automated gate-script tests against the real published delta shape (new at/above 30, regression at/above epsilon, improvements, moved/renamed/ambiguous entries, malformed reports); freshness checks with twice-regenerated coverage for repeatability (same-lcov-twice only proves parsing determinism); explicit **M1→M2 user approval gate** before enabling blocking checks, scratch probes never entering the canonical baseline; artifact preservation (upload reports even on gate failure; never let CRAP failure block Codecov uploads); explicit wording that the regression rule is **whole-function repo-wide, stricter than patch-scoped, explicitly accepted**; baseline-maintenance rules (reviewed improvements/identity updates allowed, unexplained increases prohibited); explicit `--missing pessimistic` and `--epsilon 0.01`.

Reviewer removals/adjustments (all adopted): dedup-timeout fix out; local Windows step not described as a blocking mirror; annotations preferably emitted by the policy script from the same delta JSON (a second `--format github` pass acceptable but must not be confused with the gate verdict).

**Resulting plan**: `docs/plans/2026-10-10-crap-complexity-gates-plan.md` (manual-planning v2.0.0, four milestones). The current primary-worktree plan and companion decisions file are the execution ledger; its initial tracking warning was resolved when the plan was committed. Tasks 1.7 and 1.8 were added for the inherited security prerequisite and the user-requested full self-check. Merge review, a canonical master baseline, and separate blocking-gate approval remain pending.
