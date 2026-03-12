# Repository Configuration

This directory stores GitHub repository settings as code.

## Layout

```text
.github/config/
├── README.md
├── repo.json
└── branch-protection/
    ├── main.json
    └── develop.json
```

## Applying settings

This repository uses [`gh-repo-config`](https://github.com/twelvelabs/gh-repo-config)
as the primary branch-protection source of truth.

```bash
gh extension install twelvelabs/gh-repo-config
gh repo-config apply
```

If the extension is not usable in the current shell environment, apply the same
payloads directly via the GitHub REST API.

## Branch protection rules

### main

- PR only
- Required checks:
  - Check PR Source (release/* only)
  - Markdown Lint
  - Rust (fmt/clippy/test) ubuntu-latest
  - Rust (fmt/clippy/test) macos-latest
  - Rust (fmt/clippy/test) windows-latest

### develop

- Required checks:
  - Commit Message Lint
  - Markdown Lint
  - Rust Format & Clippy
  - Rust Tests (ubuntu-latest)
  - Rust Tests (windows-latest)
  - OpenAI API Compatibility Tests
  - Playwright E2E Tests

## Required check policy

- Required jobs must always emit a status.
- Path filtering should skip heavy work inside a job, not suppress the job.
- Helper jobs such as `Detect Changes` are not merge gates.
