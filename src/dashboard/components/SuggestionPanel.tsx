import { createSignal, For, Show, onMount, onCleanup } from "solid-js";
import { listenAiSuggestion, listenAiError, type SuggestionPayload, type AiErrorPayload } from "../../lib/events";
import type { UnlistenFn } from "@tauri-apps/api/event";

interface Suggestion {
  id: number;
  timestamp: string;
  text: string;
  source: "screen" | "audio";
  done: boolean;
}

const MAX_SUGGESTIONS = 20;

/** Right column: two-tab AI suggestion log (Screen / Audio). */
function SuggestionPanel() {
  const [activeTab, setActiveTab] = createSignal<"screen" | "audio">("screen");
  const [suggestions, setSuggestions] = createSignal<Suggestion[]>([]);
  const [aiErrors, setAiErrors] = createSignal<{ message: string; timestamp: string }[]>([]);

  const unlisteners: UnlistenFn[] = [];

  onMount(async () => {
    unlisteners.push(
      await listenAiSuggestion((p: SuggestionPayload) => {
        setSuggestions((prev) => {
          const existing = prev.find((s) => s.id === p.id);
          let updated: Suggestion[];
          if (existing) {
            updated = prev.map((s) =>
              s.id === p.id
                ? { ...s, text: s.text + p.text, done: p.done }
                : s,
            );
          } else {
            updated = [
              {
                id: p.id,
                timestamp: p.timestamp,
                text: p.text,
                source: p.source as "screen" | "audio",
                done: p.done,
              },
              ...prev,
            ];
          }
          // Keep newest first, cap at MAX_SUGGESTIONS
          return updated.slice(0, MAX_SUGGESTIONS);
        });
      }),
    );

    unlisteners.push(
      await listenAiError((p: AiErrorPayload) => {
        setAiErrors((prev) => [{ message: p.message, timestamp: p.timestamp }, ...prev].slice(0, 10));
      }),
    );
  });

  onCleanup(() => {
    for (const u of unlisteners) u();
  });

  const filtered = () => suggestions().filter((s) => s.source === activeTab());

  return (
    <section class="flex flex-col min-h-0 h-full">
      {/* Tab bar */}
      <div class="flex border-b border-zinc-200 dark:border-zinc-700 shrink-0">
        <button
          class={`flex-1 px-4 py-2 text-sm font-medium transition-colors ${
            activeTab() === "screen"
              ? "text-blue-600 dark:text-blue-400 border-b-2 border-blue-600 dark:border-blue-400"
              : "text-zinc-500 dark:text-zinc-400 hover:text-zinc-700 dark:hover:text-zinc-300"
          }`}
          onClick={() => setActiveTab("screen")}
        >
          Screen
        </button>
        <button
          class={`flex-1 px-4 py-2 text-sm font-medium transition-colors ${
            activeTab() === "audio"
              ? "text-blue-600 dark:text-blue-400 border-b-2 border-blue-600 dark:border-blue-400"
              : "text-zinc-500 dark:text-zinc-400 hover:text-zinc-700 dark:hover:text-zinc-300"
          }`}
          onClick={() => setActiveTab("audio")}
        >
          Audio
        </button>
      </div>

      {/* Inline AI errors */}
      <Show when={aiErrors().length > 0}>
        <div class="px-3 pt-2">
          <For each={aiErrors()}>
            {(err) => (
              <div class="text-xs text-red-600 dark:text-red-400 mb-1">
                <span class="text-zinc-400 mr-2">{err.timestamp}</span>
                {err.message}
              </div>
            )}
          </For>
        </div>
      </Show>

      {/* Suggestion list */}
      <div class="flex-1 min-h-0 overflow-y-auto p-3">
        <Show
          when={filtered().length > 0}
          fallback={
            <p class="text-zinc-400 text-sm text-center mt-8">
              Suggestions will appear here once AI is connected.
            </p>
          }
        >
          <ul class="flex flex-col gap-2">
            <For each={filtered()}>
              {(item) => <SuggestionEntry suggestion={item} />}
            </For>
          </ul>
        </Show>
      </div>
    </section>
  );
}

function SuggestionEntry(props: { suggestion: Suggestion }) {
  return (
    <li class="rounded-md border border-zinc-200 dark:border-zinc-700 bg-white dark:bg-zinc-800 p-3 text-sm">
      <div class="flex items-center justify-between mb-1">
        <span class="text-[10px] text-zinc-400">{props.suggestion.timestamp}</span>
        <div class="flex items-center gap-1.5">
          <Show when={!props.suggestion.done}>
            <span class="text-[10px] text-yellow-600 dark:text-yellow-400 animate-pulse">streaming…</span>
          </Show>
          <span
            class={`text-[10px] font-medium px-1.5 py-0.5 rounded ${
              props.suggestion.source === "screen"
                ? "bg-purple-100 text-purple-700 dark:bg-purple-900 dark:text-purple-300"
                : "bg-teal-100 text-teal-700 dark:bg-teal-900 dark:text-teal-300"
            }`}
          >
            {props.suggestion.source}
          </span>
        </div>
      </div>
      <p class="text-zinc-700 dark:text-zinc-300 whitespace-pre-wrap">{props.suggestion.text}</p>
    </li>
  );
}

export default SuggestionPanel;
