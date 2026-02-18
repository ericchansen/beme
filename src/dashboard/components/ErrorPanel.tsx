import { createSignal, For, Show } from "solid-js";
import type { Accessor } from "solid-js";

interface ErrorEntry {
  id: number;
  timestamp: string;
  message: string;
}

interface ErrorPanelProps {
  errors: Accessor<ErrorEntry[]>;
}

/** Collapsible error panel above the status bar. */
function ErrorPanel(props: ErrorPanelProps) {
  const [expanded, setExpanded] = createSignal(false);

  return (
    <div class="shrink-0 border-t border-zinc-200 dark:border-zinc-700 bg-white dark:bg-zinc-900">
      {/* Toggle bar */}
      <button
        class="w-full flex items-center gap-2 px-4 py-1.5 text-xs text-zinc-500 dark:text-zinc-400 hover:bg-zinc-50 dark:hover:bg-zinc-800 transition-colors"
        onClick={() => setExpanded(!expanded())}
      >
        <span>⚠</span>
        <span class="inline-flex items-center justify-center min-w-[18px] h-[18px] rounded-full bg-zinc-200 dark:bg-zinc-700 text-[10px] font-medium">
          {props.errors().length}
        </span>
        <span class="ml-1">{expanded() ? "Hide errors" : "Show errors"}</span>
      </button>

      {/* Error list */}
      <Show when={expanded()}>
        <div class="max-h-40 overflow-y-auto border-t border-zinc-200 dark:border-zinc-700">
          <Show
            when={props.errors().length > 0}
            fallback={<p class="px-4 py-3 text-xs text-zinc-400">No errors.</p>}
          >
            <ul class="divide-y divide-zinc-100 dark:divide-zinc-800">
              <For each={props.errors()}>
                {(err) => (
                  <li class="px-4 py-2 text-xs flex gap-3">
                    <span class="text-zinc-400 shrink-0">{err.timestamp}</span>
                    <span class="text-red-600 dark:text-red-400">
                      {err.message}
                    </span>
                  </li>
                )}
              </For>
            </ul>
          </Show>
        </div>
      </Show>
    </div>
  );
}

export default ErrorPanel;
