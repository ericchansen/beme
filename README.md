# beme

[![CI](https://github.com/ericchansen/beme/actions/workflows/ci.yml/badge.svg)](https://github.com/ericchansen/beme/actions/workflows/ci.yml)

A desktop app that capturesyour screen in near real-time and streams it to Azure OpenAI GPT-4o for **next best action** suggestions. Built with Tauri 2 and SolidJS.

## Features

- **Screen capture with frame diffing** — efficient capture that only sends changed frames
- **AI-powered suggestions** — GPT-4o vision analyzes your screen and recommends actions
- **System tray + global shortcut** — toggle with `Ctrl+Shift+B`
- **Streaming SSE responses** — suggestions appear in real time
- **Settings persistence** — endpoint, deployment, and token saved locally
- **Bearer token auth** — authenticate via Azure Entra ID

## Prerequisites

- [Rust](https://rustup.rs/) (stable toolchain)
- [Node.js](https://nodejs.org/) or [Bun](https://bun.sh/)
- Tauri 2 CLI: `cargo install tauri-cli@^2`
- [Azure CLI](https://learn.microsoft.com/en-us/cli/azure/install-azure-cli) (for infrastructure)

## Quick Start

```bash
git clone https://github.com/ericchansen/beme.git
cd beme && bun install
cargo tauri dev
```

## Azure Setup

1. Deploy infrastructure:

   ```bash
   az deployment group create \
     --resource-group rg-beme-dev \
     --template-file infra/main.bicep
   ```

2. Get a bearer token:

   ```bash
   az account get-access-token --resource https://cognitiveservices.azure.com
   ```

3. Open **Settings** in the app and paste your endpoint, deployment name, and token.

## Architecture

| Layer | Technology |
|-------|-----------|
| Backend | Tauri 2 (Rust) — screen capture, system tray, IPC |
| Frontend | SolidJS + Tailwind CSS |
| AI | Azure OpenAI GPT-4o vision |

## Building

Generate a production installer (NSIS/MSI on Windows):

```bash
cargo tauri build
```

The installer will be in `src-tauri/target/release/bundle/`.

## License

[MIT](LICENSE)

