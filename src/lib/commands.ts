import { invoke } from "@tauri-apps/api/core";
import type { Settings } from "../dashboard/settingsStore";

/** Toggle capture on/off. Returns the new capturing state. */
export function toggleCapture(): Promise<boolean> {
  return invoke<boolean>("toggle_capture");
}

/** Persist settings to TOML config file via Rust backend. */
export function saveSettings(settings: Settings): Promise<void> {
  return invoke<void>("save_settings", { settings });
}

/** Load settings from TOML config file. Returns defaults if none saved. */
export function loadSettings(): Promise<Settings> {
  return invoke<Settings>("load_settings");
}

/** Configure the AI endpoint. */
export function configureAi(
  endpoint: string,
  apiKey: string,
  deployment: string,
  systemPrompt: string,
  useBearer?: boolean,
): Promise<void> {
  return invoke<void>("configure_ai", { endpoint, apiKey, deployment, systemPrompt, useBearer: useBearer ?? false });
}

/** Check whether the AI backend is already configured. */
export function isAiConfigured(): Promise<boolean> {
  return invoke<boolean>("is_ai_configured");
}
