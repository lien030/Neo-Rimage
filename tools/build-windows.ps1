param(
    [switch]$DependenciesOnly,
    [string[]]$CargoArguments = @('test', '--locked', '--manifest-path', 'src-tauri/Cargo.toml', '--lib')
)
$ErrorActionPreference = 'Stop'
$project = Split-Path $PSScriptRoot -Parent
$native = Join-Path $project 'src-tauri/target/native'
$vcpkg = Join-Path $native 'vcpkg'
$baseline = '2c60af75f9d1ea85143242f92864ffa0dd2f78e7'
New-Item -ItemType Directory -Force $native | Out-Null
if (!(Test-Path -LiteralPath (Join-Path $vcpkg '.git'))) {
    git clone https://github.com/microsoft/vcpkg.git $vcpkg
    if ($LASTEXITCODE) { throw 'vcpkg clone failed' }
}
git -C $vcpkg checkout --detach $baseline
if ($LASTEXITCODE) { throw 'vcpkg checkout failed' }
if (!(Test-Path -LiteralPath (Join-Path $vcpkg 'vcpkg.exe'))) {
    & (Join-Path $vcpkg 'bootstrap-vcpkg.bat') -disableMetrics
    if ($LASTEXITCODE) { throw 'vcpkg bootstrap failed' }
}
$env:VCPKG_DOWNLOADS = Join-Path $native 'downloads'
$env:VCPKG_DEFAULT_BINARY_CACHE = Join-Path $native 'cache'
New-Item -ItemType Directory -Force $env:VCPKG_DEFAULT_BINARY_CACHE | Out-Null
& (Join-Path $vcpkg 'vcpkg.exe') install dav1d:x64-windows-static-md pkgconf:x64-windows --classic --disable-metrics
if ($LASTEXITCODE) { throw 'Native dependency build failed' }
$env:PKG_CONFIG = Join-Path $vcpkg 'installed/x64-windows/tools/pkgconf/pkgconf.exe'
$env:PKG_CONFIG_PATH = Join-Path $vcpkg 'installed/x64-windows-static-md/lib/pkgconfig'
$env:SYSTEM_DEPS_DAV1D_LINK = 'static'
$env:SYSTEM_DEPS_DAV1D_BUILD_INTERNAL = 'never'
if (!$DependenciesOnly) {
    Push-Location $project
    try {
        & cargo @CargoArguments
        if ($LASTEXITCODE) { throw 'Cargo command failed' }
    } finally { Pop-Location }
}
