param(
    [string] $Version = $env:MESHY_DCC_VERSION,
    [string] $InstallDir = $env:MESHY_DCC_INSTALL_DIR
)

# Installs meshy-dcc and its gateway from the Meshy CDN, or from the GitHub
# Release when the CDN cannot be reached (meshy.ai is not reachable everywhere):
# checks the release manifest's URLs and SHA-256 digests before replacing an
# existing meshy-dcc, then installs the meshy-dcc Skill for the agents it finds.
# A download that fails moves on to the next source; a failed check stops.

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"
$sources = [ordered]@{
    cdn = "https://cdn.meshy.ai/meshy-dcc-core"
    github = "https://github.com/meshy-dev/meshy-dcc-core/releases"
}
$platform = "windows-x86_64"

if ([string]::IsNullOrWhiteSpace($Version)) {
    $Version = "latest"
}
if ([string]::IsNullOrWhiteSpace($InstallDir)) {
    $InstallDir = Join-Path $HOME ".local\bin"
}

# The CDN keeps every release under a version dir and never overwrites it, so
# the latest version comes from the GitHub Release redirect:
# releases/latest -> releases/tag/v<version>.
if ($Version -eq "latest") {
    $latest = $null
    try {
        $request = [System.Net.WebRequest]::Create("$($sources.github)/latest")
        $request.Method = "HEAD"
        $request.Timeout = 15000
        $response = $request.GetResponse()
        $latest = $response.ResponseUri.AbsoluteUri
        $response.Close()
    } catch {}
    if ($latest -notmatch '/releases/tag/(v[^/]+)$') {
        throw "Could not find the latest release on GitHub; set MESHY_DCC_VERSION to install a given version"
    }
    $Version = $Matches[1]
}
if ($Version -notmatch '^v?([0-9]+\.[0-9]+\.[0-9]+([.+-][0-9A-Za-z.-]+)?)$') {
    throw "Invalid release version: $Version"
}
$requestedVersion = $Matches[1]

# CDN names follow the CDN rules (lowercase letters, digits, hyphens): the
# version dir for 0.2.0 is v0-2-0 and the platform is windows-x64. GitHub
# keeps the release names.
$cdnDir = "$($sources.cdn)/v" + ($requestedVersion.ToLowerInvariant() -replace '[^0-9a-z]+', '-')
$githubDir = "$($sources.github)/download/v$requestedVersion"

# Release-Url <source> <name>: where <name> for this platform lives.
function Get-ReleaseUrl([string] $source, [string] $name, [string] $extension) {
    if ($source -eq "cdn") { return "$cdnDir/$name-windows-x64$extension" }
    return "$githubDir/$name-$platform$extension"
}

# A failed download is a NetworkFailure, so the caller tries the next source.
class NetworkFailure : System.Exception {
    NetworkFailure([string] $message) : base($message) {}
}
function Get-File([string] $uri, [string] $outFile) {
    Write-Host "Downloading $uri"
    try {
        Invoke-WebRequest -Uri $uri -OutFile $outFile -TimeoutSec 60
    } catch {
        throw [NetworkFailure]::new("download failed: $uri")
    }
}

New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
$manifestTmp = Join-Path $InstallDir (".meshy-dcc-manifest-" + [System.Guid]::NewGuid() + ".json")
$binaries = @("meshy-dcc", "meshy-dcc-server")
$downloads = @{}
foreach ($binary in $binaries) {
    $downloads[$binary] = Join-Path $InstallDir (".$binary-" + [System.Guid]::NewGuid() + ".exe")
}

function Get-VerifiedBinaries([string] $source) {
    Get-File (Get-ReleaseUrl $source "meshy-dcc-update-manifest" ".json") $manifestTmp
    $manifestText = Get-Content -LiteralPath $manifestTmp -Raw
    try {
        $manifest = $manifestText | ConvertFrom-Json
    } catch {
        throw "Official release manifest is not valid JSON: $($_.Exception.Message)"
    }

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
        if ($manifestVersion -ne $requestedVersion) {
            throw "Official release manifest version does not match requested version $requestedVersion"
        }
        $expectedUrl = Get-ReleaseUrl $source $binary ".exe"
        if ($assetUrl -cne $expectedUrl) {
            throw "Official release manifest contains a non-official $binary URL"
        }
        if ($expectedSha256 -notmatch '^[0-9A-Fa-f]{64}$') {
            throw "Official release manifest contains an invalid SHA-256"
        }

        Get-File $assetUrl $downloads[$binary]
        $actualSha256 = (Get-FileHash -LiteralPath $downloads[$binary] -Algorithm SHA256).Hash
        if ($actualSha256 -ine $expectedSha256) {
            throw "$binary SHA-256 does not match the official release manifest"
        }
    }
    return $manifestVersion
}

try {
    # Both are verified before either replaces an installed file.
    $manifestVersion = $null
    foreach ($source in $sources.Keys) {
        try {
            $manifestVersion = Get-VerifiedBinaries $source
            break
        } catch [NetworkFailure] {
            Write-Warning "Could not download from $source; trying the next source."
        }
    }
    if (-not $manifestVersion) {
        throw "meshy-dcc could not be downloaded from the Meshy CDN or GitHub"
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
