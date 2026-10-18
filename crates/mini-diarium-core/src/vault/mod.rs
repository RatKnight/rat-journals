//! Encrypted file vaults: a folder of individually encrypted files that can live in a
//! cloud-sync folder without the provider being able to read names or contents.
//!
//! The on-disk format is specified in `docs/VAULT_FORMAT.md`. A vault is independent of
//! any journal: everything needed to open it is the folder plus a password or key file.

use crate::auth::{KeyFileMethod, PasswordMethod};
use crate::crypto::cipher;
use crate::crypto::stream::{self, ObjectKind, StreamHeader};
use base64::Engine;
use hkdf::Hkdf;
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use zeroize::Zeroizing;

const HEADER_FILE: &str = "rat-vault.json";
const INDEX_FILE: &str = "index.rjv";
const DATA_DIR: &str = "data";
const BLOB_EXT: &str = "rjf";
const FORMAT_NAME: &str = "rat-journal-vault";
const FORMAT_VERSION: u32 = 1;
const OBJECT_KEY_INFO: &[u8] = b"rat-vault:object:v1";
const MAX_NAME_CHARS: usize = 255;

/// Returned when unlocking with credentials that open no slot.
pub const ERR_WRONG_CREDENTIAL: &str = "That password or key file does not open this vault";

/// One stored file, as listed in the vault.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultFile {
    /// 128-bit object id (lowercase hex); also names the encrypted blob.
    pub id: String,
    pub name: String,
    pub folder: Option<String>,
    /// Plaintext size in bytes.
    pub size: u64,
    /// RFC 3339 timestamp of when the file was added.
    pub added_at: String,
    /// SHA-256 of the plaintext (lowercase hex).
    pub sha256: String,
    pub mime: Option<String>,
}

/// An unlock method registered in the vault header (no secrets).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VaultSlotInfo {
    pub id: u32,
    pub slot_type: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HeaderDoc {
    format: String,
    version: u32,
    vault_id: String,
    created_at: String,
    slots: Vec<SlotRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SlotRecord {
    id: u32,
    #[serde(rename = "type")]
    slot_type: String,
    label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    slot_id: Option<String>,
    wrapped_key: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct IndexDoc {
    version: u32,
    folders: Vec<String>,
    files: Vec<VaultFile>,
}

/// An unlocked vault. Dropping it wipes the master key from memory.
pub struct Vault {
    dir: PathBuf,
    vault_id: [u8; 16],
    master: Zeroizing<[u8; 32]>,
    header: HeaderDoc,
    index: IndexDoc,
}

impl std::fmt::Debug for Vault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Vault")
            .field("dir", &self.dir)
            .field("files", &self.index.files.len())
            .finish_non_exhaustive()
    }
}

fn b64() -> base64::engine::GeneralPurpose {
    base64::engine::general_purpose::STANDARD
}

fn random_id() -> [u8; 16] {
    let mut id = [0u8; 16];
    OsRng.fill_bytes(&mut id);
    id
}

fn parse_id(hex_id: &str) -> Result<[u8; 16], String> {
    hex::decode(hex_id)
        .ok()
        .and_then(|b| b.try_into().ok())
        .ok_or_else(|| "Invalid vault object id".to_string())
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// Writes `path` atomically: a temporary file in the same folder, then a rename.
fn write_atomic(
    path: &Path,
    write: impl FnOnce(&mut BufWriter<fs::File>) -> Result<(), String>,
) -> Result<(), String> {
    let dir = path.parent().ok_or("Invalid vault path")?;
    fs::create_dir_all(dir).map_err(|e| format!("Failed to create vault folder: {}", e))?;
    let tmp = dir.join(format!(".tmp-{}", hex::encode(random_id())));
    let result = (|| {
        let file = fs::File::create(&tmp).map_err(|e| format!("Failed to write vault: {}", e))?;
        let mut w = BufWriter::new(file);
        write(&mut w)?;
        let file = w
            .into_inner()
            .map_err(|e| format!("Failed to write vault: {}", e.error()))?;
        file.sync_all()
            .map_err(|e| format!("Failed to write vault: {}", e))?;
        drop(file);
        fs::rename(&tmp, path).map_err(|e| format!("Failed to write vault: {}", e))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

/// Validates a display name for a file or folder.
fn validate_name(name: &str, what: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(format!("A {what} name cannot be empty"));
    }
    if trimmed.chars().count() > MAX_NAME_CHARS {
        return Err(format!(
            "A {what} name can be at most {MAX_NAME_CHARS} characters"
        ));
    }
    if trimmed.chars().any(|c| c.is_control()) {
        return Err(format!("A {what} name cannot contain control characters"));
    }
    Ok(trimmed.to_string())
}

/// Best-effort MIME type from a file name, used to decide what can be previewed.
pub fn guess_mime(name: &str) -> Option<String> {
    let ext = Path::new(name).extension()?.to_str()?.to_ascii_lowercase();
    let mime = match ext.as_str() {
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "txt" => "text/plain",
        "md" => "text/markdown",
        "csv" => "text/csv",
        "json" => "application/json",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "zip" => "application/zip",
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        "mp3" => "audio/mpeg",
        _ => return None,
    };
    Some(mime.to_string())
}

/// Counts and hashes everything read through it.
struct HashingReader<R> {
    inner: R,
    hasher: Sha256,
    len: u64,
}

impl<R: Read> Read for HashingReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.hasher.update(&buf[..n]);
        self.len += n as u64;
        Ok(n)
    }
}

/// Returns true when `dir` holds a vault header.
pub fn is_vault(dir: &Path) -> bool {
    dir.join(HEADER_FILE).is_file()
}

/// Reads the unlock methods of a locked vault (types and labels only).
pub fn peek_slots(dir: &Path) -> Result<Vec<VaultSlotInfo>, String> {
    Ok(read_header(dir)?
        .slots
        .iter()
        .map(|s| VaultSlotInfo {
            id: s.id,
            slot_type: s.slot_type.clone(),
            label: s.label.clone(),
        })
        .collect())
}

fn read_header(dir: &Path) -> Result<HeaderDoc, String> {
    let text = fs::read_to_string(dir.join(HEADER_FILE))
        .map_err(|_| "This folder is not a vault".to_string())?;
    let header: HeaderDoc =
        serde_json::from_str(&text).map_err(|_| "The vault header is damaged".to_string())?;
    if header.format != FORMAT_NAME {
        return Err("This folder is not a vault".to_string());
    }
    if header.version != FORMAT_VERSION {
        return Err("This vault was made by a newer version of Rat Journal".to_string());
    }
    Ok(header)
}

impl Vault {
    /// Creates a new vault in `dir` (which must be empty or not exist) protected by a
    /// password, a key file, or both. At least one is required.
    pub fn create(
        dir: &Path,
        password: Option<&str>,
        key_file: Option<&KeyFileMethod>,
    ) -> Result<Vault, String> {
        if password.is_none() && key_file.is_none() {
            return Err("A vault needs a password or a key file".to_string());
        }
        if matches!(password, Some(p) if p.is_empty()) {
            return Err("Password cannot be empty".to_string());
        }
        if dir.exists() {
            let mut entries =
                fs::read_dir(dir).map_err(|e| format!("Failed to read the vault folder: {}", e))?;
            if entries.next().is_some() {
                return Err("Choose an empty folder for a new vault".to_string());
            }
        }
        fs::create_dir_all(dir.join(DATA_DIR))
            .map_err(|e| format!("Failed to create the vault folder: {}", e))?;

        let mut master = Zeroizing::new([0u8; 32]);
        OsRng.fill_bytes(master.as_mut());
        let vault_id = random_id();
        let mut header = HeaderDoc {
            format: FORMAT_NAME.to_string(),
            version: FORMAT_VERSION,
            vault_id: hex::encode(vault_id),
            created_at: now_rfc3339(),
            slots: Vec::new(),
        };
        if let Some(p) = password {
            header.slots.push(password_slot(1, "Password", p, &master)?);
        }
        if let Some(kf) = key_file {
            let id = header.slots.len() as u32 + 1;
            header
                .slots
                .push(key_file_slot(id, "Key file", kf, &master)?);
        }
        let vault = Vault {
            dir: dir.to_path_buf(),
            vault_id,
            master,
            header,
            index: IndexDoc {
                version: 1,
                ..IndexDoc::default()
            },
        };
        vault.save_index()?;
        vault.save_header()?;
        Ok(vault)
    }

    /// Unlocks a vault with its password.
    pub fn open_with_password(dir: &Path, password: &str) -> Result<Vault, String> {
        let header = read_header(dir)?;
        let method = PasswordMethod::new(password.to_string());
        for slot in header.slots.iter().filter(|s| s.slot_type == "password") {
            let wrapped = b64()
                .decode(&slot.wrapped_key)
                .map_err(|_| "The vault header is damaged".to_string())?;
            if let Ok(master) = method.unwrap_master_key(&wrapped) {
                return Self::finish_open(dir, header, &master);
            }
        }
        Err(ERR_WRONG_CREDENTIAL.to_string())
    }

    /// Unlocks a vault with a key file.
    pub fn open_with_key_file(dir: &Path, key_file: &KeyFileMethod) -> Result<Vault, String> {
        let header = read_header(dir)?;
        let slot_id = hex::encode(key_file.slot_id());
        for slot in header
            .slots
            .iter()
            .filter(|s| s.slot_type == "keyfile" && s.slot_id.as_deref() == Some(&slot_id))
        {
            let wrapped = b64()
                .decode(&slot.wrapped_key)
                .map_err(|_| "The vault header is damaged".to_string())?;
            if let Ok(master) = key_file.unwrap_master_key(&wrapped) {
                return Self::finish_open(dir, header, &master);
            }
        }
        Err(ERR_WRONG_CREDENTIAL.to_string())
    }

    fn finish_open(dir: &Path, header: HeaderDoc, master: &[u8]) -> Result<Vault, String> {
        let master: [u8; 32] = master
            .try_into()
            .map_err(|_| "The vault header is damaged".to_string())?;
        let vault_id = parse_id(&header.vault_id)?;
        let mut vault = Vault {
            dir: dir.to_path_buf(),
            vault_id,
            master: Zeroizing::new(master),
            header,
            index: IndexDoc::default(),
        };
        vault.index = vault.load_index()?;
        Ok(vault)
    }

    /// The vault folder.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Every stored file, newest first.
    pub fn files(&self) -> Vec<VaultFile> {
        let mut files = self.index.files.clone();
        files.sort_by(|a, b| b.added_at.cmp(&a.added_at).then(b.id.cmp(&a.id)));
        files
    }

    /// Folder names, alphabetically.
    pub fn folders(&self) -> Vec<String> {
        let mut folders = self.index.folders.clone();
        folders.sort_by_key(|f| f.to_lowercase());
        folders
    }

    /// The unlock methods registered in this vault.
    pub fn slots(&self) -> Vec<VaultSlotInfo> {
        self.header
            .slots
            .iter()
            .map(|s| VaultSlotInfo {
                id: s.id,
                slot_type: s.slot_type.clone(),
                label: s.label.clone(),
            })
            .collect()
    }

    fn object_key(&self, kind: ObjectKind, id: &[u8; 16]) -> Result<cipher::Key, String> {
        let hk = Hkdf::<Sha256>::new(Some(&self.vault_id), self.master.as_ref());
        let mut info = Vec::with_capacity(OBJECT_KEY_INFO.len() + 17);
        info.extend_from_slice(OBJECT_KEY_INFO);
        info.push(kind as u8);
        info.extend_from_slice(id);
        let mut okm = Zeroizing::new([0u8; 32]);
        hk.expand(&info, okm.as_mut())
            .map_err(|_| "Key derivation failed".to_string())?;
        cipher::Key::from_slice(okm.as_ref()).ok_or_else(|| "Key derivation failed".to_string())
    }

    fn blob_path(&self, id_hex: &str) -> PathBuf {
        self.dir
            .join(DATA_DIR)
            .join(&id_hex[..2])
            .join(format!("{id_hex}.{BLOB_EXT}"))
    }

    fn save_header(&self) -> Result<(), String> {
        let json = serde_json::to_string_pretty(&self.header)
            .map_err(|e| format!("Failed to save the vault header: {}", e))?;
        write_atomic(&self.dir.join(HEADER_FILE), |w| {
            w.write_all(json.as_bytes())
                .map_err(|e| format!("Failed to save the vault header: {}", e))
        })
    }

    fn save_index(&self) -> Result<(), String> {
        let json = Zeroizing::new(
            serde_json::to_vec(&self.index)
                .map_err(|e| format!("Failed to save the vault index: {}", e))?,
        );
        // A fresh id (and so a fresh key) for every write of the index.
        let id = random_id();
        let key = self.object_key(ObjectKind::Index, &id)?;
        let header = StreamHeader::new(ObjectKind::Index, id);
        write_atomic(&self.dir.join(INDEX_FILE), |w| {
            stream::encrypt_stream(&key, &header, json.as_slice(), w)
                .map(|_| ())
                .map_err(|e| format!("Failed to save the vault index: {}", e))
        })
    }

    fn load_index(&self) -> Result<IndexDoc, String> {
        let file = fs::File::open(self.dir.join(INDEX_FILE))
            .map_err(|_| "The vault index is missing".to_string())?;
        let mut src = BufReader::new(file);
        let header = stream::read_header(&mut src).map_err(|e| e.to_string())?;
        if header.kind != ObjectKind::Index {
            return Err("The vault index is damaged".to_string());
        }
        let key = self.object_key(ObjectKind::Index, &header.object_id)?;
        let mut json = Zeroizing::new(Vec::new());
        stream::decrypt_stream(&key, &header, src, &mut *json)
            .map_err(|_| "The vault index is damaged or was modified".to_string())?;
        let index: IndexDoc =
            serde_json::from_slice(&json).map_err(|_| "The vault index is damaged".to_string())?;
        if index.version != 1 {
            return Err("This vault was made by a newer version of Rat Journal".to_string());
        }
        Ok(index)
    }

    fn check_folder(&self, folder: Option<&str>) -> Result<Option<String>, String> {
        match folder {
            None => Ok(None),
            Some(f) => {
                if self.index.folders.iter().any(|x| x == f) {
                    Ok(Some(f.to_string()))
                } else {
                    Err("That folder does not exist in this vault".to_string())
                }
            }
        }
    }

    /// Encrypts everything `src` yields as a new file. The blob is written before the
    /// index, so a crash can only leave an unreferenced blob.
    pub fn add_reader<R: Read>(
        &mut self,
        name: &str,
        folder: Option<&str>,
        src: R,
    ) -> Result<VaultFile, String> {
        let name = validate_name(name, "file")?;
        let folder = self.check_folder(folder)?;
        let id = random_id();
        let id_hex = hex::encode(id);
        let key = self.object_key(ObjectKind::File, &id)?;
        let header = StreamHeader::new(ObjectKind::File, id);
        let mut reader = HashingReader {
            inner: src,
            hasher: Sha256::new(),
            len: 0,
        };
        let path = self.blob_path(&id_hex);
        write_atomic(&path, |w| {
            stream::encrypt_stream(&key, &header, &mut reader, w)
                .map(|_| ())
                .map_err(|e| format!("Failed to store the file: {}", e))
        })?;
        let entry = VaultFile {
            id: id_hex,
            mime: guess_mime(&name),
            name,
            folder,
            size: reader.len,
            added_at: now_rfc3339(),
            sha256: hex::encode(reader.hasher.finalize()),
        };
        self.index.files.push(entry.clone());
        if let Err(e) = self.save_index() {
            self.index.files.pop();
            let _ = fs::remove_file(&path);
            return Err(e);
        }
        Ok(entry)
    }

    /// Adds a file from disk, keeping its name.
    pub fn add_path(&mut self, src: &Path, folder: Option<&str>) -> Result<VaultFile, String> {
        let name = src
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or("That file name cannot be read")?
            .to_string();
        let file = fs::File::open(src).map_err(|e| format!("Failed to read the file: {}", e))?;
        self.add_reader(&name, folder, BufReader::new(file))
    }

    fn entry(&self, id: &str) -> Result<&VaultFile, String> {
        self.index
            .files
            .iter()
            .find(|f| f.id == id)
            .ok_or_else(|| "That file is not in this vault".to_string())
    }

    /// Decrypts a stored file into `dst`. On error, discard what was written.
    pub fn read_to<W: Write>(&self, id: &str, dst: W) -> Result<u64, String> {
        let entry = self.entry(id)?;
        let id_bytes = parse_id(&entry.id)?;
        let file = fs::File::open(self.blob_path(&entry.id))
            .map_err(|_| format!("The data for “{}” is missing", entry.name))?;
        let mut src = BufReader::new(file);
        let header = stream::read_header(&mut src).map_err(|e| e.to_string())?;
        if header.kind != ObjectKind::File || header.object_id != id_bytes {
            return Err(format!("The data for “{}” is damaged", entry.name));
        }
        let key = self.object_key(ObjectKind::File, &id_bytes)?;
        let n = stream::decrypt_stream(&key, &header, src, dst)
            .map_err(|_| format!("The data for “{}” is damaged or was modified", entry.name))?;
        if n != entry.size {
            return Err(format!("The data for “{}” is damaged", entry.name));
        }
        Ok(n)
    }

    /// Decrypts a stored file into memory, refusing files larger than `max_bytes`.
    pub fn read_bytes(&self, id: &str, max_bytes: u64) -> Result<Zeroizing<Vec<u8>>, String> {
        let entry = self.entry(id)?;
        if entry.size > max_bytes {
            return Err("That file is too large to preview".to_string());
        }
        let mut out = Zeroizing::new(Vec::with_capacity(entry.size as usize));
        self.read_to(id, &mut *out)?;
        Ok(out)
    }

    /// Decrypts a stored file to `dest`. A partially written file is removed on error.
    pub fn save_copy(&self, id: &str, dest: &Path) -> Result<(), String> {
        let file = fs::File::create(dest).map_err(|e| format!("Failed to save the file: {}", e))?;
        let mut w = BufWriter::new(file);
        let result = self.read_to(id, &mut w).and_then(|_| {
            w.flush()
                .map_err(|e| format!("Failed to save the file: {}", e))
        });
        drop(w);
        if result.is_err() {
            let _ = fs::remove_file(dest);
        }
        result
    }

    /// Renames a stored file.
    pub fn rename_file(&mut self, id: &str, new_name: &str) -> Result<(), String> {
        let name = validate_name(new_name, "file")?;
        self.update_entry(id, |f| {
            f.mime = guess_mime(&name);
            f.name = name;
        })
    }

    /// Moves a stored file into a folder, or out of all folders with `None`.
    pub fn move_file(&mut self, id: &str, folder: Option<&str>) -> Result<(), String> {
        let folder = self.check_folder(folder)?;
        self.update_entry(id, |f| f.folder = folder)
    }

    fn update_entry(
        &mut self,
        id: &str,
        change: impl FnOnce(&mut VaultFile),
    ) -> Result<(), String> {
        let before = self.index.clone();
        let entry = self
            .index
            .files
            .iter_mut()
            .find(|f| f.id == id)
            .ok_or_else(|| "That file is not in this vault".to_string())?;
        change(entry);
        if let Err(e) = self.save_index() {
            self.index = before;
            return Err(e);
        }
        Ok(())
    }

    /// Deletes a stored file. The index is rewritten first, then the blob is removed.
    pub fn delete_file(&mut self, id: &str) -> Result<(), String> {
        let before = self.index.clone();
        let pos = self
            .index
            .files
            .iter()
            .position(|f| f.id == id)
            .ok_or_else(|| "That file is not in this vault".to_string())?;
        let entry = self.index.files.remove(pos);
        if let Err(e) = self.save_index() {
            self.index = before;
            return Err(e);
        }
        let _ = fs::remove_file(self.blob_path(&entry.id));
        Ok(())
    }

    /// Creates a folder.
    pub fn create_folder(&mut self, name: &str) -> Result<(), String> {
        let name = validate_name(name, "folder")?;
        if self
            .index
            .folders
            .iter()
            .any(|f| f.eq_ignore_ascii_case(&name))
        {
            return Err("A folder with that name already exists".to_string());
        }
        self.index.folders.push(name);
        if let Err(e) = self.save_index() {
            self.index.folders.pop();
            return Err(e);
        }
        Ok(())
    }

    /// Renames a folder and every file in it.
    pub fn rename_folder(&mut self, old: &str, new: &str) -> Result<(), String> {
        let new = validate_name(new, "folder")?;
        if !self.index.folders.iter().any(|f| f == old) {
            return Err("That folder does not exist in this vault".to_string());
        }
        if self
            .index
            .folders
            .iter()
            .any(|f| f != old && f.eq_ignore_ascii_case(&new))
        {
            return Err("A folder with that name already exists".to_string());
        }
        let before = self.index.clone();
        for f in self.index.folders.iter_mut().filter(|f| *f == old) {
            *f = new.clone();
        }
        for file in self
            .index
            .files
            .iter_mut()
            .filter(|f| f.folder.as_deref() == Some(old))
        {
            file.folder = Some(new.clone());
        }
        if let Err(e) = self.save_index() {
            self.index = before;
            return Err(e);
        }
        Ok(())
    }

    /// Deletes an empty folder.
    pub fn delete_folder(&mut self, name: &str) -> Result<(), String> {
        if self
            .index
            .files
            .iter()
            .any(|f| f.folder.as_deref() == Some(name))
        {
            return Err("Move or delete the files in this folder first".to_string());
        }
        let before = self.index.clone();
        let len = self.index.folders.len();
        self.index.folders.retain(|f| f != name);
        if self.index.folders.len() == len {
            return Err("That folder does not exist in this vault".to_string());
        }
        if let Err(e) = self.save_index() {
            self.index = before;
            return Err(e);
        }
        Ok(())
    }

    /// Replaces the password (or adds one if the vault has none).
    pub fn set_password(&mut self, password: &str) -> Result<(), String> {
        if password.is_empty() {
            return Err("Password cannot be empty".to_string());
        }
        let before = self.header.clone();
        self.header.slots.retain(|s| s.slot_type != "password");
        let id = self.next_slot_id();
        let slot = password_slot(id, "Password", password, &self.master)?;
        self.header.slots.push(slot);
        if let Err(e) = self.save_header() {
            self.header = before;
            return Err(e);
        }
        Ok(())
    }

    /// Adds a key file as an unlock method.
    pub fn add_key_file(&mut self, label: &str, key_file: &KeyFileMethod) -> Result<(), String> {
        let label = validate_name(label, "key file")?;
        let slot_id = hex::encode(key_file.slot_id());
        if self
            .header
            .slots
            .iter()
            .any(|s| s.slot_id.as_deref() == Some(&slot_id))
        {
            return Err("That key file already opens this vault".to_string());
        }
        let before = self.header.clone();
        let id = self.next_slot_id();
        let slot = key_file_slot(id, &label, key_file, &self.master)?;
        self.header.slots.push(slot);
        if let Err(e) = self.save_header() {
            self.header = before;
            return Err(e);
        }
        Ok(())
    }

    /// Removes an unlock method; the last one cannot be removed.
    pub fn remove_slot(&mut self, slot_id: u32) -> Result<(), String> {
        if self.header.slots.len() <= 1 {
            return Err("A vault needs at least one way to unlock it".to_string());
        }
        let before = self.header.clone();
        let len = self.header.slots.len();
        self.header.slots.retain(|s| s.id != slot_id);
        if self.header.slots.len() == len {
            return Err("That unlock method does not exist".to_string());
        }
        if let Err(e) = self.save_header() {
            self.header = before;
            return Err(e);
        }
        Ok(())
    }

    fn next_slot_id(&self) -> u32 {
        self.header.slots.iter().map(|s| s.id).max().unwrap_or(0) + 1
    }

    /// Checks every stored file decrypts intact. Returns the names of damaged files.
    pub fn verify(&self) -> Vec<String> {
        self.index
            .files
            .iter()
            .filter(|f| self.read_to(&f.id, io::sink()).is_err())
            .map(|f| f.name.clone())
            .collect()
    }

    /// Removes blobs and temporary files that no index entry refers to (left by an
    /// interrupted write). Returns how many were removed.
    pub fn remove_orphans(&self) -> Result<usize, String> {
        let known: std::collections::HashSet<String> = self
            .index
            .files
            .iter()
            .map(|f| format!("{}.{BLOB_EXT}", f.id))
            .collect();
        let mut removed = 0;
        let data = self.dir.join(DATA_DIR);
        let Ok(shards) = fs::read_dir(&data) else {
            return Ok(0);
        };
        for shard in shards.flatten() {
            let Ok(entries) = fs::read_dir(shard.path()) else {
                continue;
            };
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if !known.contains(&name) && fs::remove_file(entry.path()).is_ok() {
                    removed += 1;
                }
            }
        }
        for entry in fs::read_dir(&self.dir).into_iter().flatten().flatten() {
            if entry.file_name().to_string_lossy().starts_with(".tmp-")
                && fs::remove_file(entry.path()).is_ok()
            {
                removed += 1;
            }
        }
        Ok(removed)
    }
}

fn password_slot(
    id: u32,
    label: &str,
    password: &str,
    master: &[u8; 32],
) -> Result<SlotRecord, String> {
    let wrapped = PasswordMethod::new(password.to_string()).wrap_master_key(master)?;
    Ok(SlotRecord {
        id,
        slot_type: "password".to_string(),
        label: label.to_string(),
        slot_id: None,
        wrapped_key: b64().encode(wrapped),
    })
}

fn key_file_slot(
    id: u32,
    label: &str,
    key_file: &KeyFileMethod,
    master: &[u8; 32],
) -> Result<SlotRecord, String> {
    let wrapped = key_file.wrap_master_key(master)?;
    Ok(SlotRecord {
        id,
        slot_type: "keyfile".to_string(),
        label: label.to_string(),
        slot_id: Some(hex::encode(key_file.slot_id())),
        wrapped_key: b64().encode(wrapped),
    })
}

#[cfg(test)]
mod tests;
