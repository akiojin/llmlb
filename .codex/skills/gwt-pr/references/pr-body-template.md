<!-- ============================================================
PR Body Template — gwt-pr skill

Rules:
- REQUIRED sections: Summary, Changes, Testing, Closing Issues, Related Issues, Checklist
- CONDITIONAL sections: Context, Risk/Impact, Screenshots, Deployment
- OPTIONAL sections: Notes
- Remove CONDITIONAL sections entirely if not applicable
- Remove all <!-- GUIDE: ... --> comments before creating the PR
- No "TODO" may remain in REQUIRED sections
============================================================ -->

## Summary

<!-- GUIDE: 1-3 bullet points. Each bullet = one sentence. Include both WHAT changed and WHY. -->

- {what changed and why}

## Changes

<!-- GUIDE: List changes grouped by file or module. Each bullet must reference a specific file/module name. -->

- `{file or module}`: {what changed}

## Testing

<!-- GUIDE: Provide reproducible steps. Include exact commands, expected output, or manual verification steps. "Tested" alone is NOT acceptable. -->

- [ ] `{command}` — {expected result}

## Closing Issues

<!-- GUIDE: Issues to auto-close when this change is released to main.
     Use "Closes #N" syntax (one per line). Write "None" if no issues to close.
     Note: develop PRs do NOT auto-close issues directly;
     the release workflow re-collects these into the Release PR.
     If an issue should close on release, it must appear here even if it is also listed below. -->

- Closes #{issue number}

## Related Issues / Links

<!-- GUIDE: Reference-only links (NOT auto-closed). Use #number or URL. Write "None" if none.
     Any #number listed only here is treated as reference-only and will not auto-close. -->

- {#issue or URL or "None"}

## Checklist

- [ ] Tests added/updated
- [ ] Lint/format passed (`cargo clippy`, `cargo fmt`, `svelte-check`)
- [ ] Documentation updated (if user-facing change)
- [ ] Migration/backfill plan included (if schema/data change)
- [ ] CHANGELOG impact considered (breaking change flagged in commit)

<!-- ============================================================
CONDITIONAL SECTIONS — Delete entire section if not applicable
============================================================ -->

## Context

<!-- GUIDE: Required if 3+ files changed or non-obvious motivation. Explain background, link tickets/incidents. -->

- {why this PR is needed — background, ticket, or incident}

## Risk / Impact

<!-- GUIDE: Required if breaking change, performance impact, or rollback needed. -->

- **Affected areas**: {components/services impacted}
- **Rollback plan**: {steps to revert or "No rollback needed"}

## Screenshots

<!-- GUIDE: Required if UI changed. Attach before/after screenshots. -->

| Before | After |
|--------|-------|
| {img}  | {img} |

## Deployment

<!-- GUIDE: Include only if special deployment steps, feature flags, or env changes needed. -->

- {deployment steps or "Standard deploy — no special steps"}

<!-- ============================================================
OPTIONAL SECTIONS — Delete if nothing to add
============================================================ -->

## Notes

<!-- GUIDE: Reviewer hints, known limitations, follow-up items. -->

- {optional notes for reviewers}
