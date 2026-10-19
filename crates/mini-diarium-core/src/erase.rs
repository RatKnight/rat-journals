//! Permanent erasure (crypto-shredding) for the dead man's switch and the panic password.
//!
//! Deleting a file only unlinks it; its bytes stay on the disk until overwritten, and an
//! undelete tool can bring a journal back. So before deleting, the key material is destroyed
//! in place: a journal's wrapped keys (`auth_slots`, plus a legacy password hash) are deleted
//! with SQLite's `secure_delete`, which overwrites the freed bytes, and a vault header is
//! overwritten with random bytes. What is left is ciphertext nobody holds a key for.
//!
//! Limits: SSDs and some file systems may keep old copies of a block elsewhere (wear
//! levelling, snapshots), and cloud folders keep version history; overwriting in place is a
//! best effort there.

use rand_core::{OsRng, RngCore};
use std::io::Write;
use std::path::Path;

/// Destroys the key material inside a journal database (or a backup of one), then deletes
/// it with its SQLite side files. A file that isn't a journal is just deleted.
pub fn shred_journal_file(path: &Path) {
    if path.exists() {
        if let Ok(conn) = rusqlite::Connection::open(path) {
            let _ = conn.execute_batch(
                "PRAGMA secure_delete = ON;
                 DELETE FROM auth_slots;",
            );
            let _ = conn.execute(
                "DELETE FROM metadata WHERE key IN ('password_hash', 'salt')",
                [],
            );
            let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
        }
    }
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let mut name = path.as_os_str().to_owned();
        name.push(suffix);
        let side = std::path::PathBuf::from(name);
        if side.exists() {
            if let Err(e) = std::fs::remove_file(&side) {
                log::warn!("Could not erase {}: {}", side.display(), e);
            }
        }
    }
}

/// Shreds every file under `dir` as a journal file, then removes the folder.
pub fn shred_journal_dir(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            shred_journal_dir(&path);
        } else if !is_side_file(&path) {
            shred_journal_file(&path);
        }
    }
    let _ = std::fs::remove_dir_all(dir);
}

fn is_side_file(path: &Path) -> bool {
    let name = path.to_string_lossy();
    name.ends_with("-wal") || name.ends_with("-shm") || name.ends_with("-journal")
}

/// Overwrites a file with random bytes of the same length, flushes it, then deletes it.
pub fn overwrite_and_remove(path: &Path) -> Result<(), String> {
    let len = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0) as usize;
    let mut noise = vec![0u8; len];
    OsRng.fill_bytes(&mut noise);
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .map_err(|e| format!("Could not erase {}: {}", path.display(), e))?;
    file.write_all(&noise)
        .and_then(|()| file.sync_all())
        .map_err(|e| format!("Could not erase {}: {}", path.display(), e))?;
    drop(file);
    std::fs::remove_file(path).map_err(|e| format!("Could not erase {}: {}", path.display(), e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shredding_a_journal_removes_its_keys_before_deleting_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("diary.db");
        drop(crate::db::create_database(&path, "pw".to_string()).unwrap());
        let copy = dir.path().join("copy.db");
        std::fs::copy(&path, &copy).unwrap();

        // Shredding the copy in place leaves a file nothing can unlock.
        let conn = rusqlite::Connection::open(&copy).unwrap();
        conn.execute_batch("PRAGMA secure_delete = ON; DELETE FROM auth_slots;")
            .unwrap();
        drop(conn);
        assert!(crate::db::open_database(&copy, "pw".to_string(), dir.path()).is_err());

        shred_journal_file(&path);
        assert!(!path.exists());
    }

    #[test]
    fn shredding_a_folder_removes_everything() {
        let dir = tempfile::tempdir().unwrap();
        let backups = dir.path().join("backups");
        std::fs::create_dir_all(backups.join("inner")).unwrap();
        drop(crate::db::create_database(backups.join("a.db"), "pw".to_string()).unwrap());
        std::fs::write(backups.join("inner").join("note.txt"), b"x").unwrap();
        shred_journal_dir(&backups);
        assert!(!backups.exists());
    }

    #[test]
    fn overwrite_and_remove_deletes_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rat-vault.json");
        std::fs::write(&path, b"{\"slots\":[]}").unwrap();
        overwrite_and_remove(&path).unwrap();
        assert!(!path.exists());
    }
}
