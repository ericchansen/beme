/** Fixed status bar at the bottom of the dashboard. */
function StatusBar() {
  return (
    <footer class="flex items-center justify-between px-6 py-1.5 text-xs text-zinc-500 dark:text-zinc-400 border-t border-zinc-200 dark:border-zinc-700 bg-white dark:bg-zinc-900 shrink-0 select-none">
      <span>FPS: — | Diff: — | Tokens: — | Cost: —</span>
      <span class="flex items-center gap-1.5">
        <span class="inline-block w-2 h-2 rounded-full bg-orange-400" />
        Disconnected
      </span>
      <span>beme v0.1.0</span>
    </footer>
  );
}

export default StatusBar;
