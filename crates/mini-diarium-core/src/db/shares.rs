//! Cryptographic enforcement of "require all unlock methods".
//!
//! Normally every auth slot wraps the full master key, so any single credential opens the
//! journal. When the user requires all methods, the master key is instead split into XOR
//! shares — `master = s_1 ⊕ s_2 ⊕ … ⊕ s_n` with `s_1..s_{n-1}` random — and slot `i`
//! wraps only `s_i`. Each share alone is uniformly random, so any proper subset of the
//! credentials reveals nothing about the master key: opening the journal needs every one.
//!
//! The slot rows keep their types, labels, and formats (password slot v2, key-file slot
//! v2), so listing methods and the unlock screen work unchanged. Two plaintext settings
//! mark the mode: `auth_slots_shared = "true"`, and `auth_shares_check`, an HKDF tag of the
//! master key that confirms the recombined key before any data is read. Tampering with
//! either can only make the journal fail to open (fail closed): dropping the marker makes
//! single-credential unlocks recover a share, which fails the key check and decrypts
//! nothing.
//!
//! While shares are active, adding or removing an unlock method is refused (a new slot
//! would need a fresh split, and removing one would destroy a share). Changing the
//! password re-wraps that slot's share and needs no special handling.

use crate::auth::{KeyFileMethod, PasswordMethod, SecretBytes};
use crate::crypto::cipher;
use crate::db::schema::compat::ensure_journal_readable;
use crate::db::schema::{open_connection, open_connection_readonly};
use crate::db::DatabaseConnection;
use hkdf::Hkdf;
use rand_core::{OsRng, RngCore};
use rusqlite::{params, Connection};
use sha2::Sha256;
use std::path::Path;
use zeroize::Zeroizing;

const SHARED_SETTING: &str = "auth_slots_shared";
const CHECK_SETTING: &str = "auth_shares_check";
const CHECK_INFO: &[u8] = b"rat-journal:auth-shares-check:v1";

/// Returned by single-credential opens of a journal whose key is split across its methods.
pub const ERR_REQUIRES_ALL_METHODS: &str =
    "This journal requires all of its unlock methods at once. Use the combined unlock.";
/// Returned when adding or removing an unlock method while all methods are required.
pub const ERR_SHARED_METHODS_FIXED: &str =
    "Turn off \"Require all unlock methods\" before adding or removing an unlock method.";

/// The credentials for every non-auto slot of a journal.
pub struct SlotCredentials<'a> {
    pub password: Option<&'a str>,
    pub key_files: &'a [KeyFileMethod],
}

struct Slot {
    id: i64,
    slot_type: String,
    public_key: Option<Vec<u8>>,
    wrapped: Vec<u8>,
}

fn read_slots(conn: &Connection) -> Result<Vec<Slot>, String> {
    let mut stmt = conn
        .prepare("SELECT id, type, public_key, wrapped_key FROM auth_slots ORDER BY id ASC")
        .map_err(|e| format!("Failed to read unlock methods: {}", e))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(Slot {
                id: row.get(0)?,
                slot_type: row.get(1)?,
                public_key: row.get(2)?,
                wrapped: row.get(3)?,
            })
        })
        .map_err(|e| format!("Failed to read unlock methods: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Failed to read unlock methods: {}", e))?;
    Ok(rows)
}

fn setting(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row(
        "SELECT value FROM db_settings WHERE key = ?1",
        params![key],
        |r| r.get(0),
    )
    .ok()
}

fn shares_check(master: &[u8]) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(None, master);
    let mut out = [0u8; 32];
    hk.expand(CHECK_INFO, &mut out)
        .expect("HKDF expand: 32 bytes always fits");
    out
}

/// True when this journal's slots hold shares (all methods required to open it).
pub(crate) fn is_shared_conn(conn: &Connection) -> bool {
    setting(conn, SHARED_SETTING).as_deref() == Some("true")
}

/// True when the open journal's slots hold shares.
pub fn uses_key_shares(db: &DatabaseConnection) -> bool {
    is_shared_conn(db.conn())
}

/// True when the journal file at `db_path` requires all methods (no key needed).
pub fn journal_uses_key_shares(db_path: &Path) -> bool {
    open_connection_readonly(db_path)
        .map(|c| is_shared_conn(&c))
        .unwrap_or(false)
}

/// Checks that a recombined key is this journal's master key.
fn verify_master(conn: &Connection, master: &[u8]) -> Result<(), String> {
    let stored = setting(conn, CHECK_SETTING)
        .and_then(|h| hex::decode(h).ok())
        .ok_or("This journal's unlock data is damaged")?;
    let expected = shares_check(master);
    // Constant-time comparison: no early exit on the first differing byte.
    let diff = stored
        .iter()
        .zip(expected.iter())
        .fold(stored.len() ^ expected.len(), |acc, (a, b)| {
            acc | usize::from(a ^ b)
        });
    if diff != 0 {
        return Err("Those credentials do not open this journal".to_string());
    }
    Ok(())
}

/// Unwraps one slot with the matching credential from `creds`.
fn unwrap_slot(slot: &Slot, creds: &SlotCredentials<'_>) -> Result<SecretBytes, String> {
    match slot.slot_type.as_str() {
        "password" => {
            let password = creds
                .password
                .ok_or("This journal requires its password as well")?;
            PasswordMethod::new(password.to_string())
                .unwrap_master_key(&slot.wrapped)
                .map_err(|_| "Incorrect password".to_string())
        }
        "keypair" => {
            let kf = creds
                .key_files
                .iter()
                .find(|kf| slot.public_key.as_deref() == Some(&kf.slot_id()[..]))
                .ok_or("This journal requires every one of its key files")?;
            kf.unwrap_master_key(&slot.wrapped)
                .map_err(|_| "Key file authentication failed".to_string())
        }
        other => Err(format!("Unsupported unlock method: {}", other)),
    }
}

fn wrap_for_slot(
    slot: &Slot,
    creds: &SlotCredentials<'_>,
    value: &[u8],
) -> Result<Vec<u8>, String> {
    match slot.slot_type.as_str() {
        "password" => PasswordMethod::new(creds.password.unwrap_or_default().to_string())
            .wrap_master_key(value),
        "keypair" => {
            let kf = creds
                .key_files
                .iter()
                .find(|kf| slot.public_key.as_deref() == Some(&kf.slot_id()[..]))
                .ok_or("This journal requires every one of its key files")?;
            kf.wrap_master_key(value)
        }
        other => Err(format!("Unsupported unlock method: {}", other)),
    }
}

/// Recombines the master key from every non-auto slot. Returns the key and the slot ids.
fn recombine(
    conn: &Connection,
    creds: &SlotCredentials<'_>,
) -> Result<(cipher::Key, Vec<i64>), String> {
    let slots = read_slots(conn)?;
    let mut master = Zeroizing::new([0u8; 32]);
    let mut ids = Vec::new();
    for slot in slots.iter().filter(|s| s.slot_type != "auto") {
        let share = unwrap_slot(slot, creds)?;
        if share.len() != 32 {
            return Err("This journal's unlock data is damaged".to_string());
        }
        for (m, s) in master.iter_mut().zip(share.iter()) {
            *m ^= s;
        }
        ids.push(slot.id);
    }
    if ids.is_empty() {
        return Err("This journal has no unlock methods".to_string());
    }
    verify_master(conn, master.as_ref())?;
    let key = cipher::Key::from_slice(master.as_ref()).ok_or("Invalid master key size")?;
    Ok((key, ids))
}

/// Opens a journal whose key is split across its methods, with every credential.
pub fn open_database_with_all_credentials<P1: AsRef<Path>, P2: AsRef<Path>>(
    db_path: P1,
    creds: &SlotCredentials<'_>,
    backups_dir: P2,
) -> Result<DatabaseConnection, String> {
    let conn = open_connection(db_path.as_ref())?;
    ensure_journal_readable(&conn)?;
    if !is_shared_conn(&conn) {
        return Err("This journal does not use split keys".to_string());
    }
    let (key, ids) = recombine(&conn, creds)?;
    let db = DatabaseConnection::new(conn, key);
    for id in ids {
        crate::db::queries::update_slot_last_used(&db, id)?;
    }
    crate::db::schema::migrate_with_pre_migration_snapshot(
        &db,
        db_path.as_ref(),
        backups_dir.as_ref(),
    )?;
    Ok(db)
}

/// The master key of a snapshot (backup) whose key is split, recovered with the live
/// journal's key: valid only when the snapshot was taken under the same master key, which
/// its key check confirms.
pub(crate) fn snapshot_key_from_live(
    conn: &Connection,
    live_master: &[u8; 32],
) -> Result<cipher::Key, String> {
    verify_master(conn, live_master).map_err(|_| {
        "This backup was taken before the journal's key last changed, and it needs all of \
         the unlock methods the journal had then."
            .to_string()
    })?;
    cipher::Key::from_slice(live_master).ok_or_else(|| "Invalid master key size".to_string())
}

/// Splits the master key across every non-auto slot (turns enforcement on).
///
/// Needs the credential for every slot. Refused while a Local Key slot exists (it would
/// open the journal on its own) or while any slot still uses a legacy format.
pub fn enable_key_shares(
    db: &DatabaseConnection,
    creds: &SlotCredentials<'_>,
) -> Result<(), String> {
    let conn = db.conn();
    if is_shared_conn(conn) {
        return Ok(());
    }
    let slots = read_slots(conn)?;
    if slots.iter().any(|s| s.slot_type == "auto") {
        return Err(
            "A journal that opens without a password on this device cannot require all \
             unlock methods. Remove the Local Key first."
                .to_string(),
        );
    }
    if slots.len() < 2 {
        return Err("Requiring all unlock methods needs at least two of them.".to_string());
    }
    if slots.iter().filter(|s| s.slot_type == "password").count() > 1 {
        return Err("Requiring all unlock methods supports one password.".to_string());
    }
    for s in &slots {
        let legacy = match s.slot_type.as_str() {
            "password" => PasswordMethod::is_legacy_blob(&s.wrapped),
            "keypair" => !KeyFileMethod::is_v2_blob(&s.wrapped),
            _ => true,
        };
        if legacy {
            return Err(
                "Finish this journal's encryption upgrade first: unlock it once with all of \
                 its unlock methods."
                    .to_string(),
            );
        }
        // Every credential must open its slot and every slot must hold this master key.
        let value = unwrap_slot(s, creds)?;
        if value[..] != db.key().as_bytes()[..] {
            return Err("An unlock method does not hold this journal's key".to_string());
        }
    }

    // master = s_1 ⊕ … ⊕ s_n: the first n-1 shares are random, the last makes it add up.
    let master = db.key().as_bytes();
    let mut last = Zeroizing::new(*master);
    let mut shares: Vec<Zeroizing<[u8; 32]>> = Vec::with_capacity(slots.len());
    for _ in 1..slots.len() {
        let mut s = Zeroizing::new([0u8; 32]);
        OsRng.fill_bytes(s.as_mut());
        for (l, b) in last.iter_mut().zip(s.iter()) {
            *l ^= b;
        }
        shares.push(s);
    }
    shares.push(last);

    let wrapped: Vec<Vec<u8>> = slots
        .iter()
        .zip(shares.iter())
        .map(|(slot, share)| wrap_for_slot(slot, creds, share.as_ref()))
        .collect::<Result<_, _>>()?;

    let tx = conn
        .unchecked_transaction()
        .map_err(|e| format!("Failed to start a transaction: {}", e))?;
    for (slot, blob) in slots.iter().zip(wrapped.iter()) {
        tx.execute(
            "UPDATE auth_slots SET wrapped_key = ?1 WHERE id = ?2",
            params![blob, slot.id],
        )
        .map_err(|e| format!("Failed to update an unlock method: {}", e))?;
    }
    for (k, v) in [
        (SHARED_SETTING, "true".to_string()),
        (CHECK_SETTING, hex::encode(shares_check(master))),
    ] {
        tx.execute(
            "INSERT OR REPLACE INTO db_settings (key, value) VALUES (?1, ?2)",
            params![k, v],
        )
        .map_err(|e| format!("Failed to save a setting: {}", e))?;
    }
    tx.commit()
        .map_err(|e| format!("Failed to save the change: {}", e))
}

/// Gives every slot the full master key again (turns enforcement off). Needs the
/// credential for every slot, so a stolen single credential cannot undo the split.
pub fn disable_key_shares(
    db: &DatabaseConnection,
    creds: &SlotCredentials<'_>,
) -> Result<(), String> {
    let conn = db.conn();
    if !is_shared_conn(conn) {
        return Ok(());
    }
    // Recombining proves every credential and that they rebuild this journal's key.
    let (key, _) = recombine(conn, creds)?;
    if key.as_bytes() != db.key().as_bytes() {
        return Err("Those credentials do not open this journal".to_string());
    }
    let slots: Vec<Slot> = read_slots(conn)?
        .into_iter()
        .filter(|s| s.slot_type != "auto")
        .collect();
    let wrapped: Vec<Vec<u8>> = slots
        .iter()
        .map(|slot| wrap_for_slot(slot, creds, db.key().as_bytes()))
        .collect::<Result<_, _>>()?;

    let tx = conn
        .unchecked_transaction()
        .map_err(|e| format!("Failed to start a transaction: {}", e))?;
    for (slot, blob) in slots.iter().zip(wrapped.iter()) {
        tx.execute(
            "UPDATE auth_slots SET wrapped_key = ?1 WHERE id = ?2",
            params![blob, slot.id],
        )
        .map_err(|e| format!("Failed to update an unlock method: {}", e))?;
    }
    tx.execute(
        "DELETE FROM db_settings WHERE key IN (?1, ?2)",
        params![SHARED_SETTING, CHECK_SETTING],
    )
    .map_err(|e| format!("Failed to save a setting: {}", e))?;
    tx.commit()
        .map_err(|e| format!("Failed to save the change: {}", e))
}

#[cfg(test)]
mod tests;
