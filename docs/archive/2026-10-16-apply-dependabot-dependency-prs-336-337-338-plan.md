# Apply Dependabot dependency PRs 336 337 338

## Metadata

- Plan Status: COMPLETED
- Plan Format: manual-planning v2.0.0
- Template: milestoned
- Tracking: tracked (by user request)

## Status Legend

- Plan Status values: DRAFT, QUESTIONS PENDING, READY FOR APPROVAL, APPROVED, IN PROGRESS, COMPLETED, BLOCKED
- Task/Milestone Status values: TO BE DONE, IN PROGRESS, COMPLETED, BLOCKED, SKIPPED

## Context For A Clean Session

- Repository: `D:\Repos\mini-diarium`, branch `master`, HEAD `e1e5790` ("docs(runbooks): require npmDepsHash refresh in pre-release after lockfile regen").
- **The worktree is NOT clean.** `git status --short` lists 8 pre-existing staged entries from earlier user work: `.claude/scheduled_tasks.lock` (modified), 4 `docs/plans/*.md` → `docs/archive/*.md` renames, `docs/explorations/2026-09-20-rebrand-name-shortlist.md` (modified), and 2 new exploration docs. They must survive untouched and must never enter this plan's commits.
- Shell: Windows PowerShell. Project commands go through `cmd.exe /c ...`; `cargo` runs bare from the repo root (root `CLAUDE.md` "Execution Environment").
- Tool versions verified: bun 1.4.2, npm 11.19.0, cargo 1.95.0, Python 3.13.4. `nix` is NOT installed (`nix --version` → `CommandNotFoundException`), so `npmDepsHash` cannot be computed locally.
- The local E2E lane is usable: `tauri-driver` is on PATH, `.drivers/` holds cached msedgedriver binaries, and `.e2e-stateful/` exists — this machine has run the lane before.

### Incoming PRs (verified with `gh`)

| PR | Head branch | Labels | Ecosystem path | Updates (from → to) |
| --- | --- | --- | --- | --- |
| #336 | `dependabot/npm_and_yarn/dev-dependencies-7df0e98a16` | `dependencies`, `javascript` | npm (`procedures/npm.md`) | `@wdio/cli`, `@wdio/local-runner`, `@wdio/mocha-framework`, `@wdio/spec-reporter`: `^9.32.0`→`^10.0.0`; `@wdio/types`: `^9.31.2`→`^10.0.0`; `webdriverio`: `^9.31.2`→`^10.0.0`; `eslint-plugin-solid`: `^0.18.0`→`^0.18.1`; `typescript-eslint`: `^8.70.1`→`^8.71.1`; `vite`: `^8.3.1`→`^8.3.3` |
| #337 | `dependabot/npm_and_yarn/prod-dependencies-89d4624268` | `dependencies`, `javascript` | npm (`procedures/npm.md`) | `@tauri-apps/plugin-opener`: `~2.6.0`→`~2.7.0`; the nine `@tiptap/*` packages: `3.31.3`→`3.31.4` (`extension-text-style` is an exact pin, the other eight use `^`); `marked`: `^18.0.14`→`^18.1.0` |
| #338 | `dependabot/cargo/minor-and-patch-9ccef912fb` | `dependencies`, `rust` | cargo, `Cargo.lock` only → Lockfile-Only Procedure (`procedures/cargo.md`) | `tauri` 2.12.0→2.12.1; `tauri-plugin-opener` 2.6.0→2.7.0; `tauri-plugin-dialog` 2.8.0→2.8.1; `objc2` 0.6.4→0.6.5 |

- None of the three is applied yet: `package.json` still holds the old ranges, `bun.lock` resolves 9.x WebdriverIO / 3.31.3 Tiptap / marked 18.0.14, and repo-root `Cargo.lock` still has tauri 2.12.0 / opener 2.6.0 / dialog 2.8.0 / objc2 0.6.4.
- There is no overlap between #336 and #337 packages, and no conflicting version target anywhere.
- PR #335 (`chore(release): clear latest changelog after v0.7.5`) is open but is not a dependency PR — out of scope.
- The two npm PRs have disjoint change sets (`#336` = 9 devDependency lines, `#337` = 11 dependency lines), so they are applied as one npm change set and committed together.

### Repository facts

| Fact | Value | How it was verified |
| --- | --- | --- |
| Dual npm lockfiles | `bun.lock` + `package-lock.json` must both be regenerated | root `CLAUDE.md` "Updating Dependencies (npm/bun)"; `procedures/npm.md` |
| Dependabot never touches `bun.lock` | `bun install` regenerates it locally | `procedures/npm.md` Gotchas |
| npm command needs `--legacy-peer-deps` | `eslint-plugin-solid` peers on `eslint@^9`, project uses `eslint@10` | `procedures/npm.md` Gotchas |
| JS/Rust Tauri plugin minors must agree | the Tauri CLI refuses a JS/Rust major-minor mismatch for a plugin | `CHANGELOG.md:126` |
| Cargo.lock is committed at repo root | single workspace lockfile for the 3 crates | `src-tauri/CLAUDE.md` "Workspace layout" |
| TODO-0152 tracks the WebdriverIO 10 upgrade | acceptance: v10 breaking-change check, local `bun run test:e2e` green, CI E2E green, no `extract-zip` / `@puppeteer/browsers@2.x` in either lockfile | `docs/todo/TODO.md:46` |
| npmDepsHash follows `package-lock.json` | Windows cannot compute it; the Nix CI workflow prints the new hash after push | `procedures/npm.md` Phase 3 step 4 / Phase 5; `nix --version` (not installed) |
| Local E2E lane | `bun run test:e2e:local` = `bun run tauri build` + `bun run test:e2e` | `scripts/e2e-local.js:45-51` |
| E2E rule for dependency updates | the repo checklist recommends a local E2E run for dependency updates | `docs/best-practices/POST_TASK_BEST_PRACTICES.md:31-37` |
| CHANGELOG has no open-unreleased section | top heading is `## [0.7.5] - 12-10-2026` (released) | `CHANGELOG.md:37`; precedent commit `3560fd8` opened `## [0.7.5] - Unreleased` right after 0.7.4 |
| wdio.conf.ts uses no removed v10 API | standard `$`/`$$`, `browser.keys`, `browser.action`, `waitFor*`; `wdio:enforceWebDriverClassic` only | read of `wdio.conf.ts:150-182` and `e2e/specs/*.ts` (no `executeAsync`, `switchToFrame`, `touchAction`, `throttle`, `uploadFile`, `multiremote`) |

### Cargo analysis of PR #338's Cargo.lock diff (read before Task 2.1)

#338's lockfile diff removes the duplicate `windows 0.61.x` family (`windows 0.61.3`, `windows-collections 0.2.0`, `windows-core 0.61.2`, `windows-future 0.2.1`, `windows-link 0.1.3`, `windows-numerics 0.2.0`, `windows-result 0.3.4`, `windows-strings 0.4.2`, `windows-threading 0.1.0`) because `tauri-plugin-opener` 2.7.0 drops its `windows 0.61` dependency. The surviving `windows`/`windows-core` 0.62.2 and `webview2-com` keep their versions; the remaining diff lines only drop the now-unneeded `windows 0.62.2`-style version disambiguation (a mechanical consequence of the duplicate being gone). That is duplicate removal, not a non-additive version change to a `windows-*` crate, so the abort condition in `ENTRY.md` Cross-Cutting Gotchas / `procedures/cargo.md` Gotchas does not apply — Task 2.1 re-checks this on the locally regenerated lockfile.

### Hard constraints

1. Never run plain `git add -A` or `git commit` without a pathspec — the index already holds 8 unrelated staged files; a bare commit would include the user's work. Commits must name their paths: `git commit -m "<msg>" -- <paths>`.
2. Never hand-edit `bun.lock`, `package-lock.json`, or `Cargo.lock` — regenerate with the documented commands (`procedures/npm.md` Phase 3 step 5).
3. Keep all nine `@tiptap/*` packages on one version (3.31.4). A partial bump creates duplicate copies with incompatible TypeScript types (`procedures/npm.md` Gotchas).
4. Apply both #337 and #338 before any `bun run tauri build` / E2E run: JS `@tauri-apps/plugin-opener` 2.7.0 and Rust `tauri-plugin-opener` 2.7.0 must match (see Repository facts).
5. No push in this plan; the `npmDepsHash` follow-up happens after the user pushes (`procedures/npm.md` Phase 5).
6. Do not modify PR #335, and do not close Dependabot PRs — Dependabot auto-closes them once the base branch reflects the bumps (`procedures/npm.md` Phase 2 step 3).

## Goal

Apply Dependabot PRs #336 and #337 (npm, both lockfiles regenerated) and #338 (Cargo.lock only) to the workspace, commit them scoped per ecosystem with the user's git identity, verify the WebdriverIO 10 major bump with the local E2E suite on Windows/WebView2, and record the CHANGELOG entry plus the TODO-0152 progress. No push.

## Scope

- `package.json` edits for all 20 npm range changes, plus regenerated `bun.lock` and `package-lock.json` (PRs #336, #337).
- Repo-root `Cargo.lock` update for 4 crates (PR #338).
- Local E2E run (`bun run test:e2e:local`) as the WebdriverIO 10 verification.
- `CHANGELOG.md` Internal entry; TODO-0152 progress note via the `todo-manager` skill.
- Five commits: npm, cargo, E2E spec fix, changelog+todo, plan archival. No push.

## Non-Goals

- Refreshing `nix/package.nix` `npmDepsHash` — requires Linux+Nix and happens after the user pushes (`procedures/npm.md` Phase 5).
- Pushing, opening/merging GitHub PRs, or closing Dependabot PRs.
- PR #335 (release housekeeping, not a dependency PR).
- Archiving TODO-0152 — its acceptance includes the CI E2E job, which runs only after a push.
- Any modification of the user's 8 pre-existing staged files.
- Prose/packaging changes unrelated to the dependency updates.

## Assumptions

- The next unreleased CHANGELOG heading is `## [0.7.6] - Unreleased` (v0.7.5 released 12-10-2026; precedent commit `3560fd8`). The user can correct the number at approval.
- `^10.0.0` resolves to a 10.x version at or above the 10.0.2 that TODO-0152 verified as available (2026-10-11).
- The local E2E lane completes successfully on this machine (tauri-driver + cached msedgedriver verified; the WebView2 runtime version is detected at run time by `wdio.conf.ts`).
- No `windows-*` / `webview2-com` version changes appear beyond the duplicate-family removal described above; Task 2.1 re-checks the locally regenerated lockfile.
- Commits use the user's git identity (`git config user.name` / `user.email`).

## Open Questions

- Q (asked 2026-10-16): PR #336 carries a MAJOR bump of the WebdriverIO family (9→10). Apply, defer, or apply-and-verify? **A (user, 2026-10-16): apply everything, including WebdriverIO 10, and run the local E2E suite to verify** (aligns with TODO-0152's acceptance criteria).

## Milestones

### Milestone 1: npm bumps (PRs #336, #337)

- Status: TO BE DONE
- Purpose: Apply all 20 npm range changes and regenerate both lockfiles consistently, so the frontend build/test tooling and runtime dependencies match the PR targets.
- Exit Criteria: `package.json` carries all 20 new ranges; `bun.lock` and `package-lock.json` resolve them; the resolved+integrity completeness check passes; type-check, lint, and test:run exit 0; a commit exists that contains exactly `package.json`, `bun.lock`, `package-lock.json`.

#### Task 1.1: Edit package.json with all npm bumps

- Status: COMPLETED
- Depends On: none
- Objective: `package.json` matches the union of PR #336 and #337 diffs (20 changed lines: 9 devDependencies + 11 dependencies).
- Steps:
  1. devDependencies (#336): `@wdio/cli` `^9.32.0`→`^10.0.0`; `@wdio/local-runner` `^9.32.0`→`^10.0.0`; `@wdio/mocha-framework` `^9.32.0`→`^10.0.0`; `@wdio/spec-reporter` `^9.32.0`→`^10.0.0`; `@wdio/types` `^9.31.2`→`^10.0.0`; `webdriverio` `^9.31.2`→`^10.0.0`; `eslint-plugin-solid` `^0.18.0`→`^0.18.1`; `typescript-eslint` `^8.70.1`→`^8.71.1`; `vite` `^8.3.1`→`^8.3.3`.
  2. dependencies (#337): `@tauri-apps/plugin-opener` `~2.6.0`→`~2.7.0`; `@tiptap/core`, `@tiptap/extension-color`, `@tiptap/extension-highlight`, `@tiptap/extension-image`, `@tiptap/extension-placeholder`, `@tiptap/extension-text-align`, `@tiptap/pm`, `@tiptap/starter-kit` `^3.31.3`→`^3.31.4`; `@tiptap/extension-text-style` `3.31.3`→`3.31.4` (exact pin, no caret); `marked` `^18.0.14`→`^18.1.0`.
  3. Change only version strings; do not reorder dependencies or alter formatting.
- Validation:
  - `git diff --numstat -- package.json` prints `20	20	package.json`.
  - `Select-String -Path package.json -Pattern '"@wdio/cli": "\^10\.0\.0"','"webdriverio": "\^10\.0\.0"','"@tauri-apps/plugin-opener": "~2\.7\.0"','"@tiptap/extension-text-style": "3\.31\.4"','"marked": "\^18\.1\.0"'` returns one match per pattern.
  - `Select-String -Path package.json -Pattern '3\.31\.3','\^9\.32\.0','\^9\.31\.2','\^8\.3\.1','\^18\.0\.14','~2\.6\.0','\^0\.18\.0','\^8\.70\.1'` returns nothing (no-match exits 1 — that exit code is the PASS condition).
- Notes: Affected file: `package.json` only.

#### Task 1.2: Regenerate bun.lock

- Status: COMPLETED
- Depends On: 1.1
- Objective: `bun.lock` resolves the new package.json ranges.
- Steps:
  1. Run `cmd.exe /c bun install` (this also runs `postinstall` → `install-hooks.js` + `sync-skills.js`).
- Validation: command exits 0; `git diff --stat -- bun.lock` shows `bun.lock` modified.
- Notes: Dependabot never touches `bun.lock` (`procedures/npm.md` Gotchas). The postinstall only touches gitignored skill mirrors and `.git/hooks`; Task 1.6 catches any unexpected tracked-file change.

#### Task 1.3: Regenerate package-lock.json

- Status: COMPLETED
- Depends On: 1.2
- Objective: `package-lock.json` reflects the new versions without touching `node_modules/`.
- Steps:
  1. Run `cmd.exe /c "npm install --package-lock-only --legacy-peer-deps"`.
  2. If Task 1.4 finds entries missing `resolved`/`integrity`, delete `node_modules/`, run `cmd.exe /c "npm install --legacy-peer-deps"`, then re-run `cmd.exe /c bun install` to re-sync `bun.lock`.
- Validation: command exits 0; `git diff --stat -- package-lock.json` shows it modified.
- Notes: `--legacy-peer-deps` is mandatory (`eslint-plugin-solid` peer mismatch). `--package-lock-only` keeps `node_modules/` untouched in the common case (`procedures/npm.md` Gotchas).

#### Task 1.4: Verify npm lockfile contents and Flatpak integrity

- Status: COMPLETED
- Depends On: 1.3
- Objective: Both lockfiles resolve every bumped package at or above the PR target, and `package-lock.json` has no missing `resolved`/`integrity` fields.
- Steps:
  1. Read `package-lock.json` as JSON and print the resolved versions of: all nine `node_modules/@tiptap/*`, `node_modules/@tauri-apps/plugin-opener`, `node_modules/marked`, `node_modules/@wdio/cli`, `node_modules/@wdio/local-runner`, `node_modules/@wdio/mocha-framework`, `node_modules/@wdio/spec-reporter`, `node_modules/@wdio/types`, `node_modules/webdriverio`, `node_modules/eslint-plugin-solid`, `node_modules/typescript-eslint`, `node_modules/vite`.
     ```powershell
     $lock = Get-Content package-lock.json -Raw | ConvertFrom-Json -AsHashtable
     @('@tiptap/core','@tiptap/extension-color','@tiptap/extension-highlight','@tiptap/extension-image','@tiptap/extension-placeholder','@tiptap/extension-text-align','@tiptap/extension-text-style','@tiptap/pm','@tiptap/starter-kit','@tauri-apps/plugin-opener','marked','@wdio/cli','webdriverio','eslint-plugin-solid','typescript-eslint','vite') | % { "$_ = $($lock.packages["node_modules/$_"].version)" }
     ```
  2. Assert: the WebdriverIO family majors are `10`; `@tiptap/*` are all exactly `3.31.4`; `@tauri-apps/plugin-opener` is `2.7.x`; `marked` ≥ `18.1.0`; `eslint-plugin-solid` ≥ `0.18.1`; `typescript-eslint` ≥ `8.71.1`; `vite` ≥ `8.3.3` (with `^` ranges the newest 8.x/0.18.x may exceed the target; that is expected).
  3. Grep `bun.lock` for the same resolved versions, e.g. `Select-String -Path bun.lock -Pattern '"@tiptap/core@3.31.4"','"marked@18.1.0"','"vite@8.3.3"' -SimpleMatch`.
  4. Confirm the root `"version"` in `package-lock.json` is `0.7.5` (matches `package.json`).
  5. Flatpak/completeness check (PowerShell 5.1 cannot parse `package-lock.json` — the empty root key fails `ConvertFrom-Json` — so use Node):
     ```bash
     node -e 'const l=require(`./package-lock.json`);const nm=Object.keys(l.packages).filter(k=>k.startsWith(`node_modules/`));const c=nm.filter(k=>l.packages[k].resolved&&l.packages[k].integrity);console.log(`${nm.length} entries, ${c.length} with resolved+integrity`)'
     ```
- Validation: step 2 assertions hold; grep finds the expected entries in `bun.lock`; the two counts in step 5 are equal (any missing entry → execute Task 1.3 step 2 fallback).
- Notes: The completeness check mirrors `procedures/npm.md` Phase 4 step 4 and protects the Flatpak `npm ci --offline` path.

#### Task 1.5: Run the JS validation suite

- Status: COMPLETED
- Depends On: 1.4
- Objective: The frontend still type-checks, lints, and passes its unit tests with the new dependency set.
- Steps:
  1. Run `cmd.exe /c bun run type-check`.
  2. Run `cmd.exe /c bun run lint`.
  3. Run `cmd.exe /c bun run test:run`.
- Validation: all three commands exit 0.
- Notes: This also type-checks `wdio.conf.ts`-adjacent types indirectly (the config itself is checked by the E2E tooling in Task 3.1).

#### Task 1.6: Verify npm change set and commit

- Status: COMPLETED
- Depends On: 1.5
- Objective: A scoped commit contains exactly the three npm files, leaving the user's staged work untouched.
- Steps:
  1. Run `git diff --stat` — it must list only `package.json`, `bun.lock`, `package-lock.json` (the plan file is untracked and does not appear).
  2. Read `git config user.name` and `git config user.email`; set `GIT_AUTHOR_*` / `GIT_COMMITTER_*` to those values for the commit.
  3. Commit with a pathspec, no push:
     ```
     git commit -m "Dependency Update: @tiptap/* 3.31.4, @tauri-apps/plugin-opener 2.7.0, marked 18.1.0, @wdio/* 10.0.0, webdriverio 10.0.0, eslint-plugin-solid 0.18.1, typescript-eslint 8.71.1, vite 8.3.3" -m "Applies Dependabot PRs #336 and #337. Both lockfiles regenerated. nix/package.nix npmDepsHash refresh omitted (Linux+Nix only); the Nix CI workflow prints the new hash after push." -- package.json bun.lock package-lock.json
     ```
- Validation: `git show --stat --format=%s HEAD` shows exactly those 3 files and the expected subject; `git status --short` still lists the 8 pre-existing staged entries and nothing else.
- Notes: The `-- <paths>` form is mandatory (Hard constraint 1). No co-author, no LLM identity (`procedures/npm.md` Phase 4 step 6).

### Milestone 2: cargo lockfile-only bump (PR #338)

- Status: TO BE DONE
- Purpose: Move the four locked Rust crates to the PR targets without touching any manifest.
- Exit Criteria: repo-root `Cargo.lock` resolves tauri 2.12.1, tauri-plugin-opener 2.7.0, tauri-plugin-dialog 2.8.1, objc2 0.6.5; no other `windows-*` / `webview2-com` version changed; `cargo test --workspace` and `cargo build -p mini-diarium --features custom-protocol` exit 0; a commit contains exactly `Cargo.lock`.

#### Task 2.1: Apply the four cargo updates

- Status: COMPLETED
- Depends On: 1.6
- Objective: `Cargo.lock` resolves the four target versions, removing the duplicate `windows 0.61.x` family.
- Steps:
  1. From the repo root, run one command per crate (`--precise` cannot repeat in one invocation, and `-p <crate>@<version>` does not match versions absent from the lockfile): `cargo update -p tauri --precise 2.12.1`; `cargo update -p tauri-plugin-opener --precise 2.7.0`; `cargo update -p tauri-plugin-dialog --precise 2.8.1`; `cargo update -p objc2 --precise 0.6.5`.
  2. Inspect `git diff Cargo.lock`.
- Validation:
  - Command exits 0.
  - The four `version = ` lines changed to `2.12.1`, `2.7.0`, `2.8.1`, and `0.6.5`.
  - Every other diff hunk is either the removal of a `windows 0.61.x`-family entry or a dependency-list disambiguation line for the surviving 0.62.2 family; no surviving `windows*` / `webview2-com` package changes version.
- Notes: If cargo picks any additional transitive version change, inspect it, decide explicitly, and record a `## Decision Log` entry before continuing. Do not proceed past a non-additive `windows-*` / `webview2-com` change without re-evaluating (`procedures/cargo.md` Gotchas; the PR's own diff analysis is in Context above).

#### Task 2.2: Run the workspace test suite

- Status: COMPLETED
- Depends On: 2.1
- Objective: All three crates' tests pass on the new dependency set.
- Steps:
  1. Run `cargo test --workspace`.
- Validation: command exits 0. (`--workspace` is required; `--manifest-path src-tauri/...` would skip core and crypto tests — root `CLAUDE.md` "Execution Environment".)
- Notes: None.

#### Task 2.3: Run the release-like build

- Status: COMPLETED
- Depends On: 2.2
- Objective: The app builds with the release feature set.
- Steps:
  1. Run `cargo build -p mini-diarium --features custom-protocol`.
- Validation: command exits 0.
- Notes: `procedures/cargo.md` Lockfile-Only Procedure step 4.

#### Task 2.4: Verify cargo change set and commit

- Status: COMPLETED
- Depends On: 2.3
- Objective: A scoped commit contains exactly `Cargo.lock`.
- Steps:
  1. Run `git diff --stat` — only `Cargo.lock` (the npm files are already committed).
  2. Commit with the user identity, pathspec, no push:
     ```
     git commit -m "Dependency Update: tauri 2.12.1, tauri-plugin-opener 2.7.0, tauri-plugin-dialog 2.8.1, objc2 0.6.5" -m "Applies Dependabot PR #338 (Cargo.lock only). Drops the duplicate windows 0.61.x crate family that tauri-plugin-opener 2.7.0 no longer needs." -- Cargo.lock
     ```
- Validation: `git show --stat --format=%s HEAD` shows exactly `Cargo.lock` and the expected subject.
- Notes: None.

### Milestone 3: E2E verification and TODO-0152

- Status: TO BE DONE
- Purpose: Prove the WebdriverIO 10 major bump works in the real WebView2 lane and record what remains for TODO-0152.
- Exit Criteria: `bun run test:e2e:local` exits 0; both lockfiles are free of `extract-zip` and `@puppeteer/browsers@2.x`; TODO-0152 carries an accurate progress note and stays open (CI E2E needs a push).

#### Task 3.1: Run the local E2E suite

- Status: COMPLETED
- Depends On: 2.4
- Objective: The full WebdriverIO suite passes against the release binary built from the updated workspace.
- Steps:
  1. Run `cmd.exe /c bun run test:e2e:local` (builds the release binary via `bun run tauri build`, then runs the clean-mode suite; expect several minutes).
  2. If it fails because of WebdriverIO 10 API changes, diagnose and fix in `wdio.conf.ts` / `e2e/specs/*` — the user approved this work. Record the failure and the fix in the `## Decision Log` and include the files in an amendment commit.
- Validation: command exits 0; the script prints `✔ E2E tests passed`; the wdio summary reports 0 failing tests.
- Notes: Runs only here, after both ecosystems are committed, because JS and Rust `plugin-opener` must be on the same minor (Hard constraint 4). Windows/WebView2 lane — CI E2E covers Linux/WebKitGTK only (`e2e/CLAUDE.md`).

#### Task 3.2: Verify extract-zip / @puppeteer/browsers removal

- Status: COMPLETED
- Depends On: 3.1
- Objective: The lockfile outcome that TODO-0152 exists for is confirmed.
- Steps:
  1. `Select-String -Path bun.lock -Pattern 'extract-zip'` and `Select-String -Path package-lock.json -Pattern '"node_modules/extract-zip"'`.
  2. Print the resolved `@puppeteer/browsers` version from both lockfiles.
- Validation: no `extract-zip` match in either lockfile (no-match exits 1 — that exit code is the PASS condition); `@puppeteer/browsers` resolves to 3.x. If `extract-zip` survives through another chain, record it and leave TODO-0152/0108 open with the finding.
- Notes: `docs/todo/TODO.md:45-46` (TODO-0108, TODO-0152) and `CHANGELOG.md:81` document why this is expected to disappear.

#### Task 3.3: Record TODO-0152 progress

- Status: COMPLETED
- Depends On: 3.2
- Objective: TODO-0152 reflects reality: applied and locally verified, CI verification still pending.
- Steps:
  1. Load the `todo-manager` skill.
  2. Add an `**Update (2026-10-16):**` note to TODO-0152 in `docs/todo/TODO.md`: WebdriverIO family applied at the resolved 10.x version, local `bun run test:e2e:local` green on Windows/WebView2, `extract-zip` gone from both lockfiles; the CI E2E and CI-lockfile acceptance is pending a push.
  3. Do NOT check the box or archive the item — its acceptance includes the CI E2E job, which cannot run without a push.
- Validation: `Select-String -Path docs/todo/TODO.md -Pattern 'TODO-0152'` shows the current text including the update note; the checkbox is still `[ ]`.
- Notes: Never assign TODO IDs by hand (repo rule 4); the note follows the TODO-0108 update-note pattern.

### Milestone 4: CHANGELOG, cleanup, final verification

- Status: TO BE DONE
- Purpose: Record the change, leave the repository with only intentional artifacts, and prove the whole change holds together.
- Exit Criteria: CHANGELOG entry committed; plan archived as COMPLETED; pre-flight checks pass; only intended files are committed, and the user's 8 pre-existing staged entries are untouched.

#### Task 4.1: Add the CHANGELOG entry and commit the docs

- Status: COMPLETED
- Depends On: 3.3
- Objective: `CHANGELOG.md` opens a `## [0.7.6] - Unreleased` section with one `### Internal` bullet covering all three PRs; TODO-0152's update note is committed.
- Steps:
  1. In `CHANGELOG.md`, insert after `# Versions` (before `## [0.7.5] - 12-10-2026`):
     ```markdown
     ## [0.7.6] - Unreleased

     ### Internal
     - **Dependency updates (Dependabot #336, #337, #338)**: Frontend production bumps for the nine `@tiptap/*` packages (3.31.4), `@tauri-apps/plugin-opener` (2.7.0, kept on the Rust plugin's minor), and `marked` (18.1.0). Development bumps for the WebdriverIO family (`@wdio/*`, `webdriverio` 10.0.0), which drops the unmaintained `extract-zip` that `@puppeteer/browsers` 2.x pulled (TODO-0152, Dependabot alerts #51/#58), plus `eslint-plugin-solid` (0.18.1), `typescript-eslint` (8.71.1), and `vite` (8.3.3). Cargo.lock patch bumps for `tauri` (2.12.1), `tauri-plugin-opener` (2.7.0), `tauri-plugin-dialog` (2.8.1), and `objc2` (0.6.5), which also removes the duplicate `windows` 0.61 dependency family. Both lockfiles regenerated and aligned; the local WebdriverIO suite passes on Windows/WebView2 (the Ctrl+[ day-navigation spec was hardened against a date-dependent cross-spec fixture collision found during that run). The `nix/package.nix` `npmDepsHash` refresh is a Linux-side follow-up after push.
     ```
  2. Commit with a pathspec, no push:
     ```
     git commit -m "docs(changelog): record dependency updates (Dependabot #336, #337, #338)" -- CHANGELOG.md docs/todo/TODO.md
     ```
- Validation: heading exists at the right position; `git show --stat --format=%s HEAD` lists exactly `CHANGELOG.md` and `docs/todo/TODO.md`.
- Notes: Wording follows the changelog style (audience rule → `Internal`; `docs/best-practices/POST_TASK_BEST_PRACTICES.md:46-70`). Adjust the bullet if Task 3.1 needed a v10 migration fix.

#### Task 4.2: Cleanup and archive the plan

- Status: COMPLETED
- Depends On: 4.1
- Objective: No stray intermediate artifacts; the completed plan is retired to `docs/archive/` per repository convention.
- Steps:
  1. Inspect `git status --short`; confirm the only entries are the 8 pre-existing staged ones (and nothing from `.drivers/`, `.e2e-stateful/`, `node_modules/` — all gitignored). Remove any scratch file this run created that is not part of the final state.
  2. Set this plan's `Plan Status` to `COMPLETED`, replace the `## Approval Gate` boilerplate with `Approved by <user> on <date>`, and refresh `## Plan Self-Check` with the final `check-plan.py` output.
  3. Move the plan: `git add docs/archive/2026-10-16-apply-dependabot-dependency-prs-336-337-338-plan.md` after moving the file there with `git mv`.
  4. Commit with a pathspec, no push: `git commit -m "docs(plans): archive 2026-10-16 dependency-PR run plan" -- docs/plans/2026-10-16-apply-dependabot-dependency-prs-336-337-338-plan.md docs/archive/2026-10-16-apply-dependabot-dependency-prs-336-337-338-plan.md`.
- Validation: `git status --short` shows only the 8 pre-existing staged entries; `git log --oneline -5` shows the five plan commits; the archived plan reads `Plan Status: COMPLETED`.
- Notes: Do not remove or re-stage the user's pre-existing changes (`manual-planning` cleanup rule: do not remove unrelated worktree changes).

#### Task 4.3: Final verification

- Status: COMPLETED
- Depends On: 4.2
- Objective: The repository is in a shippable state for this change set.
- Steps:
  1. Run every item in `## Pre-flight Checks`.
  2. Confirm nothing but docs changed after the E2E run: `git log --stat -3` shows only CHANGELOG.md, docs/todo/TODO.md, and the plan file.
  3. Confirm the pre-existing staged set is unchanged: `git diff --cached --stat` equals the discovery snapshot (8 files, 810 insertions, 8 deletions at HEAD `e1e5790`).
- Validation: all pre-flight commands exit 0; `git diff --stat` (unstaged) is empty; the diff-cached snapshot matches `e1e5790`'s staged state.
- Notes: If any pre-flight check fails, record the blocker; do not mark the plan COMPLETED.

## Project Gates

- **Command wrappers**: project commands run via `cmd.exe /c ...`; `cargo` runs bare from the repo root. Backend tests must use `--workspace`.
- **Commit contract**: `Dependency Update: <short-summary>` for dependency commits; user identity (no LLM author, no co-author); no push (`procedures/npm.md` Phase 4 step 6).
- **Commit scoping**: one logical change per commit; always commit with explicit pathspecs (Hard constraint 1).
- **CHANGELOG**: required (post-task checklist step 3; dependency precedent commit `60b93f8`).
- **TODO bookkeeping**: TODO-0152 progress recorded via `todo-manager`; no manual ID assignment.
- **Formatting hooks**: `.githooks/pre-commit` formats staged `src/**` and `src-tauri/**` files only — none of this change set matches those globs, so it is a no-op; do not bypass it.
- **Coverage/CRAP gates**: unaffected (no source lines change); optional `cmd.exe /c bun run coverage:diff` produces the same verdict as HEAD.
- **E2E rule**: dependency updates warrant a local E2E run (`docs/best-practices/POST_TASK_BEST_PRACTICES.md:31-37`); Task 3.1 satisfies it.

## Pre-flight Checks

Run before the plan may reach `COMPLETED`:

- [ ] `cmd.exe /c bun run type-check`
- [ ] `cmd.exe /c bun run lint`
- [ ] `cmd.exe /c bun run test:run`
- [ ] `cargo test --workspace`
- [ ] `cargo build -p mini-diarium --features custom-protocol`
- [ ] `cmd.exe /c bun run test:e2e:local -- --skip-build` (reuses the Task 3.1 binary; final end-to-end proof)
- [ ] `git status --short` shows only the 8 pre-existing staged entries

## Decision Log

Write an entry **before moving to the next task**, never retrospectively.
An entry is required when implementation diverges from what this plan specifies (different path,
signature, or approach), when a validation failure forces the plan to adapt, when an unplanned
problem is found, or when a validation is deliberately deferred.
No entry is needed when execution matches the plan.

_No entries yet — the section is append-only._

### DEC-001 — Tiptap family kept at the PR's 3.31.4 with the documented exact-pin dance

- Date: 2026-10-16
- Task: 1.2
- Decision: `bun install` re-resolved the eight caret `^3.31.4` ranges to 3.32.0 (published after PR #337 was created) while `@tiptap/extension-text-style` (exact `3.31.4`) stayed put, producing unsatisfied peer warnings and splitting the Tiptap monorepo family. Following the repo precedent (`docs/archive/2026-09-07-apply-dependabot-dependency-prs-plan.md` DEC-001; `docs/archive/2026-09-15-apply-dependabot-npm-dependency-prs-296-297-298-with-vitest-5-migration-plan.md` Task 1.2 step 2), all nine `@tiptap/*` entries are temporarily exact-pinned to `3.31.4` to regenerate both lockfiles at the PR target, then `package.json` returns to the PR's caret form (`^3.31.4` for eight, exact `3.31.4` for `extension-text-style`) once both lockfiles pin 3.31.4.
- Rationale: keeps the versions the dependency PR proposes and the monorepo lockstep rule (Hard constraint 3), and avoids adopting the unvetted 3.32.0 minor that no dependency PR proposed.

### DEC-002 — Validation commands adapted to this shell, plus lockfile-stability proofs

- Date: 2026-10-16
- Task: 1.4
- Decision: `ConvertFrom-Json -AsHashtable` is PowerShell 7 only; this shell is Windows PowerShell 5.1, and even its `ConvertFrom-Json` cannot parse `package-lock.json` (empty root key). All lockfile JSON assertions use Node one-liners instead (Task 1.4 step 5 updated). Additionally, two stability proofs were run after the DEC-001 dance restored the PR's caret form: a non-frozen `bun install` and a non-frozen `npm install --package-lock-only --legacy-peer-deps` both kept every `@tiptap/*` resolution at 3.31.4 and only realigned the recorded specifiers, and `bun install --frozen-lockfile` plus `npm ci --legacy-peer-deps --dry-run` both exit 0.
- Rationale: proves the committed lockfiles satisfy CI's frozen/sync checks and will not drift to 3.32.0 on a normal install; using Node avoids silent no-op validations on this shell.

### DEC-003 — cargo update command form corrected to per-crate `--precise`

- Date: 2026-10-16
- Task: 2.1
- Decision: The runbook's `cargo update -p <crate>@<version>` form fails when the target version is not yet in the lockfile (`error: package ID specification ... did not match any packages`), and a single invocation accepts `--precise` only once. Ran four sequential commands (`cargo update -p tauri --precise 2.12.1`, then opener/dialog/objc2) and updated Task 2.1 step 1 to that form.
- Rationale: `--precise` pins the exact PR target; the resulting lockfile diff contains only the four target versions plus the expected `windows 0.61.x` duplicate-family removal, with no other version change.

### DEC-004 — Ctrl+[ E2E failure diagnosed as a date-dependent fixture collision; spec hardened

- Date: 2026-10-16
- Task: 3.1
- Decision: The full-suite run failed one assertion in `header-actions.spec.ts` ("does not step onto an empty previous day via Ctrl+["). Diagnosis: the specs share one clean-room data dir, and `diary-workflow.spec.ts` writes its entry to `Math.min(today, 15)` (`diary-workflow.spec.ts:25-26`), which is the day before today exactly when today is the 16th — so the "adjacent empty day" the test assumes had an entry, and Ctrl+[ correctly jumped to it. Proof: `bun run test:e2e -- --spec e2e/specs/header-actions.spec.ts` alone (fresh data dir, no adjacent entry) passes 7/7 with WebdriverIO 10, and the app-side handler (`src/lib/keyboard-shortcuts.ts:19`) is defined as "previous day with an entry". The test is changed to read the adjacent calendar day's `aria-label` entry marker (the cell is always mounted; the collapsed sidebar only transforms it) and to expect the jump exactly when that day holds an entry, and not to land there when it does not.
- Rationale: fixes the real defect (an invalid assumption, not a WebdriverIO 10 regression), keeps the test's intent, and makes it correct for every day of the month. A separate commit (`fix(e2e): ...`) keeps the dependency commits scoped.

### DEC-005 — Plan archived with a plain move plus `git add` (`docs/plans/` is locally excluded)

- Date: 2026-10-16
- Task: 4.2
- Decision: `.git/info/exclude` line 18 excludes `docs/plans/`, so the working plan was never tracked (which is why `check-plan.py` reported W001 and why `git status` never listed it). `git mv` cannot archive an untracked file, so the plan is moved with a filesystem move into the tracked `docs/archive/` path and then staged with `git add`, matching the existing archived-plan convention (the four staged `docs/plans/ → docs/archive/` renames).
- Rationale: keeps the plan-as-record workflow intact without touching the user's staged renames, and the final `check-plan.py` run is repeated after the file becomes tracked. The status flip to `COMPLETED` and the archive move happen after Task 4.3's checks, so the retired file passes the checker cleanly.

## Final Verification

The end-to-end check is the full pre-flight list above plus the Task 3.1 E2E run. Nothing but
documentation changes after Task 3.1, so its E2E result remains valid; the `--skip-build` pre-flight
re-run proves the same binary still passes the suite. `git log --oneline -5` must show the five
scoped commits, and the worktree must contain only the user's pre-existing staged changes.

## Approval Gate

Approved by the user on 2026-10-16.

## Plan Self-Check

```text
$ python "C:\Users\Francisco\.config\opencode\skills\manual-planning\scripts\check-plan.py" docs/archive/2026-10-16-apply-dependabot-dependency-prs-336-337-338-plan.md --dir docs/archive
0 error(s), 0 warning(s)
```

Run: 2026-10-16 (final, after the completed plan was staged in `docs/archive/`).

An earlier run while the plan was untracked reported the expected `W001` (tracking says tracked,
git does not track the file). `docs/plans/` is listed in `.git/info/exclude`, so working plans stay
untracked until archival; staging the archived file cleared the warning (DEC-005).

## Execution Notes

- Update milestone and task status before starting and after validation.
- Update each task to COMPLETED immediately after its validation passes.
- Mark tasks or milestones BLOCKED with a short reason when progress cannot continue.
- Task numbering is not an execution order. Follow `Depends On`.
- Write a `## Decision Log` entry **before starting the next task** whenever execution diverges from
  this plan, an unplanned problem is found, or a validation is deferred — never retrospectively.
