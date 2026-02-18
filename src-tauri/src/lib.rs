pub mod ai;
mod capture;
mod settings;
pub mod stream_manager;
mod tray;

#[allow(unused_imports)]
use tauri::Manager;

use base64::Engine as _;
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

/// List available monitors.
#[tauri::command]
fn list_monitors() -> Result<Vec<capture::screen::MonitorInfo>, String> {
    capture::screen::list_monitors()
}

/// Select which monitor to capture.
#[tauri::command]
fn select_monitor(
    state: tauri::State<'_, Arc<capture::screen::ScreenCapture>>,
    monitor_id: Option<u32>,
) -> Result<(), String> {
    state.set_monitor(monitor_id);
    log::info!("Monitor selection changed to {:?}", monitor_id);
    Ok(())
}

/// Configure the AI provider with Azure OpenAI credentials.
#[tauri::command]
async fn configure_ai(
    state: tauri::State<'_, Arc<stream_manager::StreamManager>>,
    endpoint: String,
    api_key: String,
    deployment: String,
    system_prompt: String,
    use_bearer: Option<bool>,
) -> Result<(), String> {
    state.configure_azure(
        &endpoint,
        &api_key,
        &deployment,
        &system_prompt,
        use_bearer.unwrap_or(false),
    );
    Ok(())
}

/// Check whether an AI provider has been configured.
#[tauri::command]
async fn is_ai_configured(
    state: tauri::State<'_, Arc<stream_manager::StreamManager>>,
) -> Result<bool, String> {
    Ok(state.is_configured())
}

/// List available audio output devices.
#[tauri::command]
fn list_audio_devices() -> Result<Vec<capture::audio::AudioDeviceInfo>, String> {
    capture::audio::list_audio_devices()
}

/// Select which audio device to capture from.
#[tauri::command]
fn select_audio_device(
    state: tauri::State<'_, Arc<capture::audio::AudioCapture>>,
    device_name: Option<String>,
) -> Result<(), String> {
    state.set_device(device_name);
    Ok(())
}

/// Start or stop audio capture. Returns the new capturing state.
#[tauri::command]
async fn toggle_audio_capture(
    state: tauri::State<'_, Arc<capture::audio::AudioCapture>>,
    sm_state: tauri::State<'_, Arc<stream_manager::StreamManager>>,
    app_handle: tauri::AppHandle,
) -> Result<bool, String> {
    let now_capturing = state.toggle();
    if now_capturing {
        log::info!("Audio capture toggled ON");
        let sm = Arc::clone(&*sm_state);
        state.start_loop(app_handle, Some(sm));
    } else {
        log::info!("Audio capture toggled OFF");
    }
    Ok(now_capturing)
}

/// Start audio AI session — opens WebSocket to Azure Realtime API.
#[tauri::command]
async fn start_audio_ai(
    sm_state: tauri::State<'_, Arc<stream_manager::StreamManager>>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    sm_state.start_audio_session(app_handle).await
}

/// Stop audio AI session — closes the WebSocket.
#[tauri::command]
async fn stop_audio_ai(
    sm_state: tauri::State<'_, Arc<stream_manager::StreamManager>>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    sm_state.stop_audio_session(&app_handle).await
}

/// Send an audio chunk to the AI session.
#[tauri::command]
async fn send_audio_chunk(
    sm_state: tauri::State<'_, Arc<stream_manager::StreamManager>>,
    data: String,
) -> Result<(), String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&data)
        .map_err(|e| format!("Invalid base64: {}", e))?;
    sm_state.process_audio_chunk(&bytes).await
}

/// Get the current vision and audio prompts.
#[tauri::command]
async fn get_prompts(
    state: tauri::State<'_, Arc<stream_manager::StreamManager>>,
) -> Result<std::collections::HashMap<String, String>, String> {
    let (vision, audio) = state.get_prompts();
    let mut map = std::collections::HashMap::new();
    map.insert("vision".to_string(), vision);
    map.insert("audio".to_string(), audio);
    Ok(map)
}

/// Update a prompt (vision or audio) and persist to settings.
#[tauri::command]
async fn update_prompt(
    state: tauri::State<'_, Arc<stream_manager::StreamManager>>,
    app_handle: tauri::AppHandle,
    source: String,
    text: String,
) -> Result<(), String> {
    state.update_prompt(&source, &text);

    // Persist to settings.toml
    if let Ok(mut s) = settings::Settings::load_from_app(&app_handle) {
        match source.as_str() {
            "vision" => s.vision_prompt = text,
            "audio" => s.audio_prompt = text,
            _ => {}
        }
        let dir = app_handle
            .path()
            .app_config_dir()
            .map_err(|e| e.to_string())?;
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = dir.join("settings.toml");
        let content = toml::to_string_pretty(&s).map_err(|e| e.to_string())?;
        std::fs::write(&path, content).map_err(|e| e.to_string())?;
        log::info!("Prompt '{}' persisted to settings", source);
    }

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();

    let screen_capture = Arc::new(capture::screen::ScreenCapture::new(2000, 1024, 75));
    let audio_capture = Arc::new(capture::audio::AudioCapture::new(24000, 250));
    let stream_mgr = Arc::new(stream_manager::StreamManager::new());

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(screen_capture)
        .manage(audio_capture)
        .manage(stream_mgr)
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(tray::on_shortcut_event)
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            greet,
            toggle_capture,
            list_monitors,
            select_monitor,
            toggle_audio_capture,
            list_audio_devices,
            select_audio_device,
            configure_ai,
            is_ai_configured,
            start_audio_ai,
            stop_audio_ai,
            send_audio_chunk,
            get_prompts,
            update_prompt,
            settings::save_settings,
            settings::load_settings
        ])
        .setup(|app| {
            tray::setup_tray(app)?;

            use tauri_plugin_global_shortcut::GlobalShortcutExt;
            app.global_shortcut().register("ctrl+shift+b")?;
            log::info!("Global shortcut Ctrl+Shift+B registered");

            // Auto-configure AI provider from saved settings
            let sm = app.state::<Arc<stream_manager::StreamManager>>();
            if let Ok(s) = settings::Settings::load_from_app(app.handle()) {
                if !s.endpoint.is_empty() && !s.api_key.is_empty() {
                    sm.configure_azure(
                        &s.endpoint,
                        &s.api_key,
                        &s.vision_deployment,
                        &s.vision_prompt,
                        s.use_bearer,
                    );
                    if !s.audio_deployment.is_empty() {
                        sm.configure_audio(
                            &s.endpoint,
                            &s.api_key,
                            &s.audio_deployment,
                            &s.audio_prompt,
                        );
                    }
                    log::info!("AI provider auto-configured from saved settings");
                }
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
