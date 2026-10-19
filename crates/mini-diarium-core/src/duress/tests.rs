use super::*;
use crate::db::{create_database, open_database};

fn journal() -> (
    tempfile::NamedTempFile,
    tempfile::TempDir,
    crate::db::DatabaseConnection,
) {
    let tmp = tempfile::Builder::new().suffix(".db").tempfile().unwrap();
    let backups = tempfile::tempdir().unwrap();
    let db = create_database(tmp.path(), "real".to_string()).unwrap();
    (tmp, backups, db)
}

fn open_err(path: &std::path::Path, backups: &std::path::Path, password: &str) -> Option<String> {
    open_database(path, password.to_string(), backups).err()
}

#[test]
fn set_clear_and_is_set() {
    let (_tmp, _b, db) = journal();
    assert!(!is_set(&db));
    set(&db, "panic").unwrap();
    assert!(is_set(&db));
    clear(&db).unwrap();
    assert!(!is_set(&db));
}

#[test]
fn refuses_empty_or_the_real_password() {
    let (_tmp, _b, db) = journal();
    assert!(set(&db, "").is_err());
    assert!(set(&db, "real").is_err());
    assert!(!is_set(&db));
}

#[test]
fn the_file_check_tells_duress_real_and_wrong_passwords_apart() {
    let (tmp, backups, db) = journal();
    set(&db, "panic").unwrap();
    drop(db);
    assert!(matches_file(tmp.path(), "panic"));
    assert!(!matches_file(tmp.path(), "real"));
    assert!(!matches_file(tmp.path(), "nope"));
    // The real password still opens it; the panic password alone opens nothing.
    assert_eq!(open_err(tmp.path(), backups.path(), "real"), None);
    assert_eq!(
        open_err(tmp.path(), backups.path(), "panic"),
        Some("Incorrect password".to_string())
    );
}

#[test]
fn without_a_duress_password_it_is_just_wrong() {
    let (tmp, backups, db) = journal();
    drop(db);
    assert_eq!(
        open_err(tmp.path(), backups.path(), "panic"),
        Some("Incorrect password".to_string())
    );
}
