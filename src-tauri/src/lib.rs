mod ai;
mod capture;
mod settings;
pub mod stream_manager;
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
    sm_state: tauri::State<'_, Arc<stream_manager::StreamManager>>,
    app_handle: tauri::AppHandle,
) -> Result<bool, String> {
    let now_capturing = state.toggle();
    if now_capturing {
        log::info!("Screen capture toggled ON");
        let sm = Arc::clone(&*sm_state);
        state.start_loop(app_handle, Some(sm)).await;
    } else {
        log::info!("Screen capture toggled OFF");
    }
    Ok(now_capturing)
}

/// Configure the AI provider with Azure OpenAI credentials.
#[tauri::command]
async fn configure_ai(
    state: tauri::State<'_, Arc<stream_manager::StreamManager>>,
    endpoint: String,
    api_key: String,
    deployment: String,
    system_prompt: String,
) -> Result<(), String> {
    state.configure_azure(&endpoint, &api_key, &deployment, &system_prompt);
    Ok(())
}

/// Check whether an AI provider has been configured.
#[tauri::command]
async fn is_ai_configured(
    state: tauri::State<'_, Arc<stream_manager::StreamManager>>,
) -> Result<bool, String> {
    Ok(state.is_configured())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let screen_capture = Arc::new(capture::screen::ScreenCapture::new(2000, 1024, 75));
    let stream_mgr = Arc::new(stream_manager::StreamManager::new());

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(screen_capture)
        .manage(stream_mgr)
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(tray::on_shortcut_event)
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            greet,
            toggle_capture,
            configure_ai,
            is_ai_configured,
            settings::save_settings,
            settings::load_settings
        ])
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
