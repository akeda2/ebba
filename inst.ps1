Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

Write-Host "Installing ebba with cargo..."
cargo install --path .

$cargoHome = if ($env:CARGO_HOME) {
    $env:CARGO_HOME
} else {
    Join-Path $HOME ".cargo"
}
$cargoBin = Join-Path $cargoHome "bin"
$ebbaExe = Join-Path $cargoBin "ebba.exe"

if (-not (Test-Path $ebbaExe)) {
    throw "ebba.exe was not found at '$ebbaExe' after install."
}

$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
$pathEntries = @()
if ($userPath) {
    $pathEntries = $userPath.Split(";") | Where-Object { $_.Trim() -ne "" }
}

$exists = $false
foreach ($entry in $pathEntries) {
    if ($entry.TrimEnd("\") -ieq $cargoBin.TrimEnd("\")) {
        $exists = $true
        break
    }
}

if (-not $exists) {
    $newPath = if ($userPath) { "$userPath;$cargoBin" } else { $cargoBin }
    [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
    Write-Host "Added '$cargoBin' to your user PATH."
    Write-Host "Open a new terminal session to use the updated PATH."
}

Write-Host "Installed: $ebbaExe"
Write-Host "You can verify with: ebba --help"
