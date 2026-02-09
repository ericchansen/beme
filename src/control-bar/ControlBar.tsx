import { createSignal } from "solid-js";

/** Control bar — tiny always-on-top pill for toggling capture. */
function ControlBar() {
  const [isCapturing, setIsCapturing] = createSignal(false);

  return (
    <div
      class="flex items-center gap-2 px-3 py-2 rounded-full bg-zinc-900/90 backdrop-blur-sm text-white select-none"
      // Allow dragging the control bar window
      data-tauri-drag-region
    >
      {/* Mode indicators */}
      <span class="text-sm" title="Screen capture">🖥️</span>
      <span class="text-sm" title="Audio capture">🎤</span>

      {/* On/Off toggle */}
      <button
        class={`w-8 h-5 rounded-full transition-colors relative ${
          isCapturing() ? "bg-green-500" : "bg-zinc-600"
        }`}
        onClick={() => setIsCapturing(!isCapturing())}
        title={isCapturing() ? "Stop capture (Ctrl+Shift+B)" : "Start capture (Ctrl+Shift+B)"}
      >
        <span
          class={`block w-3.5 h-3.5 rounded-full bg-white absolute top-0.5 transition-transform ${
            isCapturing() ? "translate-x-3.5" : "translate-x-0.5"
          }`}
        />
      </button>

      {/* Status dot */}
      <span
        class={`w-2 h-2 rounded-full ${isCapturing() ? "bg-green-400 animate-pulse" : "bg-zinc-500"}`}
      />
    </div>
  );
}

export default ControlBar;
