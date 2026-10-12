param(
    [string] $Version = $env:MESHY_DCC_VERSION,
    [string] $InstallDir = $env:MESHY_DCC_INSTALL_DIR
)

# Installs meshy-dcc and its gateway from the Meshy release channel: checks the
# release manifest's URLs and SHA-256 digests before replacing an existing
# meshy-dcc, then installs the meshy-dcc Skill for the agents it finds.

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"
$officialReleases = "https://cdn.meshy.ai/meshy-dcc-core"
$platform = "windows-x86_64"

if ([string]::IsNullOrWhiteSpace($Version)) {
    $Version = "latest"
}
if ([string]::IsNullOrWhiteSpace($InstallDir)) {
    $InstallDir = Join-Path $HOME ".local\bin"
}

if ($Version -eq "latest") {
    $requestedVersion = $null
    $manifestUrl = "$officialReleases/latest/meshy-dcc-update-manifest-$platform.json"
} else {
    if ($Version -notmatch '^v?([0-9]+\.[0-9]+\.[0-9]+([.+-][0-9A-Za-z.-]+)?)$') {
        throw "Invalid release version: $Version"
    }
    $requestedVersion = $Matches[1]
    $manifestUrl = "$officialReleases/v$requestedVersion/meshy-dcc-update-manifest-$platform.json"
}

New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
$manifestTmp = Join-Path $InstallDir (".meshy-dcc-manifest-" + [System.Guid]::NewGuid() + ".json")
$binaries = @("meshy-dcc", "meshy-dcc-server")
$downloads = @{}
foreach ($binary in $binaries) {
    $downloads[$binary] = Join-Path $InstallDir (".$binary-" + [System.Guid]::NewGuid() + ".exe")
}

try {
    Write-Host "Downloading $manifestUrl"
    Invoke-WebRequest -Uri $manifestUrl -OutFile $manifestTmp
    $manifestText = Get-Content -LiteralPath $manifestTmp -Raw
    try {
        $manifest = $manifestText | ConvertFrom-Json
    } catch {
        throw "Official release manifest is not valid JSON: $($_.Exception.Message)"
    }

    # Both are verified before either replaces an installed file.
    foreach ($binary in $binaries) {
        if ([regex]::Matches($manifestText, '"' + $binary + '"\s*:').Count -ne 1) {
            throw "Official release manifest must contain one $binary entry"
        }
        $entry = $manifest.$binary
        $manifestVersion = [string] $entry.version
        $assetUrl = [string] $entry.url
        $expectedSha256 = [string] $entry.sha256
        if ($manifestVersion -notmatch '^[0-9]+\.[0-9]+\.[0-9]+([.+-][0-9A-Za-z.-]+)?$') {
            throw "Official release manifest contains an invalid $binary version"
        }
        if ($Version -ne "latest" -and $manifestVersion -ne $requestedVersion) {
            throw "Official release manifest version does not match requested version $requestedVersion"
        }
        $expectedUrl = "$officialReleases/v$manifestVersion/$binary-$platform.exe"
        if ($assetUrl -cne $expectedUrl) {
            throw "Official release manifest contains a non-official $binary URL"
        }
        if ($expectedSha256 -notmatch '^[0-9A-Fa-f]{64}$') {
            throw "Official release manifest contains an invalid SHA-256"
        }

        Write-Host "Downloading $assetUrl"
        Invoke-WebRequest -Uri $assetUrl -OutFile $downloads[$binary]
        $actualSha256 = (Get-FileHash -LiteralPath $downloads[$binary] -Algorithm SHA256).Hash
        if ($actualSha256 -ine $expectedSha256) {
            throw "$binary SHA-256 does not match the official release manifest"
        }
    }

    foreach ($binary in @("meshy-dcc-server", "meshy-dcc")) {
        $target = Join-Path $InstallDir "$binary.exe"
        if (Test-Path -LiteralPath $target) {
            [System.IO.File]::Replace($downloads[$binary], $target, $null)
        } else {
            [System.IO.File]::Move($downloads[$binary], $target)
        }
    }
} finally {
    foreach ($path in @($manifestTmp) + @($downloads.Values)) {
        if (Test-Path -LiteralPath $path) {
            Remove-Item -LiteralPath $path -Force
        }
    }
}

$cli = Join-Path $InstallDir "meshy-dcc.exe"
Write-Host "Installed verified meshy-dcc $manifestVersion to $InstallDir"

Write-Host "Installing the meshy-dcc Skill for the agents on this machine:"
Start-Process -FilePath $cli -ArgumentList @("--output", "human", "agent-skill") -Wait -NoNewWindow
$skill = Start-Process -FilePath $cli -ArgumentList @(
    "--output", "human", "agent-skill", "--execute"
) -Wait -PassThru -NoNewWindow
if ($skill.ExitCode -ne 0) {
    Write-Warning "The meshy-dcc Skill was not installed for every agent; run: meshy-dcc agent-skill"
}

$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
$pathParts = @()
if ($userPath) {
    $pathParts = $userPath -split ";"
}
if ($pathParts -notcontains $InstallDir) {
    $newPath = if ($userPath) { "$userPath;$InstallDir" } else { $InstallDir }
    [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
    Write-Host "Added $InstallDir to the user PATH. Open a new terminal before running meshy-dcc."
}
