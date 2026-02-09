import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface FramePayload {
  data: string;
  timestamp: string;
  width: number;
  height: number;
  diff_pct: number;
}

export interface AudioLevelPayload {
  level: number;
  timestamp: string;
}

export interface AudioChunkPayload {
  data: string;
  timestamp: string;
  sample_count: number;
}

export interface ToggleCapturePayload {
  source: "shortcut" | "tray";
}

export function listenCaptureFrame(
  cb: (payload: FramePayload) => void,
): Promise<UnlistenFn> {
  return listen<FramePayload>("capture:frame", (e) => cb(e.payload));
}

export function listenAudioLevel(
  cb: (payload: AudioLevelPayload) => void,
): Promise<UnlistenFn> {
  return listen<AudioLevelPayload>("capture:audio-level", (e) => cb(e.payload));
}

export function listenAudioChunk(
  cb: (payload: AudioChunkPayload) => void,
): Promise<UnlistenFn> {
  return listen<AudioChunkPayload>("capture:audio-chunk", (e) => cb(e.payload));
}

export function listenToggleCapture(
  cb: (payload: ToggleCapturePayload) => void,
): Promise<UnlistenFn> {
  return listen<ToggleCapturePayload>("toggle:capture", (e) => cb(e.payload));
}
