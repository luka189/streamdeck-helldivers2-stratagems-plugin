# Install the plugin built by build.ps1 into Stream Deck.
#   (no switch)  copy the plugin folder into Stream Deck's plugins
#   -Link        link it instead (a junction), so each build.ps1 run is picked up: for development
#   -Uninstall   remove it
param([switch]$Link, [switch]$Uninstall)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$src = Join-Path $root 'plugin\com.xiaodoudou.helldivers2-stratagems.sdPlugin'
$dest = Join-Path $env:APPDATA 'Elgato\StreamDeck\Plugins\com.xiaodoudou.helldivers2-stratagems.sdPlugin'

function Remove-Installed {
    $item = Get-Item $dest -Force -ErrorAction SilentlyContinue
    if (-not $item) { return }
    # a junction is removed on its own, never through to the repository folder it points at
    if ($item.LinkType) { $item.Delete() } else { Remove-Item -Recurse -Force $dest }
}

if ($Uninstall) {
    Remove-Installed
    Write-Host 'Removed. Restart Stream Deck.'
    return
}

if (-not (Test-Path (Join-Path $src 'hd2-plugin.exe'))) { throw 'Run build.ps1 first: the plugin is not built.' }
Remove-Installed
if ($Link) {
    New-Item -ItemType Junction -Path $dest -Target $src | Out-Null
    Write-Host "Linked $dest -> $src"
} else {
    Copy-Item -Recurse $src $dest
    Write-Host "Installed to $dest"
}
Write-Host 'Restart the Stream Deck app (quit from the tray icon, then reopen) to load the plugin.'
