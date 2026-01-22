$ErrorActionPreference = 'Stop'

# Add CUDA DLLs to PATH
$env:PATH = "C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v13.1\bin\x64;" + $env:PATH

# Run the PoC
$POC_DIR = "E:\llmlb\.worktrees\feature-support-nemotron\poc\nemotron-cuda-cpp"
$EXE = "$POC_DIR\build\Release\nemotron-cuda-poc.exe"
$MODEL = "$POC_DIR\models\minitron-8b"

Write-Host "=== Running Nemotron CUDA PoC ===" -ForegroundColor Green
Write-Host "Executable: $EXE"
Write-Host "Model: $MODEL"
Write-Host ""

& $EXE --model $MODEL --prompt "Hello" --max-tokens 10 --verbose
