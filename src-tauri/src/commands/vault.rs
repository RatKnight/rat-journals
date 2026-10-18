//! Tauri commands for encrypted file vaults (see `mini_diarium_core::vault` and
//! `docs/VAULT_FORMAT.md`).
//!
//! One vault can be open at a time. It lives in [`DiaryState::vault`] so that every lock
//! path (the Lock button, idle timer, OS screen lock, focus loss, a crashed background
//! task) closes it together with the journal; see `lock_diary_inner_with`. Closing a vault
//! drops it (wiping its master key) and wipes any decrypted copies made for "Open in app".
//!
//! Everything slow (key derivation, encrypting or decrypting files) runs on the blocking
//! pool via [`run_blocking`]. A lock that arrives while such an operation holds the vault
//! sets [`DiaryState::vault_close_requested`], and the operation closes the vault as soon
//! as it finishes instead of blocking the lock.

use super::auth::{check_key_file_path, is_network_path, write_secret_file, DiaryState};
use super::run_blocking;
use crate::auth::KeyFileMethod;
use log::{info, warn};
use mini_diarium_core::vault::{self, Vault, VaultFile, VaultSlotInfo};
use serde::Serialize;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager, State};

/// Largest file the in-app preview loads into memory.
pub const MAX_PREVIEW_BYTES: u64 = 200 * 1024 * 1024;
const OPEN_DIR: &str = "vault-open";

/// An unlocked vault plus the decrypted copies made for "Open in app".
pub struct OpenVault {
    vault: Vault,
    temp_files: Vec<PathBuf>,
}

/// What the frontend shows about the open vault.
#[derive(Debug, Serialize)]
pub struct VaultStatus {
    pub path: String,
    pub name: String,
    pub slots: Vec<VaultSlotInfo>,
}

/// Files and folders of the open vault; files newest first.
#[derive(Debug, Serialize)]
pub struct VaultListing {
    pub files: Vec<VaultFile>,
    pub folders: Vec<String>,
}

/// A locked vault folder, as seen before unlocking.
#[derive(Debug, Serialize)]
pub struct VaultFolderInfo {
    pub is_vault: bool,
    pub slots: Vec<VaultSlotInfo>,
}

/// The vault folder opened last time, ready to unlock.
#[derive(Debug, Serialize)]
pub struct VaultLastFolder {
    pub path: String,
    pub slots: Vec<VaultSlotInfo>,
}

/// The outcome of adding several files: what was added and what failed.
#[derive(Debug, Serialize)]
pub struct VaultAddResult {
    pub added: Vec<VaultFile>,
    pub failed: Vec<VaultAddFailure>,
}

#[derive(Debug, Serialize)]
pub struct VaultAddFailure {
    pub name: String,
    pub error: String,
}

const ERR_NO_VAULT: &str = "No vault is open";

// ── Open / close bookkeeping ──────────────────────────────────────────────────────────

pub(crate) fn is_vault_open(state: &DiaryState) -> bool {
    match state.vault.try_lock() {
        Ok(guard) => guard.is_some(),
        // Busy means an operation is running on an open vault.
        Err(std::sync::TryLockError::WouldBlock) => true,
        Err(std::sync::TryLockError::Poisoned(p)) => p.into_inner().is_some(),
    }
}

/// Closes the vault, or asks the running operation to close it when it finishes.
/// Never blocks: it is called from the OS screen-lock handler.
pub(crate) fn close_vault(state: &DiaryState) {
    match state.vault.try_lock() {
        Ok(mut guard) => {
            if let Some(open) = guard.take() {
                shutdown(open);
            }
            state.vault_close_requested.store(false, Ordering::SeqCst);
        }
        Err(std::sync::TryLockError::WouldBlock) => {
            state.vault_close_requested.store(true, Ordering::SeqCst);
        }
        Err(std::sync::TryLockError::Poisoned(p)) => {
            if let Some(open) = p.into_inner().take() {
                shutdown(open);
            }
        }
    }
}

fn shutdown(open: OpenVault) {
    for path in &open.temp_files {
        wipe_file(path);
    }
    drop(open.vault);
    info!("Vault closed");
}

pub(crate) fn emit_vault_locked(app: &AppHandle, reason: &str) {
    if let Err(error) = app.emit("vault-locked", reason) {
        warn!("Failed to emit vault-locked event: {}", error);
    }
}

/// Overwrites a decrypted temporary file with zeros, then deletes it and its folder.
/// Best effort: an app that still holds the file open on Windows can block deletion; the
/// leftover is removed by [`wipe_temp_dir`] at the next start.
fn wipe_file(path: &Path) {
    if let Ok(meta) = std::fs::metadata(path) {
        if let Ok(mut f) = std::fs::OpenOptions::new().write(true).open(path) {
            let zeros = vec![0u8; 64 * 1024];
            let mut left = meta.len();
            while left > 0 {
                let n = left.min(zeros.len() as u64) as usize;
                if f.write_all(&zeros[..n]).is_err() {
                    break;
                }
                left -= n as u64;
            }
            let _ = f.sync_all();
        }
    }
    let _ = std::fs::remove_file(path);
    if let Some(dir) = path.parent() {
        let _ = std::fs::remove_dir(dir);
    }
}

/// Removes decrypted copies left behind by a previous run (crash, or a file that was
/// still open in another app). Called once at startup.
pub(crate) fn wipe_temp_dir(app_data_dir: &Path) {
    let dir = app_data_dir.join(OPEN_DIR);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    for sub in entries.flatten() {
        for file in std::fs::read_dir(sub.path())
            .into_iter()
            .flatten()
            .flatten()
        {
            wipe_file(&file.path());
        }
        let _ = std::fs::remove_dir(sub.path());
    }
}

/// Runs `f` on the open vault. If a lock arrived meanwhile, closes the vault afterwards.
fn with_vault<T>(
    state: &DiaryState,
    f: impl FnOnce(&mut OpenVault) -> Result<T, String>,
) -> Result<T, String> {
    let mut guard = state
        .vault
        .lock()
        .map_err(|_| "Vault state lock failed".to_string())?;
    let open = guard.as_mut().ok_or(ERR_NO_VAULT)?;
    let result = f(open);
    if state.vault_close_requested.swap(false, Ordering::SeqCst) {
        if let Some(open) = guard.take() {
            shutdown(open);
        }
        return Err(ERR_NO_VAULT.to_string());
    }
    result
}

fn install(state: &DiaryState, vault: Vault) -> Result<VaultStatus, String> {
    // Removes leftovers from an interrupted add; never fatal.
    match vault.remove_orphans() {
        Ok(0) => {}
        Ok(n) => info!("Removed {} unreferenced vault object(s)", n),
        Err(e) => warn!("Could not clean the vault folder: {}", e),
    }
    let status = status_of(&vault);
    let mut guard = state
        .vault
        .lock()
        .map_err(|_| "Vault state lock failed".to_string())?;
    if let Some(previous) = guard.take() {
        shutdown(previous);
    }
    *guard = Some(OpenVault {
        vault,
        temp_files: Vec::new(),
    });
    state.vault_close_requested.store(false, Ordering::SeqCst);
    info!("Vault opened");
    // Offered first the next time the vault dialog opens. Never fatal.
    if let Err(e) =
        crate::config::save_last_vault_dir(&state.app_data_dir, Some(Path::new(&status.path)))
    {
        warn!("Could not remember the vault folder: {}", e);
    }
    Ok(status)
}

fn status_of(vault: &Vault) -> VaultStatus {
    VaultStatus {
        path: vault.dir().to_string_lossy().to_string(),
        name: vault
            .dir()
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default(),
        slots: vault.slots(),
    }
}

/// Requires the current password when the vault has a password slot; otherwise being
/// unlocked is enough (the same model as journals).
fn check_identity(vault: &Vault, current_password: Option<&str>) -> Result<(), String> {
    if !vault.slots().iter().any(|s| s.slot_type == "password") {
        return Ok(());
    }
    let password = current_password.ok_or("Enter the vault's current password")?;
    Vault::open_with_password(vault.dir(), password)
        .map(|_| ())
        .map_err(|_| "Incorrect password".to_string())
}

fn read_key_file(path: &str) -> Result<KeyFileMethod, String> {
    super::auth::read_key_file(path)
}

// ── Inner functions (testable without Tauri) ──────────────────────────────────────────

pub(crate) fn check_folder_inner(path: &str) -> Result<VaultFolderInfo, String> {
    let dir = Path::new(path);
    if !vault::is_vault(dir) {
        return Ok(VaultFolderInfo {
            is_vault: false,
            slots: Vec::new(),
        });
    }
    Ok(VaultFolderInfo {
        is_vault: true,
        slots: vault::peek_slots(dir)?,
    })
}

/// The remembered vault folder, if it still holds a readable vault (for example a
/// Google Drive folder that is mounted). Otherwise `None`, and the dialog asks for a folder.
pub(crate) fn last_folder_inner(app_data_dir: &Path) -> Option<VaultLastFolder> {
    let dir = crate::config::load_last_vault_dir(app_data_dir)?;
    if !vault::is_vault(&dir) {
        return None;
    }
    let slots = vault::peek_slots(&dir).ok()?;
    Some(VaultLastFolder {
        path: dir.to_string_lossy().to_string(),
        slots,
    })
}

pub(crate) fn create_inner(
    state: &DiaryState,
    path: &str,
    password: Option<String>,
    key_file_path: Option<String>,
) -> Result<VaultStatus, String> {
    let key_file = match key_file_path.as_deref() {
        Some(p) => Some((
            check_key_file_path(p)?.to_path_buf(),
            KeyFileMethod::generate(),
        )),
        None => None,
    };
    let dir = Path::new(path);
    let vault = Vault::create(dir, password.as_deref(), key_file.as_ref().map(|(_, k)| k))?;
    if let Some((target, kf)) = &key_file {
        if let Err(e) = write_secret_file(target, &kf.file_contents()) {
            // Without its key file a key-file-only vault could never be opened.
            drop(vault);
            let _ = std::fs::remove_dir_all(dir);
            return Err(e);
        }
    }
    install(state, vault)
}

pub(crate) fn unlock_password_inner(
    state: &DiaryState,
    path: &str,
    password: &str,
) -> Result<VaultStatus, String> {
    let vault = Vault::open_with_password(Path::new(path), password)?;
    install(state, vault)
}

pub(crate) fn unlock_key_file_inner(
    state: &DiaryState,
    path: &str,
    key_path: &str,
) -> Result<VaultStatus, String> {
    let key_file = read_key_file(key_path)?;
    let vault = Vault::open_with_key_file(Path::new(path), &key_file)?;
    install(state, vault)
}

pub(crate) fn status_inner(state: &DiaryState) -> Result<Option<VaultStatus>, String> {
    let guard = state
        .vault
        .lock()
        .map_err(|_| "Vault state lock failed".to_string())?;
    Ok(guard.as_ref().map(|o| status_of(&o.vault)))
}

pub(crate) fn list_inner(state: &DiaryState) -> Result<VaultListing, String> {
    with_vault(state, |o| {
        Ok(VaultListing {
            files: o.vault.files(),
            folders: o.vault.folders(),
        })
    })
}

pub(crate) fn add_files_inner(
    state: &DiaryState,
    paths: &[String],
    folder: Option<&str>,
) -> Result<VaultAddResult, String> {
    with_vault(state, |o| {
        let mut result = VaultAddResult {
            added: Vec::new(),
            failed: Vec::new(),
        };
        for path in paths {
            let p = Path::new(path);
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path.clone());
            if !p.is_file() {
                result.failed.push(VaultAddFailure {
                    name,
                    error: "Only files can be added (not folders)".to_string(),
                });
                continue;
            }
            match o.vault.add_path(p, folder) {
                Ok(file) => result.added.push(file),
                Err(error) => result.failed.push(VaultAddFailure { name, error }),
            }
        }
        Ok(result)
    })
}

/// Largest file accepted by drag and drop (its bytes cross the IPC bridge in memory);
/// bigger files go through "Add files", which streams from disk.
pub const MAX_DROP_BYTES: usize = 200 * 1024 * 1024;

pub(crate) fn add_bytes_inner(
    state: &DiaryState,
    name: &str,
    folder: Option<&str>,
    bytes: &[u8],
) -> Result<VaultFile, String> {
    if bytes.len() > MAX_DROP_BYTES {
        return Err("That file is too large to drop. Use Add files instead".to_string());
    }
    with_vault(state, |o| o.vault.add_reader(name, folder, bytes))
}

/// Decodes a base64 (UTF-8) request header; names travel in headers so the body can be
/// the raw file bytes.
fn header_text(request: &tauri::ipc::Request<'_>, key: &str) -> Result<Option<String>, String> {
    use base64::Engine;
    let Some(value) = request.headers().get(key) else {
        return Ok(None);
    };
    let raw = value.to_str().map_err(|_| "Invalid request".to_string())?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(raw)
        .map_err(|_| "Invalid request".to_string())?;
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| "Invalid request".to_string())
}

pub(crate) fn preview_inner(state: &DiaryState, id: &str) -> Result<Vec<u8>, String> {
    with_vault(state, |o| {
        let mut bytes = o.vault.read_bytes(id, MAX_PREVIEW_BYTES)?;
        // Move the buffer out; the emptied Zeroizing wrapper drops harmlessly.
        Ok(std::mem::take(&mut *bytes))
    })
}

pub(crate) fn save_copy_inner(state: &DiaryState, id: &str, dest: &str) -> Result<(), String> {
    if is_network_path(dest) {
        return Err("A decrypted copy cannot be saved to a network location".to_string());
    }
    with_vault(state, |o| o.vault.save_copy(id, Path::new(dest)))
}

/// Decrypts a file into the app's private temp folder and returns its path. The copy is
/// wiped when the vault closes.
pub(crate) fn decrypt_for_open_inner(state: &DiaryState, id: &str) -> Result<PathBuf, String> {
    with_vault(state, |o| {
        let file = o
            .vault
            .files()
            .into_iter()
            .find(|f| f.id == id)
            .ok_or("That file is not in this vault")?;
        let dir = state
            .app_data_dir
            .join(OPEN_DIR)
            .join(format!("{:016x}", rand_u64()));
        std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to open the file: {}", e))?;
        let target = dir.join(safe_file_name(&file.name));
        o.vault.save_copy(id, &target)?;
        o.temp_files.push(target.clone());
        Ok(target)
    })
}

fn rand_u64() -> u64 {
    use rand_core::{OsRng, RngCore};
    OsRng.next_u64()
}

/// A file name that is valid on every OS (the vault name may contain any character).
fn safe_file_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let cleaned = cleaned.trim().trim_end_matches('.').to_string();
    if cleaned.is_empty() {
        "file".to_string()
    } else {
        cleaned
    }
}

/// Reads the key files at `paths` and matches one to every key-file slot that stays
/// (every slot except `exclude`). Returns `(label, method)` pairs for [`Vault::rekey`],
/// or an error naming the first slot whose key file is missing.
fn match_key_files(
    vault: &Vault,
    exclude: Option<u32>,
    paths: &[String],
) -> Result<Vec<(String, KeyFileMethod)>, String> {
    let mut methods: Vec<Option<KeyFileMethod>> = paths
        .iter()
        .map(|p| read_key_file(p).map(Some))
        .collect::<Result<_, _>>()?;
    let mut out = Vec::new();
    for slot in vault
        .slots()
        .into_iter()
        .filter(|s| s.slot_type == "keyfile" && Some(s.id) != exclude)
    {
        let slot_id = slot.slot_id.as_deref().unwrap_or_default();
        let found = methods.iter_mut().find(|m| {
            m.as_ref()
                .is_some_and(|kf| hex::encode(kf.slot_id()) == slot_id)
        });
        match found {
            Some(m) => out.push((slot.label.clone(), m.take().expect("checked above"))),
            None => {
                return Err(format!(
                    "Select the key file “{}” so it can be re-registered",
                    slot.label
                ))
            }
        }
    }
    Ok(out)
}

/// Changes the password. When a password already existed this **rotates the master key**
/// (re-encrypting the whole vault), so every remaining key file must be presented via
/// `key_file_paths`; the old password stops working even against old header copies.
/// Adding a first password to a key-file-only vault revokes nothing and does not rotate.
pub(crate) fn change_password_inner(
    state: &DiaryState,
    current: Option<&str>,
    new: &str,
    key_file_paths: &[String],
) -> Result<(), String> {
    with_vault(state, |o| {
        check_identity(&o.vault, current)?;
        let had_password = o.vault.slots().iter().any(|s| s.slot_type == "password");
        if !had_password {
            return o.vault.set_password(new);
        }
        let key_files = match_key_files(&o.vault, None, key_file_paths)?;
        o.vault.rekey(Some(new), &key_files)
    })
}

pub(crate) fn add_key_file_inner(
    state: &DiaryState,
    path: &str,
    label: &str,
    current: Option<&str>,
) -> Result<(), String> {
    let target = check_key_file_path(path)?.to_path_buf();
    with_vault(state, |o| {
        check_identity(&o.vault, current)?;
        let kf = KeyFileMethod::generate();
        write_secret_file(&target, &kf.file_contents())?;
        if let Err(e) = o.vault.add_key_file(label, &kf) {
            let _ = std::fs::remove_file(&target);
            return Err(e);
        }
        Ok(())
    })
}

/// Removes an unlock method by **rotating the master key**: the whole vault is
/// re-encrypted and only the remaining methods are re-registered, so the removed
/// credential stops working even against old header copies. Every remaining key file
/// must be presented via `key_file_paths`.
pub(crate) fn remove_slot_inner(
    state: &DiaryState,
    slot_id: u32,
    current: Option<&str>,
    key_file_paths: &[String],
) -> Result<(), String> {
    with_vault(state, |o| {
        check_identity(&o.vault, current)?;
        let slots = o.vault.slots();
        let removed = slots
            .iter()
            .find(|s| s.id == slot_id)
            .ok_or("That unlock method does not exist")?;
        if slots.len() <= 1 {
            return Err("A vault needs at least one way to unlock it".to_string());
        }
        let password_stays =
            removed.slot_type != "password" && slots.iter().any(|s| s.slot_type == "password");
        let new_password = if password_stays {
            Some(current.ok_or("Enter the vault's current password")?)
        } else {
            None
        };
        let key_files = match_key_files(&o.vault, Some(slot_id), key_file_paths)?;
        o.vault.rekey(new_password, &key_files)
    })
}

// ── Commands ──────────────────────────────────────────────────────────────────────────

/// Says whether a folder is a vault, and which unlock methods it has.
#[tauri::command]
pub fn vault_check_folder(path: String) -> Result<VaultFolderInfo, String> {
    check_folder_inner(&path)
}

/// The vault folder opened last time, if it is still there.
#[tauri::command]
pub fn vault_last_folder(state: State<DiaryState>) -> Option<VaultLastFolder> {
    last_folder_inner(&state.app_data_dir)
}

/// Creates a vault in an empty folder and opens it. `key_file_path`, when given, is where
/// a new key file is written (generated here; it never reaches the frontend).
#[tauri::command]
pub async fn vault_create(
    path: String,
    password: Option<String>,
    key_file_path: Option<String>,
    app: AppHandle,
) -> Result<VaultStatus, String> {
    run_blocking(app, move |app| {
        create_inner(&app.state::<DiaryState>(), &path, password, key_file_path)
    })
    .await
}

#[tauri::command]
pub async fn vault_unlock_password(
    path: String,
    password: String,
    app: AppHandle,
) -> Result<VaultStatus, String> {
    run_blocking(app, move |app| {
        unlock_password_inner(&app.state::<DiaryState>(), &path, &password)
    })
    .await
}

#[tauri::command]
pub async fn vault_unlock_key_file(
    path: String,
    key_path: String,
    app: AppHandle,
) -> Result<VaultStatus, String> {
    run_blocking(app, move |app| {
        unlock_key_file_inner(&app.state::<DiaryState>(), &path, &key_path)
    })
    .await
}

/// Closes the vault (the journal stays as it is).
#[tauri::command]
pub fn vault_lock(state: State<DiaryState>, app: AppHandle) -> Result<(), String> {
    close_vault(&state);
    emit_vault_locked(&app, "manual");
    Ok(())
}

#[tauri::command]
pub fn vault_status(state: State<DiaryState>) -> Result<Option<VaultStatus>, String> {
    status_inner(&state)
}

#[tauri::command]
pub fn vault_list(state: State<DiaryState>) -> Result<VaultListing, String> {
    list_inner(&state)
}

/// Encrypts files into the vault (optionally into `folder`), reporting each failure.
#[tauri::command]
pub async fn vault_add_files(
    paths: Vec<String>,
    folder: Option<String>,
    app: AppHandle,
) -> Result<VaultAddResult, String> {
    run_blocking(app, move |app| {
        add_files_inner(&app.state::<DiaryState>(), &paths, folder.as_deref())
    })
    .await
}

/// Encrypts a dropped file. The body is the raw file bytes; the base64 headers
/// `x-vault-name` and optional `x-vault-folder` carry its name and folder.
#[tauri::command]
pub async fn vault_add_bytes(
    request: tauri::ipc::Request<'_>,
    app: AppHandle,
) -> Result<VaultFile, String> {
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else {
        return Err("Invalid request".to_string());
    };
    let name = header_text(&request, "x-vault-name")?.ok_or("Invalid request")?;
    let folder = header_text(&request, "x-vault-folder")?;
    let bytes = bytes.clone();
    run_blocking(app, move |app| {
        add_bytes_inner(&app.state::<DiaryState>(), &name, folder.as_deref(), &bytes)
    })
    .await
}

/// Decrypted bytes of a file for in-app preview (raw binary response; capped).
#[tauri::command]
pub async fn vault_preview(id: String, app: AppHandle) -> Result<tauri::ipc::Response, String> {
    run_blocking(app, move |app| {
        preview_inner(&app.state::<DiaryState>(), &id)
    })
    .await
    .map(tauri::ipc::Response::new)
}

/// Decrypts a file to `dest` (chosen in a save dialog).
#[tauri::command]
pub async fn vault_save_copy(id: String, dest: String, app: AppHandle) -> Result<(), String> {
    run_blocking(app, move |app| {
        save_copy_inner(&app.state::<DiaryState>(), &id, &dest)
    })
    .await
}

/// Decrypts a file to the app's private temp folder and opens it in the default app.
/// The decrypted copy is wiped when the vault closes.
#[tauri::command]
pub async fn vault_open_in_app(id: String, app: AppHandle) -> Result<(), String> {
    run_blocking(app, move |app| {
        let path = decrypt_for_open_inner(&app.state::<DiaryState>(), &id)?;
        use tauri_plugin_opener::OpenerExt;
        app.opener()
            .open_path(path.to_string_lossy().to_string(), None::<&str>)
            .map_err(|e| format!("Failed to open the file: {}", e))
    })
    .await
}

#[tauri::command]
pub fn vault_rename_file(id: String, name: String, state: State<DiaryState>) -> Result<(), String> {
    with_vault(&state, |o| o.vault.rename_file(&id, &name))
}

#[tauri::command]
pub fn vault_move_file(
    id: String,
    folder: Option<String>,
    state: State<DiaryState>,
) -> Result<(), String> {
    with_vault(&state, |o| o.vault.move_file(&id, folder.as_deref()))
}

#[tauri::command]
pub fn vault_delete_file(id: String, state: State<DiaryState>) -> Result<(), String> {
    with_vault(&state, |o| o.vault.delete_file(&id))
}

#[tauri::command]
pub fn vault_create_folder(name: String, state: State<DiaryState>) -> Result<(), String> {
    with_vault(&state, |o| o.vault.create_folder(&name))
}

#[tauri::command]
pub fn vault_rename_folder(
    old_name: String,
    new_name: String,
    state: State<DiaryState>,
) -> Result<(), String> {
    with_vault(&state, |o| o.vault.rename_folder(&old_name, &new_name))
}

#[tauri::command]
pub fn vault_delete_folder(name: String, state: State<DiaryState>) -> Result<(), String> {
    with_vault(&state, |o| o.vault.delete_folder(&name))
}

/// Changes the vault password. When one already existed the master key is rotated, which
/// needs every remaining key file in `key_file_paths` (see [`change_password_inner`]).
#[tauri::command]
pub async fn vault_change_password(
    current_password: Option<String>,
    new_password: String,
    key_file_paths: Option<Vec<String>>,
    app: AppHandle,
) -> Result<(), String> {
    run_blocking(app, move |app| {
        change_password_inner(
            &app.state::<DiaryState>(),
            current_password.as_deref(),
            &new_password,
            key_file_paths.as_deref().unwrap_or_default(),
        )
    })
    .await
}

/// Creates a new key file at `path` and adds it as a way to unlock the vault.
#[tauri::command]
pub async fn vault_add_key_file(
    path: String,
    label: String,
    current_password: Option<String>,
    app: AppHandle,
) -> Result<(), String> {
    run_blocking(app, move |app| {
        add_key_file_inner(
            &app.state::<DiaryState>(),
            &path,
            &label,
            current_password.as_deref(),
        )
    })
    .await
}

/// Removes an unlock method by rotating the master key, which needs every remaining key
/// file in `key_file_paths` (see [`remove_slot_inner`]).
#[tauri::command]
pub async fn vault_remove_unlock_method(
    slot_id: u32,
    current_password: Option<String>,
    key_file_paths: Option<Vec<String>>,
    app: AppHandle,
) -> Result<(), String> {
    run_blocking(app, move |app| {
        remove_slot_inner(
            &app.state::<DiaryState>(),
            slot_id,
            current_password.as_deref(),
            key_file_paths.as_deref().unwrap_or_default(),
        )
    })
    .await
}

/// Decrypts every file to check it is intact; returns the names of damaged files.
#[tauri::command]
pub async fn vault_verify(app: AppHandle) -> Result<Vec<String>, String> {
    run_blocking(app, move |app| {
        with_vault(&app.state::<DiaryState>(), |o| Ok(o.vault.verify()))
    })
    .await
}

#[cfg(test)]
mod tests;
