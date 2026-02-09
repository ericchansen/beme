import { invoke } from "@tauri-apps/api/core";

/** Toggle capture on/off. Returns the new capturing state. */
export function toggleCapture(): Promise<boolean> {
  return invoke<boolean>("toggle_capture");
}
