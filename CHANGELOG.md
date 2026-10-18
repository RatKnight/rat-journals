# Changelog

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
