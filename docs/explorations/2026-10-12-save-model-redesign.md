# Exploration Output: Save Model Redesign (Discussion #263)

> Artifact type: Documentation + Proposal (working document, updated as the exploration continues)
>
> IMPORTANT: This artifact contains NO implementation code. Only prose, diagrams,
> tables, and references. Implementation happens in a different mode.

## Status

| Thread | State |
|--------|-------|
| Prior work check | Done: no earlier exploration of save cadence exists |
| Current-model map | Done (code reading) |
| **A**: browsing rewrites unchanged entries | Done: confirmed in the live app |
| Wider issue sweep | Done: I1–I27 (I13–I27 from external review rounds 1–3) |
| Design v1 | Superseded (review round 1: 7 blockers) |
| Design v2 | Superseded (review round 2: 4 blockers, 12 majors) |
| Design v3 | Superseded (review round 3: 2 blockers, 6 majors; architecture converged) |
| Design v4 | Converged in round 5, but too large for one project; kept as reference |
| **Design v5 (simplified core)** | **Converged** 2026-10-13: round 8 confirms ready for an implementation plan in K1–K5 order (0 blockers, 0 majors) |
| Maintainer decisions | **All 5 decided** 2026-10-13 (see "Decisions for the maintainer") |

Last updated: 2026-10-12

## Context

- **Why this exploration started**: [Discussion #263 "Auto-save design"](https://github.com/fjrevoredo/mini-diarium/discussions/263) (paulkater, 2026-08-17). While typing, the journal file changes almost continuously, so Nextcloud re-uploads it constantly. The user moved the journal out of the synced folder and copies it hourly with cron. They suggest a save every 30–60 s and offered to share design ideas.
- **Maintainer reply (2026-08-18)**: the effective ~500 ms is "definitely too aggressive"; many events now trigger a save as a side effect of shipped features; the save model is to be revisited after the backup overhaul (now done).
- **Chosen direction**: fix the save model as a whole, not only the interval. The cadence becomes two constants inside a correct model.
- **Date**: 2026-10-12
- **Sources consulted**: editor-panel hooks (`useEntryPersistence.ts`, `useEntryLifecycle.ts`, `useMultiEntryNav.ts`, `entryHydration.ts`, `useEditorEmptyCheck.ts`), `EditorPanel.tsx`, `DiaryEditor.tsx`, `MainLayout.tsx`, `App.tsx`, `src/lib/focus-lock.ts`, `src/state/{entries,ui,auth,journals,session,entryAttachments}.ts`, `BackupInspectDialog.tsx`, `BackupsPanel.tsx`, `ExportOverlay.tsx`, `StatsOverlay.tsx`, `SearchBar.tsx`, `src-tauri/src/{lib.rs,menu.rs,screen_lock.rs}`, `src-tauri/src/commands/{entries,tags,attachments,plugin,export,backup,backup_triggers}.rs`, `commands/auth/{mod,auth_core,auth_journals}.rs`, core `entries/{update,read,blank}.rs`, `images/{refs,storage,summaries}.rs`, `backup/{policy,restore,restore_entries}.rs`, `db/schema/mod.rs`, installed `muda 0.20.0` / `tao 0.37.1` / `tauri-runtime-wry 2.12.1` sources, the TODO-0089 / TODO-0104 plans, live experiments in the `tauri-agent-dev` sandbox, and two external review rounds by an OpenCode agent (herdr pane `wK:p4`).

## Prior Work

No earlier exploration of save cadence or the save model exists. Related work made saves **correct**, not less frequent:

- TODO-0089: entries that disappeared or lost their body during navigation; introduced atomic commit, the `hydratedEntryId` gate, and payload snapshots.
- TODO-0104: consent gate before an entry with real content is erased (`canLeaveCurrentEntry`, backend on-disk check in `delete_entry_if_empty`), plus the cancel-restore addendum. The guard deliberately excludes entries with attachments, so "Delete entry" never takes files silently.

Any redesign must keep every invariant those tasks introduced.

## Findings: Current Model

### Write channels and trigger sites

```
                         ┌──────────── debouncedSave (500 ms trailing) ───┐
 body keystroke ─────────┤ EditorPanel.tsx  handleContentUpdate           │
 title keystroke ────────┤ EditorPanel.tsx  handleTitleInput              │
 font/metadata change ───┤ EditorPanel.tsx  onEntryMetadataChange         │
 blank entry loaded ─────┤ EditorPanel.tsx  onSetContent  (← a DELETE)    │
 createEntry resolved ───┘ useEntryLifecycle.ts startEntryCreation        │
                                                                          ▼
                         ┌──────────── flushCurrent(path) ──────▶ saveCurrentById ──▶ save_entry
 date change ────────────┤ loadEntriesForDate                            │        └─▶ delete_entry_if_empty
 entry prev/next ────────┤ navigateToEntry                               │        └─▶ getAllEntryDates
 "+" add entry ──────────┤ addEntry                                      │
 entry lock toggle ──────┤ toggleLock                                    │
 journal lock/switch/    ┤ lockCleanup (executeCleanupCallbacks from     │
   backup restore        │   auth.ts ×2, journals.ts, BackupsPanel.tsx)  │
 window close ───────────┘ beforeunload (not awaited)                    │
                         ┌──────────── writeSnapshot (no UI guard) ──────┘
 Timeline toggle unmount ┤ dispose
 nav during createEntry ─┘ flushPendingCreation
                         ┌──────────── canLeaveCurrentEntry ──▶ delete_entry (after a dialog)
 9 navigation points ────┘ (TODO-0104 consent guard)
```

Around these sites there are 7 `debouncedSave.cancel()` calls and 6 race guards: `hydratedEntryId`, `saveRequestId`, `loadRequestId`, `navToken`, `justCreatedEntryId`, `leaveCheckInFlight`.

### The current loss window

The 500 ms timer is a **trailing debounce with no maximum wait**: each edit resets it. So the current behavior is "a write 500 ms after the last edit, if the timer fires and the write succeeds". During continuous typing there is **no bound at all**. A failed write has no automatic retry or backoff; it is sent again only when a later trigger fires (the next edit, navigation, lock cleanup, or dispose).

## Thread A: Does browsing rewrite unchanged entries?

**Result: CONFIRMED (2026-10-12, live dev app).** Leaving an entry writes it again even when nothing changed, and every such write moves `date_updated`.

**Method.** `tauri-agent-dev` sandbox, fresh password journal. A `window.fetch` wrapper logged every IPC call (log only, nothing faked; lengths only, no content). Two entries on today, typed in each, waited for the debounce, read `date_updated`, browsed **without typing**, read it again.

| Action (no edits) | Write seen | Source path |
|--------|-----------|-------------|
| "+" add entry (entry 1 already saved) | `save_entry id=1`, same length | `addEntry` → `flushCurrent` |
| Click entry 1 (from 2) | `save_entry id=2` | `navigateToEntry` → `flushCurrent` |
| Click entry 2 (from 1) | `save_entry id=1` | `navigateToEntry` → `flushCurrent` |
| Previous day | `save_entry id=2` | `loadEntriesForDate` → `flushCurrent` |
| Next day (from an empty day) | none | nothing hydrated |
| Timeline on | `save_entry id=2` | `dispose` → `writeSnapshot` |
| Timeline off | none | none |
| Journal lock | `save_entry id=2` before `lock_diary`, and again after it | `lockCleanup`, then `dispose` |

`date_updated` moved for both entries (21:15:07 → 21:15:25 and 21:15:09 → 21:15:31).

Consequences: (1) reading an entry changes its "Updated" timestamp, a user-visible bug independent of cadence; (2) browsing alone produces writes and file changes in a synced folder; (3) in this experiment the post-lock `dispose` write was redundant and failed with "Journal must be unlocked". That failure is not a guaranteed fence: in other orderings the same late write could delete, no-op, or (if delayed past a re-unlock) succeed against a later session (see I14).

## Issue Inventory

"Confirmed" = reproduced in the live dev app on 2026-10-12. "Code" = derived from source. "Review" = raised by the external reviewer; ✓ = spot-checked against source by me.

| ID | Issue | Evidence | Severity |
|----|-------|----------|----------|
| I1 | Leaving an entry writes it again without edits; `date_updated` moves on read-only browsing | Confirmed (Thread A) | Bug, user-visible |
| I2 | Lock path sends a redundant post-lock `dispose` write | Confirmed (one ordering) | Smell; see I14 |
| I3 | **Window close loses unsaved text.** The close hook (`lib.rs` `install_exit_snapshot_hook`) holds the window, then a worker takes the connection; the frontend `beforeunload` flush is not awaited and loses the race | Confirmed: typed `SAVEDPART`, waited, typed ` UNSAVEDTAIL`, closed at once; after reopen only `SAVEDPART` | **Data loss** |
| I4 | Every save sends the whole entry including every image as a data URL; the backend decodes, fingerprints, prepares thumbnail metadata and encrypts every image again before `INSERT OR IGNORE` | Confirmed: one 600×600 PNG → 1.65 MB `save_entry` body per one-character edit. Backend cost: `images/storage.rs` | Performance |
| I5 | 500 ms trailing debounce: one full write per typing pause (#263); **no max-wait**, so continuous typing is never saved | Confirmed (IPC log); `lib/debounce.ts` | Requirement + unbounded loss |
| I6 | **OS session lock / suspend loses unsaved text.** `auto_lock_diary_if_unlocked` emits `journal-locking` and locks in the same call, from the OS callback | Code | Data loss |
| I7 | Search, export, statistics, print/PDF, "Back up now" read the DB without an editor barrier and without binding to a DB session | Code; review R2-12 | Minor now; real with a longer interval |
| I8 | Save failures are swallowed; navigation proceeds after a failed save and loads the next entry | Code; review F7 | **Major now** |
| I9 | Save and blank-entry delete share one timer (`onSetContent` delete, `justCreatedEntryId`, "Entry not found" race) | Code | Complexity / race surface |
| I10 | `getAllEntryDates` after every save | Confirmed (IPC log) | Minor cost |
| I11 | Writes are not ordered: overlapping requests are not cancelled once sent; request tokens guard UI updates, not the committed row | Code; review F7 | Major |
| I12 | 13 trigger sites, 7 `cancel()` calls, 6 race guards over 4 files | Code | Maintainability |
| I13 | **Native Quit bypasses every exit hook.** On Windows the predefined Quit item calls `PostQuitMessage`; tao's `GetMessageW` loop breaks and tauri-runtime-wry maps loop-destroyed straight to `RunEvent::Exit`, so neither `CloseRequested` nor a preventable `ExitRequested` runs | Review M1, R2-02 ✓ (`menu.rs:73`; `muda-0.20.0 …/windows/mod.rs:1172-1173`; `tao-0.37.1 …/event_loop.rs:250-254`; `tauri-runtime-wry-2.12.1 lib.rs:4131-4133`) | Data loss path |
| I14 | **Entry mutations are not bound to a DB session.** `save_entry`/`delete_entry_if_empty`/`delete_entry`/`set_entry_locked` take an entry id, `create_entry` takes a date; none carries a session identity. Ids repeat across journals and restored files, so a delayed or retried call after switch, lock/re-unlock or whole restore can hit an unrelated row | Review M2 ✓ (`session_id()` in core `db/schema/mod.rs:60`, `with_same_session` in `commands/auth/mod.rs:193`, not used by entry commands) | **Blocker for any longer interval or retry** |
| I15 | **Selective backup restore discards unsaved edits** when a restored entry lands on the open date: it calls the discard-without-save reload | Review M3 ✓ (`BackupInspectDialog.tsx:243-245`) | Data loss (narrow) |
| I16 | **Attachment-lookup failure turns a cleared entry into a blank save.** `entryHasAttachments` returns `true` on error → `isBlankEntry` false → `save_entry` writes the blank over real text | Review M4 ✓ (`entryAttachments.ts:129-133`) | Data loss (on IPC error) |
| I17 | **Destroyed-editor emptiness differs.** `computeIsEmpty` falls back to `!content.trim()`, so `<p></p>` counts as non-empty in teardown; dispose can save a blank shell over a real row | Review M5 ✓ (`useEditorEmptyCheck.ts:24`) | Data loss (teardown) |
| I18 | Async content producers (image paste/drop resize, Markdown import after a native dialog) insert into the captured editor without re-checking which entry it now shows | Review M6 (`DiaryEditor.tsx`, `EditorPanel.tsx:217-255`) | Wrong-entry content |
| I19 | `beforeunload` does not call `flushPendingCreation`; the pending-create flush captures once and misses input typed while the create is in flight | Review M7 (`EditorPanel.tsx:259-263`, `useEntryLifecycle.ts:390-417`) | Data loss (narrow) |
| I20 | A second close request skips `prevent_close` (the guard returns first), and the `closing` flag never resets | Review M9, R2-13 (`lib.rs:96-131`) | Data loss path |
| I21 | Unnecessary writes **can** cause extra backup snapshots: browsing changes the SQLite change counter after the unlock snapshot recorded it | Review F5 (`backup/policy.rs:268-294`) | Minor |
| I22 | **Explicit "Delete entry" reads its target after the dialog.** Navigation during the dialog changes `currentIndex`, so a different entry of the same journal can be deleted | Review R2-11 ✓ (`useMultiEntryNav.ts:157-166`) | Wrong-entry delete |
| I23 | Input stays enabled during every controlled transition (fetch, hydration, dialogs); text typed after the flush and before the commit is lost | Review R2-04 (`useMultiEntryNav.ts:74-98`, `useEntryLifecycle.ts:220-245`, `BackupsPanel.tsx:218-233`) | Data loss (narrow) |
| I24 | Journal create and whole restore install a new connection without returning its identity; a failed restore may keep, reopen, or roll back the connection, and the UI reloads only on success | Review R2-10 (`auth_core.rs:282-294`, `backup/restore.rs:107-218`, `BackupsPanel.tsx:224-258`) | Blocks epoch fencing |
| I25 | **A stale attachment cache can authorize a blank save over real text.** Attachment removal commits on the backend before the frontend list update; in that interval the cache still says "has files", so a cleared entry is saved blank without consent | Review R3-02 ✓ (`EntryAttachments.tsx:175-183`, `entryAttachments.ts:112-130`, `entries.rs:47-59`) | Data loss (narrow) |
| I26 | **A mandatory lock does not stop a connection installation that is already in progress.** Unlock and backup inspection open the file first and install the handle later without a check; a lock that runs between them finds no connection and returns | Review R3-06 ✓ (`auth_core.rs:152-178,245-258`, `backup_inspect.rs:140-159`) | Security boundary |
| I27 | Import, hydration image reads and attachment save-a-copy are not bound to a DB session | Review R3-07 (`plugin.rs:48-97`, `entryHydration.ts:33-40`, `attachments.rs:220-225`) | Wrong-journal write / read |

## Requirements

| ID | Requirement | Source |
|----|-------------|--------|
| R1 | Far fewer disk writes while typing; roughly one write per 30–60 s of continuous typing is acceptable | #263, maintainer agreed |
| R2 | No lost text on a **user-present transition**: entry/date/view navigation, manual lock, journal switch, restore, window close, Quit. For **unattended locks** (idle lock, focus-loss lock, OS lock, suspend, logoff, poisoned state) security wins: the app locks, and edits newer than the last durable revision may be lost. This loss is documented, not hidden | I3, I6, I13; review D10, R2-03, R2-14 |
| R3 | Reading an entry that has content never writes it. Exception: an abandoned **blank** row is still cleaned up when the user leaves it | I1; review R2-17 |
| R4 | A reader (search, export, stats, print, backup) includes every edit accepted **before the reader's barrier starts**, runs against the same DB session, and stops with a visible error if the barrier fails. Exception: a pending protected clear (rule 3) is not applied; the reader uses the stored text and the UI says so | I7; review D15, R2-01, R2-12 |
| R5 | A failed save is visible and retried within its session; it never silently disappears | I8 |
| R6 | Keep TODO-0089 / TODO-0104 invariants: atomic commit, hydration gate, captured snapshots, backend on-disk refusal, **consent before erasing real stored content**, **never delete files implicitly** | Prior work; review R2-05 |
| R7 | Fewer moving parts than today, without hiding different semantics behind one name | I9, I12; review S1/S2 |
| R8 | Save delays are stated as scheduling targets plus write latency; abnormal delays show as "unsaved/failed", not as a broken promise | Review T3, R2-17 |
| R9 | No new plaintext storage of any kind (no localStorage drafts, no plaintext recovery after a lock) | Security rules; review S5 |
| R10 | Every command in the audited fenced scope (rule 11) is bound to the DB session it was prepared in, and no connection is installed after a mandatory lock that started later | I14, I24, I26, I27 |

## Proposed Design v5: Simplified Core

v4 converged in review, but it bundled three different things into one design: the save model that #263 needs, existing bugs found during the sweep, and rare races found only by static review. v5 separates them. (v5 as shown here includes the round-6 corrections.)

```
                     v4 (full, 20 rules)
                            │
      ┌─────────────────────┼─────────────────────────┐
      ▼                     ▼                         ▼
  CORE (v5)            SEPARATE TODOs             ACCEPTED RISKS
  save model for #263  existing bugs, not made    policy choices;
  12 rules, 5 steps    worse by the core;         documented
                       own priority each
```

**Rule for the split:** an item is in the core if a longer save interval makes it worse, or if the core cannot work safely without it. Every other item is a separate TODO or an accepted risk. The core must not make any item outside it worse.

### Core rules

| # | Rule | Fixes |
|---|------|-------|
| C1 | **Clean flag.** The active entry record keeps an edit revision and a durable revision. Hydration sets both to 0. Assigning an id to a new entry does **not** reset them. A save is skipped only when they are equal **and** no task for the entry is queued or in flight. No content comparison (an undo back to the stored text writes once; that is harmless). Late receipts update only their own record, never a newly hydrated one. | I1, I2 (partly), I21 |
| C2 | **One write queue.** One task in flight per journal. A task carries `{epoch, entry id, payload, revision}`. On `Saved` the durable revision moves to that task's revision. A transient failure keeps the edit unsaved, backs off, and retries within the epoch. **Terminal errors** (entry not found, entry locked, epoch mismatch) stop retries and show the error. | I8, I9, I11 |
| C3 | **Timers.** Idle: 10 s after the last edit. Max-wait: 30 s after the oldest unsaved edit (decision 1). Both values are internal constants in one place, not user preferences. Max-wait is recomputed after each receipt. A timer writes the latest snapshot once. While a protected clear is pending (C6), timers stay idle until the next edit; a past deadline never causes a retry loop. | I5, R1 |
| C4 | **`flush()` owns one payload.** At call time it captures the latest snapshot (payload + revision) as one owned task. A later edit may replace it only with a later **allowed** payload; a later clear never erases it. It resolves `durable` when its own payload is `Saved`, `pendingClear` when its own payload is refused as a protected clear (C6), and rejects on failure. No revision history is kept. | I3, I7 (partly) |
| C5 | **Reversible local navigation** (entry, date, view, timeline): await `flush()` before changing state. Navigation to the entry that is already open is a no-op. If the edit revision moved during the destination fetch, flush again **and re-fetch the destination** before the commit. The final check and the local commit run in one synchronous call stack (an invariant, not an accepted loss). On failure, stay on the entry and show the error. The existing navigation tokens, `leaveCheckInFlight` and the TODO-0104 consent guard stay. | I8, I23 |
| C6 | **Backend blank-over-text decision.** `save_entry` classifies inside its transaction (v4 rule 3 table): blank incoming text over a stored row with title or body and no attachments → no write, `ProtectedClear`. `Saved` means the stored row equals the complete payload. Pending clear: leave → existing consent dialog; end → keep the stored row. Because the backend uses `is_blank_html` (which treats `<p></p>` as blank), this also removes the stale-cache and teardown-emptiness paths. | I16, I17, I25 |
| C7 | **Session epoch on entry mutations.** `create_entry`, `save_entry`, `delete_entry_if_empty`, `delete_entry`, `set_entry_locked` take the expected epoch and refuse a mismatch under the DB mutex. Unlock and create return the epoch. A wrong epoch only causes a **refusal**, never a write to the wrong row. Queued payloads are never relabeled with a new epoch. | I14, I2 |
| C8 | **End gate (narrow admission).** For lock (manual, idle, focus loss), journal switch, whole restore, selective restore, entry lock, and exit: one admission owner closes editor mutation admission **before** the final drain and keeps it closed until the operation completes or is canceled. The gate covers keyboard input, TipTap commands (a transaction filter: TipTap's `CommandManager` does not check `isEditable`), title and metadata inputs, attachment/tag mutations, and async producers (image paste, Markdown import), which check the gate before inserting. Cancel reopens the gate. Reversible navigation (C5) does not use the gate. **Hydration never runs through a closed gate:** when an end operation must rehydrate (restore outcome, cancel-restore), the owner first completes every await, then opens the gate and hydrates in **one synchronous call stack**, with no await or external mutation between them; TipTap's appended normalization (TrailingNode) runs inside that same `apply`. The clean record (C1) is set after this step. No per-transaction hydration mark is needed. | I23, R6-01 |
| C9 | **Pending create is owned.** From the first keystroke the record owns the typed document and its revision, even with no id. `flush()` and every end wait for the create, then write the latest payload. A create failure stops a voluntary transition (error shown); a retry creates a fresh row and keeps the document. A possible orphan blank row is accepted (A2); document loss is not. | I19, R6-05 |
| C10 | **Restore keeps the document safe.** Whole restore: end gate → flush → restore. The backend returns the outcome with the surviving epoch (or none) on success **and** failure; the frontend drops old-epoch tasks, rehydrates from the surviving connection, or shows the locked state when there is none. Selective restore: end gate → flush → restore → refresh the day list **without discarding** the active document; keep a pending create; on an empty day with no local content, hydrate the restored row. | I15, I24 (minimum), R6-04, R6-06 |
| C11 | **Exit owner.** A custom Quit menu item replaces the predefined one. One backend owner prevents every close request (window, Escape, Quit), asks the frontend for an end (C8 gate + `flush()`) with a request id, waits off the event thread with no mutex held, and exits only on success. The gate stays closed through the actual exit. Failure or timeout → exit canceled, gate reopened, the failure dialog (decision 3); the owner resets. | I3, I13, I20 |
| C12 | **Visible save state.** *Saved*, *Saving…*, *Unsaved changes*, *Not saved*, *Clear not confirmed*; *Not saved – retrying* only once C2 retries exist. Errors through `mapTauriError`. | I8, R5 |
| C13 | **Save on blur** (decision 2, A3). When the main window loses focus, the queue gets one immediate save demand for the active entry (same as a timer demand; skipped when clean). This adds writes only when the user leaves the app, not while typing. It reduces the loss on an OS lock or app switch. Whether Win+L, sleep and lid-close blur the window first is verified in the K5 gate. | A3 |

**Locks in the core.** Manual lock: end gate → flush; on failure the failure dialog (decision 3). Idle and focus-loss locks: end gate → flush → lock **even if the flush fails** (today `lockJournal()` catches a cleanup failure and returns to unlocked, `auth.ts:252-267`; that must change). OS lock / suspend: lock at once, as today.

**Readers in the core** (search, export, stats, print, "Back up now") call `flush()` first, so they include edits made before they start. No epoch binding (that is TODO S5).

**Loss windows, honestly.** Today: during continuous typing there is no bound (the debounce keeps resetting), and after a pause the window is about 500 ms. Core: during continuous typing the bound is about 30 s + write latency (better); after a pause it is about 10 s + latency (worse than 500 ms). A failed write has no time bound in either model; the core shows it. This applies to crashes and unattended locks (A3, A5).

### Core delivery

| Step | Content | Standalone? | Gate |
|------|---------|-------------|------|
| K1 | C1 + C12 (clean flag, visible state; no "retrying" yet) | Yes, at 500 ms. First-create and blank cleanup unchanged; no claim of ordered or failure-safe navigation | Browse: zero writes; first create; undo during a write; failed save visible |
| K2 | C6 **with** frontend adoption of `Saved` / `ProtectedClear` in the same release | Yes | Blank over text refused; metadata-only change saved; title-only row protected; consent dialog on leave |
| K3 | C2 + C4 + C5 + C7 + C8 + C9 + C10 + lock changes. The epoch fence, restore outcome adoption and the end gate ship **together**, so no restore path can rehydrate without the gate (R7-01) | Yes, at 500 ms | Ordered writes; flush keeps its payload when a clear follows; same-entry no-op; re-fetch after re-flush; end gate blocks a late image insert, also when restore fails and keeps the original connection; gate-open + hydrate in one synchronous step; late write after lock / switch refused; pending-create failure stops the transition; selective restore keeps typed text; idle lock still locks when the flush fails |
| K4 | C11 | Only after K3 | Close, Escape, Quit (Windows and macOS), double close, slow save, failed save, cancel then retry |
| K5 | C3 (10 s / 30 s) + C13 (save on blur) | Only after K1–K4 | Blur triggers one save (Windows: Win+L, app switch, sleep; macOS: lock, app switch); write count under continuous typing and slow writes; deadline recompute; protected clear does not loop |

### Separate TODOs (existing bugs; not made worse by the core)

| ID | Item | Issue | Why it is not in the core |
|----|------|-------|---------------------------|
| S1 | Selective restore during concurrent navigation or delete | I15 (rest) | The core keeps the document safe (C10); this is list reconciliation only |
| S2 | Explicit "Delete entry" target taken after the dialog | I22 | Same risk at 500 ms |
| S3 | Async producers insert into the wrong entry during reversible navigation; entry-session generation | I18 | Same risk at 500 ms; producers already obey the end gate (C8) |
| S4 | Mandatory lock does not stop an in-progress connection install; locked-picker selection race; auth generation | I26, R4-02 | Security race independent of saving |
| S5 | Reader, import, hydration image and attachment-copy commands not bound to the session | I7, I27 | Wrong-journal read/write independent of saving |
| S6 | Full typed restore-outcome contract beyond C10's minimum | I24 (rest) | Refinement |
| S7 | `getAllEntryDates` after every save | I10 | Optimization |
| S8 | Backend image fingerprint fast path | I4 | Optimization |
| S9 | Idempotent `create_entry` (request key) | R3-03 | C9 keeps the document; only an orphan blank row remains (A2) |

### Accepted risks (documented; maintainer approval required)

| ID | Risk | Why acceptable |
|----|------|----------------|
| A2 | An ambiguous `create_entry` response can leave one orphan **blank** row with an unknown id | C9 keeps the document; the orphan row has no content |
| A3 | Unattended locks (idle, focus loss, OS lock, suspend) can lose edits newer than the last durable save (see "Loss windows") | Security has priority; documented |
| A4 | A reader does not include edits typed **while** it runs | Normal snapshot semantics; C4 keeps every edit made before the call |
| A5 | Crash or power loss: edits newer than the last durable save | Inherent to any interval; the indicator shows the state |

A1 (local commit race) is now an invariant in C5. A6 (no freeze) is withdrawn: the end gate (C8) returns for end and destructive operations, but reversible navigation still has no freeze.

### What v5 removes from v4

Freeze for reversible navigation and the hydration window; transition tickets with commit-point states; immutable barrier targets (one owned flush payload instead); entry-session generation; create request key; auth generation; epoch on ~20 commands (now 5); reader epoch binding; locked-picker branch. Each moves to a separate TODO or an accepted risk above.

## Reference: Full Design v4 (superseded by the v5 core; kept for the separate TODOs)

v4 keeps the v3 architecture. Round 3 confirmed it has converged: active record, demand-driven pump, separate pending intent, entry-session generation, and transition freeze. v4 closes the contracts that round 3 found incomplete. The main simplification: **the backend decides blank-over-text atomically**, so the frontend attachment cache no longer authorizes anything, and the HOLD state disappears.

Four separate facts:

| Fact | Question it answers | Owner |
|------|---------------------|-------|
| **Durable content** | Is this content on disk? | The pump, from save receipts, or from baseline equality when no task for the entry is queued or in flight |
| **Pending intent** | Did the backend refuse a blank-over-text save that needs consent? | The active entry record |
| **Entry session** | Which document is the editor showing, regardless of id? | A local generation number |
| **Transition ticket** | Which transition is in progress, is it still valid, and has it passed its first irreversible step? | One frontend transition owner |

### Identities

| Identity | Scope | Fences |
|----------|-------|--------|
| **DB epoch** (`session_id()` of the open connection) | One opened connection | Backend commands in the fenced scope (rule 11). Returned by every create / unlock / restore outcome. Checked under the DB mutex. Mismatch = refusal, never a fallback to the current journal |
| **Auth generation** (backend counter) | Increments on every mandatory lock **and on every successful journal selection or path change** | Connection **installation**: unlock, journal create, restore reopen/rollback, and backup inspection capture the selected journal id and the generation at start, and recheck both under the same state guard that performs the installation (check and install are one guarded step, not a load followed by an unprotected install). A stale operation never installs readable state, and an open of journal A never installs after B was selected (R3-06, R4-02). Lock events carry it |
| **Entry-session generation** (local counter) | One hydrated document; stable across eager id assignment; new on hydration, cancel-restore, replacement | Editor callbacks and content producers |
| **Transition ticket** (local id + state: preparing → committing → completed, or canceled) | One transition | Late continuations, late acknowledgements, freeze release |

### Operations

| Operation | Meaning | Callers |
|-----------|---------|---------|
| `requestSave()` | Ask the pump for one scheduled save of the latest snapshot. Never blocks | idle timer, max-wait timer |
| `awaitDurable()` | Capture the latest snapshot **as an immutable target**. Resolve `durable` when that payload (or a later allowed payload of the same entry) is committed, or when it equals the baseline and no task for the entry is queued or in flight. Resolve `pendingClear` only after all earlier allowed work for the entry has settled and the backend refused the latest payload as a protected clear. Reject on failure or epoch change | readers; step 2 of every transition |
| `beginTransition(kind)` → ticket | Through the single transition owner: freeze admission, `awaitDurable`, resolve pending intent, then the caller commits or cancels | all transitions |

### Writer and records

```
                ┌────────── active entry record (one) ──────────┐
 edit ────────▶ │ session generation · entry id (or pending)     │
 (admitted      │ baseline (from hydration or last receipt)      │
  only if not   │ latest owned snapshot · emptiness verdict      │
  frozen)       │ pending intent · barrier targets (immutable)   │
                └──────────────┬─────────────────────────────────┘
                               │ task {epoch, generation, id, payload}
                               ▼
                ┌──── journal write pump (one in flight) ────────┐
 demand ──────▶ │ timer demand: write latest snapshot once        │
  (timer or     │ barrier demand: write its own target payload    │
   barrier)     │ backend outcome: Saved | ProtectedClear | Error │
                │ Saved → baseline := that payload                │
                │ ProtectedClear → pending intent, no write       │
                │ Error → backoff, keep demand, show state        │
                └─────────────────────────────────────────────────┘
```

### Rules

1. **Baseline.** One comparison form: title, the editor's own `getHTML()` after hydration with `emitUpdate:false`, and metadata compared by value. The baseline changes only on (a) authoritative hydration, (b) cancel-restore, (c) a `Saved` receipt, using that task's payload. A promised write never moves it.
2. **Barrier targets (R3-01).** A barrier owns its captured payload. A later edit or clear does not erase that demand: the pump still writes the target payload (or a later **allowed** payload of the same entry, which supersedes it). Equal targets share completion. A target equal to the baseline is satisfied without IPC only when no task for the entry is queued or in flight. Only the latest snapshot and outstanding barrier targets are retained, not every edit.
3. **Atomic blank-over-text decision (R3-02, R4-01, I16, R2-05).** `save_entry` classifies inside its own DB transaction, after the existing missing-row and locked-row checks, with the same blank test as `delete_entry_if_empty` (`is_blank_entry_text`: title **and** body blank; a title-only row has content):

   | Incoming title + body | Stored row (read in the same transaction) | Backend outcome |
   |-----------------------|-------------------------------------------|-----------------|
   | not blank | any | write → `Saved` |
   | blank | has attachments | write blank text and metadata, keep entry and files → `Saved` |
   | blank | no attachments, title and body blank | write only if title, body or metadata differ from the stored values → `Saved` |
   | blank | no attachments, title or body not blank | **no write → `ProtectedClear`** |

   `Saved` always means the stored row now equals the **complete** submitted payload (title, body, metadata). A no-write `Saved` is allowed only when that is already true. The pump sets the baseline only from such a receipt.

   The frontend attachment cache only drives display. A lookup failure no longer changes any decision. Pending-intent outcomes:
   - **Reader**: proceeds with the stored content; the editor shows "Clear not confirmed".
   - **Leave** (user present): existing consent dialog. Confirm → `delete_entry` with an epoch check and a "no attachments, text unchanged since refusal" recheck under the DB mutex; a changed row returns a typed refusal, never a silent delete. Cancel → restore the exact row and reset the baseline.
   - **End** (lock, close, switch, restore): keep the stored row and drop the clear.
   - Blank-row cleanup on leave (R3 exception) keeps using `delete_entry_if_empty`.
4. **Eager creation (R3-03, R2-08).** Keep immediate blank-row creation on the first keystroke. `create_entry` takes a frontend request key; the backend keeps a key → id map **in memory for the epoch** and returns the existing id on a repeated key. No schema change and no plaintext. The baseline of the new row is the blank row it returns; the typed revision stays unsaved and owned. After an ambiguous failure the retry reuses the same key. The map dies with the epoch, which the fence already makes safe.
5. **One emptiness verdict.** Each hydration seeds the verdict from the live editor at once, including equal-HTML hydration with no `setContent` callback. Each admitted edit updates it. Teardown never recomputes it from an HTML string (I17).
6. **Demand-driven pump (R2-06).** Timer demand writes the latest snapshot once; newer edits wait for the timers. Barrier demand writes its target. One task in flight per journal. Failures back off and keep the demand; a fired max-wait does not bypass backoff.
7. **Freeze is mutation admission (R3-05, R2-04, I23).** A freeze is a synchronous admission rule, not only a read-only prop:
   - A TipTap transaction filter rejects every document-changing transaction while frozen (TipTap's `CommandManager` and `dispatchTransaction` do not check `isEditable`). Selection-only transactions pass. Ticket-owned hydration is admitted by a **synchronous hydration window**: the owner opens it, calls `setContent`, and closes it in the same call stack. Appended normalization (StarterKit's TrailingNode `appendTransaction`) runs inside the same `apply` and therefore inside the window. A per-transaction meta mark is not used: ProseMirror filters an appended transaction before it sets `appendedTransaction` (`prosemirror-state/src/state.ts:148-154`). The baseline is taken after the window closes.
   - `setEditable(false)` for the visual state; title and metadata inputs read-only.
   - Attachment and tag mutations and the "Insert reference" command check admission first.
   - An open IME composition is ended and its result admitted before the barrier target is captured.
   - Retry / Discard / Cancel controls stay enabled.
8. **One transition owner and commit points (R3-04).** A new transition either coalesces with the current one, is refused, or takes over explicitly (an end takes over a leave). Only the **current** ticket can release or transfer the freeze; a late continuation of a canceled ticket does nothing. The ticket moves to *committing* at its **first irreversible step** (consent delete, blank-row cleanup, backend switch/restore call). After that:
   - Step succeeds, a later step fails (destination load, list refresh) → report "deleted, could not open the next entry" and reconcile to a coherent selection; never resume the deleted record.
   - Step fails without committing → restore the suspended write ownership of the record and cancel normally.
   - Step response is ambiguous → query that exact row under the same epoch, then choose one of the two branches.
   Deadlines apply only before *committing*.
9. **Content producers (I18, R2-09).** Capture the entry-session generation, editor instance and write permission before the first await. Recheck after each await and at insertion time. Acceptance = the actual insertion transaction, which the freeze filter also guards. On mismatch, drop the result with a notice in the current UI only.
10. **Teardown is a fallback.** `dispose` synchronously hands the active record (latest snapshot, verdict, pending create key) to the pump as an owned task and invalidates the generation. Epoch reset clears all retained plaintext buffers, timers and listeners. Tests in dev and release Solid builds.
11. **Fenced command scope (R3-07, R10).** One audited list, two classes:
    - **Durability readers** (barrier + epoch): search, export, statistics, print, plugin export, "Back up now".
    - **Identity-fenced** (epoch only, no editor barrier): `create_entry`, `save_entry`, `delete_entry_if_empty`, `delete_entry`, `set_entry_locked`, tag add/remove, attachment add/remove/save-a-copy, `get_entries_for_date`, `get_entry_images` and other per-entry media reads, `run_import_plugin` (rechecked after parsing, before insert), selective restore, whole restore, `switch_journal` (open branch).
    - **Unfenced by design**: locked-screen metadata (journal list, auth methods).
    Slow commands recheck the epoch when they reacquire state. Native dialogs run before the barrier and the dispatch.
12. **Journal selection (R3-08).** Two branches with explicit expectations:
    - **Switch from an open session**: ticket → freeze → drain → `switch_journal(expected epoch)`; the backend checks it before removing the connection.
    - **Locked-state selection** (journal picker, first-use creation): `switch_journal(expect none, auth generation)`; in one guarded step the backend refuses if a connection appeared or the generation changed, else changes the selection, increments the generation, and returns the new generation. Any pending open of the previous journal then fails at installation. No editor barrier (there is no active entry). While a mandatory lock is still completing, selection and fresh unlock are refused.
13. **Explicit delete and entry lock (I22).**
    - **Delete entry**: capture target id, generation and epoch **before** the dialog. On confirm, suspend that entry's tasks, delete exactly that id, then apply rule 8 (success → terminal; failure → restore ownership; ambiguous → query).
    - **Lock entry**: freeze, drain, set locked, release as read-only. **Unlock entry** never waits on a write that a locked row refused.
14. **Readers (R4).** Durability readers call `awaitDurable()` outside reactive tracking at the operation start (after any native dialog), pass that epoch to the command, and recheck their own generation before committing a UI result.
15. **Restore.**
    - **Whole restore**: ticket → freeze → drain → epoch-checked restore → act on the **returned outcome**: success → new epoch, hydrate; failure before swap → old epoch survives, cancel; failure after swap or rollback → adopt the returned epoch, hydrate; no connection → locked state. Installation of the new handle also checks the auth generation (rule "Identities").
    - **Selective restore**: additive. Barrier, refresh the day list by id, keep partial results. Active entry → keep identity and document; no active entry and no local intent → hydrate the restored row; local intent or pending create → keep it. Recheck date and generation before the list commit.
16. **Exits and locks (R3-06, R2-02, R2-13, R2-14).**

    | Trigger | User present? | Flow | On save failure |
    |---------|---------------|------|-----------------|
    | Navigation (entry, date, view) | yes | ticket → freeze → drain → intent → commit | stay, show error, unfreeze |
    | Manual lock | yes | same | Retry / Discard / Cancel |
    | Journal switch, whole restore | yes | frontend-owned, rules 12 and 15 | Retry / Discard / Cancel |
    | Window close, Escape, Quit | yes | backend exit owner prevents **every** close; asks frontend (request id + epoch) for a voluntary end; waits off the event thread with no mutex held | exit canceled; Retry / Discard / Cancel; owner resets |
    | Idle lock, focus-loss lock, OS lock / suspend / logoff, poisoned state | no | **Immediate**: increment the auth generation and close command admission at once, then lock the targeted epoch; no frontend wait | n/a (fail-closed; possible loss documented) |

    - **Quit**: a custom menu item (same label and accelerator) replaces the predefined one and routes to the exit owner. macOS native termination is a separate verification gate.
    - **Admission vs cleanup**: closing admission is immediate; finishing work already holding a mutex (backup, restore) is eventual. The lock completes when that work releases the mutex; that work cannot install a handle because the auth generation changed. Fresh unlock is allowed again once the lock completes.
    - Mandatory locks take priority over any voluntary ticket; they cancel it and close any Retry / Discard / Cancel dialog.
    - Lock notifications (`journal-locking`, `journal-locked`) carry the epoch and auth generation; the frontend ignores a notification for a session it no longer has.
    - Acknowledgements carry only ids, epoch, revision and outcome; never content.
    - A grace flush for unattended locks is deferred (it needs heavy saves off the event thread first).
17. **Visible save state (R5).** Derived from durable content, in-flight task, pending intent and last error: *Saved*, *Saving…*, *Unsaved changes*, *Not saved – retrying*, *Clear not confirmed*. Retries back off, are bounded by the epoch, and stop on permanent errors. Save receipts are separate from UI refreshes. Errors go through `mapTauriError`.
18. **Timers.** Idle fires N s after the last admitted edit. Max-wait fires M s after the oldest edit not yet durable, recomputed after each receipt. Proposed: **N = 10 s, M = 30 s**.
19. **Dates refresh.** Refresh `entryDates` on create, delete, import, and both restore kinds; not on content saves. Audit consumers.
20. **Image cost.** Backend only: validate MIME and size, then look up the keyed fingerprint before encryption and thumbnail work; reuse the stored row on a hit. Lazy metadata repair stays in `images/summaries.rs`. Frontend refs are out of scope.

### Delivery order

The 500 ms cadence stays until increments 1–5 pass. Each increment ships an API change **together with** its caller migration and outcome adoption, with no new regression.

| # | Increment | Fixes | Gate |
|---|-----------|-------|------|
| 1 | **Small bug fix (optional, standalone):** visible save failures; skip a save only when the entry has had no admitted edit since authoritative hydration and **no task queued or in flight**; blank cleanup and first-create unchanged; no claim of failure-safe navigation | I1, I2 (partly), I8 (visibility), I21 | Browse, first-create, undo-in-flight, no-edit blank cases |
| 2 | **Connection identity and security admission:** epoch in create / unlock / restore outcomes with frontend adoption (also on failed restore); the full fenced scope (rule 11); locked-state selection branch; auth generation on installation; epoch-targeted lock; lock notifications with identity | I14, I24, I25, I26, I2 | Late call after lock / switch / restore refused; mandatory lock during unlock, inspection, restore reopen; picker flows still work |
| 3 | **Persistence + transitions:** active record, barrier targets, pump, backend blank classification, create key, freeze filter, single transition owner with commit points, explicit delete, entry lock, producers, teardown, compatibility adapters | I9, I11, I16–I19, I22, I23 | Rule-specific race cases below |
| 4 | **Readers and restore:** durability barriers for readers; whole-restore outcomes; additive selective refresh | I7, I15 | Reader and restore cases |
| 5 | **Exits and locks:** custom Quit, exit owner, ticket correlation, immediate unattended lock | I3, I6, I13, I20 | Close / Escape / Quit / double close / cancel-then-retry; macOS termination check |
| 6 | **Cadence:** N = 10 s, M = 30 s (or as decided) | I5, R1 (#263) | Write counts with several reader demands, slow saves, backoff, protected intent, clean undo |
| 7 | **Optimizations:** dates refresh audit; backend image fast path | I10, I4 | Measured CPU per save; consumer tests |
| later | Draft sessions; frontend image refs; unattended-lock grace flush | — | Own designs |

### Acceptance cases

Round 1:

- Read an image-rich entry, navigate away, toggle timeline, lock, unlock: no write, no `date_updated` change.
- Clear a stored real entry, wait past idle and max-wait, then leave: one consent dialog; Cancel restores body and metadata of the correct entry; an *end* never overwrites it.
- Delay write of revision 1, type revision 2, then export / lock / navigate: the barrier includes revision 2.
- Save B from baseline A, undo to A while B is in flight: final disk state is A.
- Navigate twice rapidly; same date/view; invalid target; next load fails: no orphaned session.
- End while a leave dialog is open: no late delete/reload in the new epoch.
- Teardown in dev and release Solid builds.
- Continuous input longer than two max-wait windows; input during a write; backoff.

Round 2:

- Protected clear followed by reader, navigation, manual lock and restore: no permanent wait, no false receipt, no blank overwrite.
- Clear text on an entry with attachments: no whole-entry or file deletion.
- Windows File → Quit runs the exit owner; Cancel, then Quit again.
- Delayed lock worker for epoch A cannot lock a re-unlocked epoch B; also after a poisoned mutex and with an open inspection.
- Type during the next-entry fetch; producer after the barrier; destination load fails.
- Producers across empty days, before eager id binding, after entry lock, A → B → A.
- First edit while create is delayed, then immediate close: typed content written.
- Equal HTML on two entries with different titles; teardown: correct verdict and identity.
- Slow writes with continuous typing: write count follows the timers.
- Cancel close while a worker waits on a mutex; retry gets a new ticket.
- Restore fails before swap, after swap, after rollback: surviving epoch adopted.
- Selective restore with an active entry, an empty day, a pending create, and during navigation/delete.
- Reader barrier passes, then switch before dispatch: wrong epoch refused.
- Explicit delete while navigation races the dialog: the confirmed entry is deleted.

Round 3:

- Reader queued behind a slow write, then the user clears; repeat with undo-to-baseline and with the slow write failing: each caller gets its specified result, no extra unchanged write, no permanent wait.
- Attachment removal commits on the backend, its frontend list update is delayed, then the user clears text: the backend returns `ProtectedClear`, no blank write.
- Lose the create response, insert a restored blank row, retry: the same key returns the original id; no adoption of the other row, no duplicate.
- Confirm a clear/delete, then fail the destination load; fail the delete itself; lose the delete response; deliver a canceled leave continuation after an end took over.
- Freeze during a slow transition, then run Insert reference, finish an async producer, try native undo/paste, and cancel: editor HTML, snapshot and metadata still agree.
- Mandatory lock during unlock, during inspection open, during restore reopen and rollback, while backup holds its mutex, and while a Retry dialog is open: no stale installation; a late old notification does not reset a newer session.
- Switch after import parsing starts, between hydration's entry read and image read, and after the attachment Save dialog opens: wrong connection refused before any write.
- Fresh profile; locked picker; local-only journal selection; unlock racing a locked selection.

Round 4:

- Blank row with changed metadata; title-only stored row cleared; semantically blank but different HTML; unchanged blank row: no false durable baseline, no lost metadata, no unconfirmed title erasure.
- Hold unlock / create of journal A before installation, select B while still locked, release A: A cannot install. Two competing locked selections. First-use picker flow unchanged.
- Hydrate a document that ends in an image or a heading (TrailingNode appends a paragraph) while frozen, then unfreeze: baseline equals the live document; no spurious dirty state.

### Save delays (R8)

With N = 10 s and M = 30 s, in normal operation an edit is written about 10 s after typing stops and about 30 s into continuous typing, **plus write latency**. Slow disks, large image saves, lock contention and failed writes can exceed this; the indicator then shows *Unsaved changes* or *Not saved – retrying*. A crash or power loss, and any unattended lock (idle, focus loss, OS lock, suspend), can lose edits newer than the last durable save. User docs (`website/docs-src/`) must say this plainly.

## Decisions for the maintainer

| # | Question | Decision (2026-10-13) |
|---|----------|-----------------------|
| 1 | Max-wait 30 s or 60 s? | **30 s.** The value lives in an internal constant (not a user preference), so it can change later without a design change. The idle value uses the same mechanism. |
| 2 | Accepted risks A2–A5 and the longer loss window after a pause | **A2 accepted** (S9 stays a separate TODO). **A3: add save on blur** (rule C13) and document the rest. **A4 accepted.** **A5 accepted with idle = 10 s.** |
| 3 | Failure UX for user-present ends | **Decided** (see "Failure UX" below) |
| 4 | Ship K1 now as a standalone bug fix? | **Yes**: K1 is the first step |
| 5 | Accept the v5 split (core K1–K5, separate TODOs S1–S9, accepted risks A2–A5)? | **Yes** |

### Failure UX (decision 3)

Two error classes. **Transient** (disk full, file busy, I/O): a retry can succeed. **Terminal** (entry locked, entry not found, epoch mismatch): a retry never succeeds.

**User-present ends** (close, Quit, manual lock, journal switch, whole or selective restore): a dialog with plain words. *Keep editing* is the default and the safe choice. *Try again* is hidden for a terminal error. The last button's verb follows the action.

```
┌──────────────────────────────────────────┐
│  Your changes could not be saved         │
│                                          │
│  The disk is full. (mapTauriError text)  │
│                                          │
│  [Keep editing]  [Try again]             │
│                  [Lock without saving]   │
└──────────────────────────────────────────┘
  Close / Quit / Lock / Switch / Restore without saving
```

**Reversible navigation** (entry, date, timeline): no dialog. The editor stays on the entry, and the navigation does nothing until the save succeeds. An inline banner above the editor shows the error and *Try again* (no *Try again* for a terminal error).

```
┌─────────────────────────────────────────────┐
│ ⚠ Not saved: the disk is full. [Try again]  │
├─────────────────────────────────────────────┤
│ Title                                       │
│ entry text…                                 │
└─────────────────────────────────────────────┘
```

**Save state indicator**: quiet in normal use, loud on error. Low-contrast text next to the entry timestamps (`Created 09:12 · Updated 09:40 · Saved`) for *Saved*, *Saving…*, *Unsaved changes*. Only *Not saved* and *Clear not confirmed* use a visible color and the banner. Normal typing causes no visible movement.

**No "Copy text" action**: the app never puts diary text on the system clipboard by its own action (clipboard managers and cloud clipboard can store it; R9). The user can still select and copy text manually.

All new strings go through i18n (en/es/de).

## Validation Log

### Round 1 (OpenCode, herdr `wK:p4`, 2026-10-12): v1

Full report: `.agent-dev/save-model-review-r1.md` (gitignored). Verdict: *do not implement v1 as written*.

- **Accepted, design changed:** D1 persist-before-consent erases content; D2 joining a write is not a barrier; D3/D4 baseline semantics; D5/S2 settle must not close the session; D6 keep tokens and dialog coalescing; D7/D8 cleanup cannot be awaited; D9 no blocking wait on the OS callback thread; D10/D13 voluntary vs mandatory; D11 correlation and fencing; D12 one deadline; D14 restore phases; D15/D16 reader barriers; D17/D18 image refs are a protocol; D19 dates audit; D20/D21 drafts deferred; D22 richer save state; T1–T4.
- **Accepted, inventory corrected:** F1–F7.
- **Added issues:** M1–M7, M9 → I13–I20. M8 → folded into I4.
- **Spot-checked:** M1, M2, M3, M4, M5, F1 (confirmed).

### Round 2 (OpenCode, herdr `wK:p4`, 2026-10-12): v2

Full report: `.agent-dev/save-model-review-r2.md` (gitignored). Verdict: *revise before implementation*. 19 findings: 4 blockers, 12 majors, 3 minors. Round-1 traceability: 47 ids checked; many marked "partial".

- **Blockers, all accepted:**
  - R2-01 protected clear made `awaitDurable` impossible or falsely durable → pending intent is a separate fact; `awaitDurable` returns `pendingClear` (rule 5, R4 exception).
  - R2-02 Windows Quit bypasses `ExitRequested` too → custom Quit item (rule 13, I13 updated).
  - R2-03 a best-effort worker is not fail-closed and could lock a newer epoch → immediate lock, epoch-targeted lock primitive.
  - R2-04 transition-tail edits lost → freeze (rule 6, I23).
- **Majors, all accepted:** R2-05 attachment semantics (outcome table); R2-06 demand-driven pump; R2-07 one active record + owned tasks; R2-08 blank baseline for eager create, seeded verdict, no blind create retry; R2-09 entry-session generation; R2-10 epoch from create/restore outcomes (I24); R2-11 fenced scope, delete target before dialog (I22); R2-12 epoch-bound readers; R2-13 ticket states; R2-14 lock policy table (R2 narrowed); R2-15 selective-restore branches; R2-16 increment regrouping.
- **Minors, all accepted:** R2-17 wording (loss window, `create_entry`, latency, R3 exception, gate 5); R2-18 image fast path validation; R2-19 no backend handshake for frontend-owned switch/restore.
- **Spot-checked by me:** R2-02 (muda/tao/tauri-runtime-wry sources), R2-05 (`useEntryLifecycle.ts:445-450`), R2-11 (`useMultiEntryNav.ts:157-166`): confirmed.
- **Not adopted:** none.

### Round 3 (OpenCode, herdr `wK:p4`, 2026-10-12): v3

Full report: `.agent-dev/save-model-review-r3.md` (gitignored). Verdict: *architecture converged; close specific contracts before an implementation plan*. 8 findings: 2 blockers, 6 majors. Of the 19 round-2 findings, 9 addressed, 10 partial.

- **Blockers, accepted:**
  - R3-02 HOLD was incomplete, and a stale attachment cache could authorize a blank save → the backend classifies blank-over-text inside the save transaction and returns `ProtectedClear`; HOLD removed (rule 3, I25).
  - R3-06 a mandatory lock did not stop an in-progress unlock / inspection / restore from installing a connection → auth generation checked at installation; admission closed at once; lock events carry identity (rule 16, I26).
- **Majors, accepted:** R3-01 barriers own immutable target payloads; equality satisfies a target only with no queued or in-flight task (rule 2). R3-03 create request key, epoch-scoped in memory, replaces the day-list heuristic (rule 4). R3-04 commit points, ambiguous-response query, only the current ticket releases the freeze (rule 8). R3-05 freeze as a TipTap transaction filter plus admission checks; `CommandManager` does not check `isEditable` (rule 7). R3-07 audited fenced scope with durability readers vs identity-fenced commands (rule 11, I27). R3-08 locked-state journal selection branch (rule 12).
- **Simplifications adopted:** backend owns the destructive predicate; barrier targets instead of a history; deterministic create identity; one frontend transition owner; not every read is a save barrier; **all unattended locks are immediate** (the idle / focus-loss budget is removed, which also removes the event-thread budget problem); tickets and security revocation stay separate.
- **Delivery:** increment 2 now carries frontend epoch adoption, the full fenced scope, locked selection and the auth generation; increment 3 carries freeze enforcement and commit points.
- **Spot-checked by me:** R3-02 (`EntryAttachments.tsx:175-183`), R3-05 (`CommandManager.ts` has no `isEditable` check), R3-06 (`auth_core.rs:245-250` installs without a check), R3-08 (`JournalPicker.tsx:40-45`, `auth_journals.rs:424-432`): confirmed.
- **Not adopted:** none. Maintainer decision 5 (create retry policy) is closed by the request key.

### Round 4 (OpenCode, herdr `wK:p4`, 2026-10-13): v4

Full report: `.agent-dev/save-model-review-r4.md` (gitignored). Verdict: *not ready as written; two small contract changes, then ready for an implementation plan*. Findings: 1 blocker, 1 major, 2 minors. R3-01…R3-07 closed; R3-08 not fully closed.

- **R4-02 (blocker), accepted:** the auth generation advanced only on mandatory lock, so an unlock of journal A could install after the locked picker selected B → the generation also increments on every successful selection; open captures the selected journal and the generation, and installation checks both in one guarded step (Identities, rule 12).
- **R4-01 (major), accepted:** "blank and stored blank → no-write `Saved`" ignored metadata → `Saved` now means the stored row equals the complete payload; "stored text blank" means title and body (rule 3).
- **Minor M1, accepted:** a hydration meta mark cannot admit TrailingNode's appended transaction (filtered before `appendedTransaction` is set) → synchronous hydration window (rule 7).
- **Minor M2, accepted:** rule pointer and decision 2 wording.
- **Spot-checked by me:** `trailing-node.ts:66-80`, `prosemirror-state/src/state.ts:146-156`, `blank.rs:127-143`, `auth_core.rs:160-178`: confirmed.
- **Not adopted:** none.

### Round 5 (OpenCode, herdr `wK:p4`, 2026-10-13): v4 with round-4 fixes

Full report: `.agent-dev/save-model-review-r5.md` (gitignored). Confirmation round only. Verdict: **ready to become an implementation plan.** R4-01, R4-02, M1 and M2 closed; 0 new blockers, 0 new majors. The reviewer confirmed against `prosemirror-state/src/state.ts:137-168` and `@tiptap/core/src/Editor.ts:690-707` that appended transactions apply synchronously inside the hydration window. The maintainer decisions stay as approval gates before implementation; design approval is not implementation verification.

### Convergence summary

| Round | Design | Blockers | Majors | Minors |
|-------|--------|----------|--------|--------|
| 1 | v1 | 7 | many (D/M/F series) | — |
| 2 | v2 | 4 | 12 | 3 |
| 3 | v3 | 2 | 6 | 0 |
| 4 | v4 | 1 | 1 | 2 |
| 5 | v4 + fixes | 0 | 0 | — |

### v5 simplification (2026-10-13)

The maintainer judged v4 "still a little complicated". The first v5 draft split v4 into a 10-rule core needed for #263, nine separate TODOs for existing bugs, and six documented accepted risks. Key replacements: the editor freeze becomes a final revision recheck before the synchronous commit (C5); barrier targets become "flush always writes the latest snapshot" (C4); the epoch fence covers 5 entry-mutation commands instead of ~20 (C7). The backend `ProtectedClear` decision (C6) stays because it both fixes data-loss paths and removes frontend logic. I checked that `is_blank_html` (`blank.rs:62`) strips tags, so `<p></p>` is blank, and that idle lock runs the cleanup flush today (`auth.ts:248-254`).

### Round 6 (OpenCode, herdr `wK:p4`, 2026-10-13): v5 core

Full report: `.agent-dev/save-model-review-r6.md` (gitignored). Verdict: *the split is useful, but the core is not safe alone as written*. Findings: 1 blocker, 5 majors, 3 minors. It does not require a return to v4.

- **R6-01 (blocker), accepted:** a final recheck cannot protect the time after an IPC call (lock, switch, restore, exit); an async image insert can land after the flush and be lost → narrow end gate for end and destructive operations only (C8). A6 withdrawn.
- **R6-02 (major), accepted:** a later clear could erase an earlier flush demand → `flush()` owns one captured payload (C4).
- **R6-03 (major), accepted:** a re-flush does not refresh a destination fetched earlier (same entry can move backwards) → same-entry no-op; re-fetch after re-flush (C5).
- **R6-04 (major), accepted:** selective restore loss gets worse with a 10 s idle timer → minimum preservation moved into the core (C10); S1 reduced to list reconciliation.
- **R6-05 (major), accepted:** A2 covered only the orphan row, not the typed document → pending create owned by the record (C9).
- **R6-06 (major), accepted:** a current-epoch query is not restore ownership → restore returns its outcome with the surviving epoch; the frontend rehydrates (C10, K2).
- **Minors, accepted:** M1 the final check is an invariant, A1 removed; M2 honest loss-window comparison (paused typing: worse; continuous typing: better); M3 terminal errors, timer suspension during a pending clear, no "retrying" state in K1.
- **Also accepted:** idle / focus-loss lock must lock even when the flush fails; today `lockJournal()` returns to unlocked (`auth.ts:252-267`).
- **Spot-checked by me:** `useMultiEntryNav.ts:53-59` (no same-entry return), `useEntryLifecycle.ts:272-275` (discard on restore), `auth.ts:248-267`: confirmed.
- **Not adopted:** none.

Result: the core grows from 10 to 12 rules. It is still much smaller than v4: there are no tickets, no barrier history, no entry-session generation, no auth generation, and 5 fenced commands instead of ~20.

### Round 7 (OpenCode, herdr `wK:p4`, 2026-10-13): corrected v5 core

Full report: `.agent-dev/save-model-review-r7.md` (gitignored). Verdict: *not ready as written; two integration gaps*. R6-01…R6-06 and M1–M3 closed. Findings: 0 blockers, 2 majors.

- **R7-01 (major), accepted:** K2 adopted the restore outcome and rehydrated after a failure, but the end gate came only in K3, so a late image insert during a failed restore could be discarded → the epoch fence (C7), restore outcome adoption (C10) and the end gate (C8) now ship together in K3; K2 is only C6.
- **R7-02 (major), accepted:** the transaction filter had no authorized path for rehydration during an end operation → gate-open and hydrate run in one synchronous call stack after every await (C8); no hydration mark.
- **Not adopted:** none.

### Round 8 (OpenCode, herdr `wK:p4`, 2026-10-13): v5 core, final

Full report: `.agent-dev/save-model-review-r8.md` (gitignored). Verdict: **ready for an implementation plan in K1–K5 order.** R7-01 and R7-02 closed; 0 new blockers, 0 new majors. Gates kept: maintainer approval of A2–A5 and the per-step tests. This is design confirmation, not implementation verification, and it does not approve the separate known bugs S1–S9.

| Round | Design | Blockers | Majors |
|-------|--------|----------|--------|
| 6 | v5 core (first draft) | 1 | 5 |
| 7 | v5 core + round-6 fixes | 0 | 2 |
| 8 | v5 core + round-7 fixes | 0 | 0 |

## Next Steps

- [x] A: confirm finding 2 in the real app
- [x] Wider issue sweep
- [x] v1 → round 1 → v2 → round 2 → v3 → round 3 → v4 → round 4 → v4 fixes
- [x] Confirmation round 5: converged
- [x] v5 simplified core
- [x] Review round 6 on the v5 core (fixes applied)
- [x] Confirmation round 7 (2 majors fixed)
- [x] Confirmation round 8: converged
- [x] Maintainer decisions (all 5 decided 2026-10-13)
- [ ] Turn the increments into TODOs (`todo-manager`)
- [ ] Draft a reply to paulkater on #263
