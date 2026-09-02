param(
    [string]$Project = ".\tests\image-flow-vault",
    [string]$Image = "E:\AI_diary\tests\1.png",
    [switch]$UseFolder,
    [switch]$RealOcr,
    [switch]$ReviewOnly,
    # Kept for compatibility; automatic application is now the default.
    [switch]$AcceptAll,
    [switch]$NoCopy,
    [string]$PromptFile = "",
    [string]$TesseractBin = "tesseract",
    [string]$Query = "沉默",
    [ValidateSet("mock", "http", "codex-cli")]
    [string]$LlmProvider = "mock",
    [string]$Preset = "",
    [string]$Model = "",
    [ValidateSet("default", "none", "low", "medium", "high")]
    [string]$Thinking = "",
    [ValidateSet("low", "mid", "high")]
    [string]$Speed = "",
    [int]$TimeoutSeconds = 300
)

$ErrorActionPreference = "Stop"
# PowerShell otherwise lets a failed native `cargo run` continue, which can
# make a failed build look like a successful end-to-end demo.
$PSNativeCommandUseErrorActionPreference = $true

if ($TimeoutSeconds -lt 1) {
    throw "TimeoutSeconds must be positive"
}
$env:READTRACE_TIMEOUT_SECONDS = "$TimeoutSeconds"

if (-not (Test-Path -LiteralPath $Image -PathType Leaf)) {
    throw "image sample not found: $Image"
}

if (-not (Test-Path -LiteralPath (Join-Path $Project "metadata.json"))) {
    & cargo run --quiet -p readtrace-cli -- init $Project
}

$importArgs = if ($UseFolder) {
    @("import-folder", $Project, (Split-Path -Parent $Image), "--mode", "generic", "--order", "filename")
} else {
    @("import-file", $Project, $Image, "--mode", "generic")
}
if ($NoCopy) { $importArgs += "--no-copy" }
$batchText = (& cargo run --quiet -p readtrace-cli -- --format json @importArgs) -join "`n"
$batch = $batchText | ConvertFrom-Json
$batchId = $batch.batch_id
Write-Host "Imported batch: $batchId"

$ocrProvider = if ($RealOcr) { "real" } else { "mock" }
if ($RealOcr -and $TesseractBin -eq "tesseract") {
    $localTesseract = Join-Path $PSScriptRoot "..\tmp\tesseract\tesseract.exe"
    if (Test-Path -LiteralPath $localTesseract -PathType Leaf) {
        $TesseractBin = (Resolve-Path -LiteralPath $localTesseract).Path
    }
}
if ($RealOcr -and $TesseractBin -ne "tesseract") {
    if (-not (Test-Path -LiteralPath $TesseractBin -PathType Leaf)) {
        throw "tesseract executable not found: $TesseractBin"
    }
    $tesseractDir = Split-Path -Parent (Resolve-Path -LiteralPath $TesseractBin)
    $env:PATH = "$tesseractDir;$env:PATH"
    $tessdata = Join-Path $tesseractDir "tessdata"
    if (Test-Path -LiteralPath $tessdata -PathType Container) {
        $env:TESSDATA_PREFIX = $tessdata
    }
}
& cargo run --quiet -p readtrace-cli -- ocr $Project $batchId --provider $ocrProvider
$proposeArgs = @("repair", $Project, $batchId, "--provider", $LlmProvider)
if (-not [string]::IsNullOrWhiteSpace($Preset)) {
    $proposeArgs += @("--preset", $Preset)
}
if (-not [string]::IsNullOrWhiteSpace($Model)) {
    $proposeArgs += @("--model", $Model)
}
if (-not [string]::IsNullOrWhiteSpace($Thinking)) {
    $proposeArgs += @("--thinking", $Thinking)
}
if (-not [string]::IsNullOrWhiteSpace($Speed)) { $proposeArgs += @("--speed", $Speed) }
if (-not [string]::IsNullOrWhiteSpace($PromptFile)) { $proposeArgs += @("--prompt-file", $PromptFile) }
& cargo run --quiet -p readtrace-cli -- @proposeArgs

$repairPath = Join-Path $Project "generated\$batchId\repair.json"
$repair = Get-Content -LiteralPath $repairPath -Raw | ConvertFrom-Json
Write-Host "Repaired pages: $($repair.pages.Count); errors: $($repair.errors.Count)"

if (-not $ReviewOnly -or $AcceptAll) {
    if (@($batch.source_files).Count -gt 1) {
        Write-Host "Multiple sources detected; accepting the generated merge plan."
        & cargo run --quiet -p readtrace-cli -- merge $Project $batchId --confirm
    } else {
        & cargo run --quiet -p readtrace-cli -- build $Project $batchId
    }
} else {
    Write-Host "Review-only mode; inspect generated\$batchId\repair and run:"
    Write-Host "  cargo run -p readtrace-cli -- build $Project $batchId"
}

& cargo run --quiet -p readtrace-cli -- reindex $Project
& cargo run --quiet -p readtrace-cli -- search $Project $Query
& cargo run --quiet -p readtrace-cli -- answer $Project $Query --provider mock
& cargo run --quiet -p readtrace-cli -- usage $Project --batch-id $batchId

Write-Host "Project: $((Resolve-Path -LiteralPath $Project).Path)"
Write-Host "Raw OCR: $((Join-Path $Project "raw\$batchId"))"
Write-Host "Generated artifacts: $((Join-Path $Project "generated\$batchId"))"
