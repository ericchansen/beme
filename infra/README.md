# beme — Azure Infrastructure

Bicep templates for provisioning the beme AI backend on Azure.

## What Gets Created

| Resource | Type | Name Pattern |
|----------|------|-------------|
| AI Foundry | `Microsoft.CognitiveServices/accounts` (AIServices) | `{env}-beme-ai` |
| gpt-4o deployment | `accounts/deployments` | `gpt-4o` (80K TPM, GlobalStandard) |
| gpt-4o-realtime-preview | `accounts/deployments` | `gpt-4o-realtime-preview` (6 capacity) |
| Log Analytics | `Microsoft.OperationalInsights/workspaces` | `{env}-beme-logs` |
| Diagnostic Settings | `Microsoft.Insights/diagnosticSettings` | Audit, RequestResponse, Trace → Log Analytics |

## Prerequisites

- [Azure CLI](https://learn.microsoft.com/cli/azure/install-azure-cli) (v2.60+)
- An Azure subscription with Cognitive Services / AI Services quota
- A resource group (e.g. `rg-beme-dev`)
- Logged in: `az login`

## Deploy

```bash
# Create a resource group (if needed)
az group create --name rg-beme-dev --location eastus2

# Deploy with defaults (dev environment)
az deployment group create \
  --resource-group rg-beme-dev \
  --template-file infra/main.bicep

# Deploy with custom parameters
az deployment group create \
  --resource-group rg-beme-dev \
  --template-file infra/main.bicep \
  --parameters environmentName=staging projectName=beme location=eastus2

# Or use the .bicepparam file
az deployment group create \
  --resource-group rg-beme-dev \
  --template-file infra/main.bicep \
  --parameters infra/main.bicepparam
```

## Parameters

| Parameter | Default | Description |
|-----------|---------|-------------|
| `location` | `eastus2` | Azure region |
| `environmentName` | `dev` | Prefix for resource names |
| `projectName` | `beme` | Project identifier |

## File Structure

```
infra/
├── main.bicep              # Orchestrator — calls modules, defines params
├── main.bicepparam         # Default parameter values
├── modules/
│   ├── foundry.bicep       # Foundry resource + model deployments
│   └── monitoring.bicep    # Log Analytics + diagnostic settings
└── README.md               # This file
```

## Cost Notes

- **AI Foundry (S0)**: No base cost — you pay per API call (token usage).
- **gpt-4o**: Billed per 1K tokens at [current pricing](https://azure.microsoft.com/pricing/details/cognitive-services/openai-service/).
- **gpt-4o-realtime-preview**: Billed per session minute + tokens.
- **Log Analytics (PerGB2018)**: ~$2.76/GB ingested. 30-day retention is free.
- Estimated dev cost with light usage: **< $10/month** (excluding model token costs).
