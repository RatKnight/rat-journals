# CRAP complexity gates decisions

Companion to [the implementation plan](2026-10-10-crap-complexity-gates-plan.md). Entries are append-only.

### DEC-001 — Initial approval and separate blocking approval

- Date: 2026-10-10
- Task: 1.1
- Decision: The user's request to explore and then implement approves initial execution. It does not replace Task 1.6's separate approval after Linux acceptance. Execute Tasks 1.1 and 1.3 and the independent Tasks 3.1 to 3.4 first.
- Rationale: The dependency graph permits the independent lint work, but forbids all M2 work before Task 1.6.

### DEC-002 — Coverage upload failure path

- Date: 2026-10-10
- Task: 1.1
- Decision: Add `if: always()` to the two existing Codecov upload steps, without changing their inputs or gate configuration.
- Rationale: Current uploads use the default success condition, so they do not run after a failed test. Task 1.1 explicitly requires upload attempts on that path. The CRAP cache and install can also run after uploads, so their runtime cannot delay coverage uploads.

### DEC-003 — Verify relative baseline comparison before M2

- Date: 2026-10-10
- Task: 1.2
- Decision: Include an unchanged-code comparison against a normalized report in Linux acceptance. Do not assume absolute current paths match the committed relative baseline.
- Rationale: Installed cargo-crap 0.6.1 `src/delta.rs:150–179` normalizes slashes but does not remove producer roots. Exact matches include file, function, and line; fallback name matching can be ambiguous. Resolve the comparison approach from real reports before M2 implementation.

### DEC-004 — Local validation before the Linux checkpoint

- Date: 2026-10-10
- Task: 1.1, 1.3
- Decision: Implement and validate the normalizer together with advisory CI wiring. Keep Task 1.1 IN PROGRESS until its Linux workflow validation finishes; do not claim a Linux report or baseline from a local run.
- Rationale: The lint job invokes the normalizer, so the two files must ship together. A real local 0.6.1 report can validate the path-only transformation before CI, without serving as the canonical baseline.

### DEC-005 — Remove an obsolete MSRV setting

- Date: 2026-10-10
- Task: 3.1, 3.2
- Decision: Remove `msrv = "1.75"` with the old app-only config. Do not replace it with the pinned toolchain version or declare an MSRV. Fix the single newly exposed `manual_inspect` lint with the equivalent `inspect_err` call in a separate commit.
- Rationale: Clippy documentation confirms that MSRV means the minimum supported compiler, not today's compiler. The full `-D warnings` run found one `map_err` callback that returns its input error unchanged after cleanup (`auth_core.rs:467`). `inspect_err` keeps that cleanup and error propagation unchanged. No auth policy, key handling, or IPC error changes are needed.

### DEC-006 — Non-blocking frontend complexity and maintained controls

- Date: 2026-10-10
- Task: 3.3
- Decision: Keep classic complexity max 15 at `warn`; promotion to `error` needs a separate decision. Test each exemption with ESLint `lintText` using a virtual test-file path, and keep these controls plus the normalizer self-test in `scripts/crap-complexity.test.mjs`.
- Rationale: Lint has no `--max-warnings 0`. Virtual paths exercise the real file overrides without leaving scratch files on disk. The existing `test:scripts` runner now provides CI and pre-commit coverage for these controls.

### DEC-007 — Coverage validation timeout

- Date: 2026-10-10
- Task: 3.4
- Decision: The first forced-failure coverage run exceeded 120 seconds before any test results or lcov were written. Retry with a 600-second tool limit; do not count the timeout as the required failing-assertion control.
- Rationale: The validation requires an actual failed assertion and a report newer than run start. A killed process proves neither.

### DEC-008 — Test the newly exposed lint fix instead of weakening coverage

- Date: 2026-10-10
- Task: 3.2
- Decision: Extract only the existing `create_diary_auto` body into a private `&DiaryState` helper and test both successful creation and database-creation failure. Reopen Task 3.2 until the fresh coverage gate passes. Keep the thin IPC wrapper and all behavior unchanged.
- Rationale: Fresh local patch coverage failed at 0/1 on the new `inspect_err` line. Tauri `State` has no public constructor (installed `tauri/src/state.rs:20`), while nearby command tests use private state-reference helpers. This small extraction allows real tests of config rollback and error propagation without a new Tauri test dependency, a lint suppression, or a lower coverage threshold. It is limited to validation of the required lint fix, not a refactor of the legacy CRAP offenders.

### DEC-009 — Run local E2E for the auth helper extraction

- Date: 2026-10-10
- Task: 3.2
- Decision: Run `cmd.exe /c bun run test:e2e:local` before closing Task 3.2. Correct the earlier note that exempted this extraction from E2E.
- Rationale: The post-task checklist requires local E2E when a change touches auth. Equivalent behavior does not remove that requirement. The command builds the current source before it tests the Windows app.

### DEC-010 — Exclude confirmed test files and validate aligned baseline paths

- Date: 2026-10-10
- Task: 1.2
- Decision: Add `--exclude '**/tests.rs' --exclude '**/test_support.rs'` consistently to advisory analysis and its provenance, retaining all default exclusions. Run another disposable Linux success control with that scope and an input copy of the relative baseline whose entry paths have the current producer root restored. Do not add a gate or commit a baseline.
- Rationale: Linux run 38072720599 includes helpers from six files whose parent declarations use `#[cfg(test)]`: config/tests.rs, fonts/tests.rs, and four core test_support.rs files. The unchanged-source comparison also reports seven false new identities because cargo-crap compares absolute current paths to relative baseline paths. A local diagnostic root-aligned input already matched all 643 identities; Linux must confirm that behavior before M2.

### DEC-011 — Preserve the inherited prerequisite and stop at its quality gate

- Date: 2026-10-10
- Task: 1.4, 1.7
- Decision: Record an approval-blocked prerequisite task and do not merge PR #327, edit the inherited runner guard, or dismiss its security finding without a user decision. Promote the eleven-entry log to this companion file under the manual-planning convention.
- Rationale: The public SonarCloud API reports `new_security_rating = 2` (required 1), caused only by `javascript:S4036` at `scripts/check-test-runners.mjs:299`, issue `AaEm5TptZUoGI0Yrksmb`. `git log origin/master..ci/crap-phase0-advisory` shows the pre-existing `5a82c70` runner-infrastructure commit in the PR ancestry. That commit is not yet on origin/master. It is user work outside the CRAP scope, and the plan requires a green advisory PR before merge. No normalizer security or duplication finding was reported.

### DEC-012 — User authorizes a separate prerequisite fix

- Date: 2026-10-10
- Task: 1.7
- Decision: The user selected “Fix separately (Recommended)”: fix and test the inherited PATH finding in a separate scoped commit, then return to advisory merge review. Select an absolute Git executable from the standard OS installation, with a documented explicit absolute-path override for nonstandard installations; never discover it through inherited PATH. Retain direct execFileSync arguments and fail loudly if the selected executable cannot run.
- Rationale: Node's child-process documentation confirms that bare commands use inherited PATH and that absolute executable paths work without a shell. On this verified Windows machine, Git is at C:\Program Files\Git\cmd\git.exe. Unix defaults use /usr/bin/git; Windows uses the OS Program Files location. Nonstandard developer installations need an explicit trusted executable choice, not an implicit PATH fallback. Tests will cover platform selection, invalid relative paths, and the real CLI with PATH empty. This approval does not replace Task 1.6 or authorize a gate.

### DEC-013 — Carry the verified path-alignment requirement into pending M2 steps

- Date: 2026-10-10
- Task: 1.2
- Decision: Update the still-unstarted M2 instructions to prepare an ignored, temporary absolute-path input copy from the committed relative baseline before cargo-crap computes its delta. A future `--for-comparison` mode of the existing path-only normalizer will perform that transformation with tests. Keep the committed baseline relative and unchanged. Do not implement the mode or any M2 task before Task 1.6 approval.
- Rationale: Linux acceptance run 38074856678 matched all 622 unchanged functions and zero removed when the current producer root was restored to an input copy. Direct relative-baseline comparison still produced seven false new identities. The original M2 command examples would therefore be incorrect even though the canonical relative-baseline requirement is correct. The transformation changes paths only, never scores, function sets, or identities.

### DEC-014 — Add the missing inherited security-gate diagnosis recipe

- Date: 2026-10-10
- Task: 1.7
- Decision: Under the skill-improver workflow, add a narrow security-rating API recipe and an inherited-commit check to the canonical CI diagnosis skill in a separate documentation change. Sync generated mirrors after that edit. Do not add M2 gate documentation or alter SonarCloud configuration.
- Rationale: The existing skill explains duplication and coverage failures, but not a security-rating failure. Querying the issue API located the sole finding in earlier unmerged work and prevented an unauthorized change. The missing recipe is an obvious documentation gap; the current fix and its tests already address the repo artifact gap. No structural skill redesign is needed.

### DEC-015 — Full self-check before advisory merge review

- Date: 2026-10-11
- Task: 1.8
- Decision: The user requested a full correctness, completeness, and accuracy check instead of approving the advisory merge. Add Task 1.8 as a merge prerequisite. Review all implemented work and run fresh integrated checks. Fix small in-scope defects with regression tests; do not merge PR #327, create a canonical baseline from a PR artifact, or begin M2.
- Rationale: Passing PR checks do not replace a code and plan audit. The primary branch also contains independent lint, reporting, and auth validation changes that are deliberately absent from the advisory-only PR; both scopes must be checked and described accurately.

### DEC-016 — Reject ambiguous report paths and strengthen CLI controls

- Date: 2026-10-11
- Task: 1.8
- Decision: Reject Windows drive-relative report paths such as `C:outside.rs` on every consumer OS, and reject malformed producer roots. Put normalizer-only controls in their own script suite so the advisory PR can execute them without the independent ESLint configuration change. Add maintained CLI success and failure controls, including no output overwrite on failure. Correct stale current-state documentation separately from the source fix.
- Rationale: A direct negative control accepted `C:outside.rs` with both `/repo` and `C:/Repo` roots in baseline mode. This violates the portable baseline contract and can resolve through a drive's current directory later. Existing inline tests did not cover this ambiguity or CLI behavior. The fix stays within Task 1.3's path-only contract; it adds no comparison mode, metrics calculation, baseline, or gate.

### DEC-017 — Conditional advisory merge approval

- Date: 2026-10-11
- Task: 1.4, 1.8
- Decision: The user instructed: “finish the self-check and then merge the pr 327 when finished”. Finish Task 1.8, require every current PR check to pass, then mark PR #327 ready and merge its checked head without bypassing branch protection. Verify the post-merge master CI report before completing Task 1.4. Keep Task 1.6's approval separate.
- Rationale: This is explicit conditional approval for the advisory-only merge after validation, not permission to merge while checks run or enable a blocking CRAP gate. At approval, Linux/macOS builds, tests, lint, SonarCloud, CodeQL, and Codecov pass; Windows build, Flatpak, and E2E still run.

### DEC-018 — Update from master before baseline review

- Date: 2026-10-11
- Task: 1.5
- Decision: The user requested updating this branch from latest master and continuing the plan. Merge master `f1bb84f73f44dd40028299682d0b6ef2f2326ca7` without rewriting this branch. Resolve the changelog conflict by retaining the newer filename rules and all independent branch entries. Use master CI run 38103562280 for Task 1.5 after its coverage/report and checks pass, rather than the older merge report. Run fresh local integrated checks after the merge. Keep Task 1.6 approval separate.
- Rationale: Master now contains panic-lock recovery and legacy filename fixes that change the analyzed Rust source. A baseline from the older merge SHA would omit that work. The continuation request permits baseline preparation but does not replace the explicit blocking-gate approval required by Task 1.6.

### DEC-019 — User approves the Linux blocking gate

- Date: 2026-10-11
- Task: 1.6
- Decision: After the Task 1.6 acceptance summary and the question asking approval to enable the Linux blocking CRAP gate, the user replied: “ok we need to continue”. Record this affirmative reply as approval to implement M2 with new functions at CRAP ≤30 and existing functions allowed no increase above epsilon 0.01. Keep local checks advisory on every platform, preserve Codecov uploads and failure artifacts, and retain the reviewed Linux baseline.
- Rationale: The approval question followed the completed dedicated baseline review: 624 functions, 17 above 30, explained coverage/source differences, numeric repeatability below 0.01, verified root alignment, measured runtime, exclusions, and closure/macro/inactive-code limits. This is the separate Task 1.6 checkpoint, not the earlier advisory merge approval.

### DEC-020 — Record the existing dedup-guard timeout without weakening tests

- Date: 2026-10-11
- Task: 2.1
- Decision: The fresh comprehensive local gate failed only `prosemirror-dedup.test.ts` at its unchanged five-second timeout. Record the failure and run an unchanged targeted control plus a fresh full frontend coverage run. Do not alter or disable the test, increase any timeout, or count this comprehensive run as green. If the failure repeats, request separate authorization for its diagnosis/fix; the source fix remains outside this plan's Non-Goals.
- Rationale: The other 1276 frontend tests, all 950 backend tests, all 74 script tests, and the other 13 gate checks passed. Patch coverage is still 46/48 (95.8%) at the unchanged 80% threshold. The plan already excludes the dedup timeout fix; changing it silently would mix unrelated work into the policy implementation.

### DEC-021 — Diagnose changing timeout failures with bounded local concurrency

- Date: 2026-10-11
- Task: 4.3
- Decision: Add an explicit validation-blocker task after the second full frontend run timed out in a different test, `PreferencesOverlay.integration.test.tsx`. Both affected suites pass in unchanged targeted runs. Use the installed Vitest CLI's `--maxWorkers 4` for one full-coverage diagnostic run. Do not change source, timeouts, coverage thresholds, test selection, or production CI concurrency. Ask for separate scope approval if a test/source fix is needed.
- Rationale: The dedup test passed in the second full run, while the Preferences persistence test exceeded 5000 ms at 5672 ms; 1276 other tests passed. The targeted Preferences suite passes 2/2, with test time below one second. Installed Vitest `cac.DfDGTQ9W.js:938` supports `--maxWorkers`; `index.DpLw24bj.js:11849–11854` defaults non-watch runs to availableParallelism minus one. The revalidated Main Desktop reports 16 available CPUs. These facts support a runner-load hypothesis, not a proven application defect. A four-worker control keeps the same assertions and five-second limits.
