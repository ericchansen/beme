import { createSignal, For, Show, onMount, onCleanup } from "solid-js";
import { marked } from "marked";
import { listenAiSuggestion, listenAiError, listenAudioStatus, type SuggestionPayload, type AiErrorPayload, type AudioStatusPayload } from "../../lib/events";
import { getPrompts, updatePrompt } from "../../lib/commands";
import type { UnlistenFn } from "@tauri-apps/api/event";

interface Suggestion {
  id: number;
  timestamp: string;
  text: string;
  source: "screen" | "audio";
  done: boolean;
}

const MAX_SUGGESTIONS = 20;

/** Right columns: side-by-side Screen & Audio AI suggestion panels. */
function SuggestionPanel() {
  const [suggestions, setSuggestions] = createSignal<Suggestion[]>([]);
  const [aiErrors, setAiErrors] = createSignal<{ message: string; timestamp: string }[]>([]);
  const [audioStatus, setAudioStatus] = createSignal<AudioStatusPayload>({ status: "disconnected", message: null });
  const [visionPrompt, setVisionPrompt] = createSignal("");
  const [audioPrompt, setAudioPrompt] = createSignal("");
  const [visionSaveStatus, setVisionSaveStatus] = createSignal<"idle" | "saving" | "saved">("idle");
  const [audioSaveStatus, setAudioSaveStatus] = createSignal<"idle" | "saving" | "saved">("idle");

  let visionTimer: ReturnType<typeof setTimeout> | undefined;
  let audioTimer: ReturnType<typeof setTimeout> | undefined;

  function handlePromptChange(source: "vision" | "audio", value: string) {
    const setStatus = source === "vision" ? setVisionSaveStatus : setAudioSaveStatus;
    const setPrompt = source === "vision" ? setVisionPrompt : setAudioPrompt;

    setPrompt(value);
    setStatus("saving");

    if (source === "vision") {
      clearTimeout(visionTimer);
      visionTimer = setTimeout(async () => {
        try {
          await updatePrompt(source, value);
          setVisionSaveStatus("saved");
          setTimeout(() => setVisionSaveStatus("idle"), 2000);
        } catch (e) {
          console.error("Failed to save prompt:", e);
          setVisionSaveStatus("idle");
        }
      }, 500);
    } else {
      clearTimeout(audioTimer);
      audioTimer = setTimeout(async () => {
        try {
          await updatePrompt(source, value);
          setAudioSaveStatus("saved");
          setTimeout(() => setAudioSaveStatus("idle"), 2000);
        } catch (e) {
          console.error("Failed to save prompt:", e);
          setAudioSaveStatus("idle");
        }
      }, 500);
    }
  }

  const unlisteners: UnlistenFn[] = [];

  onMount(async () => {
    try {
      const prompts = await getPrompts();
      setVisionPrompt(prompts.vision);
      setAudioPrompt(prompts.audio);
    } catch (e) {
      console.warn("Failed to load prompts:", e);
    }

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
          return updated.slice(0, MAX_SUGGESTIONS);
        });
      }),
    );

    unlisteners.push(
      await listenAiError((p: AiErrorPayload) => {
        setAiErrors((prev) => [{ message: p.message, timestamp: p.timestamp }, ...prev].slice(0, 10));
      }),
    );

    unlisteners.push(
      await listenAudioStatus((p: AudioStatusPayload) => {
        setAudioStatus(p);
      }),
    );
  });

  onCleanup(() => {
    for (const u of unlisteners) u();
  });

  const screenSuggestions = () => suggestions().filter((s) => s.source === "screen");
  const audioSuggestions = () => suggestions().filter((s) => s.source === "audio");

  return (
    <>
      {/* Screen: prompt box + suggestion panel */}
      <div class="flex flex-col min-h-0 h-full gap-2">
        <PromptEditor
          prompt={visionPrompt()}
          onPromptChange={(v) => handlePromptChange("vision", v)}
          saveStatus={visionSaveStatus()}
          label="Screen Prompt"
          accentColor="text-purple-600 dark:text-purple-400"
        />
        <SuggestionColumn
          title="Screen"
          titleColor="text-purple-600 dark:text-purple-400"
          items={screenSuggestions()}
          errors={aiErrors()}
        />
      </div>

      {/* Audio: prompt box + suggestion panel */}
      <div class="flex flex-col min-h-0 h-full gap-2">
        <PromptEditor
          prompt={audioPrompt()}
          onPromptChange={(v) => handlePromptChange("audio", v)}
          saveStatus={audioSaveStatus()}
          label="Audio Prompt"
          accentColor="text-teal-600 dark:text-teal-400"
        />
        <SuggestionColumn
          title="Audio"
          titleColor="text-teal-600 dark:text-teal-400"
          items={audioSuggestions()}
          errors={[]}
          statusDot={<AudioStatusDot status={audioStatus().status} />}
          statusBanner={
            audioStatus().status === "error" && audioStatus().message
              ? audioStatus().message
              : undefined
          }
        />
      </div>
    </>
  );
}

/** Prompt editor box — sits above the suggestion column as a separate card. */
function PromptEditor(props: {
  prompt: string;
  onPromptChange: (value: string) => void;
  saveStatus: "idle" | "saving" | "saved";
  label: string;
  accentColor: string;
}) {
  return (
    <div class="shrink-0 rounded-lg border border-zinc-200 dark:border-zinc-700 bg-zinc-50 dark:bg-zinc-800/50">
      <div class="flex items-center justify-between px-3 py-1.5 border-b border-zinc-200 dark:border-zinc-700">
        <span class={`text-xs font-semibold ${props.accentColor}`}>{props.label}</span>
        <Show when={props.saveStatus !== "idle"}>
          <span class={`text-[10px] ${
            props.saveStatus === "saving" ? "text-yellow-500" : "text-green-500"
          }`}>
            {props.saveStatus === "saving" ? "Saving..." : "✓ Saved"}
          </span>
        </Show>
      </div>
      <div class="p-2">
        <textarea
          class="w-full text-xs font-mono bg-white dark:bg-zinc-900 border border-zinc-300 dark:border-zinc-600 rounded px-2 py-1.5 resize-vertical focus:outline-none focus:ring-1 focus:ring-blue-400"
          style="min-height: 40px; max-height: 200px;"
          rows={2}
          value={props.prompt}
          onInput={(e) => props.onPromptChange(e.currentTarget.value)}
          placeholder="System prompt..."
        />
      </div>
    </div>
  );
}

/** A single suggestion column with header, optional status, and scrollable list. */
function SuggestionColumn(props: {
  title: string;
  titleColor: string;
  items: Suggestion[];
  errors: { message: string; timestamp: string }[];
  statusDot?: any;
  statusBanner?: string | null;
}) {
  return (
    <section class="flex flex-col min-h-0 flex-1 rounded-lg border border-zinc-200 dark:border-zinc-700 bg-zinc-50 dark:bg-zinc-800/50">
      {/* Column header */}
      <div class="flex items-center gap-1.5 px-3 py-2 border-b border-zinc-200 dark:border-zinc-700 shrink-0">
        <span class={`text-sm font-semibold ${props.titleColor}`}>{props.title}</span>
        {props.statusDot}
      </div>

      {/* Status error banner */}
      <Show when={props.statusBanner}>
        <div class="mx-3 mt-2 px-3 py-1.5 text-xs text-red-700 dark:text-red-300 bg-red-50 dark:bg-red-900/30 rounded border border-red-200 dark:border-red-800">
          {props.statusBanner}
        </div>
      </Show>

      {/* Inline AI errors */}
      <Show when={props.errors.length > 0}>
        <div class="px-3 pt-2">
          <For each={props.errors}>
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
          when={props.items.length > 0}
          fallback={
            <p class="text-zinc-400 text-sm text-center mt-8">
              Suggestions will appear here.
            </p>
          }
        >
          <ul class="flex flex-col gap-2">
            <For each={props.items}>
              {(item) => <SuggestionEntry suggestion={item} />}
            </For>
          </ul>
        </Show>
      </div>
    </section>
  );
}

/** Small colored dot indicating audio AI connection state. */
function AudioStatusDot(props: { status: string }) {
  const color = () => {
    switch (props.status) {
      case "connected": return "bg-green-500";
      case "connecting": return "bg-yellow-400 animate-pulse";
      case "error": return "bg-red-500";
      default: return "bg-zinc-400";
    }
  };

  return <span class={`inline-block w-2 h-2 rounded-full ${color()}`} title={props.status} />;
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
      <div
        class="text-zinc-700 dark:text-zinc-300 suggestion-prose text-sm"
        innerHTML={marked.parse(props.suggestion.text, { async: false }) as string}
      />
    </li>
  );
}

export default SuggestionPanel;
