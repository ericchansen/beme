// beme — Azure infrastructure orchestrator
targetScope = 'resourceGroup'

// ── Parameters ────────────────────────────────────────────────────────────────

@description('Azure region for all resources')
param location string = 'eastus2'

@description('Environment name used as resource prefix (e.g. dev, staging, prod)')
param environmentName string = 'dev'

@description('Project name used in resource naming')
param projectName string = 'beme'

// ── Variables ─────────────────────────────────────────────────────────────────

var foundryName = '${environmentName}-${projectName}-ai'
var logAnalyticsName = '${environmentName}-${projectName}-logs'
var tags = {
  project: projectName
  environment: environmentName
}

// ── Modules ───────────────────────────────────────────────────────────────────

module foundry 'modules/foundry.bicep' = {
  name: 'foundry'
  params: {
    foundryName: foundryName
    location: location
    tags: tags
  }
}

module monitoring 'modules/monitoring.bicep' = {
  name: 'monitoring'
  params: {
    logAnalyticsName: logAnalyticsName
    location: location
    foundryResourceId: foundry.outputs.id
    tags: tags
  }
}

// ── Outputs ───────────────────────────────────────────────────────────────────

@description('Foundry endpoint URL')
output foundryEndpoint string = foundry.outputs.endpoint

@description('Foundry resource name')
output foundryName string = foundry.outputs.name

@description('Log Analytics workspace name')
output logAnalyticsName string = monitoring.outputs.workspaceName
