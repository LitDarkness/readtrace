[CmdletBinding()]
param(
    [string]$Version = "",
    [string]$OutputRoot = "dist",
    [string]$TesseractBin = "",
    [string]$PopplerBin = "",
    [string]$TessdataRoot = "",
    [string]$TessdataRef = "4.1.0"
)

$ErrorActionPreference = "Stop"

$projectRoot = [IO.Path]::GetFullPath(
    (Join-Path $PSScriptRoot "..")
)

Set-Location $projectRoot

if ([string]::IsNullOrWhiteSpace($Version)) {
    $Version = (& git describe --tags --always --dirty 2>$null)

    if ([string]::IsNullOrWhiteSpace($Version)) {
        $Version = "dev"
    }
}

$Version = $Version.Trim() -replace '[^A-Za-z0-9._-]', '-'

$distRoot = if ([IO.Path]::IsPathRooted($OutputRoot)) {
    [IO.Path]::GetFullPath($OutputRoot)
} else {
    [IO.Path]::GetFullPath(
        (Join-Path $projectRoot $OutputRoot)
    )
}

$stage = Join-Path `
    $distRoot `
    "readtrace-$Version-windows-x86_64"

$archive = Join-Path `
    $distRoot `
    "readtrace-$Version-windows-x86_64.zip"


function Resolve-Tool {
    param(
        [string]$Requested,
        [string]$Name
    )

    if (-not [string]::IsNullOrWhiteSpace($Requested)) {
        $candidate = [IO.Path]::GetFullPath($Requested)

        if (-not (Test-Path -LiteralPath $candidate -PathType Leaf)) {
            throw "$Name not found at $candidate"
        }

        return $candidate
    }

    $command = Get-Command $Name -ErrorAction SilentlyContinue

    if ($null -eq $command) {
        throw "$Name was not found."
    }

    return $command.Source
}


function Get-ToolVersion {
    param(
        [string]$Executable,
        [string]$Arguments
    )

    try {
        $startInfo = New-Object System.Diagnostics.ProcessStartInfo

        $startInfo.FileName = $Executable
        $startInfo.Arguments = $Arguments
        $startInfo.UseShellExecute = $false
        $startInfo.CreateNoWindow = $true
        $startInfo.RedirectStandardOutput = $true
        $startInfo.RedirectStandardError = $true

        $process = New-Object System.Diagnostics.Process
        $process.StartInfo = $startInfo

        try {
            [void]$process.Start()

            $stdout = $process.StandardOutput.ReadToEnd()
            $stderr = $process.StandardError.ReadToEnd()

            $process.WaitForExit()

            $combined = "$stdout`n$stderr"

            $firstLine = $combined -split "\r?\n" |
                Where-Object {
                    -not [string]::IsNullOrWhiteSpace($_)
                } |
                Select-Object -First 1

            if (-not [string]::IsNullOrWhiteSpace($firstLine)) {
                return $firstLine.Trim()
            }
        }
        finally {
            $process.Dispose()
        }
    }
    catch {
        Write-Warning (
            "Could not determine version of {0}: {1}" `
                -f $Executable, $_.Exception.Message
        )
    }

    return "unknown"
}


if (Test-Path -LiteralPath $stage) {
    Remove-Item `
        -LiteralPath $stage `
        -Recurse `
        -Force
}

New-Item `
    -ItemType Directory `
    -Force `
    -Path `
        $stage,
        "$stage\tools\tesseract",
        "$stage\tools\poppler",
        "$stage\LICENSES" |
    Out-Null


# Build Rust binary with the MSVC CRT statically linked.
$previousRustFlags = $env:RUSTFLAGS

if ([string]::IsNullOrWhiteSpace($previousRustFlags)) {
    $env:RUSTFLAGS = "-C target-feature=+crt-static"
}
elseif ($previousRustFlags -notmatch 'target-feature=\+crt-static') {
    $env:RUSTFLAGS = (
        "$previousRustFlags -C target-feature=+crt-static"
    )
}

& cargo build --release --locked

if ($LASTEXITCODE -ne 0) {
    throw "cargo build failed with exit code $LASTEXITCODE"
}

if ($null -eq $previousRustFlags) {
    Remove-Item Env:RUSTFLAGS -ErrorAction SilentlyContinue
}
else {
    $env:RUSTFLAGS = $previousRustFlags
}


$readtrace = Join-Path `
    $projectRoot `
    "target\release\readtrace-cli.exe"

if (-not (Test-Path -LiteralPath $readtrace -PathType Leaf)) {
    throw "Release binary not found: $readtrace"
}

Copy-Item `
    -LiteralPath $readtrace `
    -Destination (Join-Path $stage "readtrace.exe")


$tesseract = Resolve-Tool `
    $TesseractBin `
    "tesseract.exe"

$pdftoppm = Resolve-Tool `
    $PopplerBin `
    "pdftoppm.exe"

$pdfinfo = Join-Path `
    (Split-Path $pdftoppm -Parent) `
    "pdfinfo.exe"

if (-not (Test-Path -LiteralPath $pdfinfo -PathType Leaf)) {
    throw "pdfinfo.exe must be beside pdftoppm.exe: $pdfinfo"
}


$tessDir = Split-Path $tesseract -Parent
$popplerDir = Split-Path $pdftoppm -Parent


# Copy the complete executable directories so their Windows DLL
# dependencies remain beside the executables.
Get-ChildItem -LiteralPath $tessDir |
    Copy-Item `
        -Destination "$stage\tools\tesseract" `
        -Recurse `
        -Force

Get-ChildItem -LiteralPath $popplerDir |
    Copy-Item `
        -Destination "$stage\tools\poppler" `
        -Recurse `
        -Force


# Poppler distributions may store runtime data one or two levels
# above bin/.
$popplerShare = Join-Path `
    (Split-Path $popplerDir -Parent) `
    "share"

if (Test-Path -LiteralPath $popplerShare -PathType Container) {
    Copy-Item `
        -LiteralPath $popplerShare `
        -Destination "$stage\tools\poppler\share" `
        -Recurse `
        -Force
}

$popplerDistRoot = Split-Path `
    (Split-Path $popplerDir -Parent) `
    -Parent

$popplerDistShare = Join-Path `
    $popplerDistRoot `
    "share"

if (Test-Path -LiteralPath $popplerDistShare -PathType Container) {
    New-Item `
        -ItemType Directory `
        -Force `
        -Path "$stage\tools\poppler\share" |
        Out-Null

    Get-ChildItem -LiteralPath $popplerDistShare |
        Copy-Item `
            -Destination "$stage\tools\poppler\share" `
            -Recurse `
            -Force
}


# Ensure exactly the OCR languages ReadTrace requires are available.
$stageTessdata = Join-Path `
    $stage `
    "tools\tesseract\tessdata"

New-Item `
    -ItemType Directory `
    -Force `
    -Path $stageTessdata |
    Out-Null


if (-not [string]::IsNullOrWhiteSpace($TessdataRoot)) {
    $sourceTessdata = [IO.Path]::GetFullPath(
        $TessdataRoot
    )

    if (-not (
        Test-Path `
            -LiteralPath $sourceTessdata `
            -PathType Container
    )) {
        throw "Tessdata directory not found: $sourceTessdata"
    }

    Get-ChildItem `
        -LiteralPath $sourceTessdata `
        -Filter "*.traineddata" |
        Copy-Item `
            -Destination $stageTessdata `
            -Force
}


foreach ($language in @("chi_sim", "eng")) {
    $languageFile = Join-Path `
        $stageTessdata `
        "$language.traineddata"

    if (-not (
        Test-Path `
            -LiteralPath $languageFile `
            -PathType Leaf
    )) {
        $url = (
            "https://raw.githubusercontent.com/" +
            "tesseract-ocr/tessdata_fast/" +
            "$TessdataRef/$language.traineddata"
        )

        Invoke-WebRequest `
            -Uri $url `
            -OutFile $languageFile
    }
}


foreach ($language in @("chi_sim", "eng")) {
    $languageFile = Join-Path `
        $stageTessdata `
        "$language.traineddata"

    if (-not (
        Test-Path `
            -LiteralPath $languageFile `
            -PathType Leaf
    )) {
        throw "$language.traineddata is missing from the release"
    }
}


# Collect third-party licenses when available.
$licenseSources = @(
    @{
        Prefix = "tesseract"
        Root = $tessDir
    },
    @{
        Prefix = "poppler"
        Root = $popplerDir
    },
    @{
        Prefix = "poppler-package"
        Root = (Split-Path $popplerDir -Parent)
    },
    @{
        Prefix = "poppler-distribution"
        Root = $popplerDistRoot
    }
)

foreach ($source in $licenseSources) {
    if (
        Test-Path `
            -LiteralPath $source.Root `
            -PathType Container
    ) {
        Get-ChildItem `
            -LiteralPath $source.Root `
            -File `
            -Recurse `
            -ErrorAction SilentlyContinue |
            Where-Object {
                $_.Name -match '^(LICENSE|COPYING|NOTICE)'
            } |
            ForEach-Object {
                $destination = Join-Path `
                    "$stage\LICENSES" `
                    "$($source.Prefix)-$($_.Name)"

                Copy-Item `
                    -LiteralPath $_.FullName `
                    -Destination $destination `
                    -Force
            }
    }
}


Copy-Item `
    -LiteralPath (Join-Path $projectRoot "LICENSE") `
    -Destination "$stage\LICENSES\readtrace-MIT.txt" `
    -Force

if (
    Test-Path `
        -LiteralPath (Join-Path $projectRoot "LICENSES") `
        -PathType Container
) {
    Get-ChildItem `
        -LiteralPath (Join-Path $projectRoot "LICENSES") `
        -File |
        Where-Object {
            $_.Name -ne "README.md"
        } |
        Copy-Item `
            -Destination "$stage\LICENSES" `
            -Force
}


Copy-Item `
    -LiteralPath (Join-Path $projectRoot "THIRD_PARTY_NOTICES.md") `
    -Destination "$stage\THIRD_PARTY_NOTICES.md" `
    -Force

Copy-Item `
    -LiteralPath (Join-Path $projectRoot "README.md") `
    -Destination "$stage\README.md" `
    -Force

Copy-Item `
    -LiteralPath (Join-Path $projectRoot "docs\QUICK_START.md") `
    -Destination "$stage\QUICK_START.md" `
    -Force

Copy-Item `
    -LiteralPath (Join-Path $projectRoot ".env.example") `
    -Destination "$stage\.env.example" `
    -Force


# Do not let version-reporting quirks from native programs abort
# an otherwise valid release.
$tesseractVersion = Get-ToolVersion `
    $tesseract `
    "--version"

$popplerVersion = Get-ToolVersion `
    $pdftoppm `
    "-v"


@{
    version = $Version
    target = "windows-x86_64"
    readtrace = "readtrace.exe"
    tesseract = $tesseractVersion
    poppler = $popplerVersion
    tessdata_ref = $TessdataRef
    packaged_at_utc = [DateTime]::UtcNow.ToString("o")
} |
    ConvertTo-Json -Depth 4 |
    Set-Content `
        -Encoding UTF8 `
        (Join-Path $stage "release-manifest.json")


New-Item `
    -ItemType Directory `
    -Force `
    -Path $distRoot |
    Out-Null

if (Test-Path -LiteralPath $archive) {
    Remove-Item `
        -LiteralPath $archive `
        -Force
}


Compress-Archive `
    -Path "$stage\*" `
    -DestinationPath $archive `
    -CompressionLevel Optimal

Write-Output $archive