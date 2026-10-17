# GitHub CI Best Practices

Language-agnostic guidelines for GitHub Actions workflows. Applicable to any repo type.

---

## Triggers & Branch Strategy

- **CI runs on `push` + `pull_request` to `master`/`main` only** — avoids noisy runs on feature branches
- **Releases trigger on `v*.*.*` tags** — keeps the release pipeline separate from CI
- **Post-release automations trigger on `release: [published]`** — decouples store/registry publishing from the build pipeline. Use `[released]` instead to exclude pre-releases
- **Add `workflow_dispatch` to release automations** — enables manual re-runs without re-tagging

```yaml
# CI
on:
  push:
    branches: [master]
  pull_request:
    branches: [master]

# Release
on:
  push:
    tags:
      - 'v*.*.*'

# Post-release automation (e.g. publish to package registry)
on:
  release:
    types: [published]   # or [released] to exclude pre-releases
  workflow_dispatch:     # manual trigger as fallback
    inputs:
      tag:
        description: 'Release tag (e.g. v1.2.3)'
        required: true
```

---

## Permissions (Least Privilege)

- **Default `contents: read`** on every workflow — explicit minimum
- **Elevate only where needed**: `contents: write` + `pull-requests: write` only on release workflows
- **Never use write permissions globally** — scope elevated permissions to the job that needs them

```yaml
# Top of every workflow file
permissions:
  contents: read

# Override per-job only where needed
jobs:
  publish:
    permissions:
      contents: write
      pull-requests: write
```

---

## Concurrency Control

```yaml
concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: true   # CI: cancel superseded runs on the same branch
  # cancel-in-progress: false  # Release: never cancel mid-deploy
```

- **CI**: cancel in progress — fast feedback, no wasted runner time
- **Release**: never cancel — a partial release is worse than a slow one

---

## Job DAG (Dependency Ordering)

Structure jobs so expensive work is gated behind cheap, fast-failing work:

```
lint ──┐
       ├─→ build (per platform) ──→ e2e / integration tests
test ──┘
```

- **Gate builds behind lint + test** — catch cheap failures before spending minutes on platform builds
- **E2E / integration tests depend only on the artifact they need** — decouple from unrelated slow builds

```yaml
jobs:
  lint:
    ...
  test:
    ...
  build:
    needs: [lint, test]
    ...
  e2e:
    needs: build
    ...
```

---

## Caching Strategy

**Lock-file keyed cache for package managers:**

```yaml
- uses: actions/cache@v4
  with:
    path: ~/.npm
    key: ${{ runner.os }}-npm-${{ hashFiles('package-lock.json') }}
    restore-keys: ${{ runner.os }}-npm-
```

- **Key on the lockfile hash** — cache busts automatically when deps change
- **Add `restore-keys` fallback** — a partial cache hit is better than a cold start
- **Always install with a frozen/locked flag** to prevent silent dep drift:

```bash
npm ci                          # npm
yarn install --frozen-lockfile  # yarn
bun install --frozen-lockfile   # bun
pip install --require-hashes    # pip
```

**Caching external tools:**

```yaml
- uses: actions/cache@v4
  id: cache-my-tool
  with:
    path: ~/.local/bin/my-tool
    key: my-tool-v1.2.3-${{ runner.os }}  # bump this key when upgrading

- name: Install my-tool
  if: steps.cache-my-tool.outputs.cache-hit != 'true'
  run: <install command>
```

- **Pin tools to a specific version** in the cache key, with a comment to bump it on upgrade
- **Only install if cache missed** — skip install entirely on a hit
- **Disable caching for large release artifacts** (e.g. LTO/optimized builds) that are non-incrementally reusable
- **Pin `bunx`/`npx` invocations to an exact version** matching the lockfile (e.g. `bunx playwright@1.61.1 install`), not a bare package name. A bare name resolves to whatever the registry serves today, which SonarCloud flags as `githubactions:S8543` (installing unverified releases). Bump the pin alongside the corresponding dependency in `package.json`. **Bump the cache-key suffix in the same commit** — if only `package.json`/the lockfiles move, CI installs the previous release's browser build while the test runner resolved from `node_modules` expects the new revision, and the job fails with `Executable doesn't exist at .../chromium_headless_shell-<revision>` (the cache `restore-keys` partial-hit hides the staleness because the install step still runs on a suffix miss).
- **Never use `curl -L`/`--location` in CI downloads** (SonarCloud `githubactions:S6506`, former hotspot, flags every redirect-following `curl` invocation — and does **not** recognize `--proto-redir` as a mitigation). For GitHub release URLs (which 302 to a signed `release-assets.githubusercontent.com` URL), resolve the redirect manually with a HEAD request, assert the resolved URL is HTTPS, then GET it directly:

  ```bash
  URL="https://github.com/OWNER/REPO/releases/download/V/TARBALL"
  FINAL="$(curl -fsSI -o /dev/null -w '%{redirect_url}' "$URL")"
  case "$FINAL" in https://*) ;; *) echo "not HTTPS: $FINAL" >&2; exit 1 ;; esac
  curl -fsS -o /tmp/file "$FINAL"
  ```

---

## Dependency Management (Dependabot)

```yaml
version: 2

updates:
  - package-ecosystem: npm       # or pip, maven, gradle, go, etc.
    directory: /
    schedule:
      interval: weekly
      day: monday                # batch on a fixed day, not random noise
    open-pull-requests-limit: 10
    groups:
      dev-dependencies:
        dependency-type: development
      prod-dependencies:
        dependency-type: production

  - package-ecosystem: github-actions
    directory: /
    schedule:
      interval: weekly
      day: monday
    open-pull-requests-limit: 5
    groups:
      minor-and-patch:           # batch non-breaking updates together
        update-types:
          - minor
          - patch
```

- **Group minor + patch** (via `update-types`) — one PR for all non-breaking updates
- **Separate prod/dev into different groups** — different risk profiles, different review urgency
- **Set `open-pull-requests-limit`** — prevents a PR flood during ecosystem churn
- **Pin schedule to a specific day** — predictable, batched instead of continuous noise
- **Always include `github-actions`** as an ecosystem to keep action versions current

---

## Matrix Builds

```yaml
strategy:
  fail-fast: false    # a macOS flake shouldn't abort Windows
  matrix:
    include:
      - os: ubuntu-latest
        platform: linux
      - os: macos-latest
        platform: macos
      - os: windows-latest
        platform: windows
```

- **Use `fail-fast: false`** — one platform failure should not cancel others
- **Use explicit `include:` objects with named keys** — more readable than bare OS names
- **Gate platform-specific steps on named keys** (`if: matrix.platform == 'linux'`) rather than `runner.os`

---

## Artifact Passing Between Jobs

```yaml
# Producer job:
- uses: actions/upload-artifact@v4
  with:
    name: app-linux-binary
    path: dist/app
    if-no-files-found: error   # or 'ignore' for optional artifacts

# Consumer job (needs: producer):
- uses: actions/download-artifact@v4
  with:
    name: app-linux-binary
    path: dist/
- run: chmod +x dist/app  # restore executable bit — upload strips it
```

- **Upload the minimum needed** — binary only (not the full build dir) for fast inter-job handoff
- **`if-no-files-found: error`** for required artifacts; `ignore` for optional ones
- **Restore executable bit** after downloading binaries on Linux/macOS — it is stripped on upload

---

## Release Pipeline Safeguards

1. **Validate release notes before doing anything** — fail at `create-release`, not at publish
2. **Create release as `draft: true` first** — gate publishing behind an asset verification job
3. **Verify all expected assets exist before publishing** — explicit checklist per platform
4. **Generate SHA256 checksums** per artifact and upload them alongside the artifacts
5. **Automate post-publish cleanup** (changelog stubs, version files) via automated PR — never direct-commit to `master`

Release job order:
```
create-release (draft) → build-release (matrix) → publish-release (verify assets → publish)
```

**Validate release notes example:**

```yaml
- name: Validate release notes
  run: |
    if [ ! -f release-notes.md ] || ! grep -q '[^[:space:]]' release-notes.md; then
      echo "release-notes.md is missing or empty"
      exit 1
    fi
    if grep -q 'REPLACE BEFORE TAGGING' release-notes.md; then
      echo "release-notes.md still contains placeholder text"
      exit 1
    fi
```

---

## Secret Hygiene

- **Validate required secrets at job start** — fail with a clear message, not a cryptic 401/403

```yaml
- name: Verify token is configured
  run: |
    if [ -z "${{ secrets.MY_PUBLISH_TOKEN }}" ]; then
      echo "MY_PUBLISH_TOKEN is not set. Add it at Settings → Secrets → Actions."
      exit 1
    fi
```

- **Scope secrets to the minimum job** — publish tokens only in the publish job, not the build job
- **Use `github.token` for intra-repo operations** — only add PATs for cross-repo operations (e.g., pushing to another repo, updating a Homebrew tap)

---

## Linting in CI

Run all static checks in a **single `lint` job** to share toolchain and cache overhead:

- **Formatter** — `--check` mode only, never auto-fix in CI
- **Linter** — treat warnings as errors
- **Type checker**
- **Generated artifact verification** — lockfiles, generated code, diagrams

Rat Journal enables `clippy::cognitive_complexity` through the workspace lint table and `[lints] workspace = true` in all three crates. Root `clippy.toml` sets the threshold to 30. The lint is `warn`, but the existing `cargo clippy --workspace --all-targets -- -D warnings` command makes violations blocking. Do not add a crate-local Clippy config that hides the root config. Removing the obsolete `msrv = "1.75"` setting did not establish a new minimum supported Rust version.

```yaml
- name: Format check
  run: prettier --check .         # or: black --check, gofmt -l, etc.

- name: Lint
  run: eslint . --max-warnings 0  # or: flake8, golint-ci run, etc.

- name: Type check
  run: tsc --noEmit               # or equivalent

- name: Verify generated files are up-to-date
  run: |
    make generate
    git diff --exit-code          # fails if generated output changed
```

---

## Local Pre-commit Hook

CI is the safety net, but a fast local hook catches style violations on the developer's machine before they ever push. The hook must stay **fast** (sub-second-to-low-seconds, scoped to staged files only) so it doesn't slow the commit cycle. Anything heavier (type-check, lint, tests, coverage) belongs in CI or a manual `pre-commit` script, not in the hook.

In this repo, `.githooks/pre-commit` runs Prettier on staged `src/**/*.{ts,tsx,css}` files and `cargo fmt` on staged `src-tauri/**/*.rs` files. It is activated automatically by setting `git config core.hooksPath .githooks`, which `scripts/install-hooks.js` does as the `postinstall` step of `bun install`. Manual install: `bun run hooks:install`. Bypass for a single commit: `git commit --no-verify`.

The hook is a complement to CI, not a replacement: it runs `--write`/auto-fix locally, while CI uses `--check` to fail on drift.

---

## Test Runner Ownership

**Every test file must belong to a runner that a gate executes.** A test file that no runner collects never fails, so it reports nothing when the code it covers breaks. It looks like protection, but it gives none.

- **Map each test location to one runner** — and map each runner to the CI job and the local gate that execute it
- **Exclude a path from one runner only when another runner owns it** — an exclude (for example, of a tooling folder) is a common way for tests to become orphans
- **Fail the build on orphans** — a cheap guard lists the tracked test files and fails when a file matches no runner's include rules. Keep its rules next to the runner configs and change both together
- **Pass explicit file lists to a runner when its directory or glob handling differs across versions or shells** — a pattern that silently matches nothing is a passing gate with zero tests
- **Fail a runner that finds zero tests** — an empty run must not report success

For the Rat Journal ownership guard, select Git by an absolute executable path, not inherited PATH. The standard OS installation and explicit nonstandard-installation override are documented in [scripts/README.md](../../scripts/README.md#git-executable-for-the-ownership-guard). A missing selected executable must fail rather than fall back to PATH.

---

## Coverage Gating

Don't discover coverage failures only on CI. A global coverage threshold (e.g. "≥70% overall") catches gross regressions but misses the two checks coverage services actually enforce on PRs:

- **Patch coverage** — new/modified lines in the diff must be ≥ N% covered (typically 80%).
- **Project regression** — total coverage must not drop vs the base branch.

Both are computed from the diff against the base branch, so a suite that passes a global floor can still fail them. Mirror them locally with the same coverage artifact CI uploads (lcov) plus the diff against the merge-base:

```bash
# Compute patch coverage over new/changed lines, fail below threshold
diff-cover coverage/lcov.info --compare-branch=origin/main --fail-under=80
```

- **Reuse the exact lcov CI uploads** — local and CI numbers then match
- **Compare against the merge-base**, not `HEAD`, so only PR-side changes count
- **Run it as a pre-push gate**, not every commit — full coverage runs are slow
- **Surface uncovered new lines as `file:line`** so the fix is actionable, not just a percentage

> Rat Journal: `bun run coverage:diff` (`scripts/check-diff-coverage.mjs`) implements this gate dependency-free over both `coverage/lcov.info` and `src-tauri/lcov.info`; threshold defaults to 80 to match `codecov.yml`. See root `CLAUDE.md` Gotcha #6.

---

## Rust CRAP Gate

The Linux CI `test` job enforces a **whole-function, repository-wide** policy. This policy is stricter than a patch-only check:

- New functions must have CRAP **≤30**.
- Existing functions must not increase by more than **0.01** against `ci/crap-baseline.json`, including functions that already exceed 30. Unchanged legacy scores pass; improvements pass.
- cargo-crap computes the metrics and function identities. `scripts/check-crap-gate.mjs` only interprets its pinned `delta-v2` JSON. Renamed or ambiguous functions that the analyzer reports as New use the new-function limit. The policy does not guess identity matches.

### CI reports and failure behavior

CI uses pinned `cargo-crap 0.6.1` and successful same-run workspace coverage from `src-tauri/lcov.info`. All CRAP steps follow both Codecov upload attempts. The independent advisory report remains non-blocking for cache, install, analysis, and upload failures. Its `crap-report` artifact includes JSON and production-time provenance: source commit, run URL, producer root, runner, tool versions, coverage command, analyzer flags/configuration, checksums, and UTC time. Analysis time is printed even on failure. If backend coverage did not succeed, advisory analysis skips; a provenance-only artifact is not a usable baseline.

The separate policy gate fails when coverage is missing or unsuccessful, the analyzer is missing or has the wrong version, normalization fails, or the delta input is invalid. Exit **0** means pass, **1** means a policy violation, and **2** means an input/tool failure. The offender list and GitHub annotations use the same delta entries. The separate `crap-delta` artifact upload runs after the gate with `if: always()` and tolerates an absent file when analysis failed. Codecov uploads are not blocked by a CRAP failure.

Use identical analyzer scope for reports, baselines, and comparisons: default `tests/**`, `benches/**`, `examples/**` exclusions, plus `--exclude "**/tests.rs" --exclude "**/test_support.rs"` for confirmed test-only sources. Keep `--missing pessimistic`, epsilon 0.01, and default try-weight 1. Do not filter functions or change scores in the normalizer.

### Local comparison is advisory

`cmd.exe /c bun run pre-commit` checks both script self-tests as blocking checks. Its optional score comparison is **advisory on every local OS**, not a blocking CI mirror. It requires non-empty lcov written by successful backend coverage in that same run. It reports stale input, a missing analyzer, a wrong version, analyzer failure, or a policy verdict without failing the local run. It never installs a tool automatically. Windows-versus-Linux baseline differences are not a Linux gate verdict. A local input/tool error must still be diagnosed before claiming a successful analysis.

For a manual diagnostic, first generate fresh successful workspace coverage. Then, from the repository root, prepare a temporary comparison input and run the pinned analyzer:

```powershell
node scripts/normalize-crap-baseline.mjs --input ci/crap-baseline.json --root <current-absolute-root> --for-comparison --output crap-baseline-input.json
cargo crap --workspace --lcov src-tauri/lcov.info --baseline crap-baseline-input.json --format json --sort file --missing pessimistic --epsilon 0.01 --exclude "**/tests.rs" --exclude "**/test_support.rs" --output crap-delta.json
node scripts/check-crap-gate.mjs --report crap-delta.json --threshold 30 --epsilon 0.01
```

Comparison mode requires contained relative input paths and restores the current root in an ignored copy. It refuses to overwrite its input or the canonical baseline. cargo-crap 0.6.1 does not align relative baseline paths with absolute current paths itself; direct comparison can falsely mark repeated names as New.

### Baseline maintenance

Baseline changes need a **dedicated reviewed commit**, separate from feature changes. Do not raise the baseline to make a failing feature pass. Policy-only enforcement is acceptable initially; baseline refresh is not automatic. Reviewed improvements and identity updates are permitted. Unexplained increases are prohibited.

1. Obtain a probe-free report and its production-time provenance from successful **master Linux CI**, with the pinned analyzer and identical scope. A Windows report, scratch-PR report, failed coverage run, or provenance-only artifact is not canonical input.
2. Verify the producer facts and use the recorded producer root, not the machine that downloads the report:
   `node scripts/normalize-crap-baseline.mjs --input crap-report.json --root <recorded-producer-root> --for-baseline --output <review-copy.json>`.
3. Review identities, all score changes, counts, scope diagnostics, and probe absence. Preserve parsed numbers and order. Commit the reviewed relative JSON with an updated `ci/crap-baseline.provenance.md` that includes exact producer facts, source/run, tool/configuration versions, checksums, normalization command, and review outcome. See the [current provenance record](../../ci/crap-baseline.provenance.md).

Baseline mode refuses paths outside the producer root, including diagnostic paths. All modes reject ambiguous Windows drive-relative paths such as `C:source.rs`; roots must be fully qualified absolute paths without NUL. Normalization retains duplicate names, schema, order, and every parsed non-path value without rounding. The CLI and self-tests verify these properties.

### Analyzer limits and diagnosis

Closure branches are **not scored** in cyclomatic complexity, including `with_unlocked_db(|db| ...)`, although closure lines can affect coverage. Macros are not expanded. cfg-dead function spans in matched files can show 100% coverage. A passing gate does not prove those paths have tests. Keep `--missing pessimistic` in CI; `skip` is for diagnosis only. An all-0% table can indicate a producer-path mismatch, not absent tests. Check source/LCOV diagnostics before changing coverage or a baseline.

Use the [CI gate diagnosis skill](../../.agents/skills/ci-gate-diagnosis/SKILL.md) for failure recipes and the [implementation plan](../plans/2026-10-10-crap-complexity-gates-plan.md) for Linux acceptance evidence.

---

## CI vs Release Build Profiles

Use environment variables to tune build behavior — don't maintain separate config files:

```yaml
# CI job: faster iteration, validates correctness but not peak performance
- name: Build
  env:
    NODE_ENV: test
    BUILD_MODE: development     # or equivalent flag for your toolchain

# Release job: full optimization — omit the overrides, use production defaults
- name: Build
  # no env overrides — defaults from config files apply
```

The principle: CI builds should be fast enough to give quick feedback; release builds should be fully optimized. Control this through env vars at the job level.

---

## Debugging Failures

Add **conditional failure-only debug steps** — they run only on failure and dump internal state without cluttering normal logs:

```yaml
- name: Debug build failure
  if: failure() && matrix.platform == 'macos'
  run: |
    echo "=== Build logs ==="
    find . -name "*.log" -newer package.json -exec cat {} \;
```

Useful for: build tool scripts, generated configs, temp directories that vanish after the run.

---

## Auto-fix Steps

Some jobs can compute their own fix (a refreshed hash, a regenerated file). Treat such a step as a check first, a fixer second:

- **Fail the job when the fix cannot land.** A job that found a problem stays red until the fix is on the branch.
- **Never `exit 0` after an unchecked `git push`.** Branch protection, a race with another push, or a missing permission rejects the push; the job must not report success after that.
- **Prefer "fail and print the fix" over pushing to a protected branch.** Emit an `::error` annotation and a `$GITHUB_STEP_SUMMARY` entry that contains the exact fix, and let a human commit it. The job then needs only `contents: read`.

```yaml
- name: Build
  run: |
    set -o pipefail
    BUILD_EXIT=0
    ./build.sh 2>&1 | tee build.log || BUILD_EXIT=$?
    if [ "$BUILD_EXIT" -ne 0 ]; then
      # `|| true`: no match is normal; under `bash -e` + pipefail it would end the step early.
      FIX=$(grep -oE 'expected: [^ ]+' build.log | head -1 || true)
      echo "::error::Build failed. ${FIX:-See the log above.}"
      echo "Build failed. ${FIX:-See the job log.}" >> "$GITHUB_STEP_SUMMARY"
    fi
    exit "$BUILD_EXIT"
```

A push made with the default `GITHUB_TOKEN` also does not trigger new workflow runs, so an auto-fix commit is not verified automatically. If a job pushes anyway, arrange a separate check (a manual `workflow_dispatch` run or the next qualifying push).

---

## Step Output Passing

Prefer `$GITHUB_OUTPUT` over `echo ::set-output` (deprecated) and environment variables for passing values between steps:

```yaml
- name: Get version
  id: version
  run: echo "value=${GITHUB_REF#refs/tags/v}" >> "$GITHUB_OUTPUT"

- name: Use version
  run: echo "Building version ${{ steps.version.outputs.value }}"
```

Use `$GITHUB_ENV` to export environment variables across subsequent steps in the same job:

```yaml
- name: Set build env
  run: echo "BUILD_DATE=$(date -u +%Y-%m-%d)" >> "$GITHUB_ENV"
```
