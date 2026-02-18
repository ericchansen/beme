import { invoke } from "@tauri-apps/api/core";
import type { Settings } from "../dashboard/settingsStore";

/** Toggle capture on/off. Returns the new capturing state. */
export function toggleCapture(): Promise<boolean> {
  return invoke<boolean>("toggle_capture");
}

/** Monitor descriptor returned by the backend. */
export interface MonitorInfo {
  id: number;
  name: string;
  is_primary: boolean;
  width: number;
  height: number;
}

/** List all connected monitors. */
export function listMonitors(): Promise<MonitorInfo[]> {
  return invoke<MonitorInfo[]>("list_monitors");
}

/** Select which monitor to capture. Pass null/undefined for primary. */
export function selectMonitor(monitorId: number | null): Promise<void> {
  return invoke<void>("select_monitor", { monitorId });
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

/** Toggle audio capture on/off. Returns the new capturing state. */
export function toggleAudioCapture(): Promise<boolean> {
  return invoke<boolean>("toggle_audio_capture");
}

/** Start the audio AI WebSocket session. */
export async function startAudioAi(): Promise<void> {
  return invoke("start_audio_ai");
}

/** Stop the audio AI WebSocket session. */
export async function stopAudioAi(): Promise<void> {
  return invoke("stop_audio_ai");
}

/** Audio device descriptor returned by the backend. */
export interface AudioDeviceInfo {
  name: string;
  is_default: boolean;
}

/** List all available audio output devices. */
export function listAudioDevices(): Promise<AudioDeviceInfo[]> {
  return invoke<AudioDeviceInfo[]>("list_audio_devices");
}

/** Select which audio device to capture from. Pass null for default. */
export function selectAudioDevice(deviceName: string | null): Promise<void> {
  return invoke<void>("select_audio_device", { deviceName });
}

/** Prompt info returned by get_prompts. */
export interface PromptsInfo {
  vision: string;
  audio: string;
}

/** Get the current vision and audio prompts. */
export function getPrompts(): Promise<PromptsInfo> {
  return invoke<PromptsInfo>("get_prompts");
}

/** Update a prompt (source: "vision" or "audio"). */
export function updatePrompt(source: string, text: string): Promise<void> {
  return invoke<void>("update_prompt", { source, text });
}
