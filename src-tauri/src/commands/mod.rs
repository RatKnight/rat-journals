pub mod attachments;
pub mod auth;
pub mod backup;
pub mod backup_inspect;
pub mod backup_triggers;
pub mod deadman;
pub mod debug;
pub mod duress;
pub mod entries;
pub mod export;
pub mod files;
pub mod fonts;
pub mod images;
pub mod import;
pub mod menu;
pub mod navigation;
pub mod paths;
pub mod platform;
pub mod plugin;
pub mod search;
pub mod spellcheck;
pub mod stats;
pub mod tags;
pub mod vault;

use log::warn;
use tauri::{AppHandle, Manager};

/// The `journal-locked` reason when a background task panicked. The frontend matches it to
/// show why the journal locked.
pub(crate) const BACKGROUND_TASK_FAILED: &str = "background-task-failed";

/// Runs a command's heavy work on Tauri's blocking thread pool and waits for it.
///
/// On Windows a synchronous command runs on the WebView2 event thread, so the window cannot
/// repaint or take input until it returns. A command that reads, encrypts, or writes large
/// blobs is `async` and puts that work here instead. The closure must be `'static`, so it
/// reaches managed state through the `AppHandle` (`app.state::<DiaryState>()`), not through
/// a borrowed `State`.
///
/// A panic in `work` fails closed: the journal is locked at once through the normal
/// auto-lock path (reason [`BACKGROUND_TASK_FAILED`]), so the master key does not stay in
/// memory behind a poisoned mutex. The lock path recovers poison itself.
pub(crate) async fn run_blocking<T, F>(app: AppHandle, work: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(&AppHandle) -> Result<T, String> + Send + 'static,
{
    let worker_app = app.clone();
    run_blocking_or_lock(
        move || work(&worker_app),
        move || {
            // Never fails: the lock primitive recovers a poisoned mutex.
            let _ = auth::auto_lock_diary_if_unlocked(
                app.state::<auth::DiaryState>(),
                app.clone(),
                BACKGROUND_TASK_FAILED,
            );
        },
    )
    .await
}

/// The testable core of [`run_blocking`]: runs `work` on the blocking pool and calls `lock`
/// when it panicked. A cancelled task is not a panic and does not lock.
///
/// Never catch and swallow a panic here without locking: a poisoned `db` mutex blocks every
/// command, and the key would stay in memory until the next lock trigger.
async fn run_blocking_or_lock<T, W, L>(work: W, lock: L) -> Result<T, String>
where
    T: Send + 'static,
    W: FnOnce() -> Result<T, String> + Send + 'static,
    L: FnOnce(),
{
    match tauri::async_runtime::spawn_blocking(work).await {
        Ok(result) => result,
        Err(tauri::Error::JoinError(error)) if error.is_panic() => {
            // No payload: a panic message can carry user data, and this record reaches the
            // debug dump.
            warn!("A background task panicked; locking the journal");
            lock();
            Err("Background task failed".to_string())
        }
        Err(_) => Err("Background task failed".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::auth::test_helpers::make_state;
    use crate::commands::auth::{lock_diary_inner, DiaryState};
    use crate::db::create_database;
    use std::cell::Cell;
    use std::sync::Arc;

    /// Runs a `work` closure that panics while it holds `state.db`, the way a bug in an
    /// attach or restore would, and returns what the command returned and what the lock
    /// call reported.
    fn panic_holding_db(state: &Arc<DiaryState>) -> (Result<(), String>, Option<bool>) {
        let worker = Arc::clone(state);
        let locked = Cell::new(None);
        let result = tauri::async_runtime::block_on(run_blocking_or_lock(
            move || {
                let _guard = worker.db.lock().unwrap();
                panic!("deliberately panicking in a background task for a test");
            },
            || locked.set(Some(lock_diary_inner(state).unwrap())),
        ));
        (result, locked.get())
    }

    #[test]
    fn test_background_panic_locks_a_journal_whose_db_mutex_is_poisoned() {
        let (_fixture, state, db_path, _backups) = make_state("background_panic");
        let state = Arc::new(state);
        *state.db.lock().unwrap() = Some(create_database(&db_path, "test".to_string()).unwrap());

        let (result, locked) = panic_holding_db(&state);

        assert_eq!(result.unwrap_err(), "Background task failed");
        assert_eq!(locked, Some(true), "the journal must lock after the panic");
        let db_state = state.db.lock().expect("the db poison must be cleared");
        assert!(db_state.is_none(), "the connection must be gone");
        drop(db_state);

        // The journal can be unlocked again.
        *state.db.lock().unwrap() =
            Some(create_database(db_path.with_file_name("again.db"), "test".to_string()).unwrap());
        assert!(state.db.lock().unwrap().is_some());
    }

    #[test]
    fn test_background_panic_with_no_journal_open_reports_nothing_locked() {
        let (_fixture, state, _db_path, _backups) = make_state("background_panic_locked");
        let state = Arc::new(state);

        let (result, locked) = panic_holding_db(&state);

        assert_eq!(result.unwrap_err(), "Background task failed");
        assert_eq!(locked, Some(false));
        assert!(state.db.lock().is_ok(), "the db poison must be cleared");
    }

    #[test]
    fn test_background_task_without_a_panic_does_not_lock() {
        let locked = Cell::new(false);

        let result = tauri::async_runtime::block_on(run_blocking_or_lock(
            || Err::<(), _>("ordinary failure".to_string()),
            || locked.set(true),
        ));

        assert_eq!(result.unwrap_err(), "ordinary failure");
        assert!(!locked.get(), "an ordinary error must not lock the journal");
    }
}
