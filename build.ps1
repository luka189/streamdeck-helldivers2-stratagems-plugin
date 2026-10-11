# Build the Rust plugin and assemble the .sdPlugin folder.
#   -Package  also writes dist\com.xiaodoudou.helldivers2-stratagems.streamDeckPlugin, the file people install (what CI and releases publish)
param([switch]$Package)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$plugin = Join-Path $root 'plugin\com.xiaodoudou.helldivers2-stratagems.sdPlugin'
$uuid = 'com.xiaodoudou.helldivers2-stratagems'

Push-Location (Join-Path $root 'scanner')
cargo build --release
if ($LASTEXITCODE) { Pop-Location; throw 'cargo build failed' }
Pop-Location

# The running plugin locks its exe, and stopping it races Stream Deck relaunching it (force-killing it
# counts as a crash; a few get the plugin disabled as unstable). A running exe can still be renamed,
# so move it aside and put the new one in place; the plugin notices and hands over to it.
# hd-plugin.* are leftovers from the plugin's earlier name.
Get-ChildItem -Path "$plugin\*" -Include 'hd2-plugin.old*.exe', 'hd-plugin*.exe', 'hd-plugin.log' -File |
    Remove-Item -ErrorAction SilentlyContinue
$exe = Join-Path $plugin 'hd2-plugin.exe'
if (Test-Path $exe) { Move-Item $exe (Join-Path $plugin ("hd2-plugin.old{0}.exe" -f (Get-Date).Ticks)) }
Copy-Item (Join-Path $root 'scanner\target\release\hd2-plugin.exe') $plugin -Force
# the stratagem list and pictures are built into the exe (the Marketplace's DRM keeps a plugin from reading its own
# files); a data folder left by an older build would only hide a missing embed, so it goes
$data = Join-Path $plugin 'data'
if (Test-Path $data) { Remove-Item -Recurse -Force $data }
Write-Host "Built $plugin"

if ($Package) {
    # a .streamDeckPlugin file is a zip of the .sdPlugin folder; only what the plugin needs goes in
    $dist = Join-Path $root 'dist'
    $stage = Join-Path $dist 'stage'
    $folder = Join-Path $stage 'com.xiaodoudou.helldivers2-stratagems.sdPlugin'
    if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
    New-Item -ItemType Directory -Force $folder | Out-Null
    Copy-Item (Join-Path $plugin 'manifest.json'), $exe $folder
    Copy-Item -Recurse (Join-Path $plugin 'imgs'), (Join-Path $plugin 'pi') $folder
    $out = Join-Path $dist "$uuid.streamDeckPlugin"
    if (Test-Path $out) { Remove-Item -Force $out }
    # entries are named here: Windows PowerShell's CreateFromDirectory writes backslashes, which zip readers may refuse
    Add-Type -AssemblyName System.IO.Compression, System.IO.Compression.FileSystem
    $zip = [System.IO.Compression.ZipFile]::Open($out, [System.IO.Compression.ZipArchiveMode]::Create)
    try {
        foreach ($file in Get-ChildItem $stage -Recurse -File) {
            $name = $file.FullName.Substring($stage.Length + 1) -replace '\\', '/'
            [void][System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile($zip, $file.FullName, $name, [System.IO.Compression.CompressionLevel]::Optimal)
        }
    } finally {
        $zip.Dispose()
    }
    Remove-Item -Recurse -Force $stage
    Write-Host "Packaged $out"
}
