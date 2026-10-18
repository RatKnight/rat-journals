# Vault format (version 1)

A vault is a folder of encrypted files that can live anywhere, including a cloud-sync folder such as Google Drive. Everything needed to open it is in the folder itself plus the user's password or key file; it does not depend on any journal.

## Layout

```
<vault folder>/
  rat-vault.json            header: format, version, vault id, unlock slots (plain JSON)
  index.rjv                 encrypted index: folders and file metadata
  data/<aa>/<id>.rjf        one encrypted object per stored file
```

`<id>` is the object's 128-bit id in lowercase hex; `<aa>` is its first two hex characters.

A cloud provider holding the folder learns: how many files there are, their approximate sizes (ciphertext is plaintext + 25 bytes + 16 bytes per MiB), and when they change. It does not learn file names, folder names, contents, dates added, or which blob belongs to which folder.

## Keys

- **Vault master key (VMK):** 32 random bytes, created with the vault, never stored in plaintext.
- **Unlock slots** in `rat-vault.json` each wrap the VMK, using the journal's slot formats:
  - `password`: password slot v2 (raw Argon2id key from password + random salt, AES-256-GCM; see SECURITY.md).
  - `keyfile`: key-file slot v2 (HKDF-SHA256 from the key file's 32 bytes, AES-256-GCM, slot id as associated data).
  The VMK can only be recovered with a slot's credential; editing the header can remove slots (denial of service) but cannot add one.
- **Object keys:** `HKDF-SHA256(ikm = VMK, salt = vault id, info = "rat-vault:object:v1" || kind || object id)`. Each object (file blob or index) has its own key.

## Encrypted object (stream format)

```
header (25 bytes):  "RJV1" | kind (u8: 1 = file, 2 = index) | object id (16 bytes) | chunk size (u32 LE)
chunk 0 .. n:       AES-256-GCM(ciphertext || 16-byte tag)
```

- Plaintext is split into chunks of `chunk size` bytes (1 MiB when written); only the final chunk may be shorter. An empty plaintext is one empty final chunk.
- Nonce for chunk `i`: `i` as u64 big-endian, three zero bytes, then `0x01` for the final chunk or `0x00` otherwise.
- Associated data for every chunk: the 25-byte header.

Because every write uses a fresh random object id, and the key is derived from that id, a key never encrypts two different plaintexts, so deterministic nonces are safe. Reordering or swapping chunks fails authentication (the index is in the nonce); dropping trailing chunks fails because the new last chunk was not encrypted with the final flag; trailing data after the final chunk is rejected; moving a blob to another id's path fails because its key and header id no longer match. Chunk sizes outside 16 B..16 MiB are rejected before any allocation.

## Index

The index object (kind 2) decrypts to UTF-8 JSON:

```json
{
  "version": 1,
  "folders": ["Medical", "Insurance"],
  "files": [
    {
      "id": "3f0c…",
      "name": "Bloodwork_2026-10.pdf",
      "folder": "Medical",
      "size": 421888,
      "added_at": "2026-10-18T09:14:03Z",
      "sha256": "…",
      "mime": "application/pdf"
    }
  ]
}
```

## Writes

Every write goes to a temporary file in the same folder and is renamed into place. A file is added by writing its blob first and the index second, so a crash leaves at worst an unreferenced blob (removed by orphan cleanup), never an index entry without its data. Deleting a file rewrites the index first and removes the blob second.

## Master-key rotation

Changing the password (when one already exists) or removing an unlock method **rotates the VMK**: a fresh VMK is generated, every blob is re-encrypted under it with a fresh object id, the index follows, and only the methods that stay are wrapped in the new header. This is what makes the change a real revocation: without rotation, an old copy of `rat-vault.json` (for example in a cloud provider's version history) would keep unwrapping the unchanged VMK with the old credential. Because wrapping a slot needs its credential, every key file that stays registered must be presented during the rotation.

Order, and what a crash leaves:

1. New blobs are written (unreferenced until the new index lands — cleaned as orphans if abandoned).
2. The new index is staged as `index.rjv.new`, encrypted under the new VMK.
3. The new header is written: **the commit point**.
4. `index.rjv` is rewritten under the new VMK, the staged copy is removed, and the old blobs are deleted as orphans.

On unlock, if `index.rjv` does not decrypt but `index.rjv.new` does, the rotation committed and the swap is completed; a stale `index.rjv.new` that does not decrypt is removed. Adding a method (a key file, or a first password to a key-file-only vault) revokes nothing and does not rotate.

## Known limitations

- **Rollback:** a sync provider could serve an older, still-valid `index.rjv` or blob (from the same VMK generation). The vault cannot detect this without a trusted counter stored elsewhere. Rotation limits this: anything served from before the last rotation no longer decrypts.
- **Concurrent edits** from two computers at once can produce sync-conflict copies of `index.rjv`; use one computer at a time.
- Size and count metadata are visible, as described above.
