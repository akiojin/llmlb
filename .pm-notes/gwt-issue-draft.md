# akiojin/gwt への Issue 起票用ドラフト

Title: Issue Monitor: GUI/CLI resolve different project-state scopes; daemon endpoint unregistered; launch_now never launches agents

## Summary

With the Issue Monitor enabled and a launch profile saved in the GUI, no implementation agent is ever launched. Investigation from the resident PM session (SPEC-3431) shows the CLI (`gwtd`) and the GUI resolve the same project to two different project-state scopes, and the runtime daemon endpoint is not registered, so scan requests from the CLI never reach the scanner.

## Environment

- macOS (Darwin 25.5.0), GWT.app gwtd 9.76.0
- GUI project tab: `project_root=/Users/akiojin/Workbench/llmlb`
- PM agent session cwd: `/Users/akiojin/.gwt/projects/88e3131409d5d8ec/pm/worktree`

## Observed

1. Project scope split between GUI and CLI
   - GUI writes monitor state to `~/.gwt/projects/88e3131409d5d8ec/project-state/issue-monitor.json` -> `enabled: true, autonomous_mode: true, launch_profile: saved, priority_order: []`
   - `gwtd` invoked from the PM worktree (without `params.project_root`) resolves to `~/.gwt/projects/9d957b32b3fbd45c/project-state/issue-monitor.json` -> `enabled: false, no profile`; PM writes (`issue.monitor.priority.set`) landed there, invisible to the GUI monitor.
   - Consequence: the PM session kept reporting "monitor still disabled" after the user enabled it, and the enabled monitor ran with an empty priority order.
   - Workaround: pass `params.project_root="/Users/akiojin/Workbench/llmlb"` explicitly on every `issue.monitor.*` operation.

2. No daemon endpoint / push channel dead (FR-109 degraded path)
   - `daemon.subscribe` on `issue_monitor` always fails: `no daemon registered (endpoint=/Users/akiojin/.gwt/projects/9d957b32b3fbd45c/runtime/daemon/59e566621c3eedff.json)` — note it also points at the CLI-derived (wrong) scope.

3. `issue.monitor.launch_now` cannot trigger a scan
   - Returns `{"scan_delivery":"next-scheduled-scan","scan_requested":false}` even in the correct scope with `enabled=true` + profile saved; `active_launches` stays empty afterwards. No agent has ever been launched for a queue of 11 ready issues.

4. Minor: `gwtd issue.create` silently ignores an unknown `repo` param and creates the issue in the current project repo (stray llmlb#701, closed).

## Expected

- GUI and CLI resolve one canonical project scope for the same project (or the CLI auto-detects the GUI tab's `project_root` when run inside a managed worktree).
- `daemon.subscribe` works from agent sessions, with the endpoint registered under the canonical scope.
- `launch_now` triggers a scan that actually launches an agent when `enabled=true`, a launch profile exists, and slots are free.
- Unknown params to gwtd operations are rejected, not silently ignored.

## Notes

- Possibly related to akiojin/llmlb#653 (Windows: agent environment cannot resolve gwtd CLI) in the same product area.

## Addendum (2026-08-12, later)

5. **Board pull delivery never reaches a parked agent**
   - An agent that posted "ready for the next instruction" and parked never picked up a Board post addressed to it (`mentions: ["agent:Codex"]`) for 20+ minutes. Pull-based delivery has no intent boundaries in a parked session, so PM decisions cannot unblock a waiting agent. Workaround: revoke/relaunch the issue so a fresh session reads the decision from the Issue itself.

6. **failover relaunch always fails once an execution generation is completed**
   - After `issue.monitor.failover` freed the slot for issue #702 (whose prior session had completed its execution record), the relaunched session immediately died with `agent_failed: an execution generation already exists for issue #702 (completed...)`. The prior session's own handoff explicitly requested a "fresh linked-owner execution", but the Monitor cannot create one. Net effect: any issue whose execution completed but whose PR work remains (e.g. Draft PR awaiting CI + Ready promotion) can never be resumed by the Monitor. The PM had to take over the PR promotion manually via `pr.checks` / `pr.ready`.

7. **Work receipt binding survives failover and permanently blocks pr.create in the successor session**
   - Issue #694: after `issue.monitor.failover`, the fresh session completed implementation + 11/11 canonical verification, but both Ready and Draft `pr.create` were rejected because the final Work event receipt belongs to the predecessor execution generation. `workspace.ensure`, `workspace.create` (2 binding variants), and `execution.adopt` all failed; `available_recoveries` was empty. No state edit / gate bypass was possible. The PM had to create the PR out-of-band via `pr.create` from its own session. Combined with defect 6, any failover leaves the successor unable to deliver.

8. **`issue.spec` section readback fails after a write, forcing agents to append history instead of updating in place**
   - While updating SPEC #575 planning artifacts (research/data-model/quickstart/plan/tasks) for issue #643, the agent found written sections could not be read back through the `issue.spec` section API. It worked around this by appending the artifacts as history entries so they became re-readable. Impact: SPEC artifacts accumulate duplicate historical copies rather than being updated in place, which is the same duplication already observed on #575 (spec artifact exists twice: comment 4601798618 newest, 4182651288 older). Any workflow that reads "the current section" can silently get a stale copy.

9. **A dead launch holds its slot indefinitely; the Monitor has no activity-based reaping**
   - Issue #694's launch window (agent-13) stopped producing activity at 2026-08-12T17:26Z but stayed in `active_launches` for ~47 hours. With `max_active: 1` this froze a 10-item queue completely. `issue.monitor.status` reports `last_activity_at`, so the staleness is observable, but nothing acts on it. The PM had to detect it by hand and call `issue.monitor.stop` to release the slot. Suggest a configurable inactivity threshold that moves a silent launch to `needs_human` automatically.

10. **PRs created out-of-band by the PM never get auto-merged, unlike agent-created PRs**
   - PR #707 (created by the agent via `pr.create` in its own session) auto-merged as soon as CI went green. PR #705 (created by the PM out-of-band as the workaround for defect 7) has the identical 11 checks all SUCCESS, the same `auto-merge` label, and CodeRabbit passing, yet stays `MERGEABLE` / `merge_state: BLOCKED` indefinitely. The out-of-band creation path appears to miss whatever makes auto-merge eligible, so the defect-7 workaround produces PRs that then require manual merging.
