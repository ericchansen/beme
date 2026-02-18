import { createSignal, onMount, onCleanup } from "solid-js";
import type { UnlistenFn } from "@tauri-apps/api/event";
import CapturePreview from "./components/CapturePreview";
import SuggestionPanel from "./components/SuggestionPanel";
import StatusBar from "./components/StatusBar";
import ErrorPanel from "./components/ErrorPanel";
import SettingsPanel from "./components/SettingsPanel";
import {
  listenCaptureFrame,
  listenAudioLevel,
  listenToggleCapture,
  listenAiError,
  type FramePayload,
} from "../lib/events";
import {
  toggleCapture,
  toggleAudioCapture,
  startAudioAi,
  stopAudioAi,
  listMonitors,
  selectMonitor,
  listAudioDevices,
  selectAudioDevice,
  type MonitorInfo,
  type AudioDeviceInfo,
} from "../lib/commands";
import { initSettings } from "./settingsStore";

/** Dashboard window — shows capture preview, AI suggestions, and status. */
function Dashboard() {
  const [isCapturing, setIsCapturing] = createSignal(false);
  const [screenEnabled, setScreenEnabled] = createSignal(true);
  const [audioEnabled, setAudioEnabled] = createSignal(true);
  const [audioLevel, setAudioLevel] = createSignal(0);
  const [frameData, setFrameData] = createSignal<string | null>(null);
  const [filmstrip, setFilmstrip] = createSignal<string[]>([]);
  const [fps, setFps] = createSignal(0);
  const [diffPct, setDiffPct] = createSignal(0);
  const [errors, setErrors] = createSignal<
    { id: number; timestamp: string; message: string }[]
  >([]);
  const [settingsOpen, setSettingsOpen] = createSignal(false);
  const [monitors, setMonitors] = createSignal<MonitorInfo[]>([]);
  const [selectedMonitorId, setSelectedMonitorId] = createSignal<number | null>(
    null,
  );
  const [audioDevices, setAudioDevices] = createSignal<AudioDeviceInfo[]>([]);
  const [selectedAudioDevice, setSelectedAudioDevice] = createSignal<
    string | null
  >(null);

  const unlisteners: UnlistenFn[] = [];
  let frameTimestamps: number[] = [];

  onMount(async () => {
    await initSettings();

    // Load available monitors
    try {
      const mons = await listMonitors();
      setMonitors(mons);
      const primary = mons.find((m) => m.is_primary);
      if (primary) setSelectedMonitorId(primary.id);
    } catch (e) {
      console.error("Failed to list monitors:", e);
    }

    // Load available audio devices
    try {
      const devs = await listAudioDevices();
      setAudioDevices(devs);
    } catch (e) {
      console.error("Failed to list audio devices:", e);
    }

    unlisteners.push(
      await listenCaptureFrame((p: FramePayload) => {
        if (!screenEnabled()) return;
        setFrameData(p.data);
        setDiffPct(p.diff_pct);
        setFilmstrip((prev) => [...prev, p.data].slice(-10));

        const now = Date.now();
        frameTimestamps.push(now);
        frameTimestamps = frameTimestamps.filter((t) => now - t < 1000);
        setFps(frameTimestamps.length);
      }),
    );

    unlisteners.push(
      await listenAudioLevel((p) => {
        if (!audioEnabled()) return;
        setAudioLevel(Math.round(p.level * 100));
      }),
    );

    unlisteners.push(
      await listenToggleCapture(() => {
        setIsCapturing((prev) => !prev);
      }),
    );

    unlisteners.push(
      await listenAiError((p) => {
        setErrors((prev) => [
          ...prev,
          { id: Date.now(), timestamp: p.timestamp, message: p.message },
        ]);
      }),
    );
  });

  onCleanup(() => {
    for (const u of unlisteners) u();
  });

  async function handleToggle() {
    const newState = await toggleCapture();
    setIsCapturing(newState);
    // Also start/stop audio capture if audio is enabled
    if (audioEnabled()) {
      if (newState) {
        await toggleAudioCapture();
        try {
          await startAudioAi();
        } catch (e) {
          console.error("Failed to start audio AI:", e);
        }
      } else {
        try {
          await stopAudioAi();
        } catch (e) {
          console.error("Failed to stop audio AI:", e);
        }
        await toggleAudioCapture();
      }
    }
  }

  async function handleAudioToggle() {
    const newEnabled = !audioEnabled();
    setAudioEnabled(newEnabled);
    if (!isCapturing()) return; // Only start/stop audio if currently capturing
    if (newEnabled) {
      await toggleAudioCapture();
      try {
        await startAudioAi();
      } catch (e) {
        console.error("Failed to start audio AI:", e);
      }
    } else {
      try {
        await stopAudioAi();
      } catch (e) {
        console.error("Failed to stop audio AI:", e);
      }
      await toggleAudioCapture();
    }
  }

  return (
    <div class="h-screen flex flex-col bg-white dark:bg-zinc-900 text-zinc-900 dark:text-zinc-100">
      {/* Header */}
      <header class="flex items-center justify-between px-6 py-2.5 border-b border-zinc-200 dark:border-zinc-700 shrink-0">
        <h1 class="text-lg font-semibold tracking-tight">beme</h1>

        <div class="flex items-center gap-4">
          {/* Status indicator */}
          <span class="flex items-center gap-1.5">
            <span
              class={`inline-block w-2.5 h-2.5 rounded-full ${isCapturing() ? "bg-green-500 animate-pulse" : "bg-zinc-400"}`}
            />
            <span class="text-sm">{isCapturing() ? "Capturing" : "Idle"}</span>
          </span>

          {/* Start / Stop */}
          <button
            class={`px-3 py-1.5 text-sm font-medium rounded-md transition-colors ${
              isCapturing()
                ? "bg-red-100 text-red-700 hover:bg-red-200 dark:bg-red-900/40 dark:text-red-400 dark:hover:bg-red-900/60"
                : "bg-green-100 text-green-700 hover:bg-green-200 dark:bg-green-900/40 dark:text-green-400 dark:hover:bg-green-900/60"
            }`}
            onClick={handleToggle}
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

          {/* Monitor selector */}
          {monitors().length > 1 && (
            <select
              class="text-sm bg-zinc-100 dark:bg-zinc-800 border border-zinc-300 dark:border-zinc-600 rounded-md px-2 py-1 cursor-pointer focus:outline-none focus:ring-1 focus:ring-blue-500"
              value={selectedMonitorId() ?? ""}
              onChange={async (e) => {
                const val = e.currentTarget.value;
                const id = val ? Number(val) : null;
                setSelectedMonitorId(id);
                await selectMonitor(id);
              }}
            >
              {monitors().map((m) => (
                <option value={m.id}>
                  {m.name || `Display ${m.id}`} ({m.width}×{m.height})
                  {m.is_primary ? " ★" : ""}
                </option>
              ))}
            </select>
          )}
          <label class="flex items-center gap-1 text-sm cursor-pointer select-none">
            <input
              type="checkbox"
              checked={audioEnabled()}
              onChange={handleAudioToggle}
              class="accent-blue-600"
            />
            Audio
          </label>

          {/* Audio device selector */}
          {audioDevices().length > 1 && (
            <select
              class="text-sm bg-zinc-100 dark:bg-zinc-800 border border-zinc-300 dark:border-zinc-600 rounded-md px-2 py-1 cursor-pointer focus:outline-none focus:ring-1 focus:ring-blue-500 max-w-[160px] truncate"
              value={selectedAudioDevice() ?? ""}
              onChange={async (e) => {
                const val = e.currentTarget.value || null;
                setSelectedAudioDevice(val);
                await selectAudioDevice(val);
              }}
            >
              <option value="">Default Audio</option>
              {audioDevices().map((d) => (
                <option value={d.name}>
                  {d.name}
                  {d.is_default ? " ★" : ""}
                </option>
              ))}
            </select>
          )}

          {/* Settings */}
          <button
            class="p-1.5 rounded-md text-zinc-500 hover:text-zinc-100 hover:bg-zinc-700 transition-colors"
            onClick={() => setSettingsOpen(true)}
            aria-label="Settings"
          >
            ⚙️
          </button>
        </div>
      </header>

      {/* Main content — two-column layout */}
      <main class="flex-1 min-h-0 grid grid-cols-[1fr_1fr_1fr] gap-4 p-4">
        {/* Left: Capture Preview */}
        <CapturePreview
          isCapturing={isCapturing}
          audioLevel={audioLevel}
          frameData={frameData}
          filmstrip={filmstrip}
        />

        {/* Center + Right: AI Suggestions (side-by-side) */}
        <div class="col-span-2 grid grid-cols-2 gap-4 min-h-0">
          <SuggestionPanel />
        </div>
      </main>

      {/* Error panel (collapsible) */}
      <ErrorPanel errors={errors} />

      {/* Status bar */}
      <StatusBar fps={fps} diffPct={diffPct} isCapturing={isCapturing} />

      {/* Settings slide-over */}
      <SettingsPanel
        open={settingsOpen}
        onClose={() => setSettingsOpen(false)}
      />
    </div>
  );
}

export default Dashboard;
