<#
Starts an Aether Hub executable, checks that it is still running after a few seconds, takes a rough memory reading
and stops it again. It is a start-up check, not a benchmark and not a test of any feature.

  powershell -File scripts\smoke-windows.ps1 -Exe "C:\path\to\Aether Hub.exe" [-Label portable] [-Seconds 20]

Exit code 0 means the app stayed up for the whole wait; 1 means it exited early or could not be started.
When run by GitHub Actions the result lines are also added to the run summary.
#>
param(
  [Parameter(Mandatory = $true)][string]$Exe,
  [string]$Label = 'app',
  [int]$Seconds = 20
)
$ErrorActionPreference = 'Stop'

function Report([string]$line) {
  Write-Output $line
  if ($env:GITHUB_STEP_SUMMARY) { Add-Content -Path $env:GITHUB_STEP_SUMMARY -Value $line }
}

Report "### Start-up check: $Label"
if (-not (Test-Path -LiteralPath $Exe)) { Report "- FAIL: not found: $Exe"; exit 1 }

$failed = $false
$p = Start-Process -FilePath $Exe -PassThru
$null = $p.Handle # keeps the exit code readable after the process ends
try {
  Start-Sleep -Seconds $Seconds
  $p.Refresh()
  if ($p.HasExited) {
    Report "- FAIL: exited within $Seconds s (exit code $($p.ExitCode))"
    $failed = $true
  } else {
    Report "- PASS: still running after $Seconds s"
    $web = @(Get-Process -Name msedgewebview2 -ErrorAction SilentlyContinue)
    $bytes = $p.WorkingSet64 + ($web | Measure-Object -Property WorkingSet64 -Sum).Sum
    Report "- INFO: about $([math]::Round($bytes / 1MB)) MB working set (app plus $($web.Count) WebView2 processes), empty library, idle. A rough single reading, not a benchmark."
    $data = Join-Path $env:APPDATA 'com.aetherhub.launcher'
    if (Test-Path -LiteralPath $data) { Report '- INFO: the data folder com.aetherhub.launcher exists' }
    else { Report '- INFO: no data folder yet (it may only be created on the first save)' }
  }
} finally {
  if (-not $p.HasExited) { Stop-Process -Id $p.Id -Force }
}
if ($failed) { exit 1 }
