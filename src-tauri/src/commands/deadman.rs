//! Dead man's switch: erasing, the plaintext config copy, the startup sweep, and the
//! commands behind its settings and warnings (see `core::deadman`).

use crate::commands::auth::DiaryState;
use crate::deadman::{self, DeadMan};
use log::{info, warn};
use serde::Serialize;
use std::path::Path;
use tauri::State;

/// Permanently erases a journal: destroys the key material in its database and in every
/// backup, then deletes them (see `core::erase`).
pub(crate) fn erase_journal_files(db_path: &Path, backups_dir: &Path) {
    mini_diarium_core::erase::shred_journal_file(db_path);
    mini_diarium_core::erase::shred_journal_dir(backups_dir);
    info!("Erased a journal");
}

/// Stores the plaintext copy of the active journal's switch in config.json.
pub(crate) fn sync_config_copy(state: &DiaryState, dm: Option<DeadMan>) {
    let dir = &state.app_data_dir;
    let Some(active) = crate::config::load_active_journal_id(dir) else {
        return;
    };
    let mut journals = crate::config::load_journals(dir);
    let Some(journal) = journals.iter_mut().find(|j| j.id == active) else {
        return;
    };
    if journal.dead_man == dm {
        return;
    }
    journal.dead_man = dm;
    if let Err(e) = crate::config::save_journals(dir, &journals, &active) {
        warn!("Could not save the dead man's switch copy: {}", e);
    }
}

/// At startup, before anything is unlocked: erases every journal and the remembered vault
/// whose plaintext copy says its switch has expired.
pub(crate) fn sweep(app_data_dir: &Path) {
    let now = crate::clock::unix_now();
    for journal in crate::config::load_journals(app_data_dir) {
        // config.json's copy only says which journals are armed; the deadline itself is read
        // from the journal file, which is current even if another computer unlocked it.
        if journal.dead_man.is_none() {
            continue;
        }
        let Ok((db, backups)) = crate::config::journal_db_paths(
            Path::new(&journal.path),
            journal.db_filename.as_deref(),
        ) else {
            continue;
        };
        if deadman::peek_journal(&db).is_some_and(|dm| dm.expired(now)) {
            erase_journal_files(&db, &backups);
        }
    }
    if let Some(dir) = crate::config::load_last_vault_dir(app_data_dir) {
        match mini_diarium_core::vault::Vault::erase_if_expired(&dir, now) {
            Ok(true) => info!("Erased the vault by its dead man's switch"),
            Ok(false) => {}
            Err(e) => warn!("Could not erase an expired vault: {}", e),
        }
    }
}

/// The armed period in days (0 = off) of the open journal.
#[tauri::command]
pub fn journal_dead_man_days(state: State<DiaryState>) -> Result<u32, String> {
    let guard = state
        .db
        .lock()
        .map_err(|_| "Journal state lock failed".to_string())?;
    let db = guard.as_ref().ok_or("Journal is not unlocked")?;
    Ok(deadman::journal_dead_man(db).map_or(0, |dm| dm.days))
}

/// Arms (days > 0) or disarms (0) the open journal's switch.
#[tauri::command]
pub fn set_journal_dead_man(days: u32, state: State<DiaryState>) -> Result<(), String> {
    let days = deadman::validate_days(days)?;
    let guard = state
        .db
        .lock()
        .map_err(|_| "Journal state lock failed".to_string())?;
    let db = guard.as_ref().ok_or("Journal is not unlocked")?;
    let dm = deadman::set_journal_dead_man(db, days)?;
    drop(guard);
    sync_config_copy(&state, dm);
    Ok(())
}

/// Something that will be erased soon unless it is unlocked.
#[derive(Debug, Serialize)]
pub struct DeadManWarning {
    /// `"journal"` or `"vault"`.
    pub kind: &'static str,
    pub name: String,
    /// Whole days left, rounded up.
    pub days_left: i64,
}

/// Other journals and the remembered vault that are within their warning window.
#[tauri::command]
pub fn dead_man_warnings(state: State<DiaryState>) -> Result<Vec<DeadManWarning>, String> {
    Ok(collect_warnings(
        &state.app_data_dir,
        crate::clock::unix_now(),
    ))
}

pub(crate) fn collect_warnings(app_data_dir: &Path, now: i64) -> Vec<DeadManWarning> {
    let days = |secs: i64| (secs + deadman::DAY_SECS - 1) / deadman::DAY_SECS;
    let active = crate::config::load_active_journal_id(app_data_dir);
    let mut out: Vec<DeadManWarning> = crate::config::load_journals(app_data_dir)
        .into_iter()
        .filter(|j| Some(&j.id) != active.as_ref())
        .filter_map(|j| {
            j.dead_man?;
            let (db, _) =
                crate::config::journal_db_paths(Path::new(&j.path), j.db_filename.as_deref())
                    .ok()?;
            let left = deadman::peek_journal(&db)?.warning(now)?;
            Some(DeadManWarning {
                kind: "journal",
                name: j.name,
                days_left: days(left),
            })
        })
        .collect();
    if let Some(dir) = crate::config::load_last_vault_dir(app_data_dir) {
        if let Some(left) = mini_diarium_core::vault::Vault::dead_man_warning(&dir, now) {
            out.push(DeadManWarning {
                kind: "vault",
                name: dir
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default(),
                days_left: days(left),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::JournalConfig;

    /// A real journal in `dir` whose file records `record` (raw, as another computer or an
    /// older version could have left it), registered with config copy `copy`.
    fn journal(id: &str, dir: &Path, record: Option<&str>, copy: Option<DeadMan>) -> JournalConfig {
        let db = crate::db::create_database(dir.join("diary.db"), "pw".to_string()).unwrap();
        if let Some(record) = record {
            crate::db::set_db_setting(&db, deadman::JOURNAL_SETTING, record).unwrap();
        }
        drop(db);
        std::fs::create_dir_all(dir.join("backups").join("diary")).unwrap();
        JournalConfig {
            id: id.to_string(),
            name: id.to_string(),
            path: dir.to_string_lossy().to_string(),
            auto_key: None,
            db_filename: None,
            require_all_auth: None,
            dead_man: copy,
            duress: None,
        }
    }

    fn dirs() -> (
        tempfile::TempDir,
        std::path::PathBuf,
        std::path::PathBuf,
        std::path::PathBuf,
    ) {
        let t = tempfile::tempdir().unwrap();
        let (app, a, b) = (t.path().join("app"), t.path().join("a"), t.path().join("b"));
        for d in [&app, &a, &b] {
            std::fs::create_dir_all(d).unwrap();
        }
        (t, app, a, b)
    }

    const LONG_AGO: Option<DeadMan> = Some(DeadMan {
        days: 1,
        last_unlock: 0,
    });

    #[test]
    fn sweep_trusts_the_journal_file_not_this_computers_copy() {
        let (_t, app, a, b) = dirs();
        let now = crate::clock::unix_now();
        // Both copies here are stale and say "expired"; only `a`'s file agrees. `b` was
        // unlocked on another computer just now.
        let fresh = format!("1:{now}:00");
        let journals = [
            journal("a", &a, Some("1:0:00"), LONG_AGO),
            journal("b", &b, Some(&fresh), LONG_AGO),
        ];
        crate::config::save_journals(&app, &journals, "b").unwrap();

        sweep(&app);

        assert!(!a.join("diary.db").exists());
        assert!(!a.join("backups").join("diary").exists());
        assert!(b.join("diary.db").exists());
        assert!(b.join("backups").join("diary").exists());
    }

    #[test]
    fn sweep_leaves_journals_not_armed_here_or_without_a_record() {
        let (_t, app, a, b) = dirs();
        let journals = [
            journal("a", &a, Some("1:0:00"), None),
            journal("b", &b, None, LONG_AGO),
        ];
        crate::config::save_journals(&app, &journals, "b").unwrap();
        sweep(&app);
        assert!(a.join("diary.db").exists());
        assert!(b.join("diary.db").exists());
    }

    #[test]
    fn warnings_list_other_journals_near_their_deadline() {
        let (_t, app, a, b) = dirs();
        let now = 10_000_000;
        let record = format!("3:{}:00", now - 2 * deadman::DAY_SECS);
        let journals = [
            journal("a", &a, Some(&record), LONG_AGO),
            journal("b", &b, Some(&record), LONG_AGO),
        ];
        crate::config::save_journals(&app, &journals, "b").unwrap();

        let warnings = collect_warnings(&app, now);

        assert_eq!(warnings.len(), 1);
        assert_eq!(
            (warnings[0].kind, warnings[0].name.as_str()),
            ("journal", "a")
        );
        assert_eq!(warnings[0].days_left, 1);
    }

    #[test]
    fn erasing_shreds_the_database_and_its_backups() {
        let (_t, _app, a, _b) = dirs();
        journal("a", &a, None, None);
        let backup = a.join("backups").join("diary").join("snap.db");
        std::fs::copy(a.join("diary.db"), &backup).unwrap();
        std::fs::write(a.join("diary.db-wal"), b"x").unwrap();

        erase_journal_files(&a.join("diary.db"), &a.join("backups").join("diary"));

        for name in ["diary.db", "diary.db-wal"] {
            assert!(!a.join(name).exists(), "{name} survived");
        }
        assert!(!a.join("backups").join("diary").exists());
    }
}
