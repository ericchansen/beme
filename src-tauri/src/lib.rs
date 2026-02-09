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
    let screen_capture = Arc::new(capture::screen::ScreenCapture::new(2000, 1024, 75));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(screen_capture)
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(tray::on_shortcut_event)
                .build(),
        )
        .invoke_handler(tauri::generate_handler![greet, toggle_capture])
        .setup(|app| {
            tray::setup_tray(app)?;

            use tauri_plugin_global_shortcut::GlobalShortcutExt;
            app.global_shortcut().register("ctrl+shift+b")?;
            log::info!("Global shortcut Ctrl+Shift+B registered");

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
