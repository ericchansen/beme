// Azure AI Foundry resource + model deployments

@description('Name of the Foundry resource')
param foundryName string

@description('Azure region for deployment')
param location string

@description('Tags to apply to all resources')
param tags object = {}

// ── Foundry Resource ──────────────────────────────────────────────────────────

resource foundry 'Microsoft.CognitiveServices/accounts@2025-04-01-preview' = {
  name: foundryName
  location: location
  tags: tags
  identity: {
    type: 'SystemAssigned'
  }
  sku: {
    name: 'S0'
  }
  kind: 'AIServices'
  properties: {
    allowProjectManagement: true
    customSubDomainName: foundryName
    publicNetworkAccess: 'Enabled'
  }
}

// ── Model Deployments ─────────────────────────────────────────────────────────

resource gpt4o 'Microsoft.CognitiveServices/accounts/deployments@2024-04-01-preview' = {
  parent: foundry
  name: 'gpt-4o'
  sku: {
    name: 'GlobalStandard'
    capacity: 80
  }
  properties: {
    model: {
      format: 'OpenAI'
      name: 'gpt-4o'
      version: '2024-11-20'
    }
  }
}

resource gpt4oRealtime 'Microsoft.CognitiveServices/accounts/deployments@2024-04-01-preview' = {
  parent: foundry
  name: 'gpt-4o-realtime-preview'
  sku: {
    name: 'GlobalStandard'
    capacity: 6
  }
  properties: {
    model: {
      format: 'OpenAI'
      name: 'gpt-4o-realtime-preview'
      version: '2024-12-17'
    }
  }
  dependsOn: [gpt4o]
}

// ── Outputs ───────────────────────────────────────────────────────────────────

@description('Foundry resource ID')
output id string = foundry.id

@description('Foundry resource name')
output name string = foundry.name

@description('Foundry endpoint URL')
output endpoint string = foundry.properties.endpoint

@description('Foundry principal ID (managed identity)')
output principalId string = foundry.identity.principalId
