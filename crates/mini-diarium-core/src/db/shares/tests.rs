use super::*;
use crate::db::queries::{get_entry_by_id, insert_entry, DiaryEntry};
use crate::db::schema::{create_database, open_database, open_database_with_keypair};

struct Fixture {
    _dir: tempfile::TempDir,
    db_path: std::path::PathBuf,
    backups: std::path::PathBuf,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("diary.db");
    let backups = dir.path().join("backups");
    std::fs::create_dir_all(&backups).unwrap();
    Fixture {
        _dir: dir,
        db_path,
        backups,
    }
}

fn kf_copy(kf: &KeyFileMethod) -> KeyFileMethod {
    let bytes: [u8; 32] = hex::decode(kf.file_contents().as_str())
        .unwrap()
        .try_into()
        .unwrap();
    KeyFileMethod::from_secret(bytes)
}

fn entry() -> DiaryEntry {
    let now = "2026-10-18T00:00:00Z".to_string();
    DiaryEntry {
        id: 0,
        date: "2026-10-18".to_string(),
        title: "Secret title".to_string(),
        text: "<p>secret</p>".to_string(),
        word_count: 1,
        date_created: now.clone(),
        date_updated: now,
        metadata: None,
        locked: false,
    }
}

/// A journal with a password and one key file, an entry, and shares enabled.
fn shared_journal(f: &Fixture) -> (KeyFileMethod, i64) {
    let db = create_database(&f.db_path, "pw".to_string()).unwrap();
    let kf = KeyFileMethod::generate();
    crate::auth::add_key_file_slot(&db, "USB", &kf).unwrap();
    let id = insert_entry(&db, &entry()).unwrap();
    let keys = [kf_copy(&kf)];
    enable_key_shares(
        &db,
        &SlotCredentials {
            password: Some("pw"),
            key_files: &keys,
        },
    )
    .unwrap();
    assert!(uses_key_shares(&db));
    (kf, id)
}

#[test]
fn every_credential_is_needed_and_together_they_open_it() {
    let f = fixture();
    let (kf, id) = shared_journal(&f);
    assert!(journal_uses_key_shares(&f.db_path));

    // Each credential alone is refused outright.
    assert_eq!(
        open_database(&f.db_path, "pw".to_string(), &f.backups).unwrap_err(),
        ERR_REQUIRES_ALL_METHODS
    );
    let secret: [u8; 32] = hex::decode(kf.file_contents().as_str())
        .unwrap()
        .try_into()
        .unwrap();
    assert_eq!(
        open_database_with_keypair(&f.db_path, secret, &f.backups).unwrap_err(),
        ERR_REQUIRES_ALL_METHODS
    );

    // A missing or wrong credential fails.
    let none: [KeyFileMethod; 0] = [];
    let only_pw = SlotCredentials {
        password: Some("pw"),
        key_files: &none,
    };
    assert!(open_database_with_all_credentials(&f.db_path, &only_pw, &f.backups).is_err());
    let keys = [kf_copy(&kf)];
    let wrong_pw = SlotCredentials {
        password: Some("nope"),
        key_files: &keys,
    };
    assert!(open_database_with_all_credentials(&f.db_path, &wrong_pw, &f.backups).is_err());

    // All together: the data decrypts.
    let all = SlotCredentials {
        password: Some("pw"),
        key_files: &keys,
    };
    let db = open_database_with_all_credentials(&f.db_path, &all, &f.backups).unwrap();
    assert_eq!(
        get_entry_by_id(&db, id).unwrap().unwrap().title,
        "Secret title"
    );
}

#[test]
fn a_single_slot_holds_only_a_useless_share() {
    let f = fixture();
    let (_kf, id) = shared_journal(&f);
    // Simulate an attacker who strips the marker and unwraps the password slot directly.
    let conn = rusqlite::Connection::open(&f.db_path).unwrap();
    conn.execute(
        "DELETE FROM db_settings WHERE key = 'auth_slots_shared'",
        [],
    )
    .unwrap();
    drop(conn);
    let db = open_database(&f.db_path, "pw".to_string(), &f.backups).unwrap();
    // The "key" it recovered is a share: entries do not decrypt.
    assert!(get_entry_by_id(&db, id).is_err());
}

#[test]
fn turning_it_off_restores_single_credential_unlock() {
    let f = fixture();
    let (kf, id) = shared_journal(&f);
    let keys = [kf_copy(&kf)];
    let all = SlotCredentials {
        password: Some("pw"),
        key_files: &keys,
    };
    let db = open_database_with_all_credentials(&f.db_path, &all, &f.backups).unwrap();

    let none: [KeyFileMethod; 0] = [];
    assert!(disable_key_shares(
        &db,
        &SlotCredentials {
            password: Some("pw"),
            key_files: &none
        }
    )
    .is_err());
    disable_key_shares(&db, &all).unwrap();
    assert!(!uses_key_shares(&db));
    drop(db);

    let db = open_database(&f.db_path, "pw".to_string(), &f.backups).unwrap();
    assert_eq!(
        get_entry_by_id(&db, id).unwrap().unwrap().title,
        "Secret title"
    );
}

#[test]
fn methods_are_fixed_while_shared_but_the_password_can_change() {
    let f = fixture();
    let (kf, id) = shared_journal(&f);
    let keys = [kf_copy(&kf)];
    let all = SlotCredentials {
        password: Some("pw"),
        key_files: &keys,
    };
    let db = open_database_with_all_credentials(&f.db_path, &all, &f.backups).unwrap();
    assert_eq!(
        crate::auth::add_key_file_slot(&db, "x", &KeyFileMethod::generate()).unwrap_err(),
        ERR_SHARED_METHODS_FIXED
    );
    assert_eq!(
        crate::auth::add_password_slot(&db, "x", "y").unwrap_err(),
        ERR_SHARED_METHODS_FIXED
    );

    // Changing the password re-wraps the share (as change_password does).
    let (slot_id, wrapped) = crate::db::queries::get_password_slot(&db).unwrap().unwrap();
    let share = PasswordMethod::new("pw".into())
        .unwrap_master_key(&wrapped)
        .unwrap();
    let rewrapped = PasswordMethod::new("new pw".into())
        .wrap_master_key(&share)
        .unwrap();
    crate::db::queries::update_auth_slot_wrapped_key(&db, slot_id, &rewrapped).unwrap();
    drop(db);

    let new_all = SlotCredentials {
        password: Some("new pw"),
        key_files: &keys,
    };
    let db = open_database_with_all_credentials(&f.db_path, &new_all, &f.backups).unwrap();
    assert_eq!(
        get_entry_by_id(&db, id).unwrap().unwrap().title,
        "Secret title"
    );
}

#[test]
fn enabling_is_refused_with_a_local_key_or_one_method() {
    let f = fixture();
    let db = create_database(&f.db_path, "pw".to_string()).unwrap();
    let none: [KeyFileMethod; 0] = [];
    let creds = SlotCredentials {
        password: Some("pw"),
        key_files: &none,
    };
    assert!(enable_key_shares(&db, &creds)
        .unwrap_err()
        .contains("at least two"));

    let f2 = fixture();
    let auto = crate::db::schema::create_database_auto(&f2.db_path, &[7u8; 32]).unwrap();
    crate::auth::add_password_slot(&auto, "Password", "pw").unwrap();
    assert!(enable_key_shares(&auto, &creds)
        .unwrap_err()
        .contains("Local Key"));
}

#[test]
fn snapshots_open_with_the_live_key_only_if_it_matches() {
    let f = fixture();
    let (kf, _) = shared_journal(&f);
    let keys = [kf_copy(&kf)];
    let all = SlotCredentials {
        password: Some("pw"),
        key_files: &keys,
    };
    let db = open_database_with_all_credentials(&f.db_path, &all, &f.backups).unwrap();
    let live = *db.key().as_bytes();
    let conn = open_connection_readonly(&f.db_path).unwrap();
    assert!(snapshot_key_from_live(&conn, &live).is_ok());
    assert!(snapshot_key_from_live(&conn, &[1u8; 32]).is_err());
}
