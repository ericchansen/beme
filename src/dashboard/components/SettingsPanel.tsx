import { createSignal, Show } from "solid-js";
import {
  settings,
  setSettings,
  defaultSettings,
  DEFAULT_VISION_PROMPT,
  DEFAULT_AUDIO_PROMPT,
  type Settings,
} from "../settingsStore";

interface SettingsPanelProps {
  open: () => boolean;
  onClose: () => void;
}

/** Slide-over settings panel. */
function SettingsPanel(props: SettingsPanelProps) {
  // Local draft so we can cancel without mutating the store
  const [draft, setDraft] = createSignal<Settings>({ ...settings() });
  const [showApiKey, setShowApiKey] = createSignal(false);
  const [toast, setToast] = createSignal("");

  // Reset draft whenever the panel opens
  const isOpen = () => {
    const open = props.open();
    if (open) setDraft({ ...settings() });
    return open;
  };

  const patch = (partial: Partial<Settings>) =>
    setDraft((prev) => ({ ...prev, ...partial }));

  const handleSave = () => {
    setSettings({ ...draft() });
    props.onClose();
  };

  const handleCancel = () => {
    props.onClose();
  };

  const handleTestConnection = () => {
    setToast("Not implemented yet");
    setTimeout(() => setToast(""), 2500);
  };

  // ── Shared styles ──────────────────────────────────────────────
  const inputClass =
    "w-full bg-gray-700 border border-gray-600 text-white rounded px-3 py-2 focus:outline-none focus:ring-1 focus:ring-blue-500";
  const labelClass = "block text-sm font-medium text-gray-300 mb-1";
  const sectionClass = "space-y-3";
  const headingClass = "text-sm font-semibold text-gray-400 uppercase tracking-wide";

  return (
    <Show when={isOpen()}>
      {/* Backdrop */}
      <div class="fixed inset-0 z-40 bg-black/50" onClick={handleCancel} />

      {/* Panel */}
      <aside class="fixed inset-y-0 right-0 z-50 w-full max-w-md flex flex-col bg-gray-800 text-gray-100 shadow-xl overflow-y-auto">
        {/* Header */}
        <div class="flex items-center justify-between px-5 py-4 border-b border-gray-700">
          <h2 class="text-lg font-semibold">Settings</h2>
          <button
            class="text-gray-400 hover:text-white transition-colors"
            onClick={handleCancel}
            aria-label="Close settings"
          >
            ✕
          </button>
        </div>

        {/* Body */}
        <div class="flex-1 overflow-y-auto px-5 py-5 space-y-6">
          {/* ── Azure Connection ─────────────────────────────── */}
          <section class={sectionClass}>
            <h3 class={headingClass}>Azure Connection</h3>

            <div>
              <label class={labelClass}>Endpoint URL</label>
              <input
                type="text"
                class={inputClass}
                placeholder="https://your-resource.openai.azure.com/"
                value={draft().endpoint}
                onInput={(e) => patch({ endpoint: e.currentTarget.value })}
              />
            </div>

            <div>
              <label class={labelClass}>API Key</label>
              <div class="relative">
                <input
                  type={showApiKey() ? "text" : "password"}
                  class={`${inputClass} pr-16`}
                  placeholder="••••••••••••"
                  value={draft().apiKey}
                  onInput={(e) => patch({ apiKey: e.currentTarget.value })}
                />
                <button
                  type="button"
                  class="absolute right-2 top-1/2 -translate-y-1/2 text-xs text-gray-400 hover:text-white"
                  onClick={() => setShowApiKey(!showApiKey())}
                >
                  {showApiKey() ? "Hide" : "Show"}
                </button>
              </div>
            </div>

            <div>
              <label class={labelClass}>Vision Deployment</label>
              <input
                type="text"
                class={inputClass}
                value={draft().visionDeployment}
                onInput={(e) => patch({ visionDeployment: e.currentTarget.value })}
              />
            </div>

            <div>
              <label class={labelClass}>Audio Deployment</label>
              <input
                type="text"
                class={inputClass}
                value={draft().audioDeployment}
                onInput={(e) => patch({ audioDeployment: e.currentTarget.value })}
              />
            </div>
          </section>

          {/* ── Capture Settings ─────────────────────────────── */}
          <section class={sectionClass}>
            <h3 class={headingClass}>Capture Settings</h3>

            <div>
              <label class={labelClass}>Screen Capture Interval (seconds)</label>
              <input
                type="number"
                class={inputClass}
                min={0.5}
                max={30}
                step={0.5}
                value={draft().captureInterval}
                onInput={(e) =>
                  patch({ captureInterval: parseFloat(e.currentTarget.value) || defaultSettings.captureInterval })
                }
              />
            </div>

            <div>
              <label class={labelClass}>Screenshot Max Width (px)</label>
              <input
                type="number"
                class={inputClass}
                min={256}
                max={3840}
                step={64}
                value={draft().screenshotMaxWidth}
                onInput={(e) =>
                  patch({ screenshotMaxWidth: parseInt(e.currentTarget.value, 10) || defaultSettings.screenshotMaxWidth })
                }
              />
            </div>

            <div>
              <label class={labelClass}>
                Frame Diff Threshold — skip frames with &lt; {draft().frameDiffThreshold}% difference
              </label>
              <input
                type="range"
                class="w-full accent-blue-500"
                min={0}
                max={20}
                step={1}
                value={draft().frameDiffThreshold}
                onInput={(e) =>
                  patch({ frameDiffThreshold: parseInt(e.currentTarget.value, 10) })
                }
              />
              <div class="flex justify-between text-xs text-gray-500">
                <span>0%</span>
                <span>20%</span>
              </div>
            </div>
          </section>

          {/* ── Shortcuts ────────────────────────────────────── */}
          <section class={sectionClass}>
            <h3 class={headingClass}>Shortcuts</h3>
            <div>
              <label class={labelClass}>Global Shortcut</label>
              <input
                type="text"
                class={`${inputClass} cursor-not-allowed opacity-70`}
                value="Ctrl+Shift+B"
                readOnly
              />
            </div>
          </section>

          {/* ── System Prompts ───────────────────────────────── */}
          <section class={sectionClass}>
            <h3 class={headingClass}>System Prompts</h3>

            <div>
              <div class="flex items-center justify-between mb-1">
                <label class="text-sm font-medium text-gray-300">Vision System Prompt</label>
                <button
                  type="button"
                  class="text-xs text-blue-400 hover:text-blue-300"
                  onClick={() => patch({ visionPrompt: DEFAULT_VISION_PROMPT })}
                >
                  Reset to Default
                </button>
              </div>
              <textarea
                class={`${inputClass} min-h-[80px] resize-y`}
                rows={3}
                value={draft().visionPrompt}
                onInput={(e) => patch({ visionPrompt: e.currentTarget.value })}
              />
            </div>

            <div>
              <div class="flex items-center justify-between mb-1">
                <label class="text-sm font-medium text-gray-300">Audio System Prompt</label>
                <button
                  type="button"
                  class="text-xs text-blue-400 hover:text-blue-300"
                  onClick={() => patch({ audioPrompt: DEFAULT_AUDIO_PROMPT })}
                >
                  Reset to Default
                </button>
              </div>
              <textarea
                class={`${inputClass} min-h-[80px] resize-y`}
                rows={3}
                value={draft().audioPrompt}
                onInput={(e) => patch({ audioPrompt: e.currentTarget.value })}
              />
            </div>
          </section>
        </div>

        {/* Footer actions */}
        <div class="flex items-center justify-between gap-3 px-5 py-4 border-t border-gray-700">
          <button
            type="button"
            class="text-sm text-blue-400 hover:text-blue-300"
            onClick={handleTestConnection}
          >
            Test Connection
          </button>

          <div class="flex gap-2">
            <button
              type="button"
              class="px-4 py-2 text-sm rounded-md border border-gray-600 text-gray-300 hover:bg-gray-700 transition-colors"
              onClick={handleCancel}
            >
              Cancel
            </button>
            <button
              type="button"
              class="px-4 py-2 text-sm rounded-md bg-blue-600 text-white hover:bg-blue-500 transition-colors"
              onClick={handleSave}
            >
              Save
            </button>
          </div>
        </div>

        {/* Toast */}
        <Show when={toast()}>
          <div class="absolute bottom-16 left-1/2 -translate-x-1/2 bg-gray-700 text-sm text-gray-200 px-4 py-2 rounded shadow-lg">
            {toast()}
          </div>
        </Show>
      </aside>
    </Show>
  );
}

export default SettingsPanel;
