# Keep the Rust build folder (`target/`) from piling up.
#
# Why: a cold `cargo test --workspace --no-run` writes ~3.4 GB (measured 2026-11-22). The folder
# had nonetheless grown to 33.5 GB, because Cargo never deletes an old copy: every change of
# flags, features, toolchain or `-p` selection builds a new hashed copy of each test binary and a
# new incremental cache beside the old ones, and the old ones stay forever. Nothing in there is
# source; all of it rebuilds (a cold rebuild of the whole suite takes ~3-4 minutes).
#
# What it does: reports the folder's size, and runs `cargo clean` when it is over the limit.
# Picking stale copies out by hand is not attempted: which hash is live depends on the command
# that will run next, and a wrong guess costs a rebuild anyway.
#
# Usage (from anywhere):
#   pwsh -File W:\Claude_projects\battery\tools\trim-target.ps1            # clean if over 12 GB
#   pwsh -File W:\Claude_projects\battery\tools\trim-target.ps1 -LimitGB 8
#   pwsh -File W:\Claude_projects\battery\tools\trim-target.ps1 -ReportOnly
#
# Do not run it while a build, a test run, `sim-server` or the Godot demo is using `target/`:
# Windows will refuse to delete a running .exe or a loaded .dll and the clean stops half-way.
# After a clean, the Godot demo needs `cargo build -p sim-godot` again; `start-frontend.bat`
# rebuilds the server by itself.

param(
    [double]$LimitGB = 12,
    [switch]$ReportOnly
)

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$target = Join-Path $repo 'target'

if (-not (Test-Path $target)) {
    Write-Output "target/ does not exist: nothing to trim."
    exit 0
}

$bytes = (Get-ChildItem -LiteralPath $target -Recurse -Force -File -ErrorAction SilentlyContinue |
    Measure-Object Length -Sum).Sum
$gb = [math]::Round($bytes / 1GB, 2)
Write-Output "target/ is $gb GB (limit $LimitGB GB)."

if ($ReportOnly -or $gb -le $LimitGB) {
    exit 0
}

Write-Output "Over the limit: running cargo clean."
& cargo clean --manifest-path (Join-Path $repo 'Cargo.toml')
exit $LASTEXITCODE
