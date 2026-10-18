# CLAUDE.md — Rat Journal

**Rat Journal** is a personal, encrypted, offline desktop journal with a separate encrypted file vault (SolidJS + Rust/Tauri v2 + SQLite). Journal content is AES-256-GCM encrypted at rest; plaintext never touches disk. It is a renamed fork of an MIT-licensed project, maintained for one person's use (see `README.md`).

**Core principles:** privacy first (no network, no telemetry, no update checks), clean architecture, TypeScript strict + Rust type safety, English only.

**Platforms built:** Windows (NSIS `.exe` + `.msi`) and Linux (Flatpak).

## Domain Guides

- [Frontend (src/)](src/CLAUDE.md) — SolidJS, state, i18n, testing, TipTap, theme
- [Backend (src-tauri/)](src-tauri/CLAUDE.md) — Tauri commands, Rust patterns, plugins, security
- [E2E (e2e/)](e2e/CLAUDE.md) — WebdriverIO, tauri-driver, viewport rules
- [Best practices](docs/best-practices/README.md) — frontend, Rust, Tauri and writing rules
- [Vault format](docs/VAULT_FORMAT.md) — on-disk format of the file vault

## Workspace and commands

The backend is a Cargo **workspace** (root `Cargo.toml`) with three members, layered bottom-up:

1. `crates/mini-diarium-crypto/` — the `rusqlite`-free crypto base (cipher, Argon2id, slot formats, `crypto::stream` for the vault).
2. `crates/mini-diarium-core/` — the Tauri-free business layer (db, vault, import/export, plugins, backup, config, search). Its curated API is [`crates/mini-diarium-core/API.md`](crates/mini-diarium-core/API.md); `DatabaseConnection::key()` never leaves the crate.
3. `src-tauri/` — the app crate (`commands/*`, OS integration).

`Cargo.lock` and `target/` live at the repo root. Internal names (`mini-diarium-*` crates, `mini_diarium_lib`, `MINI_DIARIUM_*` env vars) are intentionally unchanged.

```bash
cargo test --workspace          # all Rust tests (without --workspace the core/crypto tests do not run)
cargo clippy --workspace --all-targets
bun run test:run                # frontend tests (Vitest needs Node.js on PATH)
bun run type-check
bun run lint
bun run validate:locales        # every locale file must have every key
bun run tauri build             # installers in target/release/bundle/{nsis,msi}
bun run test:e2e                # end-to-end against the release build (needs tauri-driver)
bun run pre-commit              # all of the above checks in one run
```

The git pre-commit hook (`.githooks/pre-commit`, installed by `bun install`) only formats staged files (Prettier, `cargo fmt`).

## Architecture notes

- Entries are stored encrypted in SQLite with an integer `id` (AUTOINCREMENT); several entries per date are allowed. Search decrypts entries in memory per query (`crates/mini-diarium-core/src/search/`); there is no plaintext index.
- Journals are listed in `{app_data_dir}/config.json` (`JournalConfig`: folder + `db_filename`). `DiaryState` holds one open connection; switching journals locks first. The file vault is independent of journals and lives in `DiaryState::vault`; every lock path closes it too.
- Preferences use `localStorage`. The native menu holds only Preferences + Quit (`menu-preferences` event); other shortcuts are JS handlers in `src/lib/keyboard-shortcuts.ts`.
- All Tauri commands are registered in `src-tauri/src/lib.rs` (`generate_handler![]`); frontend wrappers live in `src/lib/tauri/` (one file per command group). Rust uses `snake_case`, wrappers `camelCase`.

## Conventions

- Backend returns `Result<T, String>`; the frontend wraps `invoke()` in `try/catch` and shows errors through `mapTauriError()`. Backend commands validate their own input — every frontend-supplied file path goes through `commands/paths.rs` `check_path`.
- Any code that opens a native dialog imports `open`/`save`/`confirm` from `src/lib/dialog.ts` (focus-loss auto-lock depends on it).
- HTML from entries is untrusted when it is placed in the page: use `src/lib/sanitize.ts`.
- Text is English only; the other locale files carry English copies so the validator passes.

## Security rules

- **Never** log, print, or serialize passwords, keys, or decrypted content.
- **Never** store plaintext diary or vault content on disk (the only exception is the explicit "Open in app" copy, wiped when the vault locks).
- **Never** send data over the network.
- Auto-lock fires from three paths — the frontend idle timer (`App.tsx`), OS session events (`screen_lock.rs`), and focus loss (`src/lib/focus-lock.ts` + `window_focus.rs`). Changes to locking must cover all three.
- "Require all unlock methods" splits the master key into XOR shares (`crates/mini-diarium-core/src/db/shares.rs`); while it is on, methods cannot be added or removed.

See [Backend guide](src-tauri/CLAUDE.md) for the full auth architecture and [SECURITY.md](SECURITY.md) for the user-facing summary.

### Architecture decision records

- [`docs/decisions/2026-04-passwordless-journal.md`](docs/decisions/2026-04-passwordless-journal.md) — no-password journals (device key in `config.json`) and their threat model.
- [`docs/decisions/2026-05-settings-storage-taxonomy.md`](docs/decisions/2026-05-settings-storage-taxonomy.md) — where each kind of setting is stored.
- [`docs/decisions/2026-09-schema-forward-compatibility.md`](docs/decisions/2026-09-schema-forward-compatibility.md) — refusing journals and backups from newer schema versions.
- [`docs/decisions/2026-06-feature-flags.md`](docs/decisions/2026-06-feature-flags.md) — build-time feature flags.

## Common tasks

- **New schema migration:** bump `SCHEMA_VERSION`, describe it in `src-tauri/CLAUDE.md`, and follow the forward-compatibility checklist in `crates/mini-diarium-core/src/db/schema/compat.rs`.
- **New `data-testid` used by E2E:** add it to the table in `src/CLAUDE.md`.
- **Icons:** source is `public/logo-transparent.svg`; regenerate with `bun x tauri icon public/logo-transparent.svg`, then delete the generated `android/`, `ios/`, `Square*Logo.png` and `StoreLogo.png` (not used).
- **Dependencies:** commit `bun.lock` and `package-lock.json` together after any `package.json` change.
- **Release:** bump the version in `package.json`, `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json` (year-based: `26.0.x`), add a `CHANGELOG.md` section, build, then `gh release create rat-journal-<version> --repo RatKnight/rat-journals` with the `.exe` and `.msi` (asset names without spaces), and run the **Build Flatpak** workflow with that tag.

`AGENTS.md` files are symlinks to the sibling `CLAUDE.md`; edit the `CLAUDE.md`.
