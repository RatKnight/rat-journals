//! Validation for file paths that arrive from the frontend.
//!
//! Paths normally come from a native open/save dialog, but the backend cannot tell a path
//! the user picked from one that script in the WebView made up. So every command that
//! reads or writes a frontend-supplied path checks it here first. The checks are
//! defense in depth (the WebView already has a strict CSP and no network access): they
//! stop a compromised page from using the app to write runnable files into autostart
//! folders, reach network shares, or touch device namespaces.

use std::path::{Component, Path, PathBuf};

/// How a command uses a path.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Access<'a> {
    /// Reads an existing file.
    Read,
    /// Writes a file whose extension must be one of these (lowercase, without the dot).
    WriteAs(&'a [&'a str]),
    /// Writes a file the user names freely (a decrypted copy of an attachment or a vault
    /// file). Any extension except runnable ones.
    WriteUserNamed,
}

/// Extensions Windows (or a desktop shell) runs or interprets when opened.
const RUNNABLE_EXTENSIONS: &[&str] = &[
    "exe",
    "com",
    "bat",
    "cmd",
    "scr",
    "pif",
    "msi",
    "msp",
    "msc",
    "cpl",
    "dll",
    "ps1",
    "psm1",
    "psd1",
    "vbs",
    "vbe",
    "js",
    "jse",
    "wsf",
    "wsh",
    "hta",
    "lnk",
    "url",
    "reg",
    "inf",
    "scf",
    "appref-ms",
    "application",
    "jar",
    "desktop",
    "sh",
];

/// True for a UNC / network or device path (`\\server\share`, `//server/share`,
/// `\\?\…`, `\\.\…`).
pub(crate) fn is_network_path(path: &str) -> bool {
    path.starts_with("\\\\") || path.starts_with("//")
}

fn extension_of(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default()
}

/// True when `path` is inside a folder whose contents run at sign-in.
fn is_autostart_location(path: &Path) -> bool {
    let p = path.to_string_lossy().replace('\\', "/").to_lowercase();
    p.contains("/start menu/programs/startup/") || p.contains("/.config/autostart/")
}

/// Checks a frontend-supplied path for `access` and returns it as a `PathBuf`.
pub(crate) fn check_path(path: &str, access: Access<'_>) -> Result<PathBuf, String> {
    if path.trim().is_empty() {
        return Err("No file was chosen".to_string());
    }
    if is_network_path(path) {
        return Err("Network and device locations are not supported".to_string());
    }
    let p = Path::new(path);
    if !p.is_absolute() {
        return Err("The file location is not a full path".to_string());
    }
    if p.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err("The file location is not a plain path".to_string());
    }
    match access {
        Access::Read => {}
        Access::WriteAs(allowed) => {
            let ext = extension_of(p);
            if !allowed.contains(&ext.as_str()) {
                return Err(format!(
                    "This file must be saved as .{}",
                    allowed.join(" or .")
                ));
            }
        }
        Access::WriteUserNamed => {
            if RUNNABLE_EXTENSIONS.contains(&extension_of(p).as_str()) {
                return Err(
                    "For safety, a decrypted copy cannot be saved with a program or script extension"
                        .to_string(),
                );
            }
        }
    }
    if !matches!(access, Access::Read) && is_autostart_location(p) {
        return Err("Files cannot be saved to a startup folder".to_string());
    }
    Ok(p.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    const BASE: &str = "C:\\Users\\me\\Documents";
    #[cfg(not(windows))]
    const BASE: &str = "/home/me/Documents";

    fn at(name: &str) -> String {
        Path::new(BASE).join(name).to_string_lossy().to_string()
    }

    #[test]
    fn plain_local_paths_pass() {
        assert!(check_path(&at("a.pdf"), Access::Read).is_ok());
        assert!(check_path(&at("export.json"), Access::WriteAs(&["json"])).is_ok());
        assert!(check_path(&at("Export.JSON"), Access::WriteAs(&["json"])).is_ok());
        assert!(check_path(&at("scan.pdf"), Access::WriteUserNamed).is_ok());
        assert!(check_path(&at("no-extension"), Access::WriteUserNamed).is_ok());
    }

    #[test]
    fn network_device_relative_and_traversal_paths_fail() {
        for p in [
            "\\\\server\\share\\a.pdf",
            "//server/share/a.pdf",
            "\\\\?\\C:\\a.pdf",
            "\\\\.\\PhysicalDrive0",
            "relative/a.pdf",
            "",
            "   ",
        ] {
            assert!(check_path(p, Access::Read).is_err(), "{p:?}");
        }
        assert!(check_path(&at("../../a.pdf"), Access::Read).is_err());
    }

    #[test]
    fn writes_need_the_right_extension() {
        assert!(check_path(&at("a.bat"), Access::WriteAs(&["pdf"])).is_err());
        assert!(check_path(&at("a"), Access::WriteAs(&["pdf"])).is_err());
        assert!(check_path(&at("a.pdf.exe"), Access::WriteAs(&["pdf"])).is_err());
        for bad in [
            "evil.exe", "x.BAT", "y.ps1", "z.lnk", "q.hta", "r.vbs", "s.cmd",
        ] {
            assert!(
                check_path(&at(bad), Access::WriteUserNamed).is_err(),
                "{bad}"
            );
        }
    }

    #[test]
    fn startup_folders_are_refused_for_writes_only() {
        #[cfg(windows)]
        let startup = "C:\\Users\\me\\AppData\\Roaming\\Microsoft\\Windows\\Start Menu\\Programs\\Startup\\notes.txt";
        #[cfg(not(windows))]
        let startup = "/home/me/.config/autostart/notes.txt";
        assert!(check_path(startup, Access::WriteUserNamed).is_err());
        assert!(check_path(startup, Access::WriteAs(&["txt"])).is_err());
        assert!(check_path(startup, Access::Read).is_ok());
    }
}
