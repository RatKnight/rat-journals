//! Duress ("panic") password (opt-in).
//!
//! A journal can hold a second password that, typed at the unlock screen, makes the app
//! erase this journal (and, if chosen, the vault) and open a decoy journal instead. The
//! journal stores only a check value: a random token wrapped under the duress password with
//! the same Argon2id scheme as a password slot, in `db_settings` (`duress_check`). It wraps
//! no key, so the duress password can't open anything.
//!
//! The app checks it with [`matches_file`] before opening the journal, for every unlock
//! that includes a password (also the "All methods" unlock of a journal that needs every
//! method). When a duress password is set, that check runs one Argon2id derivation on every
//! such unlock; a real unlock then runs its own, and a duress unlock opens the decoy with
//! one, so both cost two derivations.
//!
//! Limits: copies of the journal made earlier (cloud version history, other backups) are not
//! erased; someone who inspects the file can see that a duress check exists.

use crate::auth::password::PasswordMethod;
use rand_core::{OsRng, RngCore};

/// `db_settings` key holding the hex-encoded check blob.
pub const SETTING: &str = "duress_check";

/// Sets (or replaces) the duress password of an open journal. Refuses one that opens the
/// journal itself.
pub fn set(db: &crate::db::DatabaseConnection, password: &str) -> Result<(), String> {
    if password.is_empty() {
        return Err("The panic password can't be empty".to_string());
    }
    if opens_journal(db.conn(), password) {
        return Err("The panic password must be different from the journal's password".to_string());
    }
    let mut token = [0u8; 32];
    OsRng.fill_bytes(&mut token);
    let blob = PasswordMethod::new(password.to_string()).wrap_master_key(&token)?;
    crate::db::set_db_setting(db, SETTING, &hex::encode(blob))
}

/// Removes the duress password.
pub fn clear(db: &crate::db::DatabaseConnection) -> Result<(), String> {
    crate::db::delete_db_setting(db, SETTING)
}

/// Whether the journal has a duress password.
pub fn is_set(db: &crate::db::DatabaseConnection) -> bool {
    crate::db::get_db_setting(db, SETTING).is_some()
}

/// Whether `password` is the duress password of the journal at `db_path`. Opens the file
/// read-only and changes nothing; false when none is set or the file can't be read.
pub fn matches_file(db_path: &std::path::Path, password: &str) -> bool {
    match rusqlite::Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) {
        Ok(conn) => matches_conn(&conn, password),
        Err(_) => false,
    }
}

/// Whether `password` is the duress password stored in `conn` (false when none is set).
pub(crate) fn matches_conn(conn: &rusqlite::Connection, password: &str) -> bool {
    let Some(blob) = crate::db::queries::db_settings::get_db_setting_conn(conn, SETTING)
        .and_then(|h| hex::decode(h).ok())
    else {
        return false;
    };
    PasswordMethod::new(password.to_string())
        .unwrap_master_key(&blob)
        .is_ok()
}

/// Whether any password slot of `conn` opens with `password`.
fn opens_journal(conn: &rusqlite::Connection, password: &str) -> bool {
    let wrapped: Vec<Vec<u8>> =
        match conn.prepare("SELECT wrapped_key FROM auth_slots WHERE type = 'password'") {
            Ok(mut stmt) => stmt
                .query_map([], |row| row.get::<_, Vec<u8>>(0))
                .map(|rows| rows.flatten().collect())
                .unwrap_or_default(),
            Err(_) => return false,
        };
    let method = PasswordMethod::new(password.to_string());
    wrapped.iter().any(|w| method.unwrap_master_key(w).is_ok())
}

#[cfg(test)]
mod tests;
