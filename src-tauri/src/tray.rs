// tray.rs — System tray icon + right-click menu for beme.
// The global shortcut (Ctrl+Shift+B) is registered in lib.rs via the plugin builder.

use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager,
};

/// Shared flag so the tray menu label can reflect capture state.
static CAPTURING: AtomicBool = AtomicBool::new(false);

/// Returns `true` if we are currently capturing.
#[allow(dead_code)]
pub fn is_capturing() -> bool {
    CAPTURING.load(Ordering::SeqCst)
}

/// Toggle the capture state and return the new value.
pub fn toggle_capture() -> bool {
    let prev = CAPTURING.fetch_xor(true, Ordering::SeqCst);
    !prev // new state after XOR
}

// ─── Tray setup ──────────────────────────────────────────────────────

/// Call this from `App::setup` to create the system-tray icon and menu.
pub fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let handle = app.handle();

    // Build the right-click menu items.
    let toggle_item = MenuItem::with_id(handle, "toggle_capture", "Start Capture", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(handle)?;
    let dashboard_item = MenuItem::with_id(handle, "open_dashboard", "Open Dashboard", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(handle, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(handle, &[&toggle_item, &separator, &dashboard_item, &quit_item])?;

    // Build the tray icon (uses the default app icon).
    TrayIconBuilder::new()
        .menu(&menu)
        .show_menu_on_left_click(false) // right-click opens menu
        .on_menu_event(move |app_handle, event| {
            match event.id().as_ref() {
                "toggle_capture" => {
                    let now_capturing = toggle_capture();
                    log::info!("Tray: capture toggled → {}", if now_capturing { "ON" } else { "OFF" });

                    // Update the menu item label to reflect new state.
                    if let Some(item) = menu.get("toggle_capture") {
                        if let Some(mi) = item.as_menuitem() {
                            let label = if now_capturing { "Stop Capture" } else { "Start Capture" };
                            let _ = mi.set_text(label);
                        }
                    }

                    // Emit toggle event so frontend & capture modules can react.
                    let _ = app_handle.emit("toggle:capture", serde_json::json!({ "source": "tray" }));
                }
                "open_dashboard" => {
                    log::info!("Tray: opening dashboard window");
                    // Show the dashboard window if it already exists, or ignore gracefully.
                    if let Some(win) = app_handle.get_webview_window("dashboard") {
                        let _ = win.show();
                        let _ = win.set_focus();
                    }
                }
                "quit" => {
                    log::info!("Tray: quit requested");
                    app_handle.exit(0);
                }
                _ => {}
            }
        })
        .build(app)?;

    log::info!("System tray created");
    Ok(())
}

// ─── Global shortcut handler ─────────────────────────────────────────

/// Called by the global-shortcut plugin when *any* registered shortcut fires.
/// We check for key-down and emit `toggle:capture`.
pub fn on_shortcut_event(
    app: &AppHandle,
    _shortcut: &tauri_plugin_global_shortcut::Shortcut,
    event: tauri_plugin_global_shortcut::ShortcutEvent,
) {
    // Only act on key-down (Pressed), not Released.
    if event.state() != tauri_plugin_global_shortcut::ShortcutState::Pressed {
        return;
    }

    let now_capturing = toggle_capture();
    log::info!(
        "Shortcut Ctrl+Shift+B: capture toggled → {}",
        if now_capturing { "ON" } else { "OFF" }
    );

    let _ = app.emit("toggle:capture", serde_json::json!({ "source": "shortcut" }));
}
