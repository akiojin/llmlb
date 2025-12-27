@echo off
setlocal

set CUDA_PATH=C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v13.1
set CudaToolkitDir=C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v13.1
set PATH=%CUDA_PATH%\bin;%PATH%

call "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat"

set POC_DIR=%~dp0
set BUILD_DIR=%POC_DIR%build

if exist "%BUILD_DIR%" rmdir /s /q "%BUILD_DIR%"

cmake -S "%POC_DIR%" -B "%BUILD_DIR%" -G "Visual Studio 17 2022" -DCMAKE_BUILD_TYPE=Release -Wno-dev

if %errorlevel% neq 0 (
    echo CMake configuration failed
    exit /b 1
)

cmake --build "%BUILD_DIR%" --config Release

if %errorlevel% neq 0 (
    echo Build failed
    exit /b 1
)

echo.
echo Build successful! Executable at: %BUILD_DIR%\Release\nemotron-cuda-poc.exe
