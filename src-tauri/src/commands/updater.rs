//! Manual update check (Windows). Nothing here runs unless the user asks: there is no
//! background check. The release's `latest.json` and installer come from this repo's GitHub
//! releases, and the installer's signature is verified against the public key in
//! `tauri.conf.json` before anything runs. Linux updates come through the Flatpak instead.

use crate::commands::auth::DiaryState;
use log::info;
use serde::Serialize;
use tauri::{AppHandle, State, Wry};
use tauri_plugin_updater::UpdaterExt;

pub const ERR_UNSUPPORTED: &str =
    "Updates for this version come from the GitHub releases page, not from inside the app";

/// A newer release than the running one.
#[derive(Debug, Serialize)]
pub struct UpdateInfo {
    pub version: String,
    pub notes: Option<String>,
}

fn check_error(e: impl std::fmt::Display) -> String {
    format!("Could not check for updates: {}", e)
}

async fn find_update(app: &AppHandle<Wry>) -> Result<Option<tauri_plugin_updater::Update>, String> {
    if !cfg!(windows) {
        return Err(ERR_UNSUPPORTED.to_string());
    }
    app.updater()
        .map_err(check_error)?
        .check()
        .await
        .map_err(check_error)
}

/// Asks GitHub whether a newer release exists.
#[tauri::command]
pub async fn check_for_update(app: AppHandle<Wry>) -> Result<Option<UpdateInfo>, String> {
    info!("Checking for updates");
    Ok(find_update(&app).await?.map(|u| UpdateInfo {
        version: u.version,
        notes: u.body,
    }))
}

/// Downloads the newer release, verifies its signature, locks the journal and the vault,
/// then runs the installer and restarts.
#[tauri::command]
pub async fn install_update(
    app: AppHandle<Wry>,
    state: State<'_, DiaryState>,
) -> Result<(), String> {
    let update = find_update(&app).await?.ok_or("No update is available")?;
    let bytes = update
        .download(|_, _| {}, || {})
        .await
        .map_err(|e| format!("Could not download the update: {}", e))?;
    // The installer replaces the running app: close the journal and the vault cleanly first.
    crate::commands::auth::lock_diary_inner(&state)?;
    info!("Installing update {}", update.version);
    update
        .install(bytes)
        .map_err(|e| format!("Could not install the update: {}", e))?;
    app.restart();
}
