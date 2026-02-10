import { createSignal } from "solid-js";
import { loadSettings as loadSettingsCmd } from "../lib/commands";

export const DEFAULT_VISION_PROMPT =
  "You are an AI assistant observing my screen. Analyze what you see and suggest the single best next action I should take. Be specific and actionable.";

export const DEFAULT_AUDIO_PROMPT =
  "You are listening to a conversation. Suggest the best response or follow-up question.";

export interface Settings {
  // Azure Connection
  endpoint: string;
  apiKey: string;
  visionDeployment: string;
  audioDeployment: string;
  useBearer: boolean;
  // Capture
  captureInterval: number;
  screenshotMaxWidth: number;
  frameDiffThreshold: number;
  // System Prompts
  visionPrompt: string;
  audioPrompt: string;
}

export const defaultSettings: Settings = {
  endpoint: "",
  apiKey: "",
  visionDeployment: "gpt-4o",
  audioDeployment: "gpt-4o-realtime-preview",
  useBearer: false,
  captureInterval: 2,
  screenshotMaxWidth: 1024,
  frameDiffThreshold: 5,
  visionPrompt: DEFAULT_VISION_PROMPT,
  audioPrompt: DEFAULT_AUDIO_PROMPT,
};

const [settings, setSettings] = createSignal<Settings>({ ...defaultSettings });

/** Load persisted settings from the Rust backend on startup. */
export async function initSettings(): Promise<void> {
  try {
    const loaded = await loadSettingsCmd();
    setSettings(loaded);
  } catch (e) {
    console.warn("Failed to load settings, using defaults:", e);
  }
}

export { settings, setSettings };
