# AGENTS.md — beme

Instructions for AI coding agents (GitHub Copilot, Codex, etc.) working in this repo.

## Project Overview

**beme** is a Windows desktop app that captures your screen and audio in near real-time, streams context to Azure OpenAI, and surfaces next-best-action suggestions. Built with **Tauri 2** (Rust backend) and **SolidJS** (TypeScript frontend).

## Architecture

```
src/                         # SolidJS + TypeScript frontend
├── dashboard/               #   Main UI — Dashboard.tsx, components/, settingsStore.ts
├── lib/                     #   Tauri IPC wrappers — commands.ts, events.ts
├── App.css                  #   Global styles (Tailwind)
└── vite-env.d.ts

src-tauri/                   # Tauri 2 Rust backend
├── src/
│   ├── lib.rs               #   Tauri commands + app setup (managed state, plugins)
│   ├── main.rs              #   Entry point
│   ├── stream_manager.rs    #   Orchestrates capture → AI → suggestion pipeline
│   ├── settings.rs          #   TOML settings persistence
│   ├── tray.rs              #   System tray + global shortcut handler
│   ├── ai/                  #   AI provider abstraction
│   │   ├── mod.rs            #     AiProvider trait (analyze_frame, start_audio_stream)
│   │   ├── types.rs          #     AiError, TextStream, AudioSession, ProviderConfig
│   │   ├── azure_vision.rs   #     Azure OpenAI Responses API (screen analysis)
│   │   └── azure_audio.rs    #     Azure OpenAI Realtime API (WebSocket audio)
│   └── capture/             #   Hardware capture
│       ├── screen.rs         #     Screen capture with frame diffing (xcap)
│       └── audio.rs          #     Audio capture via cpal (24kHz PCM)
├── tests/                   #   Rust integration tests
│   ├── stream_manager_test.rs
│   ├── audio_e2e.rs
│   └── full_e2e.ps1         #   Full E2E PowerShell script (requires Azure creds)
└── Cargo.toml

tests/e2e/                   # Playwright E2E tests (frontend)
└── ui-render.spec.ts        #   Dashboard rendering, Tauri event mocking

infra/                       # Azure Bicep IaC
├── main.bicep               #   Orchestrator
├── main.bicepparam           #   Default parameters
├── modules/                 #   Foundry + monitoring modules
└── README.md
```

### Key Design Patterns

| Pattern | Details |
|---------|---------|
| **Managed state** | `Arc<T>` structs registered via `tauri::Builder::manage()` — see `lib.rs` |
| **IPC commands** | `#[tauri::command]` functions in `lib.rs`; frontend wrappers in `src/lib/commands.ts` |
| **Event bus** | Rust emits events (`ai:suggestion`, `ai:error`, `ai:audio-status`, `capture:frame`, etc.); frontend listens via `src/lib/events.ts` |
| **AI pipeline** | Vision: Responses API with `previous_response_id` chaining (server-side context). Audio: Realtime API WebSocket with manual `input_audio_buffer.commit` |
| **Settings** | Persisted to `settings.toml` via `settings.rs`; loaded on app startup |
| **Concurrency** | `std::sync::Mutex` for sync state, `tokio::sync::Mutex` for async audio session |

## Build & Test Commands

All commands are run from the repo root unless noted.

### Prerequisites

- [Rust](https://rustup.rs/) stable toolchain
- [Bun](https://bun.sh/) (or Node.js)
- Tauri CLI: `cargo install tauri-cli@^2`

### Install

```bash
bun install
```

### Rust

```bash
# Type-check
cargo check --manifest-path src-tauri/Cargo.toml

# Lint (must pass with zero warnings)
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings

# Unit + integration tests
cargo test --manifest-path src-tauri/Cargo.toml
```

### TypeScript / Frontend

```bash
# Type-check (no output on success)
npx tsc --noEmit

# Production build (Vite)
bun run build
```

### E2E Tests (Playwright)

```bash
# Start Vite dev server (port 1420), then run tests
npx playwright test
```

The Playwright config (`playwright.config.ts`) auto-starts `bun run dev` as a web server.

### Run the App

```bash
cargo tauri dev
```

## Coding Conventions

### Rust

- Tauri IPC commands go in `src-tauri/src/lib.rs` as `#[tauri::command]` functions.
- Heavy logic belongs in dedicated modules (`stream_manager`, `ai/`, `capture/`), not in command handlers.
- State is shared via `Arc<T>` managed by Tauri; use `tauri::State<'_, Arc<T>>` in commands.
- Use `log::info!` / `log::error!` for logging (env_logger).
- Error handling: return `Result<T, String>` from commands (Tauri convention); use `thiserror` for internal errors (`AiError`).
- The `AiProvider` trait in `ai/mod.rs` abstracts over AI backends — implement it for new providers.

### TypeScript / SolidJS

- Frontend IPC wrappers live in `src/lib/commands.ts` (invoke calls) and `src/lib/events.ts` (event listeners).
- UI components are in `src/dashboard/components/`.
- Use SolidJS signals (`createSignal`) for reactive state — not React hooks.
- Styling: Tailwind CSS v4 via `@tailwindcss/vite` plugin. No separate CSS files for components.
- Settings state is managed in `src/dashboard/settingsStore.ts`.

### Event Names

| Event | Direction | Payload |
|-------|-----------|---------|
| `ai:suggestion` | Rust → Frontend | `{ text, timestamp, done, id, source }` |
| `ai:error` | Rust → Frontend | `{ message, timestamp }` |
| `ai:audio-status` | Rust → Frontend | `{ status, message }` |
| `capture:frame` | Rust → Frontend | `{ data, timestamp, width, height, diff_pct }` |
| `capture:audio-level` | Rust → Frontend | `{ level, timestamp }` |
| `toggle:capture` | Rust → Frontend | `{ source }` |

## Testing Guidance

### Frontend E2E (Playwright)

Tests mock `window.__TAURI_INTERNALS__` and `window.__TAURI_EVENT_PLUGIN_INTERNALS__` so the SolidJS app boots without a Tauri backend. See `tests/e2e/ui-render.spec.ts` for the mock setup pattern.

Key helpers:
- `addTauriMock(page)` — inject mocks via `addInitScript`
- `fireTauriEvent(page, event, payload)` — simulate backend events
- `waitForListener(page, event)` — wait for app to register event handlers

### Rust Unit Tests

- `StreamManager` has `inject_audio_session()`, `has_audio_session()`, and `clear_audio_session()` for testing the audio pipeline without an `AppHandle`.
- Tests live alongside code (`#[cfg(test)] mod tests`) and in `src-tauri/tests/`.

### Event Logging for Tests

Set `BEME_TEST_LOG=/path/to/file.jsonl` to have `stream_manager` write all `ai:suggestion` events as JSONL for automated validation.

## Azure Configuration

The app reads config from `settings.toml` in the Tauri app config directory:
- Windows: `%APPDATA%\com.erichansen.beme\settings.toml`

See `.env.example` for the template. Required fields: `endpoint`, `api_key`, `gpt4o_deployment`.

For the Realtime audio API: `realtime_deployment` must also be set.

**Azure Realtime API notes:**
- Must use `openai.azure.com` domain (not `cognitiveservices.azure.com`).
- Server VAD can cancel responses — the app uses `turn_detection: null` with manual commit.

## CI

GitHub Actions workflow at `.github/workflows/ci.yml` runs on push/PR to `main`:

1. **Rust Check & Clippy** — type-check + lint
2. **Rust Tests** — `cargo test`
3. **Frontend Build** — `bun install` + TypeScript check + Vite build
4. **E2E Tests** — Playwright against Vite dev server

## Guardrails

- **Never commit secrets** — no API keys, tokens, or connection strings in source.
- **Never commit `.env` or `beme.toml`** with real credentials — both are in `.gitignore`.
- **Clippy must pass with `-D warnings`** — zero warnings policy.
- **TypeScript must pass `tsc --noEmit`** — no type errors allowed.
- **All tests must pass** before pushing — `cargo test` + `npx playwright test`.
