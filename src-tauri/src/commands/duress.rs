//! Panic (duress) password: erasing the journal, opening the decoy, and the settings
//! commands (see `core::duress`).

use crate::commands::auth::DiaryState;
use crate::config::{self, DuressConfig};
use log::{info, warn};
use mini_diarium_core::vault::Vault;
use serde::Serialize;
use std::path::Path;
use tauri::State;

/// Called when a password unlock reported the duress password: erases the journal (and the
/// vault, if chosen), removes it from the journal list, and opens the decoy journal with the
/// same password. Without a usable decoy it answers like a wrong password.
pub(crate) fn handle(state: &DiaryState, password: String) -> Result<(), String> {
    let dir = &state.app_data_dir;
    let active = config::load_active_journal_id(dir);
    let mut journals = config::load_journals(dir);
    let settings = active
        .as_ref()
        .and_then(|id| journals.iter().find(|j| &j.id == id))
        .and_then(|j| j.duress.clone());

    let db_path = state
        .db_path
        .lock()
        .map_err(|_| "State lock poisoned")?
        .clone();
    let backups_dir = state
        .backups_dir
        .lock()
        .map_err(|_| "State lock poisoned")?
        .clone();
    crate::commands::deadman::erase_journal_files(&db_path, &backups_dir);
    if settings.as_ref().is_some_and(|s| s.erase_vault) {
        if let Some(vault_dir) = config::load_last_vault_dir(dir) {
            if let Err(e) = Vault::erase(&vault_dir) {
                warn!("Could not erase the vault: {}", e);
            }
            let _ = config::save_last_vault_dir(dir, None);
        }
    }
    if let Some(id) = &active {
        journals.retain(|j| &j.id != id);
    }
    info!("Panic password used");

    let decoy = settings
        .map(|s| s.decoy_id)
        .filter(|d| journals.iter().any(|j| &j.id == d));
    let next_active = decoy
        .clone()
        .or_else(|| journals.first().map(|j| j.id.clone()))
        .unwrap_or_default();
    config::save_journals(dir, &journals, &next_active)?;
    let Some(decoy) = decoy else {
        return Err("Incorrect password".to_string());
    };
    crate::commands::auth::switch_journal_inner(decoy, state)?;
    crate::commands::auth::perform_unlock(
        crate::commands::auth::UnlockMode::Password(password),
        state,
    )
}

/// What the settings screen shows about the active journal's panic password.
#[derive(Debug, Serialize)]
pub struct DuressStatus {
    pub enabled: bool,
    pub decoy_id: Option<String>,
    pub erase_vault: bool,
}

#[tauri::command]
pub fn duress_status(state: State<DiaryState>) -> Result<DuressStatus, String> {
    let guard = state.db.lock().map_err(|_| "Journal state lock failed")?;
    let db = guard.as_ref().ok_or("Journal is not unlocked")?;
    let enabled = mini_diarium_core::duress::is_set(db);
    let settings = active_settings(&state.app_data_dir);
    Ok(DuressStatus {
        enabled,
        decoy_id: settings.as_ref().map(|s| s.decoy_id.clone()),
        erase_vault: settings.is_some_and(|s| s.erase_vault),
    })
}

fn active_settings(dir: &Path) -> Option<DuressConfig> {
    let active = config::load_active_journal_id(dir)?;
    config::load_journals(dir)
        .into_iter()
        .find(|j| j.id == active)?
        .duress
}

/// Sets the panic password of the open journal. The decoy must be another journal that
/// opens with the same password.
#[tauri::command]
pub fn set_duress(
    password: String,
    decoy_id: String,
    erase_vault: bool,
    state: State<DiaryState>,
) -> Result<(), String> {
    set_duress_inner(&password, &decoy_id, erase_vault, &state)
}

pub(crate) fn set_duress_inner(
    password: &str,
    decoy_id: &str,
    erase_vault: bool,
    state: &DiaryState,
) -> Result<(), String> {
    let dir = &state.app_data_dir;
    let active = config::load_active_journal_id(dir).ok_or("No active journal")?;
    if active == decoy_id {
        return Err("Choose a different journal as the decoy".to_string());
    }
    let mut journals = config::load_journals(dir);
    let decoy = journals
        .iter()
        .find(|j| j.id == decoy_id)
        .ok_or("Decoy journal not found")?;
    let (decoy_db, decoy_backups) =
        config::journal_db_paths(Path::new(&decoy.path), decoy.db_filename.as_deref())?;
    if mini_diarium_core::duress::matches_file(&decoy_db, password) {
        return Err(
            "The decoy journal must not use this password as its own panic password".to_string(),
        );
    }
    if crate::db::open_database(&decoy_db, password.to_string(), &decoy_backups).is_err() {
        return Err("The decoy journal must open with the panic password".to_string());
    }

    let guard = state.db.lock().map_err(|_| "Journal state lock failed")?;
    let db = guard.as_ref().ok_or("Journal is not unlocked")?;
    mini_diarium_core::duress::set(db, password)?;
    drop(guard);

    if let Some(j) = journals.iter_mut().find(|j| j.id == active) {
        j.duress = Some(DuressConfig {
            decoy_id: decoy_id.to_string(),
            erase_vault,
        });
    }
    config::save_journals(dir, &journals, &active)
}

/// Removes the open journal's panic password.
#[tauri::command]
pub fn clear_duress(state: State<DiaryState>) -> Result<(), String> {
    let guard = state.db.lock().map_err(|_| "Journal state lock failed")?;
    let db = guard.as_ref().ok_or("Journal is not unlocked")?;
    mini_diarium_core::duress::clear(db)?;
    drop(guard);
    let dir = &state.app_data_dir;
    let Some(active) = config::load_active_journal_id(dir) else {
        return Ok(());
    };
    let mut journals = config::load_journals(dir);
    if let Some(j) = journals.iter_mut().find(|j| j.id == active) {
        j.duress = None;
    }
    config::save_journals(dir, &journals, &active)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::auth::test_helpers::make_state;
    use crate::commands::auth::{lock_diary_inner, perform_unlock, UnlockMode};
    use crate::config::JournalConfig;

    fn entry(id: &str, dir: &Path) -> JournalConfig {
        JournalConfig {
            id: id.to_string(),
            name: id.to_string(),
            path: dir.to_string_lossy().to_string(),
            auto_key: None,
            db_filename: None,
            require_all_auth: None,
            dead_man: None,
            duress: None,
        }
    }

    #[test]
    fn panic_password_erases_the_journal_and_opens_the_decoy() {
        let (_fixture, state, db_path, _) = make_state("duress");
        let real_dir = db_path.parent().unwrap().to_path_buf();
        let decoy_dir = real_dir.join("decoy");
        std::fs::create_dir_all(&decoy_dir).unwrap();
        std::fs::create_dir_all(&state.app_data_dir).unwrap();
        drop(crate::db::create_database(&db_path, "real".to_string()).unwrap());
        drop(crate::db::create_database(decoy_dir.join("diary.db"), "panic".to_string()).unwrap());
        config::save_journals(
            &state.app_data_dir,
            &[entry("real", &real_dir), entry("decoy", &decoy_dir)],
            "real",
        )
        .unwrap();

        perform_unlock(UnlockMode::Password("real".to_string()), &state).unwrap();
        assert!(set_duress_inner("other", "decoy", false, &state).is_err());
        set_duress_inner("panic", "decoy", false, &state).unwrap();
        lock_diary_inner(&state).unwrap();

        perform_unlock(UnlockMode::Password("panic".to_string()), &state).unwrap();

        assert!(!db_path.exists());
        assert_eq!(
            config::load_active_journal_id(&state.app_data_dir).as_deref(),
            Some("decoy")
        );
        let ids: Vec<String> = config::load_journals(&state.app_data_dir)
            .into_iter()
            .map(|j| j.id)
            .collect();
        assert_eq!(ids, vec!["decoy".to_string()]);
        assert_eq!(*state.db_path.lock().unwrap(), decoy_dir.join("diary.db"));
        assert!(state.db.lock().unwrap().is_some());
    }

    #[test]
    fn panic_password_also_works_on_the_all_methods_screen() {
        let (_fixture, state, db_path, _) = make_state("duress_all");
        let real_dir = db_path.parent().unwrap().to_path_buf();
        let decoy_dir = real_dir.join("decoy");
        std::fs::create_dir_all(&decoy_dir).unwrap();
        std::fs::create_dir_all(&state.app_data_dir).unwrap();
        drop(crate::db::create_database(&db_path, "real".to_string()).unwrap());
        drop(crate::db::create_database(decoy_dir.join("diary.db"), "panic".to_string()).unwrap());
        config::save_journals(
            &state.app_data_dir,
            &[entry("real", &real_dir), entry("decoy", &decoy_dir)],
            "real",
        )
        .unwrap();
        perform_unlock(UnlockMode::Password("real".to_string()), &state).unwrap();
        set_duress_inner("panic", "decoy", false, &state).unwrap();
        lock_diary_inner(&state).unwrap();

        let creds = vec![crate::commands::auth::MultiAuthCredential::Password {
            value: "panic".to_string(),
        }];
        perform_unlock(UnlockMode::AllMethods(creds), &state).unwrap();

        assert!(!db_path.exists());
        assert_eq!(*state.db_path.lock().unwrap(), decoy_dir.join("diary.db"));
    }
}
