import type { Accessor } from "solid-js";

interface StatusBarProps {
  fps: Accessor<number>;
  diffPct: Accessor<number>;
  isCapturing: Accessor<boolean>;
}

/** Fixed status bar at the bottom of the dashboard. */
function StatusBar(props: StatusBarProps) {
  return (
    <footer class="flex items-center justify-between px-6 py-1.5 text-xs text-zinc-500 dark:text-zinc-400 border-t border-zinc-200 dark:border-zinc-700 bg-white dark:bg-zinc-900 shrink-0 select-none">
      <span>
        FPS: {props.fps()} | Diff: {props.diffPct().toFixed(1)}% | Tokens: — |
        Cost: —
      </span>
      <span class="flex items-center gap-1.5">
        <span
          class={`inline-block w-2 h-2 rounded-full ${props.isCapturing() ? "bg-green-500" : "bg-orange-400"}`}
        />
        {props.isCapturing() ? "Capturing" : "Idle"}
      </span>
      <span>beme v0.1.0</span>
    </footer>
  );
}

export default StatusBar;
