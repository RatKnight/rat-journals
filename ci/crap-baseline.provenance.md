# Canonical Rust CRAP baseline provenance

This baseline comes from the Linux master CI report below, not a local Windows
analysis or the disposable acceptance branch. Baseline changes require a dedicated
reviewed commit. No blocking CRAP check is enabled by this change.

## Producer record

The following record is copied unchanged from the report artifact's
`crap-report.provenance.txt`. Facts were captured when CI produced the report.

```text
UTC: 2026-10-11T02:08:44Z
Producer root: /home/runner/work/mini-diarium/mini-diarium
Source commit: f1bb84f73f44dd40028299682d0b6ef2f2326ca7
Event: push
Ref: refs/heads/master
Run ID: 38103562280
Run attempt: 1
Run URL: https://github.com/fjrevoredo/mini-diarium/actions/runs/38103562280
Runner: Linux / X64
rustc 1.95.0 (59807616e 2026-04-14)
cargo-crap 0.6.1
cargo-llvm-cov 0.8.7
cargo-nextest 0.9.130 (f0feb11a1 2026-03-09)
release: 0.9.130
commit-hash: f0feb11a16eab1a138b9d7a48d323b382e4c8b22
commit-date: 2026-03-09
host: x86_64-unknown-linux-gnu
Coverage outcome: success
Coverage working directory: src-tauri
Coverage command: cargo llvm-cov nextest --workspace --no-fail-fast --lcov --output-path lcov.info
Coverage configuration: default workspace features, default nextest profile
Analyzer command: cargo crap --workspace --lcov src-tauri/lcov.info --format json --sort file --missing pessimistic --epsilon 0.01 --exclude '**/tests.rs' --exclude '**/test_support.rs' --output crap-report.json
Analyzer exclusions: built-in tests/**, benches/**, examples/**; additional **/tests.rs, **/test_support.rs (confirmed test-only sources)
Analyzer configuration: no .cargo-crap.toml; default threshold 30 and try-weight 1
a316bfdb5247646a4a62a5278c32c5122b95de673f84f81c4c26baec790b359b  Cargo.lock
453ada78713dcc68c475aafa19049f19880f2bffbca780df49c6855ec8394fbf  rust-toolchain.toml
7cce8d55ccc40066a70df130a27fd4dd8781de046c84eb936e7e14f9f7be6761  src-tauri/lcov.info
```

## Normalization

Download the `crap-report` artifact from the recorded run. From this repository's
root, with both downloaded files in the current directory, run:

```sh
node scripts/normalize-crap-baseline.mjs --input crap-report.json --root /home/runner/work/mini-diarium/mini-diarium --for-baseline --output ci/crap-baseline.json
```

The normalizer changes only paths to repository-relative forward-slash paths. It
does not compute, round, or change scores, reorder entries, remove repeated names,
or apply analyzer exclusions. Outside-root paths cause baseline mode to fail.

SHA-256 checksums:

| File | SHA-256 |
| --- | --- |
| Downloaded `crap-report.json` | `62a0869bc00090d608793ccc8cc300faf088da9acd72175464f29ac7720faad9` |
| Downloaded `crap-report.provenance.txt` | `55700fb7e8e379df947f4ebfd87bc52377ca0b4a2a2289ba7bca50c3ebf6eb57` |
| Normalized `ci/crap-baseline.json` | `a382e3d17a46c710a043cd167994ab21e9cf5f95b40e164de4ae13976ebcc318` |

## Review

Review date: 2026-10-11. The integrated local checks passed. Master CI run
38103562280 completed successfully on the recorded source commit. All seven jobs
passed: Lint, Test, Linux/macOS/Windows builds, Flatpak, and E2E. The final E2E job
completed at `2026-10-11T02:27:26Z`. This validates the master source, not the
independent lint, Vitest, and auth changes on this branch.

- 624 production function entries: app 247, core 346, crypto 31. All 624 distinct
  source locations remain in order, with unchanged non-path values.
- 17 entries exceed CRAP 30. The worst are `build_menu` (600), `perform_unlock`
  (161.5964240102171), `set_require_all_auth` (156), and `migrate_journal_plugins`
  and `unlock_diary_auto` (132 each). These match the earlier Windows census.
- Compared with master merge `8a345d7`, the report adds five helpers and removes
  no function names: `check_db_filename`, `run_blocking_or_lock`, `lock_registry`,
  `list_import_plugins_inner`, and `list_export_plugins_inner`. These are the
  latest master filename and panic-recovery changes.
- The latest master replaces poisoned-registry error branches with
  `lock_registry`. This explains the lower scores for `generate_debug_dump`
  (90 to 72) and `run_import_plugin` (72 to 56) versus the earlier Windows census.
- Master's `create_diary_auto` still scores 72. The independent auth helper/tests
  on this branch were not part of master when CI produced this baseline. Do not
  substitute their Windows scores into this Linux report.
- Fresh Windows analysis after the master merge covers 625 functions. Of the 17
  Linux scores above 30, 16 exactly match the fresh Windows scores, complexity,
  and coverage. The one different score is the explained `create_diary_auto`
  helper extraction (Linux 72, Windows wrapper 2). This is a source difference,
  not an unexplained platform coverage mismatch. All 14 integrated local checks
  passed: 1,277 frontend, 950 backend, and 67 script tests; patch coverage is
  46/48 (95.8%) with the unchanged 80% threshold.
- Coverage matches 114 of 117 analyzed source files. The unmatched sources are
  `src-tauri/build.rs` and the macOS/Windows WebView sources. The five coverage-only
  files are four excluded `test_support.rs` files and `import/mod.rs`, which has
  no scored production function. These differences are explained, not hidden by
  normalization.
- No acceptance probe or test-helper function is scored. All four disposable
  probe commits remain absent from the source master ancestry.
- Repeated same-file names remain separate: `SecretBytes::eq` at lines 38 and 50,
  `screen_lock::init` at 27, 142, and 223, and `trigger_auto_lock` at 76 and 203.
  A future comparison must align baseline paths with the analyzer's current root
  before it calculates deltas. The canonical baseline remains relative.
- No unexplained mismatch was found in the reviewed highest scores or coverage
  diagnostics. Closure branches, macro expansion, and cfg-dead spans remain
  analyzer blind spots; this baseline does not remove those limits.
- The cached analyzer step took less than one whole second in this run; artifact
  upload took one second. The earlier measured cold install took 52 seconds.

The separate Task 1.6 approval is required before any blocking CRAP gate work.
