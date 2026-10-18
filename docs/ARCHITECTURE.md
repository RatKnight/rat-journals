# Architecture

This document describes Rat Journal's high-level architecture and data flow.

- For principles and design philosophy, see [PHILOSOPHY.md](../PHILOSOPHY.md).
- For the full threat model and cryptographic details, see [SECURITY.md](../SECURITY.md).
- For a privacy overview, see [docs/PRIVACY.md](PRIVACY.md).

## Unlock Model

Rat Journal uses a wrapped master key design.

- A random master key encrypts all entries using AES-256-GCM
- Authentication methods wrap the master key
- Unlocking unwraps the master key into memory for the session

## Unlock Flow


### Password Unlock

- Argon2 key derivation
- AES-GCM unwrap of master key

### Key File Unlock

- X25519 key pair
- ECDH followed by HKDF
- AES-GCM unwrap of master key

The master key is never stored in plaintext.

---

## System Context

Everything runs locally on the user's machine.


### Properties

- The UI communicates with the Rust backend via Tauri `invoke()`
- The backend reads and writes to local SQLite
- No HTTP clients
- No background sync
- No telemetry

---

## Saving an Entry

When saving an entry:

1. The content is encrypted using the master key.
2. The encrypted content is stored in the `entries` table.

<p align="center">
  
</p>

---

## Layered Architecture

Rat Journal follows a layered structure.


