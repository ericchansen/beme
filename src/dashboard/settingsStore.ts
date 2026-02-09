import { createSignal } from "solid-js";

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
  captureInterval: 2,
  screenshotMaxWidth: 1024,
  frameDiffThreshold: 5,
  visionPrompt: DEFAULT_VISION_PROMPT,
  audioPrompt: DEFAULT_AUDIO_PROMPT,
};

const [settings, setSettings] = createSignal<Settings>({ ...defaultSettings });

export { settings, setSettings };
