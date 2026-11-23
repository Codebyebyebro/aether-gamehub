<#
Starts Aether Hub the way "start with Windows" does (quietly, in the tray) against a throwaway data folder whose saved
settings have the tray switched on, and checks three things:

  1. a normal start shows a window (so "no window" in the next check means something on this machine),
  2. a quiet start keeps the process running with no window on screen,
  3. starting Aether Hub again brings the hidden window back (the way out if the tray icon is hard to find).

It cannot look at the notification area, click the icon, or press the shortcut: those need a person. Nothing here
touches the real library (AETHER_DATA_DIR points at a temporary folder).

  powershell -File scripts\smoke-tray-windows.ps1 -Exe "C:\path\to\Aether Hub.exe" [-Seconds 15]

Exit code 0 means every check that could be made passed. When run by GitHub Actions the lines go to the run summary.
#>
param(
  [Parameter(Mandatory = $true)][string]$Exe,
  [int]$Seconds = 15
)
$ErrorActionPreference = 'Stop'
function Report([string]$line) {
  Write-Output $line
  if ($env:GITHUB_STEP_SUMMARY) { Add-Content -Path $env:GITHUB_STEP_SUMMARY -Value $line }
}
Report '### Start-up check: quiet start in the tray'
if (-not (Test-Path -LiteralPath $Exe)) { Report "- FAIL: not found: $Exe"; exit 1 }

# Windows reports a window for the process only while it is visible, so this is what "on screen" means here.
function Shown([System.Diagnostics.Process]$p) { $p.Refresh(); return ($p.MainWindowHandle -ne 0 -and $p.MainWindowTitle -like '*Aether Hub*') }
function Fresh([bool]$tray) {
  $dir = Join-Path ([IO.Path]::GetTempPath()) ('aether-smoke-' + [guid]::NewGuid().ToString('N'))
  $app = Join-Path $dir 'com.aetherhub.launcher'
  New-Item -ItemType Directory -Force (Join-Path $app 'covers') | Out-Null
  $json = '{"games":[],"settings":{"onboarded":true,"keepInTray":' + $tray.ToString().ToLower() + '}}'
  Set-Content -LiteralPath (Join-Path $app 'library.json') -Value $json -Encoding UTF8
  return $dir
}
$failed = $false
$started = New-Object System.Collections.Generic.List[System.Diagnostics.Process]
try {
  # 1. Control: a normal start must show a window, otherwise this machine cannot tell shown from hidden.
  $env:AETHER_DATA_DIR = Fresh $false
  $normal = Start-Process -FilePath $Exe -PassThru; $null = $normal.Handle; $started.Add($normal)
  Start-Sleep -Seconds $Seconds
  $canSee = Shown $normal
  if ($normal.HasExited) { Report "- FAIL: a normal start exited early (exit code $($normal.ExitCode))"; $failed = $true }
  elseif ($canSee) { Report "- PASS: a normal start shows a window titled '$($normal.MainWindowTitle)'" }
  else { Report '- INFO: a normal start shows no window this machine can see, so the hidden-window checks below cannot be judged here' }
  Stop-Process -Id $normal.Id -Force -ErrorAction SilentlyContinue; Start-Sleep -Seconds 2

  # 2. Quiet start with the tray on: alive, nothing on screen.
  $env:AETHER_DATA_DIR = Fresh $true
  $quiet = Start-Process -FilePath $Exe -ArgumentList '--tray' -PassThru; $null = $quiet.Handle; $started.Add($quiet)
  Start-Sleep -Seconds $Seconds
  if ($quiet.HasExited) { Report "- FAIL: a quiet start exited early (exit code $($quiet.ExitCode))"; $failed = $true }
  else {
    Report "- PASS: a quiet start is still running after $Seconds s"
    if ($canSee) {
      if (Shown $quiet) { Report '- FAIL: a quiet start with the tray on still shows a window (the tray icon was not created, or hiding did not work)'; $failed = $true }
      else { Report '- PASS: a quiet start with the tray on shows no window' }
    }

    # 3. Starting again must show the hidden window.
    $again = Start-Process -FilePath $Exe -PassThru; $null = $again.Handle; $started.Add($again)
    Start-Sleep -Seconds 6
    if ($canSee) {
      if (Shown $quiet) { Report '- PASS: starting Aether Hub again brought the hidden window back' }
      else { Report '- FAIL: starting Aether Hub again did not bring the hidden window back'; $failed = $true }
    }
    if ($again.HasExited) { Report "- PASS: the second launch handed over and exited (exit code $($again.ExitCode))" }
    else { Report '- INFO: the second launch was still running after 6 s'; }
  }
} finally {
  Remove-Item Env:\AETHER_DATA_DIR -ErrorAction SilentlyContinue
  foreach ($p in $started) { if (-not $p.HasExited) { Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue } }
}
Report '- NOT CHECKED: the tray icon itself, its menu, the shortcut and the controller wake-up (they need a person at a PC)'
if ($failed) { exit 1 }
