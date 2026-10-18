use super::*;

fn tmp() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

fn vault_dir(t: &tempfile::TempDir) -> PathBuf {
    t.path().join("vault")
}

fn random_bytes(len: usize) -> Vec<u8> {
    let mut d = vec![0u8; len];
    OsRng.fill_bytes(&mut d);
    d
}

fn add(v: &mut Vault, name: &str, folder: Option<&str>, data: &[u8]) -> VaultFile {
    v.add_reader(name, folder, data).unwrap()
}

fn key_file_copy(kf: &KeyFileMethod) -> KeyFileMethod {
    let bytes: [u8; 32] = hex::decode(kf.file_contents().as_str())
        .unwrap()
        .try_into()
        .unwrap();
    KeyFileMethod::from_secret(bytes)
}

/// Every regular file under `dir`, with its bytes.
fn all_files(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir).unwrap().flatten() {
        let p = entry.path();
        if p.is_dir() {
            out.extend(all_files(&p));
        } else {
            out.push((p.clone(), fs::read(&p).unwrap()));
        }
    }
    out
}

#[test]
fn create_reopen_and_wrong_password() {
    let t = tmp();
    let dir = vault_dir(&t);
    let mut v = Vault::create(&dir, Some("correct horse"), None).unwrap();
    let f = add(&mut v, "note.txt", None, b"hello vault");
    drop(v);

    assert!(is_vault(&dir));
    assert_eq!(
        Vault::open_with_password(&dir, "wrong").unwrap_err(),
        ERR_WRONG_CREDENTIAL
    );
    let v = Vault::open_with_password(&dir, "correct horse").unwrap();
    assert_eq!(v.files(), vec![f.clone()]);
    assert_eq!(
        v.read_bytes(&f.id, 1024).unwrap().as_slice(),
        b"hello vault"
    );
}

#[test]
fn key_file_and_password_each_unlock() {
    let t = tmp();
    let dir = vault_dir(&t);
    let kf = KeyFileMethod::generate();
    let v = Vault::create(&dir, Some("pw"), Some(&kf)).unwrap();
    assert_eq!(v.slots().len(), 2);
    drop(v);

    assert!(Vault::open_with_password(&dir, "pw").is_ok());
    assert!(Vault::open_with_key_file(&dir, &key_file_copy(&kf)).is_ok());
    assert_eq!(
        Vault::open_with_key_file(&dir, &KeyFileMethod::generate()).unwrap_err(),
        ERR_WRONG_CREDENTIAL
    );
    let slots = peek_slots(&dir).unwrap();
    assert_eq!(
        slots
            .iter()
            .map(|s| s.slot_type.as_str())
            .collect::<Vec<_>>(),
        ["password", "keyfile"]
    );
}

#[test]
fn create_requires_a_credential_and_an_empty_folder() {
    let t = tmp();
    let dir = vault_dir(&t);
    assert!(Vault::create(&dir, None, None).is_err());
    assert!(Vault::create(&dir, Some(""), None).is_err());
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("existing.txt"), b"x").unwrap();
    assert_eq!(
        Vault::create(&dir, Some("pw"), None).unwrap_err(),
        "Choose an empty folder for a new vault"
    );
}

#[test]
fn large_multi_chunk_file_roundtrips_and_hashes() {
    let t = tmp();
    let dir = vault_dir(&t);
    let mut v = Vault::create(&dir, Some("pw"), None).unwrap();
    let data = random_bytes(stream::CHUNK_SIZE as usize * 2 + 777);
    let f = add(&mut v, "scan.pdf", None, &data);
    assert_eq!(f.size, data.len() as u64);
    assert_eq!(f.sha256, hex::encode(Sha256::digest(&data)));
    assert_eq!(f.mime.as_deref(), Some("application/pdf"));

    let out = t.path().join("out.pdf");
    v.save_copy(&f.id, &out).unwrap();
    assert_eq!(fs::read(&out).unwrap(), data);
    assert!(v.read_bytes(&f.id, 100).is_err(), "preview cap is enforced");
}

#[test]
fn empty_file_roundtrips() {
    let t = tmp();
    let mut v = Vault::create(&vault_dir(&t), Some("pw"), None).unwrap();
    let f = add(&mut v, "empty.txt", None, b"");
    assert_eq!(f.size, 0);
    assert!(v.read_bytes(&f.id, 10).unwrap().is_empty());
}

#[test]
fn add_path_keeps_the_file_name() {
    let t = tmp();
    let src = t.path().join("Bloodwork 2026.pdf");
    fs::write(&src, b"%PDF-1.7 test").unwrap();
    let mut v = Vault::create(&vault_dir(&t), Some("pw"), None).unwrap();
    let f = v.add_path(&src, None).unwrap();
    assert_eq!(f.name, "Bloodwork 2026.pdf");
}

#[test]
fn files_are_listed_newest_first() {
    let t = tmp();
    let mut v = Vault::create(&vault_dir(&t), Some("pw"), None).unwrap();
    let a = add(&mut v, "a.txt", None, b"a");
    // Timestamps have one-second resolution; force a distinct, older time for `a`.
    v.index.files[0].added_at = "2020-01-01T00:00:00Z".to_string();
    let b = add(&mut v, "b.txt", None, b"b");
    let order: Vec<String> = v.files().into_iter().map(|f| f.id).collect();
    assert_eq!(order, vec![b.id, a.id]);
}

#[test]
fn folders_rename_move_and_delete() {
    let t = tmp();
    let dir = vault_dir(&t);
    let mut v = Vault::create(&dir, Some("pw"), None).unwrap();
    v.create_folder("Medical").unwrap();
    assert!(
        v.create_folder("medical").is_err(),
        "names are case-insensitive"
    );
    assert!(v.add_reader("x.txt", Some("Missing"), &b"x"[..]).is_err());

    let f = add(&mut v, "x.txt", Some("Medical"), b"x");
    assert!(
        v.delete_folder("Medical").is_err(),
        "non-empty folder is kept"
    );

    v.rename_folder("Medical", "Health").unwrap();
    assert_eq!(v.files()[0].folder.as_deref(), Some("Health"));

    v.rename_file(&f.id, "renamed.jpg").unwrap();
    v.move_file(&f.id, None).unwrap();
    v.delete_folder("Health").unwrap();
    drop(v);

    let v = Vault::open_with_password(&dir, "pw").unwrap();
    let file = &v.files()[0];
    assert_eq!(file.name, "renamed.jpg");
    assert_eq!(file.mime.as_deref(), Some("image/jpeg"));
    assert_eq!(file.folder, None);
    assert!(v.folders().is_empty());
}

#[test]
fn delete_removes_the_blob() {
    let t = tmp();
    let mut v = Vault::create(&vault_dir(&t), Some("pw"), None).unwrap();
    let f = add(&mut v, "x.txt", None, b"x");
    let blob = v.blob_path(&f.id);
    assert!(blob.exists());
    v.delete_file(&f.id).unwrap();
    assert!(!blob.exists());
    assert!(v.files().is_empty());
}

#[test]
fn nothing_on_disk_contains_plaintext_names_or_contents() {
    let t = tmp();
    let dir = vault_dir(&t);
    let mut v = Vault::create(&dir, Some("pw"), None).unwrap();
    v.create_folder("SecretFolderName").unwrap();
    add(
        &mut v,
        "SecretFileName.pdf",
        Some("SecretFolderName"),
        b"SecretContentMarker",
    );
    for (path, bytes) in all_files(&dir) {
        let text = String::from_utf8_lossy(&bytes);
        for needle in ["SecretFolderName", "SecretFileName", "SecretContentMarker"] {
            assert!(
                !text.contains(needle),
                "{needle} leaked in {}",
                path.display()
            );
        }
        let p = path.to_string_lossy();
        assert!(!p.contains("Secret"), "name leaked in path {p}");
    }
}

#[test]
fn tampered_blob_is_rejected() {
    let t = tmp();
    let mut v = Vault::create(&vault_dir(&t), Some("pw"), None).unwrap();
    let f = add(&mut v, "x.txt", None, &random_bytes(3000));
    let blob = v.blob_path(&f.id);
    let mut bytes = fs::read(&blob).unwrap();
    let mid = bytes.len() / 2;
    bytes[mid] ^= 0x40;
    fs::write(&blob, &bytes).unwrap();
    assert!(v.read_bytes(&f.id, 1 << 20).is_err());
    assert_eq!(v.verify(), vec!["x.txt".to_string()]);
}

#[test]
fn truncated_blob_is_rejected_and_save_copy_leaves_nothing() {
    let t = tmp();
    let mut v = Vault::create(&vault_dir(&t), Some("pw"), None).unwrap();
    let data = random_bytes(stream::CHUNK_SIZE as usize + 10);
    let f = add(&mut v, "big.bin", None, &data);
    let blob = v.blob_path(&f.id);
    let bytes = fs::read(&blob).unwrap();
    // Drop the whole final chunk: what remains is a valid-looking full first chunk.
    let first_chunk_end = stream::HEADER_LEN + stream::CHUNK_SIZE as usize + 16;
    fs::write(&blob, &bytes[..first_chunk_end]).unwrap();
    let out = t.path().join("out.bin");
    assert!(v.save_copy(&f.id, &out).is_err());
    assert!(!out.exists(), "partial plaintext must not be left behind");
}

#[test]
fn swapping_blobs_between_files_is_rejected() {
    let t = tmp();
    let mut v = Vault::create(&vault_dir(&t), Some("pw"), None).unwrap();
    let a = add(&mut v, "a.txt", None, b"aaaa");
    let b = add(&mut v, "b.txt", None, b"bbbb");
    let (pa, pb) = (v.blob_path(&a.id), v.blob_path(&b.id));
    let (ba, bb) = (fs::read(&pa).unwrap(), fs::read(&pb).unwrap());
    fs::write(&pa, &bb).unwrap();
    fs::write(&pb, &ba).unwrap();
    assert!(v.read_bytes(&a.id, 100).is_err());
    assert!(v.read_bytes(&b.id, 100).is_err());
}

#[test]
fn blob_from_another_vault_is_rejected() {
    let t = tmp();
    let mut v1 = Vault::create(&t.path().join("v1"), Some("pw"), None).unwrap();
    let mut v2 = Vault::create(&t.path().join("v2"), Some("pw"), None).unwrap();
    let f1 = add(&mut v1, "a.txt", None, b"one");
    let f2 = add(&mut v2, "a.txt", None, b"two");
    // Same password, but each vault has its own master key and id.
    fs::copy(v1.blob_path(&f1.id), v2.blob_path(&f2.id)).unwrap();
    assert!(v2.read_bytes(&f2.id, 100).is_err());
}

#[test]
fn tampered_index_or_header_is_rejected() {
    let t = tmp();
    let dir = vault_dir(&t);
    let mut v = Vault::create(&dir, Some("pw"), None).unwrap();
    add(&mut v, "a.txt", None, b"a");
    drop(v);

    let index = dir.join(INDEX_FILE);
    let good = fs::read(&index).unwrap();
    let mut bad = good.clone();
    let last = bad.len() - 1;
    bad[last] ^= 1;
    fs::write(&index, &bad).unwrap();
    assert!(Vault::open_with_password(&dir, "pw").is_err());
    fs::write(&index, &good).unwrap();

    let header = dir.join(HEADER_FILE);
    let text = fs::read_to_string(&header).unwrap();
    let mut doc: serde_json::Value = serde_json::from_str(&text).unwrap();
    doc["vault_id"] = serde_json::Value::String(hex::encode(random_id()));
    fs::write(&header, doc.to_string()).unwrap();
    assert!(
        Vault::open_with_password(&dir, "pw").is_err(),
        "a different vault id derives different object keys"
    );
}

#[test]
fn index_gets_a_fresh_object_id_on_every_write() {
    let t = tmp();
    let dir = vault_dir(&t);
    let mut v = Vault::create(&dir, Some("pw"), None).unwrap();
    let id_of = || {
        let mut f = fs::File::open(dir.join(INDEX_FILE)).unwrap();
        stream::read_header(&mut f).unwrap().object_id
    };
    let first = id_of();
    v.create_folder("A").unwrap();
    assert_ne!(first, id_of());
}

#[test]
fn password_and_key_file_management() {
    let t = tmp();
    let dir = vault_dir(&t);
    let mut v = Vault::create(&dir, Some("old"), None).unwrap();
    v.set_password("new").unwrap();
    let kf = KeyFileMethod::generate();
    v.add_key_file("Backup key", &kf).unwrap();
    assert!(v.add_key_file("Again", &key_file_copy(&kf)).is_err());
    drop(v);

    assert!(Vault::open_with_password(&dir, "old").is_err());
    let mut v = Vault::open_with_password(&dir, "new").unwrap();
    let pw_slot = v
        .slots()
        .into_iter()
        .find(|s| s.slot_type == "password")
        .unwrap();
    v.remove_slot(pw_slot.id).unwrap();
    let only = v.slots()[0].id;
    assert!(v.remove_slot(only).is_err(), "the last unlock method stays");
    drop(v);

    assert!(Vault::open_with_password(&dir, "new").is_err());
    assert!(Vault::open_with_key_file(&dir, &key_file_copy(&kf)).is_ok());
}

#[test]
fn rekey_rotates_everything_and_revokes_old_credentials() {
    let t = tmp();
    let dir = vault_dir(&t);
    let mut v = Vault::create(&dir, Some("old"), None).unwrap();
    let kf = KeyFileMethod::generate();
    v.add_key_file("Backup", &kf).unwrap();
    v.create_folder("Medical").unwrap();
    let big = random_bytes(stream::CHUNK_SIZE as usize * 2 + 7);
    let a = add(&mut v, "big.bin", Some("Medical"), &big);
    let b = add(&mut v, "note.txt", None, b"hello");
    let old_header = fs::read(dir.join(HEADER_FILE)).unwrap();
    let old_blob_a = v.blob_path(&a.id);

    v.rekey(Some("new"), &[("Backup".into(), key_file_copy(&kf))])
        .unwrap();

    // Data intact, metadata preserved, blob ids and files rotated.
    let files = v.files();
    assert_eq!(files.len(), 2);
    let a2 = files.iter().find(|f| f.name == "big.bin").unwrap();
    assert_eq!(
        (a2.folder.as_deref(), a2.size, &a2.sha256, &a2.added_at),
        (Some("Medical"), a.size, &a.sha256, &a.added_at)
    );
    assert_ne!(a2.id, a.id);
    assert!(!old_blob_a.exists(), "old blobs are removed");
    assert!(!dir.join(INDEX_NEW_FILE).exists());
    assert_eq!(
        v.read_bytes(a2.id.as_str(), big.len() as u64 + 1)
            .unwrap()
            .as_slice(),
        &big[..]
    );
    drop(v);

    // New password and the presented key file open it; the old password does not —
    // not even with the pre-rotation header restored (cloud version history).
    let v = Vault::open_with_password(&dir, "new").unwrap();
    let b2 = v
        .files()
        .into_iter()
        .find(|f| f.name == "note.txt")
        .unwrap();
    assert_eq!(v.read_bytes(&b2.id, 100).unwrap().as_slice(), b"hello");
    assert_eq!(b2.sha256, b.sha256);
    drop(v);
    Vault::open_with_key_file(&dir, &key_file_copy(&kf)).unwrap();
    assert!(Vault::open_with_password(&dir, "old").is_err());
    fs::write(dir.join(HEADER_FILE), &old_header).unwrap();
    assert!(
        Vault::open_with_password(&dir, "old").is_err(),
        "an old header copy no longer decrypts anything"
    );
}

#[test]
fn rekey_requires_a_credential_and_drops_unlisted_methods() {
    let t = tmp();
    let dir = vault_dir(&t);
    let kf = KeyFileMethod::generate();
    let mut v = Vault::create(&dir, Some("pw"), Some(&kf)).unwrap();
    assert!(v.rekey(None, &[]).is_err());
    assert!(v.rekey(Some(""), &[]).is_err());

    // Rekeying with only a password drops the key-file slot entirely.
    v.rekey(Some("pw2"), &[]).unwrap();
    assert_eq!(v.slots().len(), 1);
    drop(v);
    assert!(Vault::open_with_key_file(&dir, &key_file_copy(&kf)).is_err());
    Vault::open_with_password(&dir, "pw2").unwrap();
}

#[test]
fn interrupted_rekey_is_repaired_on_the_next_unlock() {
    let t = tmp();
    let dir = vault_dir(&t);
    let mut v = Vault::create(&dir, Some("pw"), None).unwrap();
    let f = add(&mut v, "doc.txt", None, b"contents");

    // Crash before the header commit: a staged index the old master cannot read.
    fs::write(dir.join(INDEX_NEW_FILE), b"garbage from a dead rotation").unwrap();
    drop(v);
    let v = Vault::open_with_password(&dir, "pw").unwrap();
    assert!(
        !dir.join(INDEX_NEW_FILE).exists(),
        "stale staging is removed"
    );
    assert_eq!(v.read_bytes(&f.id, 100).unwrap().as_slice(), b"contents");
    drop(v);

    // Crash after the header commit but before the index swap: the primary index is
    // still the pre-rotation one. Reconstruct that state around a completed rekey.
    let old_index = fs::read(dir.join(INDEX_FILE)).unwrap();
    let mut v = Vault::open_with_password(&dir, "pw").unwrap();
    v.rekey(Some("new"), &[]).unwrap();
    let new_index = fs::read(dir.join(INDEX_FILE)).unwrap();
    fs::write(dir.join(INDEX_NEW_FILE), &new_index).unwrap();
    fs::write(dir.join(INDEX_FILE), &old_index).unwrap();
    drop(v);

    let v = Vault::open_with_password(&dir, "new").unwrap();
    assert!(!dir.join(INDEX_NEW_FILE).exists(), "the swap was completed");
    let f2 = v.files().pop().unwrap();
    assert_eq!(v.read_bytes(&f2.id, 100).unwrap().as_slice(), b"contents");
    drop(v);
    // And the repaired primary index is now readable on a plain open.
    Vault::open_with_password(&dir, "new").unwrap();
}

#[test]
fn orphans_from_an_interrupted_add_are_removed() {
    let t = tmp();
    let dir = vault_dir(&t);
    let mut v = Vault::create(&dir, Some("pw"), None).unwrap();
    let keep = add(&mut v, "keep.txt", None, b"keep");
    let orphan = dir
        .join(DATA_DIR)
        .join("ab")
        .join(format!("ab{}.{BLOB_EXT}", "0".repeat(30)));
    fs::create_dir_all(orphan.parent().unwrap()).unwrap();
    fs::write(&orphan, b"leftover").unwrap();
    fs::write(dir.join(".tmp-deadbeef"), b"leftover").unwrap();

    assert_eq!(v.remove_orphans().unwrap(), 2);
    assert!(!orphan.exists());
    assert!(v.blob_path(&keep.id).exists());
    assert_eq!(v.read_bytes(&keep.id, 10).unwrap().as_slice(), b"keep");
}

#[test]
fn names_are_validated() {
    let t = tmp();
    let mut v = Vault::create(&vault_dir(&t), Some("pw"), None).unwrap();
    assert!(v.add_reader("  ", None, &b"x"[..]).is_err());
    assert!(v.add_reader("bad\nname", None, &b"x"[..]).is_err());
    assert!(v.add_reader(&"a".repeat(256), None, &b"x"[..]).is_err());
    assert!(v.create_folder("").is_err());
}

#[test]
fn not_a_vault_and_newer_versions_are_refused() {
    let t = tmp();
    assert!(Vault::open_with_password(t.path(), "pw").is_err());
    let dir = vault_dir(&t);
    Vault::create(&dir, Some("pw"), None).unwrap();
    let header = dir.join(HEADER_FILE);
    let mut doc: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&header).unwrap()).unwrap();
    doc["version"] = serde_json::json!(99);
    fs::write(&header, doc.to_string()).unwrap();
    assert!(Vault::open_with_password(&dir, "pw")
        .unwrap_err()
        .contains("newer version"));
}

#[test]
fn bulk_delete_move_and_save_copies() {
    let t = tmp();
    let dir = vault_dir(&t);
    let mut v = Vault::create(&dir, Some("pw"), None).unwrap();
    v.create_folder("Medical").unwrap();
    let a = add(&mut v, "a.pdf", None, b"aaa");
    let b = add(&mut v, "a.pdf", None, b"bbb");
    let c = add(&mut v, "c.txt", None, b"ccc");

    // An unknown id rejects the whole batch.
    assert!(v
        .move_files(&[a.id.clone(), "0".repeat(32)], Some("Medical"))
        .is_err());
    v.move_files(&[a.id.clone(), b.id.clone()], Some("Medical"))
        .unwrap();
    assert_eq!(
        v.files()
            .iter()
            .filter(|f| f.folder.as_deref() == Some("Medical"))
            .count(),
        2
    );

    // Same-named files are saved side by side, never overwritten.
    let out = t.path().join("out");
    fs::create_dir_all(&out).unwrap();
    fs::write(out.join("c.txt"), b"existing").unwrap();
    let written = v
        .save_copies(&[a.id.clone(), b.id.clone(), c.id.clone()], &out)
        .unwrap();
    let names: Vec<String> = written
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
        .collect();
    assert_eq!(names, vec!["a.pdf", "a (2).pdf", "c (2).txt"]);
    assert_eq!(fs::read(out.join("c.txt")).unwrap(), b"existing");
    assert_eq!(fs::read(out.join("a (2).pdf")).unwrap(), b"bbb");

    assert!(v.delete_files(&[a.id.clone(), "0".repeat(32)]).is_err());
    assert_eq!(v.files().len(), 3);
    v.delete_files(&[a.id.clone(), c.id.clone()]).unwrap();
    assert_eq!(v.files().len(), 1);
    assert!(!v.blob_path(&a.id).exists());
    drop(v);
    let v = Vault::open_with_password(&dir, "pw").unwrap();
    assert_eq!(v.files()[0].id, b.id);
}

#[test]
fn vault_clock_mark_survives_reopen() {
    let t = tmp();
    let dir = vault_dir(&t);
    let mut v = Vault::create(&dir, Some("password"), None).unwrap();
    v.update_clock_mark_at(1_000_000).unwrap();
    drop(v);

    let mut v = Vault::open_with_password(&dir, "password").unwrap();
    let check = v.update_clock_mark_at(1_000_000 + 60).unwrap();
    assert!(!check.suspicious());
    assert_eq!(check.effective_now, 1_000_060);
}

#[test]
fn vault_clock_set_back_is_flagged() {
    let t = tmp();
    let dir = vault_dir(&t);
    let mut v = Vault::create(&dir, Some("password"), None).unwrap();
    v.update_clock_mark_at(1_000_000).unwrap();
    drop(v);

    let mut v = Vault::open_with_password(&dir, "password").unwrap();
    let check = v.update_clock_mark_at(1_000_000 - 3600).unwrap();
    assert_eq!(check.set_back_by, Some(3600));
    assert_eq!(check.effective_now, 1_000_000);
    assert_eq!(v.index.clock_high_water, Some(1_000_000));

    // A further call at the same time should still report the set_back
    let check = v.update_clock_mark_at(1_000_000 - 3600).unwrap();
    assert_eq!(check.set_back_by, Some(3600));
}

#[test]
fn vault_clock_within_tolerance() {
    let t = tmp();
    let dir = vault_dir(&t);
    let mut v = Vault::create(&dir, Some("password"), None).unwrap();
    v.update_clock_mark_at(1_000_000).unwrap();
    let check = v.update_clock_mark_at(1_000_000 - 60).unwrap();
    assert!(!check.suspicious());
}

#[test]
fn vault_clock_mark_survives_rekey() {
    let t = tmp();
    let dir = vault_dir(&t);
    let mut v = Vault::create(&dir, Some("password"), None).unwrap();
    v.update_clock_mark_at(1_000_000).unwrap();
    v.rekey(Some("new"), &[]).unwrap();
    drop(v);

    let mut v = Vault::open_with_password(&dir, "new").unwrap();
    let check = v.update_clock_mark_at(1_000_000 - 3600).unwrap();
    assert_eq!(check.set_back_by, Some(3600));
}
