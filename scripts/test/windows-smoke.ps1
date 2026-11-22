# windows-smoke.ps1 — non-destructive launch/capture check for Prism Shell.
#
# This script has NOT been executed in the development environment (Linux).
# Run it from an elevated-free PowerShell on Windows 11 x64.
#
# It starts prism-shell.exe, verifies that it and Explorer stay alive, captures
# the primary screen, copies the Prism log, and then stops only the Prism
# process. It does not hide or restart Explorer, modify the registry, or change
# startup configuration. Focus/transparency/activation still require the manual
# checklist in docs/WINDOWS_VALIDATION.md.

param(
    [string]$Exe = ".\prism-shell.exe",
    [string]$OutDir = ".\artifacts\windows-smoke",
    [int]$WarmupSeconds = 5
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path -LiteralPath $Exe)) {
    throw "Prism executable not found: $Exe"
}
$Exe = (Resolve-Path -LiteralPath $Exe).Path
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

$explorer = Get-Process -Name explorer -ErrorAction SilentlyContinue
if (-not $explorer) {
    throw "Explorer is not running; refusing to test in an abnormal shell state."
}

Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

$process = Start-Process -FilePath $Exe -WorkingDirectory (Split-Path -Parent $Exe) -PassThru
try {
    Start-Sleep -Seconds $WarmupSeconds
    $process.Refresh()
    if ($process.HasExited) {
        throw "Prism exited during warmup with code $($process.ExitCode)."
    }

    $stillThere = Get-Process -Name explorer -ErrorAction SilentlyContinue
    if (-not $stillThere) {
        throw "Explorer disappeared while Prism was running."
    }

    $bounds = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
    $bitmap = New-Object System.Drawing.Bitmap $bounds.Width, $bounds.Height
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    try {
        $graphics.CopyFromScreen($bounds.Location, [System.Drawing.Point]::Empty, $bounds.Size)
    } finally {
        $graphics.Dispose()
    }
    $shot = Join-Path $OutDir ("prism-smoke-" + (Get-Date -Format "yyyyMMdd-HHmmss") + ".png")
    $bitmap.Save($shot, [System.Drawing.Imaging.ImageFormat]::Png)
    $bitmap.Dispose()

    $logSource = Join-Path $env:LOCALAPPDATA "Prism\logs\prism-shell.log"
    if (Test-Path -LiteralPath $logSource) {
        Copy-Item -LiteralPath $logSource -Destination (Join-Path $OutDir "prism-shell.log") -Force
    } else {
        Write-Warning "Prism log not found at $logSource"
    }

    $report = @(
        "Prism smoke test",
        "Timestamp: $(Get-Date -Format o)",
        "OS: $([Environment]::OSVersion.VersionString)",
        "Executable: $Exe",
        "Process alive: $(-not $process.HasExited)",
        "Explorer alive: $([bool](Get-Process -Name explorer -ErrorAction SilentlyContinue))",
        "Primary screen: $($bounds.Width)x$($bounds.Height)",
        "Screenshot: $shot",
        "DpiX: $([System.Drawing.Graphics]::FromHwnd([IntPtr]::Zero).DpiX)",
        "DpiY: $([System.Drawing.Graphics]::FromHwnd([IntPtr]::Zero).DpiY)",
        "",
        "This is a launch/capture smoke test only. See docs/WINDOWS_VALIDATION.md."
    )
    $report | Set-Content -LiteralPath (Join-Path $OutDir "report.txt") -Encoding UTF8
    Write-Host "Smoke test completed. Artifacts: $OutDir"
} finally {
    if (-not $process.HasExited) {
        Stop-Process -Id $process.Id -Force
    }
}
