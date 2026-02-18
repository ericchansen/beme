# Full E2E test: launch app, play audio, check for AI suggestion events.
#
# Prerequisites:
#   - Azure OpenAI credentials in environment (AZURE_OPENAI_ENDPOINT, etc.)
#   - Audio output device available (speakers/headphones)
#   - test-speech-24khz.wav fixture in tests/fixtures/
#
# Usage: pwsh -File src-tauri\tests\full_e2e.ps1

$ErrorActionPreference = "Stop"

$logFile = Join-Path $env:TEMP "beme-test-events.jsonl"
if (Test-Path $logFile) { Remove-Item $logFile }

# Set env vars for event logging and debugging
$env:BEME_TEST_LOG = $logFile
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=9222"
$env:RUST_LOG = "info"

$repoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$tauriDir = Join-Path $repoRoot "src-tauri"
$wavPath = Join-Path $PSScriptRoot "fixtures\test-speech-24khz.wav"

if (-not (Test-Path $wavPath)) {
    Write-Host "❌ FAIL: Test fixture not found at $wavPath"
    exit 1
}

# Build first
Write-Host "Building..."
Push-Location $tauriDir
cargo build 2>&1
Pop-Location

# Launch app in background
Write-Host "Launching app..."
$app = Start-Process -FilePath "cargo" -ArgumentList "tauri dev" -PassThru -WorkingDirectory $repoRoot

try {
    # Wait for app to start
    Write-Host "Waiting 30s for app startup..."
    Start-Sleep -Seconds 30

    # Play audio through speakers so the app's mic can capture it
    Write-Host "Playing test audio: $wavPath"
    $player = New-Object System.Media.SoundPlayer($wavPath)
    $player.PlaySync()

    # Wait for Azure AI to process the captured audio
    Write-Host "Waiting 20s for AI to process..."
    Start-Sleep -Seconds 20

    # Check results
    if (Test-Path $logFile) {
        $lines = @(Get-Content $logFile)
        Write-Host "Found $($lines.Count) events in log:"
        $lines | ForEach-Object { Write-Host "  $_" }
        if ($lines.Count -gt 0) {
            Write-Host "✅ PASS: Audio suggestions were generated"
            exit 0
        } else {
            Write-Host "❌ FAIL: Log file exists but contains no events"
            exit 1
        }
    } else {
        Write-Host "❌ FAIL: Log file was not created ($logFile)"
        exit 1
    }
} finally {
    # Cleanup — stop the app regardless of outcome
    if ($app -and -not $app.HasExited) {
        Stop-Process -Id $app.Id -Force -ErrorAction SilentlyContinue
    }
}
