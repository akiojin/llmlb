# Run the project's existing Playwright suite through Windows' pnpm command shim.
# Remaining arguments include canonical gwtd headed/reporter arguments unchanged.
param(
    [int]$Port = 32768,
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$PlaywrightArgs
)

$ErrorActionPreference = 'Stop'
$env:BASE_URL = "http://127.0.0.1:$Port"
& pnpm.cmd --dir llmlb/tests/e2e-playwright exec playwright test @PlaywrightArgs
exit $LASTEXITCODE
