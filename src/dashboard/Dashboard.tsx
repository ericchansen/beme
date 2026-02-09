import { createSignal } from "solid-js";
import CapturePreview from "./components/CapturePreview";
import SuggestionPanel from "./components/SuggestionPanel";
import StatusBar from "./components/StatusBar";
import ErrorPanel from "./components/ErrorPanel";

/** Dashboard window — shows capture preview, AI suggestions, and status. */
function Dashboard() {
  const [isCapturing, setIsCapturing] = createSignal(false);
  const [screenEnabled, setScreenEnabled] = createSignal(true);
  const [audioEnabled, setAudioEnabled] = createSignal(true);
  const [audioLevel] = createSignal(0);
  const [errors] = createSignal<{ id: number; timestamp: string; message: string }[]>([]);

  return (
    <div class="h-screen flex flex-col bg-white dark:bg-zinc-900 text-zinc-900 dark:text-zinc-100">
      {/* Header */}
      <header class="flex items-center justify-between px-6 py-2.5 border-b border-zinc-200 dark:border-zinc-700 shrink-0">
        <h1 class="text-lg font-semibold tracking-tight">beme</h1>

        <div class="flex items-center gap-4">
          {/* Status indicator */}
          <span class="flex items-center gap-1.5">
            <span class={`inline-block w-2.5 h-2.5 rounded-full ${isCapturing() ? "bg-green-500 animate-pulse" : "bg-zinc-400"}`} />
            <span class="text-sm">{isCapturing() ? "Capturing" : "Idle"}</span>
          </span>

          {/* Start / Stop */}
          <button
            class={`px-3 py-1.5 text-sm font-medium rounded-md transition-colors ${
              isCapturing()
                ? "bg-red-100 text-red-700 hover:bg-red-200 dark:bg-red-900/40 dark:text-red-400 dark:hover:bg-red-900/60"
                : "bg-green-100 text-green-700 hover:bg-green-200 dark:bg-green-900/40 dark:text-green-400 dark:hover:bg-green-900/60"
            }`}
            onClick={() => setIsCapturing(!isCapturing())}
          >
            {isCapturing() ? "Stop" : "Start"}
          </button>

          {/* Source checkboxes */}
          <label class="flex items-center gap-1 text-sm cursor-pointer select-none">
            <input
              type="checkbox"
              checked={screenEnabled()}
              onChange={() => setScreenEnabled(!screenEnabled())}
              class="accent-blue-600"
            />
            Screen
          </label>
          <label class="flex items-center gap-1 text-sm cursor-pointer select-none">
            <input
              type="checkbox"
              checked={audioEnabled()}
              onChange={() => setAudioEnabled(!audioEnabled())}
              class="accent-blue-600"
            />
            Audio
          </label>
        </div>
      </header>

      {/* Main content — two-column layout */}
      <main class="flex-1 min-h-0 grid grid-cols-[1.2fr_1fr] gap-4 p-4">
        {/* Left: Capture Preview */}
        <CapturePreview isCapturing={isCapturing} audioLevel={audioLevel} />

        {/* Right: AI Suggestions */}
        <SuggestionPanel />
      </main>

      {/* Error panel (collapsible) */}
      <ErrorPanel errors={errors} />

      {/* Status bar */}
      <StatusBar />
    </div>
  );
}

export default Dashboard;
