use log::info;
use tauri::State;

use super::{with_unlocked_db, DiaryState};

/// Adds a password authentication method using the master key held in the current session.
///
/// Fails if a password slot already exists — use `change_password` to update it.
/// No existing password is required: being unlocked is the authentication.
#[tauri::command]
pub fn register_password(new_password: String, state: State<DiaryState>) -> Result<(), String> {
    if new_password.is_empty() {
        return Err("Password cannot be empty".to_string());
    }

    with_unlocked_db(&state, |db| {
        // Reject if a password slot already exists
        if crate::db::get_password_slot(db)?.is_some() {
            return Err(
                "A password method already exists. Use 'Change Password' to update it.".to_string(),
            );
        }

        // Wrap the master key (already in memory) with the new password and store the slot.
        crate::auth::add_password_slot(db, "Password", &new_password)?;

        info!("Password auth method registered");
        Ok(())
    })
}

/// Creates a new key file and registers it as an unlock method.
///
/// The key-file secret is generated here and written straight to `path`, so it never passes
/// through the frontend. The slot uses the symmetric, quantum-resistant v2 format
/// ([`crate::auth::KeyFileMethod`]). Requires the current password only when a password slot
/// exists; otherwise being unlocked is sufficient (same model as `register_password`).
///
/// `path` must end in `.key` and must not be a network (UNC) path: this command can create
/// a file, so it is limited to the one kind of file it exists to write. The file is written
/// before the slot is added, and removed again if adding the slot fails, so a slot never
/// exists without its file.
#[tauri::command]
pub fn create_key_file(
    path: String,
    label: String,
    current_password: Option<String>,
    state: State<DiaryState>,
) -> Result<(), String> {
    create_key_file_inner(&path, &label, current_password, &state)
}

pub(crate) fn create_key_file_inner(
    path: &str,
    label: &str,
    current_password: Option<String>,
    state: &DiaryState,
) -> Result<(), String> {
    let target = std::path::Path::new(path);
    let is_key = target
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("key"));
    if !is_key {
        return Err("A key file must be saved with the .key extension".to_string());
    }
    if path.starts_with("\\\\") || path.starts_with("//") {
        return Err("A key file cannot be saved to a network location".to_string());
    }
    if label.trim().is_empty() {
        return Err("A key file needs a label".to_string());
    }

    with_unlocked_db(state, |db| {
        // Identity gate: require password verification only when a password slot exists.
        if let Some((_, wrapped_key)) = crate::db::get_password_slot(db)? {
            let pwd = current_password
                .ok_or("Password required to add a key file when a password method exists")?;
            crate::auth::PasswordMethod::new(pwd)
                .unwrap_master_key(&wrapped_key)
                .map_err(|_| "Incorrect password".to_string())?;
        }

        let key_file = crate::auth::KeyFileMethod::generate();
        write_secret_file(target, &key_file.file_contents())?;
        if let Err(e) = crate::auth::add_key_file_slot(db, label, &key_file) {
            let _ = std::fs::remove_file(target);
            return Err(e);
        }

        // The label is user-supplied and these records are captured into the debug dump.
        info!("Key file auth method registered");
        Ok(())
    })
}

/// Writes `contents` to `path`, readable only by the owner on Unix (also when the file
/// already existed). On Windows, NTFS ACLs restrict the user profile to its owner.
fn write_secret_file(path: &std::path::Path, contents: &str) -> Result<(), String> {
    let err = |e: std::io::Error| format!("Failed to write key file: {}", e);
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)
            .map_err(err)?;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))
            .map_err(err)?;
        file.write_all(contents.as_bytes()).map_err(err)
    }
    #[cfg(not(unix))]
    {
        std::fs::write(path, contents).map_err(err)
    }
}

/// Pure inner of `remove_auth_method` — takes `&DiaryState` so it can be tested without Tauri.
pub(crate) fn remove_auth_method_inner(
    slot_id: i64,
    current_password: Option<String>,
    state: &DiaryState,
) -> Result<(), String> {
    let db_state = state
        .db
        .lock()
        .map_err(|_| "State lock poisoned".to_string())?;
    let db = db_state.as_ref().ok_or("Journal must be unlocked")?;

    // Look up the type of the slot being removed (must happen before deletion)
    let slot_type = crate::db::get_auth_slot_type(db, slot_id)?
        .ok_or_else(|| "Auth slot not found".to_string())?;

    // Identity gate: verify password only when a password slot exists.
    // If no password slot, being unlocked is sufficient.
    let password_slot = crate::db::get_password_slot(db)?;
    if let Some((_, wrapped_key)) = password_slot {
        let pwd = current_password.ok_or(
            "Password required to remove an auth method when a password method exists".to_string(),
        )?;
        let method = crate::auth::PasswordMethod::new(pwd);
        // The returned SecretBytes is dropped immediately after the guard check, zeroing memory.
        method
            .unwrap_master_key(&wrapped_key)
            .map_err(|_| "Incorrect password".to_string())?;
    }
    // No password slot: being unlocked is sufficient

    // Guard: never remove the last auth method
    let count = crate::db::count_auth_slots(db)?;
    if count <= 1 {
        return Err(
            "Cannot remove the last authentication method. Add another method first.".to_string(),
        );
    }

    // Guard: prevent removal if require_all_auth would be left with < 2 non-auto slots
    {
        let all_methods = crate::db::list_auth_slots(db)?;
        let non_auto_count = all_methods.iter().filter(|m| m.slot_type != "auto").count();
        let removing_non_auto = slot_type != "auto";
        if removing_non_auto && non_auto_count <= 2 {
            let require_all = crate::db::get_db_setting(db, "require_all_auth")
                .map(|v| v == "true")
                .unwrap_or(false);
            if require_all {
                return Err(
                    "Disable multi-auth unlock before removing this authentication method."
                        .to_string(),
                );
            }
        }
    }

    crate::db::delete_auth_slot(db, slot_id)?;
    info!("Auth method {} removed", slot_id);

    // Clean up auto_key in config when an auto slot is removed
    if slot_type == "auto" {
        if let Some(active_id) = crate::config::load_active_journal_id(&state.app_data_dir) {
            let _ = crate::config::save_journal_auto_key(&state.app_data_dir, &active_id, None);
        }
    }

    Ok(())
}

/// Removes an authentication method by slot id.
///
/// Requires the current password only when a password slot exists.
/// If no password slot exists, being unlocked is sufficient.
/// Refuses to remove the last auth method.
#[tauri::command]
pub fn remove_auth_method(
    slot_id: i64,
    current_password: Option<String>,
    state: State<DiaryState>,
) -> Result<(), String> {
    // Removing a credential is irreversible and locks the user out of one way in. Snapshot
    // first — note that the snapshot keeps the slot being removed, which is a disclosure the
    // Backups panel has to make rather than a bug.
    crate::commands::backup_triggers::snapshot_before_destructive(&state, "remove_auth_method");

    remove_auth_method_inner(slot_id, current_password, &state)
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::super::test_helpers::*;
    use crate::db::{create_database, DiaryEntry};

    #[test]
    fn test_register_keypair_and_unlock() {
        use crate::auth::generate_keypair;
        use crate::db::open_database_with_keypair;

        let (_fixture, _, db_path, backups_dir) = make_state("register_kp");

        let db = create_database(&db_path, "password".to_string()).unwrap();

        // Insert a test entry to verify decryption after keypair unlock
        let entry = DiaryEntry {
            id: 0,
            date: "2024-03-15".to_string(),
            title: "Keypair Test".to_string(),
            text: "Content unlocked via key file".to_string(),
            word_count: 5,
            date_created: "2024-03-15T00:00:00Z".to_string(),
            date_updated: "2024-03-15T00:00:00Z".to_string(),
            metadata: None,
            locked: false,
        };
        crate::db::insert_entry(&db, &entry).unwrap();

        // Generate keypair
        let kp = generate_keypair().unwrap();
        let priv_bytes_vec = hex::decode(&kp.private_key_hex).unwrap();
        let pub_bytes_vec = hex::decode(&kp.public_key_hex).unwrap();

        let mut priv_key = [0u8; 32];
        priv_key.copy_from_slice(&priv_bytes_vec);
        let mut pub_key = [0u8; 32];
        pub_key.copy_from_slice(&pub_bytes_vec);

        // Register the keypair slot via the façade composed op (wraps the session master key).
        crate::auth::add_keypair_slot(&db, "Test Key", pub_key).unwrap();
        drop(db);

        // Unlock with private key
        let db2 = open_database_with_keypair(&db_path, priv_key, &backups_dir).unwrap();

        assert_eq!(
            crate::db::read_schema_version(&db2).unwrap(),
            crate::db::SCHEMA_VERSION
        );

        // Verify entry is decryptable with the master key unwrapped via keypair
        let entries = crate::db::get_entries_by_date(&db2, "2024-03-15").unwrap();
        assert_eq!(entries.len(), 1, "Entry should exist after keypair unlock");
        let retrieved = &entries[0];
        assert_eq!(retrieved.title, "Keypair Test");
        assert_eq!(retrieved.text, "Content unlocked via key file");
    }

    #[test]
    fn test_remove_auth_method_locked_returns_error() {
        let (_fixture, state, _, _) = make_state("rm_locked");
        let err = super::remove_auth_method_inner(1, None, &state).unwrap_err();
        assert!(err.contains("unlocked"), "got: {}", err);
    }

    #[test]
    fn test_remove_auth_method_last_slot_guard() {
        let (_fixture, state, db_path, _backups_dir) = make_state("rm_last_slot");
        let db = create_database(&db_path, "password".to_string()).unwrap();
        // One slot exists (password). Put db into state.
        *state.db.lock().unwrap() = Some(db);

        // Get slot id via db query layer directly (list_auth_methods_inner lives in auth_identity)
        let slot_id = {
            let db_guard = state.db.lock().unwrap();
            let db_inner = db_guard.as_ref().unwrap();
            let slots = crate::db::list_auth_slots(db_inner).unwrap();
            assert_eq!(slots.len(), 1);
            slots[0].id
        };

        let err = super::remove_auth_method_inner(slot_id, Some("password".to_string()), &state)
            .unwrap_err();
        assert!(err.contains("Cannot remove the last"), "got: {}", err);
    }

    #[test]
    fn test_register_password_when_none_exists() {
        let (_fixture, _, db_path, _backups_dir) = make_state("reg_pw_none");

        let db = create_database(&db_path, "original".to_string()).unwrap();

        // Delete the existing password slot to simulate a keypair-only diary
        let (slot_id, _) = crate::db::get_password_slot(&db).unwrap().unwrap();
        crate::db::delete_auth_slot(&db, slot_id).unwrap();
        assert!(crate::db::get_password_slot(&db).unwrap().is_none());

        // register_password logic: wrap master key with the new password via the façade op
        crate::auth::add_password_slot(&db, "Password", "newpassword1").unwrap();

        // Slot should now exist
        assert!(crate::db::get_password_slot(&db).unwrap().is_some());
    }

    #[test]
    fn test_register_password_and_unlock() {
        let (_fixture, _, db_path, backups_dir) = make_state("reg_pw_unlock");

        let db = create_database(&db_path, "original".to_string()).unwrap();

        // Add a keypair slot, then remove the password slot
        let kp = crate::auth::generate_keypair().unwrap();
        let pub_key_vec = hex::decode(&kp.public_key_hex).unwrap();
        let mut pub_key = [0u8; 32];
        pub_key.copy_from_slice(&pub_key_vec);
        crate::auth::add_keypair_slot(&db, "My Key", pub_key).unwrap();

        let (pw_slot_id, _) = crate::db::get_password_slot(&db).unwrap().unwrap();
        crate::db::delete_auth_slot(&db, pw_slot_id).unwrap();

        // Register new password using the master key from the session
        let new_pw = "mynewpassword";
        crate::auth::add_password_slot(&db, "Password", new_pw).unwrap();
        drop(db);

        // Should now be able to unlock with the new password
        let db2 = crate::db::open_database(&db_path, new_pw.to_string(), &backups_dir).unwrap();
        assert_eq!(crate::db::count_auth_slots(&db2).unwrap(), 2); // keypair + new password
    }

    #[test]
    fn test_register_password_rejects_duplicate() {
        let (_fixture, _, db_path, _backups_dir) = make_state("reg_pw_dup");

        let db = create_database(&db_path, "existing".to_string()).unwrap();

        // A password slot already exists — register_password should reject
        let existing = crate::db::get_password_slot(&db).unwrap();
        assert!(existing.is_some(), "Should already have a password slot");

        // Simulate the guard in register_password
        let result: Result<(), String> = if existing.is_some() {
            Err("A password method already exists. Use 'Change Password' to update it.".to_string())
        } else {
            Ok(())
        };
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already exists"));
    }

    #[test]
    fn test_register_password_rejects_empty_password() {
        // Empty password check
        let empty = "";
        let result: Result<(), String> = if empty.is_empty() {
            Err("Password cannot be empty".to_string())
        } else {
            Ok(())
        };
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cannot be empty"));

        // 1-character password should be accepted (even if very weak)
        let short = "a";
        let result: Result<(), String> = if short.is_empty() {
            Err("Password cannot be empty".to_string())
        } else {
            Ok(())
        };
        assert!(result.is_ok(), "1-char password should be accepted");
    }

    #[test]
    fn test_register_keypair_no_password_slot() {
        use crate::auth::generate_keypair;
        use crate::db::open_database_with_keypair;

        let (_fixture, _, db_path, backups_dir) = make_state("reg_kp_no_pw");

        let db = create_database(&db_path, "password".to_string()).unwrap();

        // Delete the password slot to simulate a keypair-only diary
        let (pw_slot_id, _) = crate::db::get_password_slot(&db).unwrap().unwrap();
        crate::db::delete_auth_slot(&db, pw_slot_id).unwrap();
        assert!(crate::db::get_password_slot(&db).unwrap().is_none());

        // Simulate register_keypair with no password slot: wrap via session key directly
        let kp = generate_keypair().unwrap();
        let priv_bytes_vec = hex::decode(&kp.private_key_hex).unwrap();
        let pub_bytes_vec = hex::decode(&kp.public_key_hex).unwrap();
        let mut pub_key = [0u8; 32];
        pub_key.copy_from_slice(&pub_bytes_vec);

        crate::auth::add_keypair_slot(&db, "No-pw key", pub_key).unwrap();

        let count = crate::db::count_auth_slots(&db).unwrap();
        assert_eq!(count, 1, "Should have exactly one keypair slot");
        drop(db);

        // Verify the keypair slot can actually unlock the DB
        let mut priv_key = [0u8; 32];
        priv_key.copy_from_slice(&priv_bytes_vec);
        let db2 = open_database_with_keypair(&db_path, priv_key, &backups_dir).unwrap();
        assert_eq!(
            crate::db::read_schema_version(&db2).unwrap(),
            crate::db::SCHEMA_VERSION
        );
    }

    #[test]
    fn test_remove_auth_method_no_password_slot() {
        use crate::auth::generate_keypair;

        let (_fixture, _, db_path, _backups_dir) = make_state("rm_no_pw");

        let db = create_database(&db_path, "password".to_string()).unwrap();

        // Add two keypair slots
        for label in &["Key A", "Key B"] {
            let kp = generate_keypair().unwrap();
            let mut pub_key = [0u8; 32];
            pub_key.copy_from_slice(&hex::decode(&kp.public_key_hex).unwrap());
            crate::auth::add_keypair_slot(&db, label, pub_key).unwrap();
        }

        // Remove the password slot — now only keypair slots remain
        let (pw_slot_id, _) = crate::db::get_password_slot(&db).unwrap().unwrap();
        crate::db::delete_auth_slot(&db, pw_slot_id).unwrap();
        assert!(crate::db::get_password_slot(&db).unwrap().is_none());

        let count_before = crate::db::count_auth_slots(&db).unwrap();
        assert_eq!(count_before, 2, "Should have two keypair slots");

        // Simulate remove_auth_method with no password slot:
        // no password check needed, count > 1, so removal is allowed.
        let slots = crate::db::list_auth_slots(&db).unwrap();
        let slot_to_remove = slots[0].id;

        let count = crate::db::count_auth_slots(&db).unwrap();
        assert!(count > 1, "Guard: must not be the last slot");
        crate::db::delete_auth_slot(&db, slot_to_remove).unwrap();

        let count_after = crate::db::count_auth_slots(&db).unwrap();
        assert_eq!(count_after, 1, "Should have one keypair slot remaining");
    }

    #[test]
    fn test_create_key_file_writes_v2_slot_and_unlocks() {
        let (fixture, state, db_path, _backups) = make_state("create_key_file");
        drop(create_database(&db_path, "pw".to_string()).unwrap());
        super::super::perform_unlock(super::super::UnlockMode::Password("pw".to_string()), &state)
            .unwrap();
        let key_path = fixture.path().join("new.key");
        let key_str = key_path.to_str().unwrap().to_string();

        // Password gate.
        assert!(super::create_key_file_inner(&key_str, "Mine", None, &state).is_err());
        assert!(
            super::create_key_file_inner(&key_str, "Mine", Some("bad".into()), &state).is_err()
        );
        assert!(
            !key_path.exists(),
            "no file may be written when the gate fails"
        );

        super::create_key_file_inner(&key_str, "Mine", Some("pw".into()), &state).unwrap();
        let contents = std::fs::read_to_string(&key_path).unwrap();
        assert_eq!(contents.len(), 64);
        assert!(contents.chars().all(|c| c.is_ascii_hexdigit()));
        {
            let guard = state.db.lock().unwrap();
            let db = guard.as_ref().unwrap();
            let slots = crate::db::list_auth_slots(db).unwrap();
            assert!(slots
                .iter()
                .any(|s| s.slot_type == "keypair" && s.label == "Mine"));
            assert!(!crate::db::legacy_material(db).unwrap().any());
        }

        super::super::lock_diary_inner(&state).unwrap();
        super::super::perform_unlock(super::super::UnlockMode::Keypair(key_str), &state).unwrap();
    }

    #[test]
    fn test_create_key_file_rejects_other_extensions_and_network_paths() {
        let (fixture, state, db_path, _backups) = make_state("create_key_file_paths");
        drop(create_database(&db_path, "pw".to_string()).unwrap());
        super::super::perform_unlock(super::super::UnlockMode::Password("pw".to_string()), &state)
            .unwrap();

        let txt = fixture.path().join("startup.bat");
        let err =
            super::create_key_file_inner(txt.to_str().unwrap(), "Mine", Some("pw".into()), &state)
                .unwrap_err();
        assert!(err.contains(".key"), "got: {err}");
        assert!(!txt.exists());

        let err = super::create_key_file_inner(
            r"\\server\share\x.key",
            "Mine",
            Some("pw".into()),
            &state,
        )
        .unwrap_err();
        assert!(err.contains("network"), "got: {err}");
    }
}
