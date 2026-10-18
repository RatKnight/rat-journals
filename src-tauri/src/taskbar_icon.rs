//! Sets the window's large (taskbar / Alt+Tab) icon on Windows.
//!
//! Tauri only sets the small title-bar icon (`WM_SETICON` / `ICON_SMALL`). Without a large
//! icon, the taskbar falls back to the shell's cached icon for the exe path, which keeps
//! showing an older build's icon after an upgrade installs to the same path. Loading the
//! icon embedded in the exe and setting it as `ICON_BIG` makes the taskbar always match.

#[cfg(target_os = "windows")]
pub fn apply(window: &tauri::WebviewWindow) {
    use log::warn;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, LoadImageW, SendMessageW, ICON_BIG, IMAGE_ICON, LR_DEFAULTCOLOR,
        SM_CXICON, SM_CYICON, WM_SETICON,
    };

    let hwnd = match window.hwnd() {
        Ok(h) => h.0 as HWND,
        Err(e) => {
            warn!("Taskbar icon: no window handle: {}", e);
            return;
        }
    };
    // The resource id tauri-build gives the app icon embedded in the exe.
    let resource_id = tauri::utils::platform::WINDOWS_APP_ICON_RESOURCE_ID as usize;

    // SAFETY: `GetModuleHandleW(null)` returns this exe's module handle (never freed).
    // `LoadImageW` with a MAKEINTRESOURCE id (an integer cast to a pointer) is the
    // documented way to load a resource by id; it returns null on failure, which is
    // checked. Icons loaded without `LR_SHARED` are owned by us; the handle is handed to
    // the window and intentionally kept for the window's lifetime (one icon, no leak loop).
    // `hwnd` comes from Tauri's `WebviewWindow::hwnd()` and is valid here.
    unsafe {
        let module = GetModuleHandleW(std::ptr::null());
        let icon = LoadImageW(
            module,
            resource_id as *const u16,
            IMAGE_ICON,
            GetSystemMetrics(SM_CXICON),
            GetSystemMetrics(SM_CYICON),
            LR_DEFAULTCOLOR,
        );
        if icon.is_null() {
            warn!(
                "Taskbar icon: could not load the app icon: {}",
                std::io::Error::last_os_error()
            );
            return;
        }
        SendMessageW(hwnd, WM_SETICON, ICON_BIG as WPARAM, icon as LPARAM);
    }
}

#[cfg(not(target_os = "windows"))]
pub fn apply(_window: &tauri::WebviewWindow) {}
