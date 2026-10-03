param()
$ErrorActionPreference = 'Stop'
$project = Split-Path $PSScriptRoot -Parent
$version = (Get-Content -Raw (Join-Path $project 'package.json') | ConvertFrom-Json).version
$release = Join-Path $project 'src-tauri/target/release'
$output = Join-Path $project "src-tauri/target/distribution/$version"
$portable = Join-Path $output 'portable'
$licenses = Join-Path $portable 'licenses'
New-Item -ItemType Directory -Force $licenses | Out-Null
Copy-Item -LiteralPath (Join-Path $release 'neo-rimage.exe') -Destination $portable -Force
$resources = (Get-Content -Raw (Join-Path $project 'src-tauri/tauri.conf.json') | ConvertFrom-Json).bundle.resources
foreach ($resource in $resources.PSObject.Properties) {
    $source = Join-Path (Join-Path $project 'src-tauri') $resource.Name
    Copy-Item -LiteralPath $source -Destination (Join-Path $portable $resource.Value) -Force
}
$artifacts = @()
foreach ($bundle in @(@('nsis', '*.exe'), @('msi', '*.msi'))) {
    $matches = @(Get-ChildItem -LiteralPath (Join-Path $release "bundle/$($bundle[0])") -Filter $bundle[1] | Where-Object { $_.Name -like "*_$($version)_*" })
    if ($matches.Count -ne 1) { throw "Expected exactly one $($bundle[0]) artifact for $version" }
    $destination = Join-Path $output $matches[0].Name
    Copy-Item -LiteralPath $matches[0].FullName -Destination $destination -Force
    $artifacts += $destination
}
$archive = Join-Path $output "neo-rimage_$($version)_x64_portable.zip"
Compress-Archive -Path (Join-Path $portable '*') -DestinationPath $archive -Force
$artifacts += $archive
$checksums = foreach ($artifact in $artifacts) {
    $hash = (Get-FileHash -LiteralPath $artifact -Algorithm SHA256).Hash.ToLowerInvariant()
    "$hash  $(Split-Path $artifact -Leaf)"
}
[System.IO.File]::WriteAllLines((Join-Path $output 'SHA256SUMS'), [string[]]$checksums, [System.Text.UTF8Encoding]::new($false))
Get-Item -LiteralPath $artifacts
