# Quickstart: SPEC-29924074

## 1. Build
```bash
cargo build -p llmlb
```

## 2. Assistant CLI usage
```bash
# execute_curl 相当
llmlb assistant curl --command "curl http://localhost:32768/v1/models"

# OpenAPI表示
llmlb assistant openapi

# APIガイド表示
llmlb assistant guide --category overview
```

## 3. Claude Code plugin files
- `.claude-plugin/marketplace.json`
- `.claude-plugin/plugins/llmlb-cli/plugin.json`
- `.claude-plugin/plugins/llmlb-cli/skills/llmlb-cli-usage/SKILL.md`

## 4. Codex skill files
- `.codex/skills/llmlb-cli-usage/SKILL.md`
- `codex-skills/dist/` (packaged `.skill` output directory)

## 5. Verify npm removal
```bash
rg -n "@llmlb/mcp-server|llmlb-mcp|npm publish" .
```

## 6. Test
```bash
cargo test -p llmlb --test cli_tests
cargo test -p llmlb assistant
```
