// Log Analytics workspace + diagnostic settings for Foundry

@description('Name of the Log Analytics workspace')
param logAnalyticsName string

@description('Azure region for deployment')
param location string

@description('Resource ID of the Foundry resource to monitor')
param foundryResourceId string

@description('Tags to apply to all resources')
param tags object = {}

// ── Log Analytics Workspace ───────────────────────────────────────────────────

resource logAnalytics 'Microsoft.OperationalInsights/workspaces@2023-09-01' = {
  name: logAnalyticsName
  location: location
  tags: tags
  properties: {
    sku: {
      name: 'PerGB2018'
    }
    retentionInDays: 30
  }
}

// ── Diagnostic Settings ───────────────────────────────────────────────────────

resource diagnosticSettings 'Microsoft.Insights/diagnosticSettings@2021-05-01-preview' = {
  name: '${logAnalyticsName}-diag'
  scope: foundry
  properties: {
    workspaceId: logAnalytics.id
    logs: [
      {
        category: 'Audit'
        enabled: true
      }
      {
        category: 'RequestResponse'
        enabled: true
      }
      {
        category: 'Trace'
        enabled: true
      }
    ]
  }
}

// Reference the existing Foundry resource by its ID
resource foundry 'Microsoft.CognitiveServices/accounts@2025-04-01-preview' existing = {
  name: last(split(foundryResourceId, '/'))
}

// ── Outputs ───────────────────────────────────────────────────────────────────

@description('Log Analytics workspace ID')
output workspaceId string = logAnalytics.id

@description('Log Analytics workspace name')
output workspaceName string = logAnalytics.name
