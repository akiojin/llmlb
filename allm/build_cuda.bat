@echo off
set CUDA_PATH=C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v13.1
set CudaToolkitDir=C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v13.1
set PATH=%CUDA_PATH%\bin;%PATH%

call "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat"

if exist "E:\llm-router\.worktrees\feature-support-nemotron\node\build" rmdir /s /q "E:\llm-router\.worktrees\feature-support-nemotron\node\build"

cmake -S "E:\llm-router\.worktrees\feature-support-nemotron\node" ^
      -B "E:\llm-router\.worktrees\feature-support-nemotron\node\build" ^
      -G "Visual Studio 17 2022" ^
      -DCMAKE_TOOLCHAIN_FILE="C:\vcpkg\scripts\buildsystems\vcpkg.cmake" ^
      -DBUILD_WITH_CUDA=ON ^
      -DLLAMA_CURL=OFF ^
      -Wno-dev
