import { createSignal } from "solid-js";

/** Dashboard window — shows capture preview, AI suggestions, and status. */
function Dashboard() {
  const [isCapturing, setIsCapturing] = createSignal(false);

  return (
    <div class="min-h-screen bg-white dark:bg-zinc-900 text-zinc-900 dark:text-zinc-100">
      {/* Header */}
      <header class="flex items-center justify-between px-6 py-3 border-b border-zinc-200 dark:border-zinc-700">
        <h1 class="text-lg font-semibold tracking-tight">beme</h1>
        <div class="flex items-center gap-3">
          <span class={`inline-block w-2.5 h-2.5 rounded-full ${isCapturing() ? "bg-green-500" : "bg-zinc-400"}`} />
          <span class="text-sm">{isCapturing() ? "Capturing" : "Idle"}</span>
          <button
            class="px-3 py-1.5 text-sm font-medium rounded-md bg-zinc-100 dark:bg-zinc-800 hover:bg-zinc-200 dark:hover:bg-zinc-700 transition-colors"
            onClick={() => setIsCapturing(!isCapturing())}
          >
            {isCapturing() ? "Stop" : "Start"}
          </button>
        </div>
      </header>

      {/* Main content — two-column layout */}
      <main class="grid grid-cols-2 gap-4 p-6 h-[calc(100vh-57px)]">
        {/* Left: Capture Preview */}
        <section class="flex flex-col gap-4">
          <h2 class="text-sm font-medium text-zinc-500 dark:text-zinc-400 uppercase tracking-wide">Capture Preview</h2>
          <div class="flex-1 rounded-lg border border-zinc-200 dark:border-zinc-700 bg-zinc-50 dark:bg-zinc-800 flex items-center justify-center text-zinc-400">
            {isCapturing() ? "Waiting for frames..." : "Start capture to see preview"}
          </div>

          {/* Audio level meter placeholder */}
          <div class="h-12 rounded-lg border border-zinc-200 dark:border-zinc-700 bg-zinc-50 dark:bg-zinc-800 flex items-center justify-center text-zinc-400 text-sm">
            Audio Level Meter
          </div>
        </section>

        {/* Right: AI Suggestions */}
        <section class="flex flex-col gap-4">
          <h2 class="text-sm font-medium text-zinc-500 dark:text-zinc-400 uppercase tracking-wide">AI Suggestions</h2>
          <div class="flex-1 rounded-lg border border-zinc-200 dark:border-zinc-700 bg-zinc-50 dark:bg-zinc-800 overflow-y-auto p-4">
            <p class="text-zinc-400 text-sm">Suggestions will appear here once AI is connected.</p>
          </div>
        </section>
      </main>

      {/* Status bar */}
      <footer class="fixed bottom-0 left-0 right-0 flex items-center justify-between px-6 py-2 text-xs text-zinc-500 dark:text-zinc-400 border-t border-zinc-200 dark:border-zinc-700 bg-white dark:bg-zinc-900">
        <span>FPS: — | Diff: — | Tokens: —</span>
        <span>beme v0.1.0</span>
      </footer>
    </div>
  );
}

export default Dashboard;
