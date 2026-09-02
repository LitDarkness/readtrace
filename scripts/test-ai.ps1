[CmdletBinding()]
param(
    [ValidateSet('http', 'codex-cli', 'mock')]
    [string]$Provider = 'http',
    [string]$Preset,
    [string]$Model,
    [ValidateSet('default', 'none', 'low', 'medium', 'high')]
    [string]$Thinking,
    [ValidateRange(1, 600)]
    [int]$TimeoutSeconds = 20
)

$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$exitCode = 0
Push-Location $projectRoot
try {
    # Keep the probe bounded; this variable only affects this PowerShell
    # process and overrides the value in the project .env.
    $env:READTRACE_TIMEOUT_SECONDS = $TimeoutSeconds.ToString()
    $cargoArgs = @('run', '--quiet', '-p', 'readtrace-cli', '--', 'ai-check', '--provider', $Provider)
    if ($Preset) { $cargoArgs += @('--preset', $Preset) }
    if ($Model) { $cargoArgs += @('--model', $Model) }
    if ($Thinking) { $cargoArgs += @('--thinking', $Thinking) }
    & cargo @cargoArgs
    $exitCode = $LASTEXITCODE
}
finally {
    Pop-Location
}
exit $exitCode
