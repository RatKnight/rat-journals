use super::*;
use crate::commands::auth::test_helpers::make_state;

fn vault_path(fixture: &crate::commands::auth::test_helpers::TestFixture) -> String {
    fixture
        .path()
        .join("My Vault")
        .to_string_lossy()
        .to_string()
}

fn write(
    fixture: &crate::commands::auth::test_helpers::TestFixture,
    name: &str,
    data: &[u8],
) -> String {
    let p = fixture.path().join(name);
    std::fs::write(&p, data).unwrap();
    p.to_string_lossy().to_string()
}

#[test]
fn create_add_preview_save_and_list() {
    let (fixture, state, _, _) = make_state("vault_basic");
    let path = vault_path(&fixture);
    let status = create_inner(&state, &path, Some("pw".into()), None).unwrap();
    assert_eq!(status.name, "My Vault");
    assert!(check_folder_inner(&path).unwrap().is_vault);

    let a = write(&fixture, "scan.pdf", b"%PDF-1.7 medical");
    let dir = fixture.path().join("a folder");
    std::fs::create_dir_all(&dir).unwrap();
    let result = add_files_inner(&state, &[a, dir.to_string_lossy().to_string()], None).unwrap();
    assert_eq!(result.added.len(), 1);
    assert_eq!(result.failed.len(), 1, "a folder is reported, not added");
    let id = result.added[0].id.clone();

    assert_eq!(preview_inner(&state, &id).unwrap(), b"%PDF-1.7 medical");
    let out = fixture.path().join("copy.pdf");
    save_copy_inner(&state, &id, out.to_str().unwrap()).unwrap();
    assert_eq!(std::fs::read(&out).unwrap(), b"%PDF-1.7 medical");
    assert!(save_copy_inner(&state, &id, r"\\server\share\x.pdf").is_err());

    let listing = list_inner(&state).unwrap();
    assert_eq!(listing.files.len(), 1);
    assert_eq!(listing.files[0].name, "scan.pdf");
}

#[test]
fn every_lock_path_closes_the_vault_even_without_a_journal() {
    let (fixture, state, _, _) = make_state("vault_lock_paths");
    create_inner(&state, &vault_path(&fixture), Some("pw".into()), None).unwrap();
    assert!(is_vault_open(&state));

    // No journal is unlocked: the journal lock reports "nothing to lock" but still closes
    // the vault, which is what the idle timer and OS screen lock rely on.
    assert!(!crate::commands::auth::lock_diary_inner(&state).unwrap());
    assert!(!is_vault_open(&state));
    assert!(status_inner(&state).unwrap().is_none());
    assert_eq!(list_inner(&state).unwrap_err(), ERR_NO_VAULT);
}

#[test]
fn a_lock_during_an_operation_closes_the_vault_when_it_finishes() {
    let (fixture, state, _, _) = make_state("vault_close_requested");
    create_inner(&state, &vault_path(&fixture), Some("pw".into()), None).unwrap();
    // What close_vault records when the vault mutex is busy.
    state.vault_close_requested.store(true, Ordering::SeqCst);
    assert_eq!(list_inner(&state).unwrap_err(), ERR_NO_VAULT);
    assert!(!is_vault_open(&state));
}

#[test]
fn open_in_app_copies_are_wiped_when_the_vault_closes() {
    let (fixture, state, _, _) = make_state("vault_open_copy");
    create_inner(&state, &vault_path(&fixture), Some("pw".into()), None).unwrap();
    let src = write(&fixture, "x.txt", b"private");
    let id = add_files_inner(&state, &[src], None).unwrap().added[0]
        .id
        .clone();

    let temp = decrypt_for_open_inner(&state, &id).unwrap();
    assert!(temp.starts_with(state.app_data_dir.join(OPEN_DIR)));
    assert_eq!(std::fs::read(&temp).unwrap(), b"private");
    close_vault(&state);
    assert!(!temp.exists());
}

#[test]
fn leftover_copies_are_wiped_at_startup() {
    let (_fixture, state, _, _) = make_state("vault_wipe_dir");
    let leftover = state.app_data_dir.join(OPEN_DIR).join("abc").join("x.pdf");
    std::fs::create_dir_all(leftover.parent().unwrap()).unwrap();
    std::fs::write(&leftover, b"plaintext").unwrap();
    wipe_temp_dir(&state.app_data_dir);
    assert!(!leftover.exists());
}

#[test]
fn key_file_vault_creates_its_key_file_and_unlocks_with_it() {
    let (fixture, state, _, _) = make_state("vault_key_file");
    let path = vault_path(&fixture);
    let key = fixture
        .path()
        .join("vault.key")
        .to_string_lossy()
        .to_string();
    assert!(create_inner(&state, &path, None, Some("vault.txt".into())).is_err());
    assert!(
        !std::path::Path::new(&path).exists(),
        "bad key path leaves nothing behind"
    );

    create_inner(&state, &path, None, Some(key.clone())).unwrap();
    assert_eq!(std::fs::read_to_string(&key).unwrap().len(), 64);
    close_vault(&state);

    assert!(unlock_password_inner(&state, &path, "pw").is_err());
    unlock_key_file_inner(&state, &path, &key).unwrap();
    assert!(is_vault_open(&state));
}

#[test]
fn unlock_with_the_wrong_password_leaves_it_locked() {
    let (fixture, state, _, _) = make_state("vault_wrong_pw");
    let path = vault_path(&fixture);
    create_inner(&state, &path, Some("pw".into()), None).unwrap();
    close_vault(&state);
    assert_eq!(
        unlock_password_inner(&state, &path, "nope").unwrap_err(),
        vault::ERR_WRONG_CREDENTIAL
    );
    assert!(!is_vault_open(&state));
    unlock_password_inner(&state, &path, "pw").unwrap();
}

#[test]
fn unlock_method_changes_need_the_current_password() {
    let (fixture, state, _, _) = make_state("vault_identity");
    let path = vault_path(&fixture);
    create_inner(&state, &path, Some("old".into()), None).unwrap();

    assert!(change_password_inner(&state, None, "new", &[]).is_err());
    assert!(change_password_inner(&state, Some("wrong"), "new", &[]).is_err());
    change_password_inner(&state, Some("old"), "new", &[]).unwrap();

    let key = fixture
        .path()
        .join("extra.key")
        .to_string_lossy()
        .to_string();
    assert!(add_key_file_inner(&state, &key, "Backup", Some("old")).is_err());
    assert!(!std::path::Path::new(&key).exists());
    add_key_file_inner(&state, &key, "Backup", Some("new")).unwrap();

    let password_slot = status_inner(&state)
        .unwrap()
        .unwrap()
        .slots
        .into_iter()
        .find(|s| s.slot_type == "password")
        .unwrap();
    // Removing the password leaves only the key file, which must be presented.
    assert!(
        remove_slot_inner(&state, password_slot.id, Some("new"), &[])
            .unwrap_err()
            .contains("Backup")
    );
    remove_slot_inner(
        &state,
        password_slot.id,
        Some("new"),
        std::slice::from_ref(&key),
    )
    .unwrap();
    close_vault(&state);

    assert!(unlock_password_inner(&state, &path, "new").is_err());
    unlock_key_file_inner(&state, &path, &key).unwrap();
}

#[test]
fn password_change_rotates_and_revokes_old_header_copies() {
    let (fixture, state, _, _) = make_state("vault_rotation");
    let path = vault_path(&fixture);
    create_inner(&state, &path, Some("old".into()), None).unwrap();
    with_vault(&state, |o| {
        o.vault
            .add_reader("doc.txt", None, &b"secret contents"[..])?;
        Ok(())
    })
    .unwrap();
    let header = std::path::Path::new(&path).join("rat-vault.json");
    let old_header = std::fs::read(&header).unwrap();

    // Changing the password with a key file registered requires presenting it.
    let key = fixture.path().join("kf.key").to_string_lossy().to_string();
    add_key_file_inner(&state, &key, "Backup", Some("old")).unwrap();
    assert!(change_password_inner(&state, Some("old"), "new", &[])
        .unwrap_err()
        .contains("Backup"));
    change_password_inner(&state, Some("old"), "new", std::slice::from_ref(&key)).unwrap();
    close_vault(&state);

    // New password and the key file still open the vault; the data survived rotation.
    let listing = {
        unlock_password_inner(&state, &path, "new").unwrap();
        list_inner(&state).unwrap()
    };
    assert_eq!(listing.files.len(), 1);
    let bytes = with_vault(&state, |o| o.vault.read_bytes(&listing.files[0].id, 1000)).unwrap();
    assert_eq!(bytes.as_slice(), b"secret contents");
    close_vault(&state);
    unlock_key_file_inner(&state, &path, &key).unwrap();
    close_vault(&state);

    // Old password is revoked even against the pre-rotation header (cloud history).
    assert!(unlock_password_inner(&state, &path, "old").is_err());
    std::fs::write(&header, &old_header).unwrap();
    assert!(unlock_password_inner(&state, &path, "old").is_err());
}

#[test]
fn safe_file_names() {
    assert_eq!(safe_file_name("a/b\\c:d*e?.pdf"), "a_b_c_d_e_.pdf");
    assert_eq!(safe_file_name(" ... "), "file");
    assert_eq!(safe_file_name("Report."), "Report");
}

#[test]
fn dropped_bytes_are_added_and_capped() {
    let (fixture, state, _, _) = make_state("vault_add_bytes");
    create_inner(&state, &vault_path(&fixture), Some("pw".into()), None).unwrap();
    let f = add_bytes_inner(&state, "photo.jpg", None, b"jpegdata").unwrap();
    assert_eq!(f.mime.as_deref(), Some("image/jpeg"));
    assert_eq!(preview_inner(&state, &f.id).unwrap(), b"jpegdata");
    let too_big = vec![0u8; MAX_DROP_BYTES + 1];
    assert!(add_bytes_inner(&state, "big.bin", None, &too_big).is_err());
}
