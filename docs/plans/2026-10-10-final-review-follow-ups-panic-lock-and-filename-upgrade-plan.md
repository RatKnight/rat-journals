# Final review follow-ups: panic lock and filename upgrade

## Metadata

- Plan Status: IN PROGRESS
- Plan Format: manual-planning v2.1.0
- Template: simple
- Tracking: tracked (this project already commits plans)

## Status Legend

- Plan Status values: DRAFT, QUESTIONS PENDING, READY FOR APPROVAL, APPROVED, IN PROGRESS, COMPLETED, BLOCKED
- Task/Milestone Status values: TO BE DONE, IN PROGRESS, COMPLETED, BLOCKED, SKIPPED

## Resume

<!--
The session handover. Written only by `plan-status.py <plan> resume`.
-->

- Updated: never
- State: not started
- Green / Red: not run
- Next runnable: 1
- Live environment: none
- Next IDs: 1; DEC-001
- Risks: none
- Stop: none — not started

## Context For A Clean Session

This plan replaces `docs/todo/TEMP_HANDOFF_final-review-p2-p3.md` (written 2026-10-10, deleted by
Task 1). Origin: the final review of commits `da8c19b` (TODO-0130), `2408a69` (TODO-0131),
`1318989` (TODO-0134), `497ec00` (TODO-0136), `5a82c70` (TODO-0093). It found two items:

- **Item A (P2, security):** since TODO-0134, four commands run their work in
  `tauri::async_runtime::spawn_blocking` through `run_blocking`. A panic there poisons the
  `DiaryState.db` mutex and the process keeps running. The lock primitive refuses a poisoned
  mutex, so the master key stays in memory and no lock path works.
- **Item B (P3, upgrade hazard):** since TODO-0131, every saved journal path is checked with the
  200-byte registration cap. A journal that an older version registered with a longer name
  (still legal for the filesystem) can no longer be opened, switched to, or reported as active.

- Repository: `D:\Repos\mini-diarium`, branch `master`, head `5a82c70`. At plan time the worktree
  had an uncommitted `docs/todo/TODO.md` change (adds TODO-0144..0146, marker at TODO-0146) and
  the untracked handoff file. Both belong to this work; Task 1 commits them.
- Stack: Rust 1.95.0 (`rustc --version`; `Mutex::clear_poison` is stable since 1.77), Tauri 2.11
  (`src-tauri/Cargo.toml:33`, features `devtools` only, no `test` feature), SolidJS + Vitest, bun.
- Shell: Windows. Run `cargo` bare from the repo root. Run bun through the PowerShell tool, or
  `MSYS_NO_PATHCONV=1 cmd.exe /c "bun run ..."` from Git Bash (plain `cmd.exe /c` from Git Bash
  silently does nothing). `rg` is not installed. Python: `uv run python ...`.
- Exact commands:
  - Test (backend): `cargo test --workspace`
  - Lint (backend): `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets -- -D warnings`
  - Frontend: `bun run type-check`, `bun run lint`, `bun run test:run`, `bun run validate:locales`
  - Full gate: `bun run pre-commit` (includes the coverage diff gate)
  - Website docs: `bun run website:build-static` (PowerShell tool only)
- Commits: never run `git commit` unless the user asks for it in this session. Each `Commit:`
  step means "stage the files, propose the message, and commit only on the user's request".
  No Claude attribution in any commit (no `Co-Authored-By`, no session footer).

### Repository facts

| Fact | Value | How it was verified |
| --- | --- | --- |
| `run_blocking` maps every `JoinError` to `"Background task failed"`, with no cleanup | `src-tauri/src/commands/mod.rs:31-39` | read |
| `run_blocking` callers | `attachments.rs:189` (attach), `backup_inspect.rs:271` (restore entries), `backup_inspect.rs:335` (`close_backup`), `plugin.rs:106` (plugin export) | `grep run_blocking` |
| The lock primitive refuses any poisoned mutex and leaves the connection in place | `src-tauri/src/commands/backup_triggers.rs:153-194` (`take_connection_and_snapshot`) | read |
| Every lock path reaches that primitive | `lock_diary` (`auth_core.rs:311`) and `auto_lock_diary_if_unlocked` (`auth/mod.rs:156`) call `lock_diary_inner` → `lock_diary_inner_with` (`auth/mod.rs:106`); idle timer and focus loss call frontend `lockJournal` → `lock_diary` (`src/App.tsx:53,83`); OS lock (`screen_lock.rs:79,206`), journal switch/remove (`auth_journals.rs:367,431`) call `auto_lock_diary_if_unlocked`; directory move / reset use `AwaitFileRelease` (`auth_directory.rs:165`, `auth_core.rs:414`); app exit calls the primitive directly (`lib.rs:120`) | `grep take_connection_and_snapshot\|lock_diary_inner\|auto_lock_diary_if_unlocked` |
| Tests that pin the current refusal (they must be inverted) | `backup_triggers.rs:322` (db_path poisoned), `backup_triggers.rs:338` (backups_dir poisoned), `auth/mod.rs:247` (path poisoned via `lock_diary_inner`). No test poisons `db` itself. | read |
| `close_inspection` already recovers a poisoned `inspection` mutex, but does not clear the poison | `backup_inspect.rs:348-357` | read |
| Restore holds `inspection` then `db` for the whole batch | `backup_inspect.rs:286-300` | read |
| Core write units roll back on an unwinding panic (drop guard) | `crates/mini-diarium-core/src/db/queries/transaction.rs:42,128`; tests `:310,:329` | grep |
| Frontend: `lockJournal` puts state back to `unlocked` on a rejected lock | `src/state/auth.ts:257-262` | read |
| Frontend: a `journal-locked` event forces the locked state unless a lock is in progress | `src/state/auth.ts:276-287` | read |
| Release profile has no `panic = "abort"` | root `Cargo.toml:9-11` | grep |
| `PluginRegistry` is mutated only at startup | `src-tauri/src/lib.rs:243`; read-only locks in `plugin.rs:17,27,54,179` and `debug.rs:138` | grep |
| Security skill says "a panic in a command aborts the process" | `.agents/skills/security-stance/SKILL.md:325` | grep |
| Filename rules and the single 200-byte cap | `crates/mini-diarium-core/src/config.rs:172-268` (`MAX_DB_FILENAME_BYTES`, `validate_db_filename`, `is_valid_db_filename(name, windows_rules)`, `journal_db_paths`) | read |
| Use-time callers (`journal_db_paths`) | `auth_journals.rs:67` (startup), `:97` (`get_active_journal_id_inner`), `:331` (removal fallback), `:404` (`switch_journal_inner`) | grep |
| Registration caller (`validate_db_filename`) | `auth_journals.rs:182` (`add_journal_inner`) | grep |
| Sibling files named after the journal file | `{name}-journal`/`-wal`/`-shm` (SQLite), `{name}.restoring.tmp` (`backup/store.rs:289`), `{name}.staging` (`auth_directory.rs:59`). Snapshot files do not contain the journal name (`store.rs:398`); the backups folder uses the stem, which is never longer than the name | grep |
| Frontend has no length check on the filename | `src/lib/native-path.ts`, `src/components/auth/JournalPicker.tsx` | grep `length\|byte` |
| 0.7.5 is unreleased, so its CHANGELOG block may be edited | `CHANGELOG.md:37` (`## [0.7.5] - Unreleased`); latest tag `v0.7.4` | grep, `git tag` |
| Inaccurate CHANGELOG sentence | `CHANGELOG.md:51` "a longer name was accepted before and then failed with an internal error" | grep |
| User docs list the 200-byte rule | `website/docs-src/00-getting-started.md:75` | grep |
| Pre-0.6.6 picker split dialog paths on `/` and `\` | `git show 2408a69^:src/components/auth/JournalPicker.tsx` lines 148-149, 202-203 | git show |
| TODO marker | `docs/todo/TODO.md:16` "Latest TODO ID: TODO-0146" (uncommitted change) | read |

### Hard constraints

1. Lock order `backup_ops → inspection → db` stays; the lock primitive never holds two
   `DiaryState` content locks at once with `backup_ops` (`backup_triggers.rs:146-152`) — breaking it
   opens an ABBA deadlock against `snapshot_after_unlock` / `snapshot_before_destructive`.
2. After any lock request, no path may leave a `DatabaseConnection` in `DiaryState.db` — that is
   the security regression this plan fixes.
3. Do not catch and swallow panics in `run_blocking` without locking — a poisoned mutex still
   blocks every DB command and the key stays in memory.
4. Never log user data at `info!`/`warn!`/`error!` (`src-tauri/CLAUDE.md` Gotcha #10) — those
   records reach the debug dump.
5. Control characters stay refused in every mode — the startup placeholder
   `no-active-journal\0` (`auth/mod.rs:44`) relies on it to never match a real journal.
6. `/` and `\` stay refused on every host, at registration and at use time — a `\` is a path
   component separator on Windows, so a moved `config.json` could escape the folder.
7. Never rewrite or redirect a saved filename silently (TODO-0131 rule) — it would point a journal
   at a different file than the one the user has.
8. Keep `is_valid_db_filename` parameterized on the Windows rule set — tests cover both rule sets
   on every CI platform through it.
9. Every new user-visible string is localized in all 7 locales (`src/i18n/locales/`:
   en.ts, de, es, fr, hi, it, pt-BR) — `bun run validate:locales` fails otherwise.

## Goal

A panic in any background task can no longer leave the journal unlocked: the backend locks the
journal at once (fail closed), every lock path works on a poisoned mutex, the user can unlock
again afterwards, and the UI says why it locked. A panic in a Rhai plugin no longer disables the
plugin commands until restart. Journals that earlier versions registered with names of 201 bytes
up to the filesystem-safe limit open, switch, and show as active again, while new registrations
keep the 200-byte rule, and the docs state the real rules and the recovery steps.

## Scope

- TODO entries for Item A (High) and Item B (Medium), via the `todo-manager` skill.
- Lock-path poison recovery in `take_connection_and_snapshot` and `close_inspection`.
- Fail-closed lock in `run_blocking` on a panic, plus a localized notice on the unlock screen.
- `PluginRegistry` poison recovery in `plugin.rs` and `debug.rs`.
- Split filename cap: registration 200 bytes, use time = 255 − longest sibling suffix.
- Docs: doc comments, `src-tauri/CLAUDE.md`, `docs/best-practices/TAURI_BEST_PRACTICES.md`,
  `docs/best-practices/RUST_BEST_PRACTICES.md`, `.agents/skills/security-stance/SKILL.md`,
  `crates/mini-diarium-core/API.md`, `CHANGELOG.md` (0.7.5), `website/docs-src/00-getting-started.md`
  (+ generated output), the TODO-0131 archive note.

## Non-Goals

- `panic = "abort"` in the release profile (rejected by the user, 2026-10-10).
- Mapping the raw `"Background task failed"` and `"No journal selected"` strings in `mapTauriError`
  — that is TODO-0145. This plan only adds the i18n key that TODO-0145 can reuse.
- Reason-specific filename errors (user chose the generic message + docs, 2026-10-10).
- An in-app rename of a refused journal file, or a first-launch migration.
- Relaxing Windows-only rules (device names, reserved characters, trailing dot/space) at use time.
- Moving more synchronous commands to `run_blocking` (TODO-0146).

## Assumptions

- A panic inside a synchronous Tauri command still ends the process on Windows (it unwinds into a
  WebView2 FFI callback, which aborts). [review, not verified] The fix does not depend on it: the
  lock-path recovery covers a poisoner from any thread.
- When the lock primitive takes the connection out of a poisoned `db` mutex, no SQLite transaction
  is open: core write units roll back during the unwind (`transaction.rs:128`), and dropping the
  connection closes it, which rolls back anything else. Verified by reading; Task 2 adds no snapshot
  on this path anyway.
- A poisoned `PathBuf` or `()` mutex holds a valid value: `PathBuf` writes are single assignments.
  A poisoned `db_path`/`backups_dir` pair may be inconsistent with each other, so this plan reads
  them only to decide "skip the snapshot", never to snapshot, and does not clear their poison.

## Open Questions

Asked 2026-10-10 with the question tool; all answered:

- Panic policy → **Recover in the lock path and auto-lock on a background-task panic** (not
  `panic = "abort"`, not lock-path-only).
- Rhai `PluginRegistry` poison → **include, in its own commit**.
- Filename approach → **split cap** (registration 200 bytes; use time ≈ 255 − longest suffix).
- Refusal message → **keep the generic `errors.invalidJournalFilename` message; docs explain
  reasons and the manual rename**.

## Tasks

### Task 1: Register the TODOs and retire the handoff file

- Status: COMPLETED
- Depends On: none
- Objective: `docs/todo/TODO.md` has one High-priority entry for Item A and one Medium-priority
  entry for Item B, each linking this plan; the handoff file is gone.
- Steps:
  1. Load the `todo-manager` skill and create Item A in `## High Priority`: "A panic in a background
     task must not leave the journal unlocked". Acceptance from this plan's Tasks 2-4; cross-link
     TODO-0145 (error mapping) and TODO-0146 (event-thread block). The skill assigns the ID
     (expected TODO-0147) and updates the marker.
  2. Create Item B in `## Medium Priority`: "Journals registered by older versions with long
     filenames must still open". Acceptance from Tasks 5-7. Expected ID TODO-0148.
  3. Add `Plan: [docs/plans/2026-10-10-final-review-follow-ups-panic-lock-and-filename-upgrade-plan.md](../plans/2026-10-10-final-review-follow-ups-panic-lock-and-filename-upgrade-plan.md)` to both entries.
  4. Replace "Item A"/"Item B" in this plan with the real IDs (Goal, Tasks 4, 7, Commit lines).
  5. Delete `docs/todo/TEMP_HANDOFF_final-review-p2-p3.md` (untracked; this plan now holds its facts).
  6. Commit: `docs(todo): add final-review follow-ups TODO-0144..0148 and their plan` — includes the
     already-uncommitted TODO-0144..0146 entries, the two new entries, and this plan file.
- Validation: the `todo-manager` skill's validation step passes; `git status --porcelain` shows no
  `TEMP_HANDOFF` file; `grep -c "TODO-0147\|TODO-0148" docs/todo/TODO.md` prints 2 or more.
- Notes: Never assign IDs by hand (root `CLAUDE.md` rule 4).

### Task 2: The lock primitive always takes the connection, even from a poisoned mutex

- Status: COMPLETED
- Depends On: 1
- Objective: With any of `backup_ops`, `db_path`, `backups_dir`, `db` poisoned, a lock request
  ends with `state.db == None`, the `db` mutex no longer poisoned, and `Ok(Some(rx))` whose `rx`
  fires; with nothing poisoned, behaviour is unchanged (detached `Lock` snapshot).
- Steps:
  1. Write the failing tests first in `backup_triggers.rs` `mod tests` (reuse its `poison` helper):
     `test_lock_takes_the_connection_from_a_poisoned_db_mutex`,
     `test_lock_takes_the_connection_when_backup_ops_is_poisoned`, and invert the two existing
     tests at `:322` and `:338` into `..._drops_the_connection_without_a_snapshot_when_db_path_is_poisoned`
     / `..._backups_dir_is_poisoned`. Each asserts: result `Ok(Some(rx))`, `rx.recv_timeout(5s)` ok,
     `state.db.lock()` is `Ok` (poison cleared) and holds `None`, and no snapshot was written
     (`list_snapshots` empty). Invert `auth/mod.rs:247` the same way through `lock_diary_inner`
     (expects `Ok(true)`). Run them and see them fail.
  2. In `take_connection_and_snapshot`, recover each guard with
     `lock().unwrap_or_else(PoisonError::into_inner)` and record whether any was poisoned. Keep each
     guard a temporary that drops at the end of its statement (hard constraint 1).
  3. After `.take()` from `db`, call `state.db.clear_poison()` (the value is now `None`, which is
     always consistent). Do not clear the poison of the path mutexes (see Assumptions).
  4. If anything was poisoned: drop the connection on this thread (closes SQLite, zeroizes the
     key), log one `warn!` with no path or user data ("a state lock was poisoned; locked without a
     snapshot"), and return `Ok(Some(rx))` where `rx` already holds a sent `()` — so
     `LockCompletion::AwaitFileRelease` callers and the exit hook (`lib.rs:120-134`) keep working
     with no signature change. Otherwise keep the existing detached-snapshot path.
  5. In `close_inspection` (`backup_inspect.rs:348`), after the take, call
     `state.inspection.clear_poison()`; add `test_close_inspection_clears_a_poisoned_mutex`
     (open a fake inspection is not needed: poison an empty mutex, call, assert `lock().is_ok()`).
  6. Rewrite the doc comment of `take_connection_and_snapshot` (`backup_triggers.rs:130-152`):
     "`Err` means a state mutex was poisoned" becomes the new contract (poison → lock without a
     snapshot; the function never returns `Err` for poison). If the function no longer returns
     `Err` at all, keep the `Result` only if a caller still needs it; record the choice in the
     Decision Log.
  7. Update `src-tauri/CLAUDE.md` Gotcha #11 ("The lock path moves the connection") with one bullet:
     a poisoned state mutex never blocks a lock — the connection is dropped without a snapshot and
     the `db` poison is cleared. Add the same rule to `docs/best-practices/RUST_BEST_PRACTICES.md`
     next to the lock-scope rules.
  8. Add a `CHANGELOG.md` 0.7.5 `### Security` (or `### Fixed`) bullet tagged with TODO-0147.
  9. Commit: `fix(lock): lock the journal even when a state mutex is poisoned (TODO-0147)`.
- Validation: `cargo test --workspace` green; the five named tests exist (`grep -n` each name) and
  were seen failing before step 2; `cargo clippy --workspace --all-targets -- -D warnings` clean;
  `grep -n "Journal state lock failed" src-tauri/src/commands/backup_triggers.rs` shows no match in
  `take_connection_and_snapshot` (grep exits 1 when nothing matches — that is the pass).
- Notes: `snapshot_detached` already skips on a poisoned `backup_ops` (`backup_triggers.rs:105-121`);
  leave it.

### Task 3: A background-task panic locks the journal and tells the user

- Status: COMPLETED
- Depends On: 2
- Objective: When a `run_blocking` closure panics, the backend locks the journal through the
  normal auto-lock path (emits `journal-locking` then `journal-locked` with reason
  `background-task-failed`), the command still returns `"Background task failed"`, and the unlock
  screen shows a localized notice.
- Steps:
  1. Split `run_blocking` (`commands/mod.rs:31`) so the panic branch is testable without an
     `AppHandle`: e.g. `fn lock_after_background_panic(state: &DiaryState) -> bool` that calls
     `lock_diary_inner` and logs `warn!("A background task panicked; journal locked")` (no payload,
     no user data). `run_blocking` matches `Err(e) if e.is_panic()` → calls it with
     `app.state::<DiaryState>()`, then emits through the existing
     `auto_lock_diary_if_unlocked(state, app, "background-task-failed")` path (prefer calling that
     function directly if it fits; it already emits both events). A cancelled (non-panic)
     `JoinError` keeps today's behaviour.
  2. Check `tauri::async_runtime::JoinError` exposes `is_panic()` in Tauri 2.11 (open the installed
     source under `~/.cargo/registry/src/*/tauri-2.11*/src/async_runtime.rs`, root `CLAUDE.md`
     rule 5). If it does not, treat every `JoinError` as a panic and record it in the Decision Log.
  3. Tests (app crate): `test_background_panic_locks_a_journal_whose_db_mutex_is_poisoned` — open a
     DB into a `make_state` state, poison `db` with a thread that panics while holding it, call the
     extracted function, assert `state.db` is `None` and not poisoned, and a second
     `create_database`/unlock-style `*state.db.lock().unwrap() = Some(..)` works. Plus one test for
     the not-unlocked case (returns false, no panic). See them fail before step 1.
  4. Frontend: in `setupAuthEventListeners` (`src/state/auth.ts:276`), when
     `reason === 'background-task-failed'`, set the auth `error` signal to a new message key
     `errors.backgroundTaskFailed` (wording: "Something went wrong in the background, so the
     journal was locked to keep it safe. Unlock it to continue."). First confirm that
     `PasswordPrompt.tsx:211` renders this module's `error()`; if it renders a local signal, show
     the notice through whatever the unlock screen reads and record the choice. Check that
     `resetForLockedSession` does not clear the error after it is set (set it after the reset).
  5. Add the key to `src/i18n/locales/en.ts` and the six JSON locales; run `bun run validate:locales`.
  6. Vitest: in `src/state/auth.test.ts`, emit a mocked `journal-locked` with that reason and assert
     the state is `locked` and `error()` is the translated text; a second test with reason `manual`
     asserts no error. Note in TODO-0145 that `"Background task failed"` should map to this key.
  7. Docs: `docs/best-practices/TAURI_BEST_PRACTICES.md` "Keep Heavy Work Off The Event Thread"
     (line ~98): panics in `run_blocking` are survivable; the wrapper locks the journal and the lock
     path recovers poison; never swallow a panic without locking.
     `.agents/skills/security-stance/SKILL.md:325`: correct "a panic in a command aborts the
     process" — true only for synchronous commands; `run_blocking` panics lock the journal.
     `website/docs-src/07-preferences.md` (auto-lock section) — one sentence that the journal also
     locks after an internal error; then `bun run website:build-static` (PowerShell tool).
  8. `CHANGELOG.md` 0.7.5 bullet; mark TODO-0147 done with the date (todo-manager).
  9. Commit: `fix(commands): lock the journal when a background task panics (TODO-0147)`.
- Validation: `cargo test --workspace`; `cargo clippy --workspace --all-targets -- -D warnings`;
  `bun run type-check`, `bun run lint`, `bun run test:run`, `bun run validate:locales` all pass; the
  named Rust and Vitest tests exist and were seen failing first.
- Notes: Do not edit `.claude/skills/` (generated mirror; root `CLAUDE.md` Gotcha #3). The frontend
  `journal-locking` listener runs the editor flush callbacks; after a panic those saves fail
  harmlessly with "Journal must be unlocked" — same as an OS lock today.

### Task 4: A Rhai plugin panic does not disable the plugin commands

- Status: COMPLETED
- Depends On: 1
- Objective: With the `Mutex<PluginRegistry>` poisoned, `list_import_plugins`,
  `list_export_plugins`, `run_import_plugin`, `run_export_plugin`, and `generate_debug_dump` still
  work.
- Steps:
  1. Failing test first in `plugin.rs` tests: poison a `Mutex<PluginRegistry>` (thread panics while
     holding it), then call the testable core that lists exporters; assert it succeeds. Extract a
     small `list_*_inner(&Mutex<PluginRegistry>)` if the command takes `State` directly.
  2. Add `fn lock_registry(registry: &Mutex<PluginRegistry>) -> MutexGuard<'_, PluginRegistry>` in
     `plugin.rs` using `unwrap_or_else(PoisonError::into_inner)` plus `clear_poison()`, with a
     comment: the registry is built once at startup (`lib.rs:243`) and only read afterwards, so a
     panic during a read cannot leave it half-changed.
  3. Replace the five `.map_err(|_| "Registry lock poisoned")` sites (`plugin.rs:17,27,54,179`,
     `debug.rs:138` area) with it. Remove `"Registry lock poisoned"` from `src/lib/errors.ts` only if
     it is matched there and now unreachable (grep first).
  4. `CHANGELOG.md` 0.7.5 bullet under TODO-0147.
  5. Commit: `fix(plugin): recover the plugin registry after a plugin panic (TODO-0147)`.
- Validation: `cargo test --workspace`; clippy clean; the new test exists and failed before step 3.
- Notes: Independent of Tasks 2-3; may run before them. If Task 3 already closed the TODO, append
  to its note instead of reopening it.

### Task 5: Fix the use-time cap from the sibling-file inventory

- Status: COMPLETED
- Depends On: 1
- Objective: A Decision Log entry fixes `MAX_DB_FILENAME_BYTES_IN_USE` with evidence.
- Steps:
  1. Inventory every file the app or SQLite creates whose name is `{db filename}` + suffix: known
     `-journal` (8), `-wal` (4), `-shm` (4), `.restoring.tmp` (14), `.staging` (8). Search again
     (`grep -rn "format!(\"{" crates/mini-diarium-core/src src-tauri/src` and `with_file_name`).
  2. Check whether SQLite ever opens the staged copies (`.restoring.tmp` in `backup/store.rs`
     `stage_restore_copy` callers; `.staging` in `auth_directory.rs`). If it does, add `-journal`
     (8) to that suffix (`.restoring.tmp-journal` = 22).
  3. Cap = 255 − longest suffix (expected 241, or 233 if step 2 finds an opened stage). UTF-8 bytes
     ≥ UTF-16 units, so the byte cap is safe on NTFS too (`config.rs:174-176`).
  4. Record DEC-00x with the table and the number.
- Validation: the Decision Log entry exists and cites the grep commands.
- Notes: Research only; no code change, no commit.

### Task 6: Split the filename cap: registration 200, use time = Task 5 value

- Status: COMPLETED
- Depends On: 5
- Objective: `validate_db_filename` (registration) still refuses > 200 bytes; `journal_db_paths`
  (use time) accepts names up to the Task 5 cap and refuses everything else it refused before.
- Steps:
  1. Failing tests first in `crates/mini-diarium-core/src/config/tests.rs`:
     `test_journal_db_paths_accepts_a_saved_name_up_to_the_use_time_cap` (both rule sets via the
     parameterized helper, boundary at cap and cap + 1, ASCII and 4-byte UTF-8 names via the
     existing `names_of_byte_length`), and
     `test_registration_still_refuses_names_over_200_bytes`. Extend the real-filesystem test at
     `tests.rs:455-480` to the use-time cap: create the database, write an entry, and create the
     longest sibling file next to it.
  2. In `config.rs`: add `MAX_DB_FILENAME_BYTES_IN_USE` with a doc comment (why it differs: old
     versions registered longer names; the filesystem still holds them). Change
     `is_valid_db_filename(name, windows_rules)` to take the cap as a parameter (or a
     `FilenameCheck::{Register, Use}` enum) — keep one rule body so the two paths cannot drift.
     `validate_db_filename` stays the registration check; `journal_db_paths` uses the use-time cap.
  3. App-crate tests in `auth_journals.rs` tests: a saved `config.json` journal with a 220-byte name
     → `startup_journal_paths` returns its real path (not the placeholder), `get_active_journal_id_inner`
     returns its id, `switch_journal_inner` switches to it, and `add_journal_inner` with the same
     220-byte name is refused. One saved-config case each for a still-refused class (control
     character, `\`, over the use-time cap) returns the placeholder / `None`. Reuse existing fixtures
     there; check which classes already have tests and add only the missing ones.
  4. Keep the NUL placeholder test green (hard constraint 5).
- Validation: `cargo test --workspace`; clippy clean; named tests exist and failed before step 2.
- Notes: The frontend has no length rule, so no `src/` change.

### Task 7: TODO-0148 docs, CHANGELOG, and TODO closure

- Status: COMPLETED
- Depends On: 6
- Objective: Every doc states the real rules, the upgrade case, and the recovery steps.
- Steps:
  1. `CHANGELOG.md` 0.7.5: fix line 51 — names of 201 to about 247 bytes worked before; about 248
     and more failed with "disk I/O error" when the database was created. State that journals
     registered before 0.7.5 with names up to the use-time cap keep working; new journals are
     limited to 200 bytes. Update the developer bullet at line 62 too.
  2. `crates/mini-diarium-core/API.md`: the two caps and which function uses which.
  3. `website/docs-src/00-getting-started.md:75`: the two limits in plain words, and a short
     "If a journal shows 'invalid journal filename'" recovery: close the app, rename the `.db` file
     to a shorter or plain name, then add it again with "Open existing". Then run
     `bun run website:build-static` with the PowerShell tool (expect `website/sitemap.xml` lastmod
     changes too).
  4. `docs/todo/TODO_ARCHIVE.md` TODO-0131 note (line 12): one sentence that TODO-0148 relaxed the
     use-time cap.
  5. Mark TODO-0148 done with the date (todo-manager).
  6. Commit: `fix(journals): keep journals with long legacy filenames usable (TODO-0148)` — Tasks
     6 and 7 together (code, tests, docs).
- Validation: `grep -n "internal error" CHANGELOG.md` shows no match in the 0.7.5 block (exit 1 is
  the pass); `bun run website:build-static` exits 0 and `git status` shows only expected generated
  files under `website/`.
- Notes: Use ASD-STE100 style in user docs (short sentences, active voice). Blog voice rules: no em
  dashes in website copy.

### Task 8: Cleanup Intermediate Artifacts

- Status: TO BE DONE
- Depends On: 3, 4, 7
- Objective: Remove artifacts created only to support implementation.
- Steps:
  1. Inspect the worktree for temporary documentation, one-off scripts, scratch tests, generated data, logs, and obsolete plan fragments.
  2. Remove only artifacts that are not part of the intended final repository state.
  3. Keep maintainable tests, fixtures, docs, and generated files that are part of the repository contract.
  4. Confirm `docs/todo/TEMP_HANDOFF_final-review-p2-p3.md` is gone (Task 1).
- Validation: `git status --porcelain` lists only intended changes (or nothing, after all commits).
- Notes: Do not remove user-provided files or unrelated worktree changes.

### Task 9: Final Verification

- Status: TO BE DONE
- Depends On: 8
- Objective: Validate the integrated change after cleanup.
- Steps:
  1. Run every item in `## Pre-flight Checks`.
  2. Run the checks under `## Final Verification`.
  3. Fix failures and rerun until verification passes, or record the blocker.
  4. Set this plan to `COMPLETED`; commit the plan status with the last commit or as
     `docs(plans): close final-review follow-ups plan` on the user's request.
- Validation: all pre-flight checks green.
- Notes: None.

## Project Gates

- Commits only on the user's explicit request; no Claude attribution (user memory).
- Post-task checklist: `docs/best-practices/POST_TASK_BEST_PRACTICES.md` (tests, formatting,
  CHANGELOG, TODO closure, summary template) for each TODO.
- CHANGELOG: every user-visible fix gets a bullet in the `## [0.7.5] - Unreleased` block.
- Docs: `website/docs-src/` must match behaviour in the same task (root `CLAUDE.md`).
- Codecov patch ≥ 80%: `bun run coverage:diff` (part of `bun run pre-commit`).
- Optional external review per commit (Herdr opencode pane, as in the previous batch) only if the
  user asks for it.

## Pre-flight Checks

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace`
- [ ] `bun run pre-commit` (type-check, lint, Vitest, Rust, coverage diff gate)
- [ ] `bun run validate:locales`
- [ ] `bun run diagrams:check`
- [ ] `git grep -n "state.db.lock().unwrap" -- src-tauri/src ':!*tests*'` shows only `#[cfg(test)]` code

## Decision Log

### DEC-001 — Fail closed on a background panic, recover poison in the lock path

- Date: 2026-10-10
- Task: 2, 3
- Decision: The lock primitive recovers every poisoned state mutex and drops the connection
  without a snapshot; `run_blocking` locks the journal on a panic `JoinError`. No `panic = "abort"`.
- Rationale: User choice (question tool, 2026-10-10). The lock-path fix covers a poisoner on any
  thread; the auto-lock removes the key at once instead of waiting for the next lock trigger;
  `abort` would also kill the backup and log workers.

### DEC-002 — Split filename cap, generic error message

- Date: 2026-10-10
- Task: 5, 6, 7
- Decision: Registration keeps 200 bytes; use time allows 255 − longest sibling suffix; refused
  journals keep the one `errors.invalidJournalFilename` message, and the docs give the recovery.
- Rationale: User choice (question tool, 2026-10-10). Existing data keeps working with no rename,
  no new error strings (TODO-0139 owns the error-contract work), and the security rules (separators,
  control characters, Windows host rules) do not change.

### DEC-003 — Unlock-screen notice uses a dedicated lockNotice signal

- Date: 2026-10-11
- Task: 3
- Decision: src/state/auth.ts exports a new lockNotice signal (reason code or null), set after resetForLockedSession on a journal-locked event with reason background-task-failed and cleared on unlock or picker navigation; PasswordPrompt.tsx renders t('errors.backgroundTaskFailed') when it is set. The auth error() signal is not used.
- Rationale: Exploration: PasswordPrompt.tsx:24 renders its own local error signal, not error() from state/auth.ts, and state/auth.ts has no t() to translate a key. A reason signal keeps translation in the component.

### DEC-004 — Tauri is 2.12.0; JoinError panic check confirmed

- Date: 2026-10-11
- Task: 3
- Decision: Cargo.lock pins tauri 2.12.0 (plan says 2.11). Awaiting async_runtime::spawn_blocking yields tauri::Result<T>; the panic case is tauri::Error::JoinError(e) with e.is_panic() (tauri-2.12.0/src/error.rs:73, tokio JoinError).
- Rationale: Root CLAUDE.md rule 5: verified in the installed source before use.

### DEC-005 — Use-time filename cap is 233 bytes

- Date: 2026-10-11
- Task: 5
- Decision: Sibling suffixes: -journal 8, -wal 4, -shm 4 (SQLite, journal opened read-write), .staging 8 (auth_directory.rs:59, only copied and renamed, never opened by SQLite), .restoring.tmp 14 (backup/store.rs:289) which precheck_restorable opens read-only with SQLite (backup/restore.rs:236), so SQLite probes .restoring.tmp-journal = 22. Cap = 255 - 22 = 233. Found with grep 'restoring\.tmp|\.staging|with_file_name|format!' over crates/ and src-tauri/src, and grep stage_restore_copy/open_connection in backup/restore.rs.
- Rationale: Task 5 step 2: the staged restore copy is opened by SQLite, so its -journal probe name must also fit in 255 bytes. UTF-8 bytes >= UTF-16 units, so the cap is safe on NTFS.

### DEC-006 — take_connection_and_snapshot returns Option, not Result

- Date: 2026-10-11
- Task: 2
- Decision: The function no longer returns Err for poison, and nothing else can fail, so its return type is Option<mpsc::Receiver<()>>. lock_diary_inner_with drops its ?; the exit hook in lib.rs uses if let Some.
- Rationale: Task 2 step 6: keep Result only if a caller still needs it. No caller does; an always-Ok Result would hide the new contract.

### DEC-007 — allow(clippy::incompatible_msrv) on clear_poison, MSRV unchanged

- Date: 2026-10-11
- Task: 2
- Decision: src-tauri/.clippy.toml declares msrv 1.75; clear_poison is stable since 1.77. Each of the two clear_poison calls carries #[allow(clippy::incompatible_msrv)] with a why comment. The MSRV setting is not changed.
- Rationale: cargo metadata shows tauri 2.12 crates need rust-version 1.90, so 1.75 is stale. Raising it to 1.90 fires a new manual_inspect lint in auth_core.rs:467, unrelated to this plan; that cleanup is out of scope.

### DEC-008 — Recovery steps exclude local-only journals

- Date: 2026-10-11
- Task: 7
- Decision: The docs and CHANGELOG say the rename-and-Open-existing recovery works only for password and key-file journals, tell the user to remove the old list entry, and send local-only users to Send feedback.
- Rationale: A local-only journal's key is saved in its config.json list entry (JournalConfig.auto_key); a new entry made by Open existing has no key, so renaming the file would lock the user out.

## Final Verification

- Task 9 runs the pre-flight checks.
- Manual (optional, Windows): with the `tauri-agent-dev` skill, unlock a journal, and confirm the
  Lock button, idle lock, and Quit still lock and close normally (no regression on the unpoisoned
  path).
- `git log --oneline -6` shows the planned commits: `docs(todo)`, `fix(lock)`, `fix(commands)`,
  `fix(plugin)`, `fix(journals)` (+ optional `docs(plans)`).

## Approval Gate

Approved by the user on 2026-10-11 ("Implement docs/plans/…"); commit per plan step, also approved
on 2026-10-11.

## Plan Self-Check

Paste the output of `check-plan.py` here, with the date it was run:

```
$ uv run python .claude/skills/manual-planning/scripts/check-plan.py docs/plans/2026-10-10-final-review-follow-ups-panic-lock-and-filename-upgrade-plan.md
W001 warning 0: Tracking says tracked but git does not track this file
0 error(s), 1 warning(s)
```

Run: 2026-10-10

W001 is acceptable: the plan is committed in Task 1 (`docs(todo)` commit), which clears it.

## Execution Notes

- Start each session with `plan-status.py <plan> brief`. Before stopping, run `plan-status.py <plan>
  resume` and end with `Stop: <reason> — <detail>`.
- Set status with `plan-status.py set` before starting a task and right after its validation passes.
- Follow `Depends On`, not task numbering. TODO-0148 (Tasks 5-7) and Task 4 can run in parallel
  with Tasks 2-3.
- Write a `## Decision Log` entry **before starting the next task** whenever execution diverges from
  this plan, an unplanned problem is found, or a validation is deferred — never retrospectively.
- Commit strategy (one logical change per commit, root `CLAUDE.md` rule 6):
  1. `docs(todo): …` — Task 1 (TODO entries + this plan).
  2. `fix(lock): …` — Task 2.
  3. `fix(commands): …` — Task 3 (closes TODO-0147).
  4. `fix(plugin): …` — Task 4.
  5. `fix(journals): …` — Tasks 6 + 7 (closes TODO-0148).
