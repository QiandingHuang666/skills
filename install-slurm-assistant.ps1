[CmdletBinding()]
param(
    [ValidateSet("install", "uninstall")]
    [string]$Action = "install"
)

$ErrorActionPreference = "Stop"

$repoOwner = "QiandingHuang666"
$repoName = "skills"
$defaultInstallDir = Join-Path $HOME "AppData\Local\Programs\slurm-assistant\bin"
$installDir = if ($env:SLURM_ASSISTANT_INSTALL_DIR) { $env:SLURM_ASSISTANT_INSTALL_DIR } else { $defaultInstallDir }
$baseUrl = if ($env:SLURM_ASSISTANT_BASE_URL) { $env:SLURM_ASSISTANT_BASE_URL } else { "https://github.com/$repoOwner/$repoName/releases/latest/download" }
$scriptPath = $MyInvocation.MyCommand.Path
if ([string]::IsNullOrWhiteSpace($scriptPath)) {
    # Script invoked via iex from remote content has no physical path.
    $scriptDir = (Get-Location).Path
}
else {
    $scriptDir = Split-Path -Parent $scriptPath
}
$packageDir = Join-Path $scriptDir "package"

function Get-PlatformSuffix {
    switch ($env:PROCESSOR_ARCHITECTURE) {
        "AMD64" { return "windows-amd64" }
        default {
            throw "unsupported Windows architecture: $($env:PROCESSOR_ARCHITECTURE). supported architectures: AMD64"
        }
    }
}

function Get-SkillRoots {
    if ($env:SLURM_ASSISTANT_SKILL_ROOTS) {
        return ($env:SLURM_ASSISTANT_SKILL_ROOTS -split ';' | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
    }

    return @(
        (Join-Path $HOME ".codex\skills"),
        (Join-Path $HOME ".claude\skills"),
        (Join-Path $HOME ".openclaw\skills")
    )
}

function Install-BundledBinary {
    param(
        [Parameter(Mandatory = $true)]
        [string]$SourceName,
        [Parameter(Mandatory = $true)]
        [string]$TargetName
    )

    New-Item -ItemType Directory -Force -Path $installDir | Out-Null
    Copy-Item -Force (Join-Path $packageDir "bin\$SourceName") (Join-Path $installDir $TargetName)
}

function Install-BundledSkill {
    $skillSource = Join-Path $packageDir "skill\slurm-assistant"
    foreach ($skillRoot in Get-SkillRoots) {
        New-Item -ItemType Directory -Force -Path $skillRoot | Out-Null
        $target = Join-Path $skillRoot "slurm-assistant"
        Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $target
        Copy-Item -Recurse -Force $skillSource $target
        Write-Host "installed skill: $target"
    }
}

function Test-PathContains {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Candidate,
        [Parameter(Mandatory = $true)]
        [string]$PathValue
    )

    if ([string]::IsNullOrWhiteSpace($PathValue)) {
        return $false
    }

    $normalizedCandidate = [System.IO.Path]::GetFullPath($Candidate).TrimEnd('\')
    foreach ($entry in ($PathValue -split ';')) {
        if ([string]::IsNullOrWhiteSpace($entry)) {
            continue
        }
        $normalizedEntry = [System.IO.Path]::GetFullPath($entry).TrimEnd('\')
        if ($normalizedEntry -ieq $normalizedCandidate) {
            return $true
        }
    }
    return $false
}

function Install-FromBundle {
    Install-BundledBinary -SourceName "slurm-client.exe" -TargetName "slurm-client.exe"
    Install-BundledBinary -SourceName "slurm-server.exe" -TargetName "slurm-server.exe"
    Install-BundledSkill

    Write-Host ""
    Write-Host "installed binaries:"
    Write-Host "  $(Join-Path $installDir 'slurm-client.exe')"
    Write-Host "  $(Join-Path $installDir 'slurm-server.exe')"

    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    $machinePath = [Environment]::GetEnvironmentVariable("Path", "Machine")
    if (-not (Test-PathContains -Candidate $installDir -PathValue $userPath) -and -not (Test-PathContains -Candidate $installDir -PathValue $machinePath)) {
        Write-Host ""
        Write-Host "warning: $installDir is not in PATH"
        Write-Host "add it with:"
        Write-Host "  [Environment]::SetEnvironmentVariable('Path', `$env:Path + ';$installDir', 'User')"
    }
}

function Uninstall-All {
    $clientPath = Join-Path $installDir "slurm-client.exe"
    $serverPath = Join-Path $installDir "slurm-server.exe"

    Remove-Item -Force -ErrorAction SilentlyContinue $clientPath, $serverPath

    foreach ($skillRoot in Get-SkillRoots) {
        $target = Join-Path $skillRoot "slurm-assistant"
        Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $target
        Write-Host "removed skill: $target"
    }

    Write-Host ""
    Write-Host "removed binaries:"
    Write-Host "  $clientPath"
    Write-Host "  $serverPath"
}

function Invoke-DownloadedBundle {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RequestedAction
    )

    $suffix = Get-PlatformSuffix
    $archiveName = "slurm-assistant-$suffix.zip"
    $tempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ([System.Guid]::NewGuid().ToString("N"))
    $archivePath = Join-Path $tempRoot $archiveName
    $extractPath = Join-Path $tempRoot "extract"

    try {
        New-Item -ItemType Directory -Force -Path $extractPath | Out-Null
        Write-Host "downloading $archiveName"
        Invoke-WebRequest -Uri "$baseUrl/$archiveName" -OutFile $archivePath
        Expand-Archive -Path $archivePath -DestinationPath $extractPath -Force

        $bundleDir = Get-ChildItem -Path $extractPath -Directory | Select-Object -First 1
        if (-not $bundleDir) {
            throw "invalid package layout in $archiveName"
        }

        $bundleScript = Join-Path $bundleDir.FullName "install-slurm-assistant.ps1"
        if (-not (Test-Path $bundleScript)) {
            throw "invalid package layout in $archiveName"
        }

        if ($env:SLURM_ASSISTANT_SKILL_ROOTS) {
            & $bundleScript -Action $RequestedAction
        }
        else {
            & $bundleScript -Action $RequestedAction
        }
    }
    finally {
        Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $tempRoot
    }
}

if ((Test-Path (Join-Path $packageDir "bin")) -and (Test-Path (Join-Path $packageDir "skill\slurm-assistant"))) {
    switch ($Action) {
        "install" { Install-FromBundle }
        "uninstall" { Uninstall-All }
    }
}
else {
    Invoke-DownloadedBundle -RequestedAction $Action
}
