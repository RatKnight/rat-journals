//! Platform-specific WebView security request handlers.
//!
//! Each platform file installs an engine-level request blocker that enforces the
//! no-external-network policy as defense-in-depth alongside CSP and the JS init script.
//! See the Network Isolation Defense-in-Depth Stack in `src-tauri/CLAUDE.md` for the
//! full picture.

#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "macos")]
mod macos;

/// True when the WebView may load or navigate to `url`: the app's own origins only.
///
/// Hosts are compared exactly (a prefix check would let `http://localhost.example.com`
/// through). The Vite dev server (`localhost` / `127.0.0.1`) is allowed only in debug
/// builds; a release build serves its pages from `tauri.localhost` and its IPC from
/// `ipc.localhost`.
pub(crate) fn is_allowed_app_url(url: &str) -> bool {
    let Ok(parsed) = tauri::Url::parse(url) else {
        return false;
    };
    match parsed.scheme() {
        "tauri" | "ipc" => true,
        "http" | "https" => {
            let host = parsed.host_str().unwrap_or("");
            matches!(host, "tauri.localhost" | "ipc.localhost")
                || (cfg!(debug_assertions) && matches!(host, "localhost" | "127.0.0.1"))
        }
        _ => false,
    }
}

/// Install platform-specific WebView engine-level request blockers.
///
/// - Windows: WebView2 `WebResourceRequested` COM event handler — returns 403 for
///   external HTTP(S) requests.
/// - macOS: `WKContentRuleList` compiled rule — blocks HTTP(S) to non-localhost domains
///   at the WebKit engine level.
#[allow(unused_variables)]
pub fn install_platform_handlers(win: &tauri::WebviewWindow) {
    #[cfg(target_os = "windows")]
    windows::install_webresource_requested_handler(win);

    #[cfg(target_os = "macos")]
    macos::install_content_rule_list(win);
}

#[cfg(test)]
mod tests {
    use super::is_allowed_app_url;

    #[test]
    fn app_origins_are_allowed() {
        assert!(is_allowed_app_url("http://tauri.localhost/index.html"));
        assert!(is_allowed_app_url("https://tauri.localhost/assets/a.js"));
        assert!(is_allowed_app_url(
            "http://ipc.localhost/plugin%3Aevent%7Clisten"
        ));
        assert!(is_allowed_app_url("tauri://localhost/index.html"));
        assert!(is_allowed_app_url("ipc://localhost/cmd"));
    }

    #[test]
    fn lookalike_and_external_hosts_are_blocked() {
        for url in [
            "http://localhost.example.com/",
            "http://tauri.localhost.example.com/",
            "http://127.0.0.1.nip.io/",
            "http://ipc.localhost.evil/",
            "https://example.com/",
            "http://user@evil.com/",
            "file:///C:/Windows/win.ini",
            "javascript:alert(1)",
            "data:text/html,hi",
            "not a url",
        ] {
            assert!(!is_allowed_app_url(url), "{url}");
        }
    }

    #[test]
    fn dev_server_only_in_debug_builds() {
        assert_eq!(
            is_allowed_app_url("http://localhost:1420/"),
            cfg!(debug_assertions)
        );
        assert_eq!(
            is_allowed_app_url("http://127.0.0.1:1420/"),
            cfg!(debug_assertions)
        );
    }
}
