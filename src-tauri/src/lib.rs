mod ai;
mod capture;
mod tray;

#[allow(unused_imports)]
use tauri::Manager;

use std::sync::Arc;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

/// Start or stop screen capture. Returns the new capturing state.
///
/// `tauri::State` gives us a shared reference to whatever we stored with
/// `.manage()` during setup.  Because `ScreenCapture` is behind an `Arc`,
/// cloning is cheap and keeps the borrow checker happy.
#[tauri::command]
async fn toggle_capture(
    state: tauri::State<'_, Arc<capture::screen::ScreenCapture>>,
    app_handle: tauri::AppHandle,
) -> Result<bool, String> {
    let now_capturing = state.toggle();
    if now_capturing {
        log::info!("Screen capture toggled ON");
        state.start_loop(app_handle).await;
    } else {
        log::info!("Screen capture toggled OFF");
    }
    Ok(now_capturing)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Create the shared ScreenCapture instance with sensible defaults:
    //   interval = 2 000 ms, max width = 1 024 px, JPEG quality = 75
    let screen_capture = Arc::new(capture::screen::ScreenCapture::new(2000, 1024, 75));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(screen_capture)
        // Register Ctrl+Shift+B as a global shortcut for toggling capture.
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(tray::on_shortcut_event)
                .build(),
        )
        .invoke_handler(tauri::generate_handler![greet, toggle_capture])
        .setup(|app| {
            // Create the system tray icon and menu.
            tray::setup_tray(app)?;

            // Register the Ctrl+Shift+B shortcut with the plugin.
            use tauri_plugin_global_shortcut::GlobalShortcutExt;
            app.global_shortcut().register("ctrl+shift+b")?;
            log::info!("Global shortcut Ctrl+Shift+B registered");

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
