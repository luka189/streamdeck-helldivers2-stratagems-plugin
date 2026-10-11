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
$data = Join-Path $plugin 'data'
New-Item -ItemType Directory -Force (Join-Path $data 'icons_144'), (Join-Path $data 'status') | Out-Null
Copy-Item (Join-Path $root 'data\stratagems.json') $data -Force
Copy-Item (Join-Path $root 'data\icons_144\*.png') (Join-Path $data 'icons_144') -Force
Copy-Item (Join-Path $root 'data\status\*.png') (Join-Path $data 'status') -Force
Write-Host "Built $plugin"

if ($Package) {
    # a .streamDeckPlugin file is a zip of the .sdPlugin folder; only what the plugin needs goes in
    $dist = Join-Path $root 'dist'
    $stage = Join-Path $dist 'stage'
    $folder = Join-Path $stage 'com.xiaodoudou.helldivers2-stratagems.sdPlugin'
    if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
    New-Item -ItemType Directory -Force (Join-Path $folder 'data') | Out-Null
    Copy-Item (Join-Path $plugin 'manifest.json'), $exe $folder
    Copy-Item -Recurse (Join-Path $plugin 'imgs'), (Join-Path $plugin 'pi') $folder
    Copy-Item -Recurse (Join-Path $data 'stratagems.json'), (Join-Path $data 'icons_144'), (Join-Path $data 'status') (Join-Path $folder 'data')
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
