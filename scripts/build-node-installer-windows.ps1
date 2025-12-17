Param(
    [Parameter(Mandatory = $true)][string]$Version,
    [Parameter(Mandatory = $true)][string]$OutputMsi
)

$ErrorActionPreference = "Stop"

$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")

$OrtDir = if ($env:ORT_DIR) { $env:ORT_DIR } else { Join-Path $env:TEMP "onnxruntime-directml" }
$OrtInstallPrefix = if ($env:ORT_INSTALL_PREFIX) { $env:ORT_INSTALL_PREFIX } else { Join-Path $OrtDir "install" }

$NodeBuildDir = if ($env:NODE_BUILD_DIR) { $env:NODE_BUILD_DIR } else { Join-Path $RepoRoot "node\\build" }

# Ensure required submodules are present (whisper.cpp is ON by default)
if (-not (Test-Path (Join-Path $RepoRoot "node\\third_party\\whisper.cpp\\CMakeLists.txt"))) {
    Write-Host "==> Initializing submodules (whisper.cpp)"
    git -C $RepoRoot submodule update --init --recursive node/third_party/whisper.cpp
}

Write-Host "==> Building onnxruntime (DirectML)"
& (Join-Path $RepoRoot "scripts\\build-onnxruntime-directml.ps1")

$OrtCmakeDir = Join-Path $OrtInstallPrefix "lib\\cmake\\onnxruntime"
if (-not (Test-Path $OrtCmakeDir)) {
    throw "onnxruntime CMake package not found: $OrtCmakeDir"
}

Write-Host "==> Installing OpenSSL via vcpkg"
$VcpkgRoot = if ($env:VCPKG_ROOT) { $env:VCPKG_ROOT } elseif ($env:VCPKG_INSTALLATION_ROOT) { $env:VCPKG_INSTALLATION_ROOT } elseif (Test-Path "C:\\vcpkg") { "C:\\vcpkg" } else { "" }
if ([string]::IsNullOrEmpty($VcpkgRoot)) {
    throw "vcpkg root not found (set VCPKG_ROOT or VCPKG_INSTALLATION_ROOT)."
}
$VcpkgExe = Join-Path $VcpkgRoot "vcpkg.exe"
if (-not (Test-Path $VcpkgExe)) {
    throw "vcpkg.exe not found: $VcpkgExe"
}

& $VcpkgExe install openssl:x64-windows
$OpenSslRoot = Join-Path $VcpkgRoot "installed\\x64-windows"
if (-not (Test-Path $OpenSslRoot)) {
    throw "OpenSSL install root not found: $OpenSslRoot"
}

Write-Host "==> Building llm-node"
cmake -S (Join-Path $RepoRoot "node") -B $NodeBuildDir `
    -DCMAKE_BUILD_TYPE=Release `
    -DBUILD_TESTS=OFF `
    -DBUILD_SHARED_LIBS=OFF `
    -Donnxruntime_DIR=$OrtCmakeDir `
    -DOPENSSL_ROOT_DIR=$OpenSslRoot
cmake --build $NodeBuildDir --config Release

$NodeExeCandidates = @(
    (Join-Path $NodeBuildDir "Release\\llm-node.exe"),
    (Join-Path $NodeBuildDir "llm-node.exe")
)
$NodeExe = $NodeExeCandidates | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $NodeExe) {
    throw "llm-node.exe not found under $NodeBuildDir"
}

Write-Host "==> Staging runtime files"
$StagingDir = Join-Path $RepoRoot "dist\\llm-node-windows-x86_64"
if (Test-Path $StagingDir) {
    Remove-Item $StagingDir -Recurse -Force
}
New-Item -ItemType Directory -Path $StagingDir | Out-Null
Copy-Item $NodeExe -Destination (Join-Path $StagingDir "llm-node.exe")

$OrtBin = Join-Path $OrtInstallPrefix "bin"
if (-not (Test-Path $OrtBin)) {
    throw "onnxruntime bin dir not found: $OrtBin"
}
Get-ChildItem -Path $OrtBin -Filter "*.dll" | ForEach-Object {
    Copy-Item $_.FullName -Destination $StagingDir -Force
}

$OpenSslBin = Join-Path $OpenSslRoot "bin"
if (-not (Test-Path $OpenSslBin)) {
    throw "OpenSSL bin dir not found: $OpenSslBin"
}
Get-ChildItem -Path $OpenSslBin -Filter "*.dll" | ForEach-Object {
    Copy-Item $_.FullName -Destination $StagingDir -Force
}

Write-Host "==> Harvesting files with heat.exe"
$HarvestWxs = Join-Path $RepoRoot "dist\\llm-node-files.wxs"
heat.exe dir $StagingDir `
    -cg NodeRuntimeComponents `
    -dr INSTALLFOLDER `
    -sreg -srd `
    -gg -g1 `
    -var var.BinariesDir `
    -out $HarvestWxs | Out-Null

Write-Host "==> Building MSI"
$NodeWxs = Join-Path $RepoRoot "installers\\windows\\llm-node.wxs"
$WixObjDir = Join-Path $RepoRoot "dist\\wixobj"
if (Test-Path $WixObjDir) {
    Remove-Item $WixObjDir -Recurse -Force
}
New-Item -ItemType Directory -Path $WixObjDir | Out-Null

$ObjMain = Join-Path $WixObjDir "llm-node.wixobj"
$ObjFiles = Join-Path $WixObjDir "llm-node-files.wixobj"

candle.exe $NodeWxs -out $ObjMain -dProductVersion="$Version" -dBinariesDir="$StagingDir"
candle.exe $HarvestWxs -out $ObjFiles -dProductVersion="$Version" -dBinariesDir="$StagingDir"
light.exe $ObjMain $ObjFiles -out $OutputMsi -sice:ICE03

Write-Host "==> Done: $OutputMsi"
