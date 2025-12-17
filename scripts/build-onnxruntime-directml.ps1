Param(
    [string]$OrtVersion = $env:ORT_VERSION,
    [string]$OrtRepoUrl = $env:ORT_REPO_URL,
    [string]$OrtDir = $env:ORT_DIR,
    [string]$OrtBuildDir = $env:ORT_BUILD_DIR,
    [string]$OrtInstallPrefix = $env:ORT_INSTALL_PREFIX,
    [string]$PythonBin = $env:PYTHON_BIN,
    [string]$CmakeBin = $env:CMAKE_BIN
)

$ErrorActionPreference = "Stop"

if ([string]::IsNullOrEmpty($OrtVersion)) {
    $OrtVersion = "v1.22.2"
}
if ([string]::IsNullOrEmpty($OrtRepoUrl)) {
    $OrtRepoUrl = "https://github.com/microsoft/onnxruntime.git"
}

if ([string]::IsNullOrEmpty($OrtDir)) {
    $OrtDir = Join-Path $env:TEMP "onnxruntime-directml"
}
if ([string]::IsNullOrEmpty($OrtBuildDir)) {
    $OrtBuildDir = Join-Path $OrtDir "build_dml"
}
if ([string]::IsNullOrEmpty($OrtInstallPrefix)) {
    $OrtInstallPrefix = Join-Path $OrtDir "install"
}

if ([string]::IsNullOrEmpty($PythonBin)) {
    $PythonBin = "python"
}
if ([string]::IsNullOrEmpty($CmakeBin)) {
    $CmakeBin = "cmake"
}

if (-not (Test-Path (Join-Path $OrtDir ".git"))) {
    Write-Host "==> Cloning onnxruntime $OrtVersion into $OrtDir"
    if (Test-Path $OrtDir) {
        Remove-Item $OrtDir -Recurse -Force
    }
    git clone --depth 1 --branch $OrtVersion $OrtRepoUrl $OrtDir
} else {
    Write-Host "==> Using existing onnxruntime checkout at $OrtDir"
}

Write-Host "==> Building onnxruntime (DirectML EP enabled)"
& $PythonBin (Join-Path $OrtDir "tools\\ci_build\\build.py") `
    --build_dir $OrtBuildDir `
    --config Release `
    --update `
    --build `
    --parallel `
    --build_shared_lib `
    --use_dml `
    --skip_tests

Write-Host "==> Installing CMake package to $OrtInstallPrefix"
if (Test-Path $OrtInstallPrefix) {
    Remove-Item $OrtInstallPrefix -Recurse -Force
}
& $CmakeBin --install (Join-Path $OrtBuildDir "Release") --prefix $OrtInstallPrefix

Write-Host "==> Done."
Write-Host ""
Write-Host "To build llm-node with this onnxruntime:"
Write-Host "  cmake -S node -B build -DCMAKE_BUILD_TYPE=Release -Donnxruntime_DIR=`"$OrtInstallPrefix\\lib\\cmake\\onnxruntime`""
