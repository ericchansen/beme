import { For, Show } from "solid-js";
import type { Accessor } from "solid-js";

interface CapturePreviewProps {
  isCapturing: Accessor<boolean>;
  audioLevel: Accessor<number>;
  frameData: Accessor<string | null>;
  filmstrip: Accessor<string[]>;
}

/** Left column: filmstrip thumbnails, large preview frame, and audio level meter. */
function CapturePreview(props: CapturePreviewProps) {
  return (
    <section class="flex flex-col gap-3 min-h-0 h-full">
      {/* Filmstrip */}
      <div class="flex gap-2 overflow-x-auto py-1 shrink-0">
        <For each={props.filmstrip()}>
          {(frame) => (
            <img
              src={`data:image/jpeg;base64,${frame}`}
              class="w-20 h-[45px] shrink-0 rounded border border-zinc-300 dark:border-zinc-600 object-cover"
              alt="capture thumbnail"
            />
          )}
        </For>
        <Show when={props.filmstrip().length === 0}>
          <div class="w-20 h-[45px] shrink-0 rounded bg-zinc-200 dark:bg-zinc-700 border border-zinc-300 dark:border-zinc-600" />
        </Show>
      </div>

      {/* Large preview */}
      <div class="flex-1 min-h-0 rounded-lg border border-zinc-200 dark:border-zinc-700 bg-zinc-50 dark:bg-zinc-800 flex items-center justify-center overflow-hidden">
        <Show
          when={props.frameData()}
          fallback={
            <span class="text-zinc-400 text-sm select-none">
              {props.isCapturing() ? "Waiting for frames…" : "No frames captured"}
            </span>
          }
        >
          <img
            src={`data:image/jpeg;base64,${props.frameData()}`}
            class="max-w-full max-h-full object-contain"
            alt="latest capture"
          />
        </Show>
      </div>

      {/* Audio level meter */}
      <div class="h-6 shrink-0 rounded-full border border-zinc-200 dark:border-zinc-700 bg-zinc-100 dark:bg-zinc-800 overflow-hidden">
        <div
          class="h-full rounded-full bg-green-500 transition-all duration-150"
          style={{ width: `${props.audioLevel()}%` }}
        />
      </div>
      <span class="text-[10px] text-zinc-400 -mt-2">Audio Level</span>
    </section>
  );
}

export default CapturePreview;
