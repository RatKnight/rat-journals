# Changelog

## 26.0.8 (2026-10-19)

### Added
- **Clock-change protection**: each journal remembers the latest time it has been open, sealed with the journal's key. If the computer's clock is set earlier than that, or the record is edited, a warning appears after unlocking. Upcoming time-based features use a time that never goes backwards, so setting the clock back can't stretch a time limit.
- **The vault has the same protection**: it remembers the latest time it was open (inside its encrypted index), and warns on opening if the clock is earlier.

## 26.0.7 (2026-10-18)

### Added
- **Black and Violet themes** (Preferences → General → Theme): a near-black dark theme and a deep purple one, alongside Light, Dark, and Auto.
- **The vault can lock on its own** after a set number of minutes without using it (Preferences → Security; 0 = off). The journal stays open, and time spent in the journal doesn't count as using the vault.
- **Paste into the vault on Linux**: files copied in the file manager can now be pasted in (Paste or Ctrl+V), as on Windows.

## 26.0.6 (2026-10-18)

### Security and reliability (from an external review)
- **Journal file names starting with a dot are refused.** A name like `...db` gave the journal a backups folder of `..`, so moving that journal's backups could delete the journal's own folder. Two journals in one folder whose names differ only by extension (`work.db`, `work.sqlite`) are also refused, since they would share one backups folder.
- **The settings file is owner-only on Linux** (it can hold a no-password journal's key); it was readable by other users under the default permissions.
- **Backup verification now checks attachments**, so a backup with a damaged attachment is no longer reported as verified.
- **Backup file names are matched against an allowlist**, which also refuses Windows alternate-data-stream names (`:`).
- **The backup list is flushed to disk before it replaces the old one**, so a power cut cannot leave it empty.
- Re-adding an attachment that is already stored no longer encrypts a second copy first.
- `SECURITY.md` had a stale Argon2 figure (64 MB / 3 passes); it now says 256 MiB / 4 passes.

## 26.0.5 (2026-10-18)

### Added
- **A navigation bar on the left** switches between the journal and the vault, with a Lock button that locks everything. Switching to the journal keeps an open vault unlocked in the background (a green dot shows it); auto-lock still closes it.
- **Select several vault files**: checkboxes, a select-all box, Ctrl-click, Shift-click ranges, Ctrl+A, and Esc to clear. The selection bar saves copies (into a chosen folder, never overwriting), moves, or deletes them all at once.
- **Paste into the vault**: copy files in File Explorer, then use Paste (or Ctrl+V) in the vault to encrypt them in. Windows only; on Linux use Add files or drag and drop.

## 26.0.4 (2026-10-18)

### Security
- **Printed and PDF-exported entries are sanitized** before they are placed in the page: scripts, event handlers, `javascript:` links, frames, forms, and embedded objects are removed (entries from imports can carry any HTML). Search snippets may contain only highlighting.
- **Plugin scripts are thread-safe by construction.** The scripting engine is now built in its thread-safe mode, replacing hand-written `unsafe` claims that its compiled scripts could be shared across threads.

### Changed
- Removed development tooling inherited from the original project that this one does not use (quality-score and coverage gates, diagram generators, AI-agent definitions, mobile and Microsoft Store icons, unused dependencies).

## 26.0.3 (2026-10-18)

### Security
- **"Require all unlock methods" is now enforced by the encryption.** Before, every unlock method still held a full copy of the journal key and the setting was only a check in the app, so the journal file plus one credential could be opened with another tool. Now the key is split into shares, one per method, and only all of them together rebuild it. Turning the setting on or off asks for your password and every key file. While it is on, methods cannot be added or removed (the password can still be changed). Journals that already had it on are split on their next combined unlock.
- **Release builds no longer include developer tools**, and no longer allow loading pages from `localhost`. The check of which addresses the window may load now compares the host exactly (a prefix check let addresses such as `http://localhost.example.com` through).
- **Every file path the window sends to the backend is checked**: no network shares, device paths, relative paths, or `..`. Exports and saved files must use their expected extension. Decrypted copies cannot be saved with a program or script extension, or into a Startup folder.
- **Deleted journal content is overwritten** (SQLite `secure_delete`) instead of lingering in free space until the file is compacted.
- **The settings file is written safely** (temporary file, flushed, then renamed), so a crash cannot leave it empty and lose your journal list.
- **The "no password" journal option now says plainly what it is**: the key is stored unencrypted in the settings file, so the journal is only as private as the computer account.

### Changed
- **The vault dialog remembers your vault.** After a vault has been created or unlocked once, the Vault button goes straight to its unlock screen (password field ready, key-file button if it has one). "Choose a different folder" still opens the folder picker. Only the folder location is remembered (in the app's settings file), never a password or key; if the folder is gone or not mounted, the picker is shown as before.

## 26.0.2 (2026-10-18)

### Security
- **Changing the vault password, or removing an unlock method, now rotates the vault's master key**: every file and the index are re-encrypted under a fresh key, and only the remaining methods are re-registered. Before this, those changes only rewrote the small header file — so a cloud provider's version history (or anyone keeping an old copy of `rat-vault.json`) could combine an old header with the revoked password or key file and still decrypt everything. Any other registered key files must be selected during the change, since re-registering them needs the file. The rotation is crash-safe: interrupted at any point, the vault repairs itself on the next unlock. Adding a method revokes nothing and is unchanged.

### Fixed
- **The taskbar showed the old icon.** The window only set its small title-bar icon, so Windows fell back to its cached icon for the exe path (left over from an earlier install). The window now also sets its large taskbar / Alt+Tab icon from the icon embedded in the exe.

## 26.0.1 (2026-10-18)

- A labeled **Vault** button in the header, and a full-width **Open a vault** button on the start screen, make the vault easy to find.

## 26.0.0 (2026-10-18)

Year-based version numbers start here (year.feature.patch). Headline: the encrypted file vault.

### Security
- **Password-protected journals could be decrypted without the password.** The inherited password slot stored the Argon2id hash string and derived the unlock key from the hash inside it. Password slots now use format v2: a raw Argon2id key derived from the password and a random salt each time, never written to disk. Existing journals are upgraded on their next password unlock, with a full master-key rotation (every entry, tag, image, and attachment re-encrypted, every unlock method re-wrapped, the file compacted, and a fresh backup taken). Backups made before the upgrade still use the old key and should be deleted.
- **Key files are quantum-resistant.** Key-file slots used X25519 and stored the key file's public key, which a future quantum computer could use to unwrap the journal without the file. Key-file slots now use a symmetric format (HKDF-SHA256 + AES-256-GCM) and store no public key. Old key files keep working and are upgraded in place; a journal with several unlock methods offers "All methods" on the unlock screen to finish the upgrade.
- **Key files are created by the backend.** The secret is generated and written in Rust and never reaches the WebView; key files can only be saved as `.key` files on a local path. The old `generate_keypair`, `write_key_file`, and `register_keypair` commands are gone.
- **A failed or pending encryption upgrade is shown** as a dismissible warning on the main screen.
- **Changing the password on an old-format journal** upgrades it and takes a fresh backup.
- **Stronger password hashing.** New password slots (journals and vaults) use Argon2id with 256 MiB and 4 passes (was 64 MiB, 3 passes): under a second per unlock, and each guess needs 256 MiB of fast memory, which sharply limits GPU cracking. Existing journal password slots are re-wrapped with the new settings on the next password unlock.
- `chacha20` moved off a yanked release (0.10.1 → 0.10.2).

### Added
- **Encrypted file vault (core and backend commands; no screen yet).** A vault is a folder (for example inside Google Drive) where each file is encrypted on its own with AES-256-GCM in 1 MiB chunks under a per-file key, and names, folders, and dates live in an encrypted index. It unlocks with its own password and/or key file, independent of any journal. Format: `docs/VAULT_FORMAT.md`. Every lock (Lock button, idle timer, screen lock, focus loss) also closes an open vault and wipes any decrypted copies made for "Open in app".

### Changed
- Renamed to **Rat Journal** (binary `rat-journal.exe`, identifier `local.ratjournal.app`, its own data folder; it never reads another app's data folder).
- Removed every link to outside websites and every update, news, feedback, and support prompt.
- Removed CI and store-publishing workflows, website, and packaging for other platforms' stores.
