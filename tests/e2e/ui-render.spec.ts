import { test, expect, type Page } from "@playwright/test";

// ---------------------------------------------------------------------------
// Tauri v2 mock helpers
// ---------------------------------------------------------------------------

/**
 * Inject a mock `__TAURI_INTERNALS__` + `__TAURI_EVENT_PLUGIN_INTERNALS__`
 * before any app code executes. This stubs `invoke` and the callback/event
 * system so the SolidJS frontend can boot without a real Tauri backend.
 */
function addTauriMock(page: Page) {
  return page.addInitScript(() => {
    // Callback registry — mirrors what the real Tauri runtime does.
    const callbacks: Record<number, Function> = {};
    let nextId = 1;

    // Map event names → list of callback IDs registered via plugin:event|listen
    const eventListeners: Record<string, number[]> = {};
    let nextEventId = 1;

    (window as any).__TAURI_INTERNALS__ = {
      transformCallback(cb: Function, _once = false) {
        const id = nextId++;
        callbacks[id] = cb;
        return id;
      },

      unregisterCallback(id: number) {
        delete callbacks[id];
      },

      async invoke(cmd: string, args: Record<string, any> = {}) {
        // --- event plugin commands ----------------------------------------
        if (cmd === "plugin:event|listen") {
          const { event, handler } = args;
          if (!eventListeners[event]) eventListeners[event] = [];
          eventListeners[event].push(handler); // handler is a callback ID
          const eventId = nextEventId++;
          return eventId;
        }
        if (cmd === "plugin:event|unlisten") {
          return;
        }

        // --- app commands -------------------------------------------------
        switch (cmd) {
          case "list_monitors":
            return [
              { id: 0, name: "Test Monitor", is_primary: true, width: 1920, height: 1080 },
            ];
          case "list_audio_devices":
            return [
              { name: "Test Speaker", is_default: true },
            ];
          case "load_settings":
            return {
              endpoint: "",
              apiKey: "",
              visionDeployment: "gpt-4o",
              audioDeployment: "gpt-4o-realtime-preview",
              useBearer: false,
              captureInterval: 2,
              screenshotMaxWidth: 1024,
              frameDiffThreshold: 5,
              visionPrompt: "test-vision-prompt",
              audioPrompt: "test-audio-prompt",
            };
          case "save_settings":
          case "configure_ai":
          case "select_monitor":
          case "select_audio_device":
          case "toggle_capture":
          case "toggle_audio_capture":
          case "start_audio_ai":
          case "stop_audio_ai":
          case "is_ai_configured":
            return null;
          case "get_prompts":
            return { vision: "test-vision-prompt", audio: "test-audio-prompt" };
          case "update_prompt":
            return null;
          default:
            console.log("[tauri-mock] unhandled invoke:", cmd, args);
            return null;
        }
      },

      convertFileSrc(path: string) {
        return path;
      },
    };

    // Event plugin internals — used by _unlisten
    (window as any).__TAURI_EVENT_PLUGIN_INTERNALS__ = {
      unregisterListener(_event: string, _eventId: number) {},
    };

    // Also set the isTauri flag so `isTauri()` returns true.
    (window as any).isTauri = true;

    // Expose helpers for test code to fire synthetic events.
    (window as any).__TEST_FIRE_EVENT__ = (
      eventName: string,
      payload: unknown,
    ) => {
      const ids = eventListeners[eventName] || [];
      for (const cbId of ids) {
        const cb = callbacks[cbId];
        if (cb) {
          cb({ event: eventName, id: cbId, payload });
        }
      }
    };

    // Expose a check for whether a given event has listeners registered.
    (window as any).__TEST_HAS_LISTENERS__ = (eventName: string): boolean => {
      return (eventListeners[eventName] || []).length > 0;
    };
  });
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Fire a Tauri event from inside the page. */
async function fireTauriEvent(page: Page, event: string, payload: unknown) {
  await page.evaluate(
    ({ event, payload }) => {
      (window as any).__TEST_FIRE_EVENT__(event, payload);
    },
    { event, payload },
  );
}

/** Wait until the app has registered listeners for the given event. */
async function waitForListener(page: Page, event: string) {
  await page.waitForFunction(
    (ev) => (window as any).__TEST_HAS_LISTENERS__(ev),
    event,
    { timeout: 5000 },
  );
}

/**
 * Locate a suggestion column by its header text ("Screen" or "Audio").
 * Each SuggestionColumn is a <section> whose first child <div> contains
 * a <span> with the column title.
 */
function suggestionColumn(page: Page, title: "Screen" | "Audio") {
  return page.locator("section", {
    has: page.locator(`div > span.font-semibold`, { hasText: title }),
  });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

test.describe("Dashboard UI rendering", () => {
  test.beforeEach(async ({ page }) => {
    await addTauriMock(page);
    await page.goto("/");
    // Wait for the SolidJS app to fully mount, including async onMount in SuggestionPanel
    await waitForListener(page, "ai:suggestion");
  });

  test("renders dashboard layout", async ({ page }) => {
    // Header with beme title
    await expect(page.locator("header h1")).toHaveText("beme");

    // Screen and Audio checkboxes
    await expect(page.getByLabel("Screen")).toBeVisible();
    await expect(page.getByLabel("Audio")).toBeVisible();

    // Both suggestion columns present with correct headers
    await expect(suggestionColumn(page, "Screen")).toBeVisible();
    await expect(suggestionColumn(page, "Audio")).toBeVisible();

    // Placeholder text in empty columns
    const placeholders = page.getByText("Suggestions will appear here.");
    await expect(placeholders).toHaveCount(2);
  });

  test("renders screen suggestion", async ({ page }) => {
    await fireTauriEvent(page, "ai:suggestion", {
      source: "screen",
      id: 1,
      text: "**Bold text** and normal text",
      done: true,
      timestamp: "2025-01-01T00:00:00Z",
    });

    const col = suggestionColumn(page, "Screen");

    // Markdown rendered: bold text in <strong>
    await expect(col.locator("strong")).toHaveText("Bold text");

    // Full text content
    await expect(col).toContainText("and normal text");

    // Source badge (exact match to avoid matching the column header)
    await expect(col.getByText("screen", { exact: true })).toBeVisible();
  });

  test("renders audio suggestion", async ({ page }) => {
    await fireTauriEvent(page, "ai:suggestion", {
      source: "audio",
      id: 2,
      text: "Audio suggestion content",
      done: true,
      timestamp: "2025-01-01T00:00:01Z",
    });

    const audioCol = suggestionColumn(page, "Audio");
    await expect(audioCol).toContainText("Audio suggestion content");

    // Source badge (exact match to avoid matching the column header)
    await expect(audioCol.getByText("audio", { exact: true })).toBeVisible();

    // Screen column should still be empty
    await expect(
      suggestionColumn(page, "Screen").getByText("Suggestions will appear here."),
    ).toBeVisible();
  });

  test("renders markdown correctly", async ({ page }) => {
    const markdown = [
      "## Heading Two",
      "",
      "Some **bold** and *italic* text.",
      "",
      "- Item one",
      "- Item two",
      "",
      "```js",
      "const x = 42;",
      "```",
    ].join("\n");

    await fireTauriEvent(page, "ai:suggestion", {
      source: "screen",
      id: 3,
      text: markdown,
      done: true,
      timestamp: "2025-01-01T00:00:02Z",
    });

    const col = suggestionColumn(page, "Screen");

    // Heading
    await expect(col.locator("h2")).toHaveText("Heading Two");

    // Bold
    await expect(col.locator("strong")).toHaveText("bold");

    // Italic
    await expect(col.locator("em")).toHaveText("italic");

    // List items (inside the rendered markdown prose area)
    const prose = col.locator(".suggestion-prose");
    await expect(prose.locator("li")).toHaveCount(2);
    await expect(prose.locator("li").first()).toHaveText("Item one");

    // Code block
    await expect(col.locator("code")).toContainText("const x = 42;");
  });

  test("shows streaming indicator for non-final suggestions", async ({ page }) => {
    await fireTauriEvent(page, "ai:suggestion", {
      source: "screen",
      id: 4,
      text: "Partial content...",
      done: false,
      timestamp: "2025-01-01T00:00:03Z",
    });

    const col = suggestionColumn(page, "Screen");

    // Streaming indicator
    await expect(col.getByText("streaming…")).toBeVisible();

    // Complete the stream
    await fireTauriEvent(page, "ai:suggestion", {
      source: "screen",
      id: 4,
      text: " finished!",
      done: true,
      timestamp: "2025-01-01T00:00:03Z",
    });

    // Streaming indicator should be gone
    await expect(col.getByText("streaming…")).not.toBeVisible();

    // Accumulated text
    await expect(col).toContainText("Partial content... finished!");
  });

  test("renders prompt editors", async ({ page }) => {
    // Wait a bit for prompts to load
    await page.waitForTimeout(300);

    // Prompt editors are separate boxes above the suggestion columns, identified by label
    const screenTextarea = page.locator("textarea").first();
    const audioTextarea = page.locator("textarea").last();

    await expect(screenTextarea).toBeVisible();
    await expect(audioTextarea).toBeVisible();

    // Verify the labels exist
    await expect(page.getByText("Screen Prompt")).toBeVisible();
    await expect(page.getByText("Audio Prompt")).toBeVisible();

    // Prompts should be loaded from mock
    await expect(screenTextarea).toHaveValue("test-vision-prompt");
    await expect(audioTextarea).toHaveValue("test-audio-prompt");

    // Editing should work
    await screenTextarea.fill("new vision prompt");
    await expect(screenTextarea).toHaveValue("new vision prompt");
  });
});
