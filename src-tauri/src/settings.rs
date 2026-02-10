use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::Manager;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub endpoint: String,
    #[serde(default)]
    pub api_key: String,
    pub vision_deployment: String,
    pub audio_deployment: String,
    #[serde(default)]
    pub use_bearer: bool,
    pub capture_interval: f64,
    pub screenshot_max_width: u32,
    pub frame_diff_threshold: u32,
    pub vision_prompt: String,
    pub audio_prompt: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            endpoint: String::new(),
            api_key: String::new(),
            vision_deployment: "gpt-4o".into(),
            audio_deployment: "gpt-4o-realtime-preview".into(),
            use_bearer: false,
            capture_interval: 2.0,
            screenshot_max_width: 1024,
            frame_diff_threshold: 5,
            vision_prompt: "You are an AI assistant observing my screen. Analyze what you see and suggest the single best next action I should take. Be specific and actionable.".into(),
            audio_prompt: "You are listening to a conversation. Suggest the best response or follow-up question.".into(),
        }
    }
}

fn config_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("settings.toml"))
}

impl Settings {
    /// Load settings from the app config directory (non-command helper).
    pub fn load_from_app(app: &tauri::AppHandle) -> Result<Self, String> {
        let path = config_path(app)?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        toml::from_str(&content).map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub async fn save_settings(app: tauri::AppHandle, settings: Settings) -> Result<(), String> {
    let path = config_path(&app)?;
    let content = toml::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    fs::write(&path, content).map_err(|e| e.to_string())?;
    log::info!("Settings saved to {}", path.display());
    Ok(())
}

#[tauri::command]
pub async fn load_settings(app: tauri::AppHandle) -> Result<Settings, String> {
    let path = config_path(&app)?;
    if !path.exists() {
        return Ok(Settings::default());
    }
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let settings: Settings = toml::from_str(&content).map_err(|e| e.to_string())?;
    Ok(settings)
}
