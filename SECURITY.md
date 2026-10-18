# Security Policy

Rat Journal is a private, single-user build. There is no public release channel or vulnerability-reporting process; security fixes are made directly in this repository.

---

## Threat Model

Rat Journal is designed to protect diary content against a specific, well-defined threat: **an attacker who obtains a copy of your diary file** (e.g. from a stolen device, a compromised cloud sync folder, or a backup). In that scenario, the attacker sees only ciphertext.

### Protected against

- **Offline file access** — reading `diary.db` directly yields only encrypted bytes. Every entry field (title, body) is encrypted with AES-256-GCM using a fresh random nonce. The encryption key is never stored in plaintext anywhere on disk.
- **Password guessing at rest** — the key derivation uses Argon2id (memory=256 MiB, iterations=4, parallelism=4), which is deliberately expensive to brute-force. This exceeds OWASP 2023 minimums.
- **Weak-credential key-file attacks** — a key file holds 256 bits of random secret; its slot wraps the master key with AES-256-GCM under an HKDF-SHA256 key derived from that secret, regardless of what the file is named or where it lives.
- **Future quantum computers ("harvest now, decrypt later")** — every primitive that protects a journal at rest is symmetric or a hash: AES-256-GCM, Argon2id, HKDF/HMAC-SHA256. Grover's algorithm only halves their effective strength (AES-256 stays at ~128-bit security). No slot stores a public key, so there is no discrete-log problem for Shor's algorithm to solve. The quantum resistance of a password slot still depends on the password's strength.
- **Multi-credential compromise** — each authentication method (password or key file) stores its own independently wrapped copy of the master key. Revoking one method does not affect others and does not require re-encrypting any diary content.
- **Clock set back** — each journal stores the latest time it has seen, MACed with a key derived from its master key (`clock_high_water`); a vault keeps it inside its encrypted index. Unlocking with the clock more than 10 minutes behind it, or with an edited record, shows a warning, and time-based features use a time that never goes backwards. Restoring an older copy of the whole file brings back an older record; nothing stored only on this computer can prevent that.
- **In-memory key leakage after lock** — locking the diary drops the database connection and clears all held key material. Auth structs are zeroized on drop; derived wrapping keys are zeroized on all exit paths, including wrong-credential error paths.

### Not protected against

- **A running session on your device** — while the diary is unlocked, decrypted content and the master key exist in process memory. An attacker with local code execution or physical access to an unlocked, running session can read diary content. Locking the diary mitigates this; full mitigation requires OS-level memory protection outside the app's control.
- **Malicious software on your device** — Rat Journal does not defend against keyloggers, screen capture malware, or processes that can read another process's memory (e.g. via `ptrace`). Protect your device first.
- **Compromise of your sync provider** — if you store `diary.db` in a cloud-sync folder, the sync provider sees only the encrypted file. However, if the sync provider is also used to sync your key file or is an attack vector for modifying files on your device, that threat is outside the app's scope.
- **Coercion or credential theft** — if an attacker can observe you typing your password or steal your key file from an unlocked device, the encryption offers no protection.

---

## Cryptographic Architecture

Each diary is protected by a **random 256-bit master key** generated at creation time. The master key encrypts all entries and is itself never written to disk in plaintext. Instead, it is wrapped once per registered authentication method and stored in the `auth_slots` table.

**Password strength is your responsibility.** The app accepts any non-empty password (minimum 1 character) and provides a visual strength indicator to guide your choices. Argon2id with 256 MiB of memory and 4 iterations provides strong protection against offline attacks regardless of password length. However, weak passwords remain vulnerable to determined attackers with sufficient resources. We strongly recommend using a strong password (12+ characters with a mix of letters, numbers, and symbols) to ensure your encrypted diary remains secure.

### Entry encryption

Every entry title and body is encrypted separately with **AES-256-GCM**. Each encryption call draws a fresh 12-byte nonce from the OS random source (`OsRng`), so nonces are never reused across writes.

### Password authentication

Password slots derive a wrapping key via raw **Argon2id** (m=262144 KiB = 256 MiB, t=4, p=4, 16-byte random salt; under a second per unlock, and each guess needs 256 MiB of fast memory, which limits GPU cracking), then wrap the master key with AES-256-GCM. The slot stores only the salt, the Argon2id parameters, and the ciphertext; the derived key is never written anywhere. The salt and parameters are authenticated as AES-GCM associated data, and a wrong password is detected by the GCM tag check. Parameters read from disk are bounds-checked before the KDF runs.

Changing your password re-derives and re-wraps the master key only. Entry ciphertext is untouched. A password slot written with the earlier, cheaper settings (64 MiB, t=3) is re-wrapped with the current ones on the next password unlock; backups made before that still hold the cheaper slot.

**Legacy password slots (before v0.7.6).** Older releases stored the Argon2id PHC hash string in the slot and derived the wrapping key from the hash inside that string. Anyone with a copy of the journal file or of any backup could therefore recover the master key without the password. On the first unlock with the password after upgrading, Rat Journal **rotates the master key**: it generates a new master key, re-encrypts every entry, tag, image, and attachment, recomputes the keyed fingerprints, re-wraps every auth method (password in the new format; key files supplied at unlock in the new symmetric format, other legacy key files via their public key; the local key if present), deletes the leftover v1/v2 password hash, and `VACUUM`s the file so the old ciphertext does not remain in free pages. All of this runs in one transaction; a failure leaves the journal unchanged and the upgrade is retried on the next unlock. A fresh backup is taken right after the rotation.

What rotation cannot fix: **copies made before the upgrade** — older backups in the backups folder, cloud-sync history, or any other copy of the old file — still expose the old master key and the entries they contain. After upgrading, delete backups taken before the rotation and purge old versions from any sync provider. A journal that is only ever unlocked with a key file or the local key keeps its legacy password slot until it is unlocked once with the password.

### Key-file authentication

A key file holds 32 random bytes (64 hex characters), generated and written by the Rust backend; the secret never passes through the WebView. The slot (format v2) stores a **slot id**, HKDF-SHA256(secret, "slot-id"), used only to find the slot, and the master key wrapped with **AES-256-GCM** under HKDF-SHA256(secret, "wrap"), with the header and slot id as associated data. Nothing stored in the journal lets anyone unwrap the key without the file, on classical or quantum hardware. Key files can only be saved with the `.key` extension and never to a network (UNC) path.

**Legacy key-file slots (before v0.7.6)** used X25519 ECDH and stored the key file's **public key** in the journal. A large quantum computer could derive the private key from it and unwrap the master key without the key file. Old key files keep working: their 32 bytes are used as the secret of an upgraded slot. A journal whose only method is a key file is upgraded on its next key-file unlock. A journal with several methods needs all of them at once — every password and upgraded key-file slot can only be re-wrapped with its own credential — so the main screen shows a notice and the unlock screen offers **All methods** until the upgrade is done. The upgrade rotates the master key the same way as the password upgrade above, and backups taken before it should be deleted for the same reason.

**Trade-off:** because no slot can be re-wrapped from public data, a future master-key rotation will always need every unlock method at once.

### Memory handling

Derived wrapping keys are stored only in Rust stack or heap memory and are **zeroized on all exit paths** — including wrong-credential error paths — using the `zeroize` crate. Auth structs holding credentials implement `ZeroizeOnDrop`. The master key returned from an unwrap operation is held in a `SecretBytes` wrapper that zeroes the backing memory when dropped.

---

## Operational Security

- **Offline only** — the app makes no network requests. No data is transmitted to any server at any point. The About screen and onboarding tour surface links to GitHub and documentation; clicking these hands the URL to the OS, which opens your system browser. Rat Journal itself makes no network call — the browser's connection is independent and outside the app's scope.
- **Defense-in-depth WebView isolation** — multiple layers prevent the embedded WebView from making outbound network requests: strict CSP (`connect-src 'self' ipc: http://ipc.localhost`; `worker-src`, `child-src`, `frame-src`, `object-src`, `form-action`, `manifest-src` all `'none'`); a document-start init script that nulls `RTCPeerConnection`, `WebTransport`, `Worker`, `SharedWorker`, `navigator.serviceWorker`, `navigator.sendBeacon`, `navigator.connection`, and `window.open` in all frames; `on_navigation` that blocks any non-app URL; and `on_new_window(Deny)` that blocks popup creation on all platforms. `fetch`/`XMLHttpRequest`/`WebSocket`/`EventSource` remain available for Tauri IPC and local app/runtime communication, with outbound access constrained by CSP and platform handlers. Windows builds additionally register a `WebResourceRequested` COM handler; macOS builds register a `WKContentRuleList` content-blocking rule.
- **Local storage only** — diary content lives in a single SQLite file. Preferences are stored in `localStorage` (non-sensitive display settings only). Automatic backups are written to a local directory alongside the diary.
- **No plaintext search index** — the SQLite FTS5 table that stored entry content in plaintext was removed in v0.2.0 (schema v4). The migration drops the table on first unlock for any database upgraded from an older version.
- **No credential storage** — passwords are never stored. Key files are written to a location you choose and are never copied by the app.
- **Protected file permissions** — key files generated by the app are written with mode `0o600` (owner read/write only) on Unix.

---

## Known Security Limitations

- **Unlocked session exposure** — content in an active, unlocked session is not protected against other processes running under the same user account. Lock the diary when stepping away from your device.
- **No password recovery** — if you lose all registered credentials, there is no recovery path. This is intentional. Registering a key file as a backup credential is strongly recommended.
- **Backup file consistency** — backups are point-in-time SQLite file copies. They are encrypted identically to the live diary and safe from offline access, but a backup taken mid-write could theoretically capture a partially-written transaction. This is mitigated by SQLite's default journal mode; WAL mode is not currently used.
- **Journals that share a folder can see each other's filenames** — several journals can live in one directory, told apart by their database filename. Anyone who browses that directory in a file manager sees every journal's database filename and its `backups/{name}` folder name. Entry content stays encrypted; only the filenames you chose are visible. If those names are sensitive, give each journal its own folder.
- **macOS `.app` distribution is not App Sandboxed** — the macOS DMG/`.app` build provides WebView-layer network isolation (CSP, init script, `WKContentRuleList`) but not OS-level process network isolation. The Rust process has normal network access in principle; this is not OS-enforced. The process makes no network calls by design, but users who require a hard OS-enforced guarantee should wait for a future sandboxed release. Sandboxing is deferred because it requires complex entitlement work for arbitrary journal file paths.
- **Linux AppImage/deb builds have no OS-level network isolation** — these builds have normal process network access. The WebView-layer defenses (CSP, init script, `on_navigation`) still apply, but there is no kernel namespace isolation. Users who require hard isolation should use the Flatpak package, which runs without `--share=network`, or wrap the binary in Firejail (`firejail --net=none mini-diarium`).
