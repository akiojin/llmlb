$ErrorActionPreference = 'Stop'

# Set CUDA paths
$env:CUDA_PATH = 'C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v13.1'
$env:CudaToolkitDir = $env:CUDA_PATH
$env:PATH = "$env:CUDA_PATH\bin;$env:PATH"

# Import Visual Studio environment
Import-Module 'C:\Program Files\Microsoft Visual Studio\2022\Community\Common7\Tools\Microsoft.VisualStudio.DevShell.dll'
Enter-VsDevShell -VsInstallPath 'C:\Program Files\Microsoft Visual Studio\2022\Community' -DevCmdArguments '-arch=x64'

$POC_DIR = 'E:\llm-router\.worktrees\feature-support-nemotron\poc\nemotron-cuda-cpp'
$BUILD_DIR = "$POC_DIR\build"

# Clean and create build directory
if (Test-Path $BUILD_DIR) { Remove-Item -Recurse -Force $BUILD_DIR }

# Run CMake
Write-Host '=== CMake Configuration ==='
cmake -S $POC_DIR -B $BUILD_DIR -G 'Visual Studio 17 2022' -A x64

if ($LASTEXITCODE -ne 0) {
    Write-Host "CMake configuration failed"
    exit 1
}

# Build
Write-Host '=== Build ==='
cmake --build $BUILD_DIR --config Release --parallel

if ($LASTEXITCODE -ne 0) {
    Write-Host "Build failed"
    exit 1
}

Write-Host "Build completed successfully!"
