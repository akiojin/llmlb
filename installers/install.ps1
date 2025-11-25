Param(
    [string]$Repo = "akiojin/ollama-router"
)

$os = $PSVersionTable.OS
$arch = $env:PROCESSOR_ARCHITECTURE

if ($IsLinux) { $assetOs = "linux" }
elseif ($IsMacOS) { $assetOs = "macos" }
elseif ($IsWindows) { $assetOs = "windows" }
else { Write-Error "Unsupported OS"; exit 1 }

switch ($arch.ToLower()) {
    "amd64" { $assetArch = "amd64" }
    "x86_64" { $assetArch = "amd64" }
    "arm64" { $assetArch = "arm64" }
    default { Write-Error "Unsupported arch $arch"; exit 1 }
}

$assetName = "ollama-node-$assetOs-$assetArch"
$ext = if ($assetOs -eq "windows") { "zip" } else { "tar.gz" }
$url = "https://github.com/$Repo/releases/latest/download/$assetName.$ext"

$tmp = New-Item -ItemType Directory -Path ([System.IO.Path]::GetTempPath()) -Name ("ollama-" + [System.Guid]::NewGuid().ToString("N"))
$archive = Join-Path $tmp "asset.$ext"

Write-Host "Downloading $url"
Invoke-WebRequest -Uri $url -OutFile $archive -UseBasicParsing

Write-Host "Extracting..."
if ($ext -eq "zip") {
    Expand-Archive -Path $archive -DestinationPath $tmp -Force
} else {
    tar -xzf $archive -C $tmp
}

$bin = Get-ChildItem -Path $tmp -Filter "ollama-node*" -Recurse | Select-Object -First 1
if (-not $bin) { Write-Error "Binary not found in archive"; exit 1 }

$target = if ($IsWindows) { "$env:ProgramFiles\OllamaNode" } else { "/usr/local/bin" }
if (-not (Test-Path $target)) { New-Item -ItemType Directory -Path $target | Out-Null }

if ($IsWindows) {
    Copy-Item $bin.FullName -Destination (Join-Path $target "ollama-node.exe") -Force
} else {
    Copy-Item $bin.FullName -Destination (Join-Path $target "ollama-node") -Force
    chmod +x (Join-Path $target "ollama-node")
}

Write-Host "Installed to $target"
