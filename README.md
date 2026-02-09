# beme — "be me"

Screen and audio AI assistant. Captures your screen and system audio in near real-time, sends it to Azure AI, and displays actionable "next best action" suggestions.

## Architecture

- **Desktop app**: [Tauri 2](https://tauri.app/) (Rust backend + SolidJS frontend)
- **AI backend**: Azure OpenAI via Microsoft Foundry (GPT-4o vision + Realtime audio)
- **Two windows**: Dashboard (full dev/review UI) + Control Bar (tiny floating toggle)

## Prerequisites

- [Rust](https://rustup.rs/) (stable toolchain)
- [Bun](https://bun.sh/)
- [Azure CLI](https://learn.microsoft.com/en-us/cli/azure/install-azure-cli) (for infrastructure deployment)

## Getting Started

```bash
# Install frontend dependencies
bun install

# Run in development mode
bun run tauri dev
```

## Configuration

Copy `.env.example` to your app data directory as `beme.toml` and fill in your Azure OpenAI credentials. See the file for details.

## Azure Infrastructure

Deploy the required Azure resources:

```bash
az deployment group create \
  --resource-group <your-rg> \
  --template-file infra/main.bicep
```

## License

[MIT](LICENSE)

