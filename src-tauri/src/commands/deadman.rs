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
/// whose switch has fired, and removes them from the app's lists.
pub(crate) fn sweep(app_data_dir: &Path) {
    sweep_except(app_data_dir, None, false);
}

/// [`sweep`], leaving out the journal at `open_db` and, with `vault_open`, the remembered
/// vault: those are open right now (see [`tick`]).
pub(crate) fn sweep_except(app_data_dir: &Path, open_db: Option<&Path>, vault_open: bool) {
    let now = crate::clock::unix_now();
    let mut erased = Vec::new();
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
        if open_db == Some(db.as_path()) {
            continue;
        }
        let fires = deadman::peek_journal_full(&db)
            .is_some_and(|(dm, mark)| dm.fires_before_unlock(now, mark.unwrap_or(0)));
        if fires {
            erase_journal_files(&db, &backups);
            erased.push(journal.id);
        }
    }
    forget_journals(app_data_dir, &erased);
    if vault_open {
        return;
    }
    if let Some(dir) = crate::config::load_last_vault_dir(app_data_dir) {
        match mini_diarium_core::vault::Vault::erase_if_expired(&dir, now) {
            Ok(true) => {
                info!("Erased the vault by its dead man's switch");
                forget_vault(app_data_dir);
            }
            Ok(false) => {}
            Err(e) => warn!("Could not erase an expired vault: {}", e),
        }
    }
}

/// Removes erased journals from config.json, so the list doesn't show they existed. The
/// active journal moves to the first that remains. Returns it (`None` when none remains).
pub(crate) fn forget_journals(app_data_dir: &Path, ids: &[String]) -> Option<String> {
    if ids.is_empty() {
        return crate::config::load_active_journal_id(app_data_dir);
    }
    let mut journals = crate::config::load_journals(app_data_dir);
    journals.retain(|j| !ids.contains(&j.id));
    let active = crate::config::load_active_journal_id(app_data_dir)
        .filter(|a| !ids.contains(a))
        .or_else(|| journals.first().map(|j| j.id.clone()));
    if let Err(e) =
        crate::config::save_journals(app_data_dir, &journals, active.as_deref().unwrap_or(""))
    {
        warn!("Could not remove erased journals from the list: {}", e);
    }
    active
}

/// Forgets an erased vault's folder, so the vault dialog doesn't offer it.
pub(crate) fn forget_vault(app_data_dir: &Path) {
    if let Err(e) = crate::config::save_last_vault_dir(app_data_dir, None) {
        warn!("Could not forget the erased vault's folder: {}", e);
    }
}

/// How often the switch is checked while the app runs.
const TICK: std::time::Duration = std::time::Duration::from_secs(60 * 60);

/// Runs the switch while the app stays open: the journal and vault that are unlocked right
/// now count as in use, so their deadline is renewed; everything else is swept as at
/// startup. Without this, an app left running would never erase anything.
pub(crate) fn tick(state: &DiaryState) {
    let open_db = match state.db.try_lock() {
        Ok(guard) => guard.as_ref().map(|db| {
            renew_journal(state, db);
        }),
        // Busy or poisoned: it is certainly open, so leave it alone.
        Err(_) => Some(()),
    }
    .and_then(|()| state.db_path.lock().ok().map(|p| p.clone()));
    let vault_open = crate::commands::vault::renew_open_vault(state);
    sweep_except(&state.app_data_dir, open_db.as_deref(), vault_open);
}

fn renew_journal(state: &DiaryState, db: &crate::db::DatabaseConnection) {
    let Some(dm) = deadman::journal_dead_man(db) else {
        return;
    };
    match deadman::set_journal_rules(db, dm.rules()) {
        Ok(renewed) => sync_config_copy(state, renewed),
        Err(e) => warn!(
            "Could not renew the open journal's dead man's switch: {}",
            e
        ),
    }
}

/// Starts the hourly [`tick`].
pub(crate) fn start_ticker(app: tauri::AppHandle) {
    use tauri::Manager;
    std::thread::spawn(move || loop {
        std::thread::sleep(TICK);
        tick(&app.state::<DiaryState>());
    });
}

/// The open journal's rules (0 = rule off).
#[tauri::command]
pub fn journal_dead_man(state: State<DiaryState>) -> Result<deadman::Rules, String> {
    let guard = state
        .db
        .lock()
        .map_err(|_| "Journal state lock failed".to_string())?;
    let db = guard.as_ref().ok_or("Journal is not unlocked")?;
    Ok(deadman::journal_dead_man(db)
        .map(|dm| dm.rules())
        .unwrap_or_default())
}

/// Sets the open journal's rules: not unlocked for `days`, clock set back by more than
/// `back_secs` (0 turns a rule off; both 0 disarm the switch).
#[tauri::command]
pub fn set_journal_dead_man(
    days: u32,
    back_secs: u64,
    state: State<DiaryState>,
) -> Result<(), String> {
    let rules = deadman::validate_rules(deadman::Rules { days, back_secs })?.unwrap_or_default();
    let guard = state
        .db
        .lock()
        .map_err(|_| "Journal state lock failed".to_string())?;
    let db = guard.as_ref().ok_or("Journal is not unlocked")?;
    let dm = deadman::set_journal_rules(db, rules)?;
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
        back_secs: 0,
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
        // The erased journal is gone from the list too.
        let ids: Vec<String> = crate::config::load_journals(&app)
            .into_iter()
            .map(|j| j.id)
            .collect();
        assert_eq!(ids, vec!["b".to_string()]);
        assert_eq!(
            crate::config::load_active_journal_id(&app).as_deref(),
            Some("b")
        );
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

    #[test]
    fn sweep_erases_a_journal_whose_clock_was_set_back_too_far() {
        let (_t, app, a, b) = dirs();
        let now = crate::clock::unix_now();
        // Clock rule only (1 hour); the file last saw a time 2 hours ahead of now.
        let ahead = format!("2:0:{}:3600:00", now + 7200);
        let near = format!("2:0:{}:3600:00", now + 600);
        let journals = [
            journal("a", &a, Some(&ahead), LONG_AGO),
            journal("b", &b, Some(&near), LONG_AGO),
        ];
        crate::config::save_journals(&app, &journals, "b").unwrap();
        sweep(&app);
        assert!(!a.join("diary.db").exists());
        assert!(b.join("diary.db").exists());
    }

    fn ids(app: &Path) -> Vec<String> {
        crate::config::load_journals(app)
            .into_iter()
            .map(|j| j.id)
            .collect()
    }

    #[test]
    fn hourly_tick_spares_the_open_journal_and_sweeps_the_rest() {
        use crate::commands::auth::test_helpers::make_state;
        let (_fixture, state, db_path, _) = make_state("deadman_tick");
        let open_dir = db_path.parent().unwrap().to_path_buf();
        let other = open_dir.join("other");
        std::fs::create_dir_all(&other).unwrap();
        std::fs::create_dir_all(&state.app_data_dir).unwrap();
        // Both records say "expired"; the open one is in use and must survive.
        let journals = [
            journal("open", &open_dir, Some("1:0:00"), LONG_AGO),
            journal("other", &other, Some("1:0:00"), LONG_AGO),
        ];
        crate::config::save_journals(&state.app_data_dir, &journals, "open").unwrap();
        *state.db.lock().unwrap() =
            Some(crate::db::open_database(&db_path, "pw".to_string(), &open_dir).unwrap());

        tick(&state);

        assert!(db_path.exists());
        assert!(!other.join("diary.db").exists());
        assert_eq!(ids(&state.app_data_dir), vec!["open".to_string()]);
    }

    #[test]
    fn hourly_tick_renews_the_open_journals_deadline() {
        use crate::commands::auth::test_helpers::make_state;
        let (_fixture, state, db_path, _) = make_state("deadman_renew");
        let dir = db_path.parent().unwrap().to_path_buf();
        std::fs::create_dir_all(&state.app_data_dir).unwrap();
        let journals = [journal("open", &dir, None, None)];
        crate::config::save_journals(&state.app_data_dir, &journals, "open").unwrap();
        let db = crate::db::open_database(&db_path, "pw".to_string(), &dir).unwrap();
        deadman::set_journal_rules(
            &db,
            deadman::Rules {
                days: 3,
                back_secs: 0,
            },
        )
        .unwrap();
        // Pretend it was unlocked long ago: a valid record, renewed by the tick.
        *state.db.lock().unwrap() = Some(db);

        tick(&state);

        let renewed = deadman::peek_journal(&db_path).unwrap();
        assert_eq!(renewed.days, 3);
        assert!(renewed.last_unlock >= crate::clock::unix_now() - 5);
        let copy = crate::config::load_journals(&state.app_data_dir)[0].dead_man;
        assert_eq!(copy, Some(renewed));
    }

    #[test]
    fn an_unlock_that_erases_removes_the_journal_from_the_list() {
        use crate::commands::auth::test_helpers::make_state;
        use crate::commands::auth::{perform_unlock, UnlockMode};
        let (_fixture, state, db_path, _) = make_state("deadman_unlock_erase");
        let dir = db_path.parent().unwrap().to_path_buf();
        let other = dir.join("other");
        std::fs::create_dir_all(&other).unwrap();
        std::fs::create_dir_all(&state.app_data_dir).unwrap();
        // A record whose MAC fails counts as expired at unlock.
        let journals = [
            journal("doomed", &dir, Some("1:0:00"), None),
            journal("other", &other, None, None),
        ];
        crate::config::save_journals(&state.app_data_dir, &journals, "doomed").unwrap();

        let result = perform_unlock(UnlockMode::Password("pw".to_string()), &state);

        assert_eq!(result.err(), Some(deadman::ERR_ERASED.to_string()));
        assert!(!db_path.exists());
        assert_eq!(ids(&state.app_data_dir), vec!["other".to_string()]);
        assert_eq!(*state.db_path.lock().unwrap(), other.join("diary.db"));
    }
}
