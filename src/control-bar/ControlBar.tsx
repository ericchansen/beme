import { createSignal, onMount, onCleanup } from "solid-js";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { listenToggleCapture } from "../lib/events";
import { toggleCapture as invokeToggleCapture } from "../lib/commands";

type CaptureState = "idle" | "capturing" | "error";

/** Control bar — tiny always-on-top pill for toggling capture. */
function ControlBar() {
  const [state, setState] = createSignal<CaptureState>("idle");
  const [screenOn, setScreenOn] = createSignal(false);
  const [audioOn, setAudioOn] = createSignal(false);

  let unlisten: UnlistenFn | undefined;

  onMount(async () => {
    unlisten = await listenToggleCapture(() => {
      const next = state() === "capturing" ? "idle" : "capturing";
      setState(next);
      setScreenOn(next === "capturing");
      setAudioOn(next === "capturing");
    });
  });

  onCleanup(() => unlisten?.());

  const isCapturing = () => state() === "capturing";
  const isError = () => state() === "error";

  async function toggleCapture() {
    try {
      const newState = await invokeToggleCapture();
      setState(newState ? "capturing" : "idle");
      setScreenOn(newState);
      setAudioOn(newState);
    } catch {
      setState("error");
    }
  }

  async function openDashboard() {
    const existing = await WebviewWindow.getByLabel("dashboard");
    if (existing) {
      await existing.setFocus();
      return;
    }
    new WebviewWindow("dashboard", {
      url: "index.html",
      title: "beme — Dashboard",
      width: 900,
      height: 670,
      center: true,
    });
  }

  function startDrag(e: MouseEvent) {
    // Only drag from background, not from buttons
    if ((e.target as HTMLElement).closest("button")) return;
    getCurrentWindow().startDragging();
  }

  // Accent ring color per state
  const ring = () =>
    isError() ? "ring-red-500/60" : isCapturing() ? "ring-green-500/50" : "ring-white/10";

  const bg = () =>
    isError()
      ? "bg-gray-900/85"
      : isCapturing()
        ? "bg-gray-900/80"
        : "bg-gray-900/80";

  return (
    <div
      onMouseDown={startDrag}
      class={`flex items-center gap-1.5 px-2.5 py-1.5 rounded-full backdrop-blur-sm text-white select-none cursor-grab
        ring-1 transition-all duration-300 ${bg()} ${ring()}`}
    >
      {/* Screen toggle */}
      <button
        onClick={() => setScreenOn(!screenOn())}
        title="Toggle screen capture"
        class={`p-1 rounded-full text-sm transition-colors duration-200
          ${screenOn() ? "text-green-400 bg-green-500/15" : "text-gray-500 hover:text-gray-300"}`}
      >
        🖥️
      </button>

      {/* Audio toggle */}
      <button
        onClick={() => setAudioOn(!audioOn())}
        title="Toggle audio capture"
        class={`p-1 rounded-full text-sm transition-colors duration-200
          ${audioOn() ? "text-green-400 bg-green-500/15" : "text-gray-500 hover:text-gray-300"}`}
      >
        🎤
      </button>

      {/* Main play/pause toggle */}
      <button
        onClick={toggleCapture}
        title={isCapturing() ? "Stop capture" : "Start capture"}
        class={`relative flex items-center justify-center w-7 h-7 rounded-full text-xs font-bold transition-all duration-200
          ${isCapturing()
            ? "bg-green-500 text-white shadow-[0_0_8px_rgba(34,197,94,0.5)]"
            : isError()
              ? "bg-red-500 text-white"
              : "bg-gray-700 text-gray-300 hover:bg-gray-600"}`}
      >
        {isCapturing() ? "⏸" : "▶"}
        {/* Pulsing dot when capturing */}
        {isCapturing() && (
          <span class="absolute -top-0.5 -right-0.5 w-2 h-2 rounded-full bg-green-400 animate-pulse" />
        )}
      </button>

      {/* Open dashboard */}
      <button
        onClick={openDashboard}
        title="Open dashboard"
        class="p-1 rounded-full text-sm text-gray-500 hover:text-gray-300 transition-colors duration-200"
      >
        ⚙
      </button>
    </div>
  );
}

export default ControlBar;
