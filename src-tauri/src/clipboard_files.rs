//! Reads the list of files the user copied in the system file manager (Copy in Windows
//! Explorer), so they can be pasted into the vault. Only paths are read; the files are then
//! encrypted through the normal "Add files" path, which checks each one.

/// Paths of the files on the clipboard (`CF_HDROP`), or an empty list when it holds none.
#[cfg(target_os = "windows")]
pub fn copied_files(_app: &tauri::AppHandle) -> Result<Vec<String>, String> {
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
    };
    use windows_sys::Win32::UI::Shell::{DragQueryFileW, HDROP};

    const CF_HDROP: u32 = 15;

    // SAFETY: the clipboard is opened with no owner window, read, and always closed before
    // returning. The CF_HDROP handle stays owned by the clipboard and is only read (never
    // freed) while the clipboard is open. DragQueryFileW is called with 0xFFFFFFFF to get the
    // count, with a null buffer to get each name's length, and then with a buffer of exactly
    // that length plus the terminator.
    unsafe {
        if IsClipboardFormatAvailable(CF_HDROP) == 0 {
            return Ok(Vec::new());
        }
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return Err("The clipboard is busy. Try again.".to_string());
        }
        let result = (|| {
            let handle = GetClipboardData(CF_HDROP) as HDROP;
            if handle.is_null() {
                return Ok(Vec::new());
            }
            let count = DragQueryFileW(handle, u32::MAX, std::ptr::null_mut(), 0);
            let mut paths = Vec::with_capacity(count as usize);
            for i in 0..count {
                let len = DragQueryFileW(handle, i, std::ptr::null_mut(), 0);
                let mut buf = vec![0u16; len as usize + 1];
                let copied = DragQueryFileW(handle, i, buf.as_mut_ptr(), buf.len() as u32);
                buf.truncate(copied as usize);
                paths.push(
                    String::from_utf16(&buf)
                        .map_err(|_| "A copied file name could not be read".to_string())?,
                );
            }
            Ok(paths)
        })();
        CloseClipboard();
        result
    }
}

/// Paths of the files copied in the file manager (`text/uri-list`). GTK clipboard calls must
/// run on the main thread, so the read is handed to it and awaited here (this runs on the
/// blocking pool, never on the main thread).
#[cfg(target_os = "linux")]
pub fn copied_files(app: &tauri::AppHandle) -> Result<Vec<String>, String> {
    let (tx, rx) = std::sync::mpsc::channel();
    app.run_on_main_thread(move || {
        let clipboard = gtk::Clipboard::get(&gtk::gdk::SELECTION_CLIPBOARD);
        let paths: Vec<String> = clipboard
            .wait_for_uris()
            .iter()
            // Only local files: `file://` URIs with no remote host.
            .filter_map(|uri| gtk::glib::filename_from_uri(uri).ok())
            .filter(|(_, host)| {
                host.as_deref()
                    .map_or(true, |h| h.is_empty() || h == "localhost")
            })
            .filter_map(|(path, _)| path.to_str().map(str::to_string))
            .collect();
        let _ = tx.send(paths);
    })
    .map_err(|e| format!("Could not read the clipboard: {}", e))?;
    rx.recv_timeout(std::time::Duration::from_secs(10))
        .map_err(|_| "Could not read the clipboard. Try again.".to_string())
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
pub fn copied_files(_app: &tauri::AppHandle) -> Result<Vec<String>, String> {
    Err(
        "Pasting copied files is not available on this system. Use Add files or drag and drop."
            .to_string(),
    )
}
