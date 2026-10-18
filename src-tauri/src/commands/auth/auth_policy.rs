use log::info;
use tauri::State;

use super::DiaryState;

/// Enables or disables "require all unlock methods" for the active journal.
///
/// Enabling splits the journal's master key across every non-auto method (see
/// `mini_diarium_core::db::shares`), so the journal cryptographically needs all of them;
/// disabling gives each method the full key again. Both need every credential:
/// `password` when the journal has one, and a path to each registered key file. Requires
/// at least two non-auto methods. The flag and its MAC stay in `db_settings` so the unlock
/// screen keeps offering the combined unlock.
#[tauri::command]
pub fn set_require_all_auth(
    enabled: bool,
    password: Option<String>,
    key_file_paths: Option<Vec<String>>,
    state: State<DiaryState>,
) -> Result<(), String> {
    set_require_all_auth_inner(
        enabled,
        password.as_deref(),
        key_file_paths.as_deref().unwrap_or_default(),
        &state,
    )
}

pub(crate) fn set_require_all_auth_inner(
    enabled: bool,
    password: Option<&str>,
    key_file_paths: &[String],
    state: &DiaryState,
) -> Result<(), String> {
    let key_files = key_file_paths
        .iter()
        .map(|p| super::read_key_file(p))
        .collect::<Result<Vec<_>, _>>()?;
    let db_state = state
        .db
        .lock()
        .map_err(|_| "State lock poisoned".to_string())?;
    let db = db_state.as_ref().ok_or("Journal must be unlocked")?;

    let creds = crate::db::SlotCredentials {
        password,
        key_files: &key_files,
    };
    if enabled {
        let methods = crate::db::list_auth_slots(db)?;
        let non_auto = methods.iter().filter(|m| m.slot_type != "auto").count();
        if non_auto < 2 {
            return Err(
                "Require-all-auth needs at least two non-auto authentication methods.".to_string(),
            );
        }
        crate::db::enable_key_shares(db, &creds)?;
        crate::db::set_db_setting(db, "require_all_auth", "true")?;
        crate::db::write_require_all_auth_mac(db)?;
    } else {
        crate::db::disable_key_shares(db, &creds)?;
        crate::db::delete_db_setting(db, "require_all_auth")?;
        crate::db::delete_db_setting(db, "require_all_auth_mac")?;
    }

    // Best-effort cleanup of legacy config.json value
    if let Some(active_id) = crate::config::load_active_journal_id(&state.app_data_dir) {
        let _ = crate::config::set_journal_require_all_auth(&state.app_data_dir, &active_id, false);
    }

    info!("require_all_auth set to {}", enabled);
    Ok(())
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::super::test_helpers::*;
    use crate::db::create_database;

    #[test]
    fn test_set_require_all_auth_true_writes_valid_mac() {
        let (_fixture, state, db_path, _backups_dir) = make_state("require_all_auth_enable");

        let db = create_database(&db_path, "password".to_string()).unwrap();

        // Add a second non-auto auth slot (required for enabling)
        crate::auth::add_password_slot(&db, "Second Password", "second_password").unwrap();

        // Enable require_all_auth using the internal db query functions
        {
            let mut db_state = state.db.lock().unwrap();
            *db_state = Some(db);
        }
        {
            let db_guard = state.db.lock().unwrap();
            let db_ref = db_guard.as_ref().unwrap();
            crate::db::set_db_setting(db_ref, "require_all_auth", "true").unwrap();
            crate::db::write_require_all_auth_mac(db_ref).unwrap();
        }

        // Verify "true" flag is set
        let db_guard = state.db.lock().unwrap();
        let db_ref = db_guard.as_ref().unwrap();
        assert_eq!(
            crate::db::get_db_setting(db_ref, "require_all_auth").unwrap(),
            "true"
        );

        // Verify MAC is present (64-char hex string)
        let mac_hex = crate::db::get_db_setting(db_ref, "require_all_auth_mac").unwrap();
        assert_eq!(mac_hex.len(), 64);

        // Verify the MAC is valid by checking verify_require_all_auth returns true
        assert!(crate::db::verify_require_all_auth(db_ref));

        drop(db_guard);
    }

    #[test]
    fn test_set_require_all_auth_false_deletes_both_rows() {
        let (_fixture, state, db_path, _backups_dir) = make_state("require_all_auth_disable");

        let db = create_database(&db_path, "password".to_string()).unwrap();

        // Add a second non-auto auth slot
        crate::auth::add_password_slot(&db, "Second Password", "second_password").unwrap();

        // Enable first via internal db query functions
        {
            let mut db_state = state.db.lock().unwrap();
            *db_state = Some(db);
        }
        {
            let db_guard = state.db.lock().unwrap();
            let db_ref = db_guard.as_ref().unwrap();
            crate::db::set_db_setting(db_ref, "require_all_auth", "true").unwrap();
            crate::db::write_require_all_auth_mac(db_ref).unwrap();
        }

        let db_guard = state.db.lock().unwrap();
        let db_ref = db_guard.as_ref().unwrap();
        assert_eq!(
            crate::db::get_db_setting(db_ref, "require_all_auth").unwrap(),
            "true"
        );
        assert!(crate::db::get_db_setting(db_ref, "require_all_auth_mac").is_some());
        drop(db_guard);

        // Disable using internal db query functions
        {
            let db_guard = state.db.lock().unwrap();
            let db_ref = db_guard.as_ref().unwrap();
            crate::db::delete_db_setting(db_ref, "require_all_auth").unwrap();
            crate::db::delete_db_setting(db_ref, "require_all_auth_mac").unwrap();
        }

        // Both rows must be deleted
        let db_guard = state.db.lock().unwrap();
        let db_ref = db_guard.as_ref().unwrap();
        assert!(crate::db::get_db_setting(db_ref, "require_all_auth").is_none());
        assert!(crate::db::get_db_setting(db_ref, "require_all_auth_mac").is_none());
        drop(db_guard);
    }

    #[test]
    fn enabling_splits_the_key_and_needs_every_credential() {
        let (fixture, state, db_path, _backups_dir) = make_state("require_all_split");
        let db = create_database(&db_path, "password".to_string()).unwrap();
        let kf = crate::auth::KeyFileMethod::generate();
        crate::auth::add_key_file_slot(&db, "USB", &kf).unwrap();
        let key_path = fixture.path().join("usb.key");
        std::fs::write(&key_path, kf.file_contents().as_str()).unwrap();
        let key_path = key_path.to_string_lossy().to_string();
        *state.db.lock().unwrap() = Some(db);

        // Missing the key file, or the password: refused, nothing changes.
        assert!(super::set_require_all_auth_inner(true, Some("password"), &[], &state).is_err());
        assert!(super::set_require_all_auth_inner(
            true,
            None,
            std::slice::from_ref(&key_path),
            &state
        )
        .is_err());
        assert!(!crate::db::uses_key_shares(
            state.db.lock().unwrap().as_ref().unwrap()
        ));

        super::set_require_all_auth_inner(
            true,
            Some("password"),
            std::slice::from_ref(&key_path),
            &state,
        )
        .unwrap();
        {
            let guard = state.db.lock().unwrap();
            let db = guard.as_ref().unwrap();
            assert!(crate::db::uses_key_shares(db));
            assert!(crate::db::verify_require_all_auth(db));
        }

        // Turning it off also needs every credential.
        assert!(super::set_require_all_auth_inner(false, Some("password"), &[], &state).is_err());
        super::set_require_all_auth_inner(
            false,
            Some("password"),
            std::slice::from_ref(&key_path),
            &state,
        )
        .unwrap();
        let guard = state.db.lock().unwrap();
        let db = guard.as_ref().unwrap();
        assert!(!crate::db::uses_key_shares(db));
        assert!(crate::db::get_db_setting(db, "require_all_auth").is_none());
    }
}
