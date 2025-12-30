#!/usr/bin/env python3
"""
inspect_pr_checks.py - Comprehensive PR inspection tool

Modes:
  - checks: CI check failures (existing functionality)
  - conflicts: Merge conflict detection
  - reviews: Change Requests and unresolved review threads
  - all: Run all inspections
"""
from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path
from typing import Any, Iterable, Sequence

FAILURE_CONCLUSIONS = {
    "failure",
    "cancelled",
    "timed_out",
    "action_required",
}

FAILURE_STATES = {
    "failure",
    "error",
    "cancelled",
    "timed_out",
    "action_required",
}

FAILURE_BUCKETS = {"fail"}

FAILURE_MARKERS = (
    "error",
    "fail",
    "failed",
    "traceback",
    "exception",
    "assert",
    "panic",
    "fatal",
    "timeout",
    "segmentation fault",
)

DEFAULT_MAX_LINES = 160
DEFAULT_CONTEXT_LINES = 30
PENDING_LOG_MARKERS = (
    "still in progress",
    "log will be available when it is complete",
)


class GhResult:
    def __init__(self, returncode: int, stdout: str, stderr: str):
        self.returncode = returncode
        self.stdout = stdout
        self.stderr = stderr


def run_gh_command(args: Sequence[str], cwd: Path) -> GhResult:
    process = subprocess.run(
        ["gh", *args],
        cwd=cwd,
        text=True,
        capture_output=True,
    )
    return GhResult(process.returncode, process.stdout, process.stderr)


def run_gh_command_raw(args: Sequence[str], cwd: Path) -> tuple[int, bytes, str]:
    process = subprocess.run(
        ["gh", *args],
        cwd=cwd,
        capture_output=True,
    )
    stderr = process.stderr.decode(errors="replace")
    return process.returncode, process.stdout, stderr


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Inspect GitHub PRs for CI failures, merge conflicts, change requests, "
            "and unresolved review threads."
        ),
        formatter_class=argparse.ArgumentDefaultsHelpFormatter,
    )
    parser.add_argument("--repo", default=".", help="Path inside the target Git repository.")
    parser.add_argument(
        "--pr", default=None, help="PR number or URL (defaults to current branch PR)."
    )
    parser.add_argument(
        "--mode",
        choices=["checks", "conflicts", "reviews", "all"],
        default="all",
        help="Inspection mode: checks (CI), conflicts, reviews, or all.",
    )
    parser.add_argument("--max-lines", type=int, default=DEFAULT_MAX_LINES)
    parser.add_argument("--context", type=int, default=DEFAULT_CONTEXT_LINES)
    parser.add_argument("--json", action="store_true", help="Emit JSON instead of text output.")
    parser.add_argument(
        "--resolve-threads",
        action="store_true",
        help="Resolve review threads after confirmation.",
    )
    parser.add_argument(
        "--add-comment",
        type=str,
        default=None,
        help="Add a comment to the PR with the specified message.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    repo_root = find_git_root(Path(args.repo))
    if repo_root is None:
        print("Error: not inside a Git repository.", file=sys.stderr)
        return 1

    if not ensure_gh_available(repo_root):
        return 1

    pr_value = resolve_pr(args.pr, repo_root)
    if pr_value is None:
        return 1

    # Handle --add-comment action
    if args.add_comment:
        success = add_pr_comment(pr_value, args.add_comment, repo_root)
        if success:
            print(f"Comment added to PR #{pr_value}")
        else:
            print(f"Failed to add comment to PR #{pr_value}", file=sys.stderr)
        return 0 if success else 1

    # Collect results based on mode
    results: dict[str, Any] = {"pr": pr_value}
    has_issues = False

    # Conflicts check
    if args.mode in ("conflicts", "all"):
        conflict_result = check_conflicts(pr_value, repo_root)
        results["conflicts"] = conflict_result
        if conflict_result.get("hasConflicts"):
            has_issues = True

    # Reviews check (Change Requests + Unresolved Threads)
    if args.mode in ("reviews", "all"):
        change_requests = fetch_change_requests(pr_value, repo_root)
        results["changeRequests"] = change_requests
        if change_requests:
            has_issues = True

        unresolved_threads = fetch_unresolved_threads(pr_value, repo_root)
        results["unresolvedThreads"] = unresolved_threads
        if unresolved_threads:
            has_issues = True

        # Handle --resolve-threads
        if args.resolve_threads and unresolved_threads:
            resolved_count = 0
            for thread in unresolved_threads:
                thread_id = thread.get("id")
                if thread_id and resolve_thread(thread_id, repo_root):
                    resolved_count += 1
            results["resolvedThreadsCount"] = resolved_count

    # CI checks
    if args.mode in ("checks", "all"):
        checks = fetch_checks(pr_value, repo_root)
        if checks is not None:
            failing = [c for c in checks if is_failing(c)]
            if failing:
                has_issues = True
                ci_results = []
                for check in failing:
                    ci_results.append(
                        analyze_check(
                            check,
                            repo_root=repo_root,
                            max_lines=max(1, args.max_lines),
                            context=max(1, args.context),
                        )
                    )
                results["ciFailures"] = ci_results
            else:
                results["ciFailures"] = []
        else:
            results["ciFailures"] = None
            results["ciError"] = "Failed to fetch CI checks"

    # Output
    if args.json:
        print(json.dumps(results, indent=2))
    else:
        render_comprehensive_results(results)

    return 1 if has_issues else 0


# =============================================================================
# Git and GH utilities
# =============================================================================

def find_git_root(start: Path) -> Path | None:
    result = subprocess.run(
        ["git", "rev-parse", "--show-toplevel"],
        cwd=start,
        text=True,
        capture_output=True,
    )
    if result.returncode != 0:
        return None
    return Path(result.stdout.strip())


def ensure_gh_available(repo_root: Path) -> bool:
    result = run_gh_command(["auth", "status"], cwd=repo_root)
    if result.returncode == 0:
        return True
    message = (result.stderr or result.stdout or "").strip()
    print(message or "Error: gh not authenticated.", file=sys.stderr)
    return False


def resolve_pr(pr_value: str | None, repo_root: Path) -> str | None:
    if pr_value:
        return pr_value
    result = run_gh_command(["pr", "view", "--json", "number"], cwd=repo_root)
    if result.returncode != 0:
        message = (result.stderr or result.stdout or "").strip()
        print(message or "Error: unable to resolve PR.", file=sys.stderr)
        return None
    try:
        data = json.loads(result.stdout or "{}")
    except json.JSONDecodeError:
        print("Error: unable to parse PR JSON.", file=sys.stderr)
        return None
    number = data.get("number")
    if not number:
        print("Error: no PR number found.", file=sys.stderr)
        return None
    return str(number)


def fetch_repo_slug(repo_root: Path) -> str | None:
    result = run_gh_command(["repo", "view", "--json", "nameWithOwner"], cwd=repo_root)
    if result.returncode != 0:
        return None
    try:
        data = json.loads(result.stdout or "{}")
    except json.JSONDecodeError:
        return None
    name_with_owner = data.get("nameWithOwner")
    if not name_with_owner:
        return None
    return str(name_with_owner)


def parse_repo_owner_name(repo_slug: str) -> tuple[str, str] | None:
    """Parse 'owner/repo' into (owner, repo)."""
    parts = repo_slug.split("/")
    if len(parts) != 2:
        return None
    return parts[0], parts[1]


# =============================================================================
# Conflict detection
# =============================================================================

def check_conflicts(pr_value: str, repo_root: Path) -> dict[str, Any]:
    """Check if PR has merge conflicts."""
    result = run_gh_command(
        ["pr", "view", pr_value, "--json", "mergeable,mergeStateStatus,baseRefName,headRefName"],
        cwd=repo_root,
    )
    if result.returncode != 0:
        return {"error": "Failed to fetch merge state", "hasConflicts": False}

    try:
        data = json.loads(result.stdout or "{}")
    except json.JSONDecodeError:
        return {"error": "Failed to parse merge state JSON", "hasConflicts": False}

    mergeable_raw = data.get("mergeable")
    merge_state_status = data.get("mergeStateStatus", "UNKNOWN")

    # Handle different return types from gh CLI:
    # - Older versions: boolean (true/false)
    # - Newer versions: string ("MERGEABLE", "CONFLICTING", "UNKNOWN")
    if isinstance(mergeable_raw, bool):
        # Older gh CLI: boolean value
        has_conflicts = not mergeable_raw
        mergeable_display = "CONFLICTING" if not mergeable_raw else "MERGEABLE"
    elif isinstance(mergeable_raw, str):
        # Newer gh CLI: string value
        has_conflicts = mergeable_raw == "CONFLICTING"
        mergeable_display = mergeable_raw
    else:
        # None or unknown type (GitHub still calculating)
        has_conflicts = False
        mergeable_display = "UNKNOWN"

    # Also check mergeStateStatus for DIRTY state
    if merge_state_status == "DIRTY":
        has_conflicts = True

    return {
        "hasConflicts": has_conflicts,
        "mergeable": mergeable_display,
        "mergeStateStatus": merge_state_status,
        "baseRefName": data.get("baseRefName", ""),
        "headRefName": data.get("headRefName", ""),
    }


# =============================================================================
# Change Request handling
# =============================================================================

def fetch_change_requests(pr_value: str, repo_root: Path) -> list[dict[str, Any]]:
    """Fetch reviews with CHANGES_REQUESTED state."""
    repo_slug = fetch_repo_slug(repo_root)
    if not repo_slug:
        return []

    result = run_gh_command(
        ["api", f"repos/{repo_slug}/pulls/{pr_value}/reviews"],
        cwd=repo_root,
    )
    if result.returncode != 0:
        return []

    try:
        reviews = json.loads(result.stdout or "[]")
    except json.JSONDecodeError:
        return []

    if not isinstance(reviews, list):
        return []

    change_requests = []
    for review in reviews:
        if review.get("state") == "CHANGES_REQUESTED":
            change_requests.append({
                "id": review.get("id"),
                "reviewer": review.get("user", {}).get("login", "unknown"),
                "body": review.get("body", ""),
                "submittedAt": review.get("submitted_at", ""),
                "htmlUrl": review.get("html_url", ""),
            })

    return change_requests


# =============================================================================
# Unresolved review threads
# =============================================================================

def fetch_unresolved_threads(pr_value: str, repo_root: Path) -> list[dict[str, Any]]:
    """Fetch unresolved review threads using GraphQL."""
    repo_slug = fetch_repo_slug(repo_root)
    if not repo_slug:
        return []

    parsed = parse_repo_owner_name(repo_slug)
    if not parsed:
        return []

    owner, repo = parsed

    query = """
    query($owner: String!, $repo: String!, $number: Int!) {
      repository(owner: $owner, name: $repo) {
        pullRequest(number: $number) {
          reviewThreads(first: 100) {
            nodes {
              id
              isResolved
              isOutdated
              path
              line
              comments(first: 10) {
                nodes {
                  author { login }
                  body
                  createdAt
                }
              }
            }
          }
        }
      }
    }
    """

    result = run_gh_command(
        [
            "api", "graphql",
            "-f", f"query={query}",
            "-f", f"owner={owner}",
            "-f", f"repo={repo}",
            "-F", f"number={pr_value}",
        ],
        cwd=repo_root,
    )

    if result.returncode != 0:
        return []

    try:
        data = json.loads(result.stdout or "{}")
    except json.JSONDecodeError:
        return []

    threads = (
        data.get("data", {})
        .get("repository", {})
        .get("pullRequest", {})
        .get("reviewThreads", {})
        .get("nodes", [])
    )

    unresolved = []
    for thread in threads:
        if not thread.get("isResolved"):
            comments = thread.get("comments", {}).get("nodes", [])
            formatted_comments = []
            for comment in comments:
                formatted_comments.append({
                    "author": comment.get("author", {}).get("login", "unknown"),
                    "body": comment.get("body", ""),
                    "createdAt": comment.get("createdAt", ""),
                })
            unresolved.append({
                "id": thread.get("id"),
                "path": thread.get("path", ""),
                "line": thread.get("line"),
                "isOutdated": thread.get("isOutdated", False),
                "comments": formatted_comments,
            })

    return unresolved


# =============================================================================
# Thread resolution
# =============================================================================

def resolve_thread(thread_id: str, repo_root: Path) -> bool:
    """Resolve a review thread using GraphQL mutation."""
    mutation = """
    mutation($threadId: ID!) {
      resolveReviewThread(input: {threadId: $threadId}) {
        thread {
          id
          isResolved
        }
      }
    }
    """

    result = run_gh_command(
        [
            "api", "graphql",
            "-f", f"query={mutation}",
            "-f", f"threadId={thread_id}",
        ],
        cwd=repo_root,
    )

    if result.returncode != 0:
        return False

    try:
        data = json.loads(result.stdout or "{}")
    except json.JSONDecodeError:
        return False

    thread = data.get("data", {}).get("resolveReviewThread", {}).get("thread", {})
    return thread.get("isResolved", False)


# =============================================================================
# PR comment
# =============================================================================

def add_pr_comment(pr_value: str, body: str, repo_root: Path) -> bool:
    """Add a comment to the PR."""
    result = run_gh_command(
        ["pr", "comment", pr_value, "-b", body],
        cwd=repo_root,
    )
    return result.returncode == 0


# =============================================================================
# CI checks (existing functionality)
# =============================================================================

def fetch_checks(pr_value: str, repo_root: Path) -> list[dict[str, Any]] | None:
    primary_fields = ["name", "state", "conclusion", "detailsUrl", "startedAt", "completedAt"]
    result = run_gh_command(
        ["pr", "checks", pr_value, "--json", ",".join(primary_fields)],
        cwd=repo_root,
    )
    if result.returncode != 0:
        message = "\n".join(filter(None, [result.stderr, result.stdout])).strip()
        available_fields = parse_available_fields(message)
        if available_fields:
            fallback_fields = [
                "name",
                "state",
                "bucket",
                "link",
                "startedAt",
                "completedAt",
                "workflow",
            ]
            selected_fields = [field for field in fallback_fields if field in available_fields]
            if not selected_fields:
                print("Error: no usable fields available for gh pr checks.", file=sys.stderr)
                return None
            result = run_gh_command(
                ["pr", "checks", pr_value, "--json", ",".join(selected_fields)],
                cwd=repo_root,
            )
            if result.returncode != 0:
                message = (result.stderr or result.stdout or "").strip()
                print(message or "Error: gh pr checks failed.", file=sys.stderr)
                return None
        else:
            print(message or "Error: gh pr checks failed.", file=sys.stderr)
            return None
    try:
        data = json.loads(result.stdout or "[]")
    except json.JSONDecodeError:
        print("Error: unable to parse checks JSON.", file=sys.stderr)
        return None
    if not isinstance(data, list):
        print("Error: unexpected checks JSON shape.", file=sys.stderr)
        return None
    return data


def is_failing(check: dict[str, Any]) -> bool:
    conclusion = normalize_field(check.get("conclusion"))
    if conclusion in FAILURE_CONCLUSIONS:
        return True
    state = normalize_field(check.get("state") or check.get("status"))
    if state in FAILURE_STATES:
        return True
    bucket = normalize_field(check.get("bucket"))
    return bucket in FAILURE_BUCKETS


def analyze_check(
    check: dict[str, Any],
    repo_root: Path,
    max_lines: int,
    context: int,
) -> dict[str, Any]:
    url = check.get("detailsUrl") or check.get("link") or ""
    run_id = extract_run_id(url)
    job_id = extract_job_id(url)
    base: dict[str, Any] = {
        "name": check.get("name", ""),
        "detailsUrl": url,
        "runId": run_id,
        "jobId": job_id,
    }

    if run_id is None:
        base["status"] = "external"
        base["note"] = "No GitHub Actions run id detected in detailsUrl."
        return base

    metadata = fetch_run_metadata(run_id, repo_root)
    log_text, log_error, log_status = fetch_check_log(
        run_id=run_id,
        job_id=job_id,
        repo_root=repo_root,
    )

    if log_status == "pending":
        base["status"] = "log_pending"
        base["note"] = log_error or "Logs are not available yet."
        if metadata:
            base["run"] = metadata
        return base

    if log_error:
        base["status"] = "log_unavailable"
        base["error"] = log_error
        if metadata:
            base["run"] = metadata
        return base

    snippet = extract_failure_snippet(log_text, max_lines=max_lines, context=context)
    base["status"] = "ok"
    base["run"] = metadata or {}
    base["logSnippet"] = snippet
    base["logTail"] = tail_lines(log_text, max_lines)
    return base


def extract_run_id(url: str) -> str | None:
    if not url:
        return None
    for pattern in (r"/actions/runs/(\d+)", r"/runs/(\d+)"):
        match = re.search(pattern, url)
        if match:
            return match.group(1)
    return None


def extract_job_id(url: str) -> str | None:
    if not url:
        return None
    match = re.search(r"/actions/runs/\d+/job/(\d+)", url)
    if match:
        return match.group(1)
    match = re.search(r"/job/(\d+)", url)
    if match:
        return match.group(1)
    return None


def fetch_run_metadata(run_id: str, repo_root: Path) -> dict[str, Any] | None:
    fields = [
        "conclusion",
        "status",
        "workflowName",
        "name",
        "event",
        "headBranch",
        "headSha",
        "url",
    ]
    result = run_gh_command(["run", "view", run_id, "--json", ",".join(fields)], cwd=repo_root)
    if result.returncode != 0:
        return None
    try:
        data = json.loads(result.stdout or "{}")
    except json.JSONDecodeError:
        return None
    if not isinstance(data, dict):
        return None
    return data


def fetch_check_log(
    run_id: str,
    job_id: str | None,
    repo_root: Path,
) -> tuple[str, str, str]:
    log_text, log_error = fetch_run_log(run_id, repo_root)
    if not log_error:
        return log_text, "", "ok"

    if is_log_pending_message(log_error) and job_id:
        job_log, job_error = fetch_job_log(job_id, repo_root)
        if job_log:
            return job_log, "", "ok"
        if job_error and is_log_pending_message(job_error):
            return "", job_error, "pending"
        if job_error:
            return "", job_error, "error"
        return "", log_error, "pending"

    if is_log_pending_message(log_error):
        return "", log_error, "pending"

    return "", log_error, "error"


def fetch_run_log(run_id: str, repo_root: Path) -> tuple[str, str]:
    result = run_gh_command(["run", "view", run_id, "--log"], cwd=repo_root)
    if result.returncode != 0:
        error = (result.stderr or result.stdout or "").strip()
        return "", error or "gh run view failed"
    return result.stdout, ""


def fetch_job_log(job_id: str, repo_root: Path) -> tuple[str, str]:
    repo_slug = fetch_repo_slug(repo_root)
    if not repo_slug:
        return "", "Error: unable to resolve repository name for job logs."
    endpoint = f"/repos/{repo_slug}/actions/jobs/{job_id}/logs"
    returncode, stdout_bytes, stderr = run_gh_command_raw(["api", endpoint], cwd=repo_root)
    if returncode != 0:
        message = (stderr or stdout_bytes.decode(errors="replace")).strip()
        return "", message or "gh api job logs failed"
    if is_zip_payload(stdout_bytes):
        return "", "Job logs returned a zip archive; unable to parse."
    return stdout_bytes.decode(errors="replace"), ""


def normalize_field(value: Any) -> str:
    if value is None:
        return ""
    return str(value).strip().lower()


def parse_available_fields(message: str) -> list[str]:
    if "Available fields:" not in message:
        return []
    fields: list[str] = []
    collecting = False
    for line in message.splitlines():
        if "Available fields:" in line:
            collecting = True
            continue
        if not collecting:
            continue
        field = line.strip()
        if not field:
            continue
        fields.append(field)
    return fields


def is_log_pending_message(message: str) -> bool:
    lowered = message.lower()
    return any(marker in lowered for marker in PENDING_LOG_MARKERS)


def is_zip_payload(payload: bytes) -> bool:
    return payload.startswith(b"PK")


def extract_failure_snippet(log_text: str, max_lines: int, context: int) -> str:
    lines = log_text.splitlines()
    if not lines:
        return ""

    marker_index = find_failure_index(lines)
    if marker_index is None:
        return "\n".join(lines[-max_lines:])

    start = max(0, marker_index - context)
    end = min(len(lines), marker_index + context)
    window = lines[start:end]
    if len(window) > max_lines:
        window = window[-max_lines:]
    return "\n".join(window)


def find_failure_index(lines: Sequence[str]) -> int | None:
    for idx in range(len(lines) - 1, -1, -1):
        lowered = lines[idx].lower()
        if any(marker in lowered for marker in FAILURE_MARKERS):
            return idx
    return None


def tail_lines(text: str, max_lines: int) -> str:
    if max_lines <= 0:
        return ""
    lines = text.splitlines()
    return "\n".join(lines[-max_lines:])


# =============================================================================
# Output rendering
# =============================================================================

def render_comprehensive_results(results: dict[str, Any]) -> None:
    """Render all inspection results in text format."""
    pr_number = results.get("pr", "?")
    print(f"PR #{pr_number}: Comprehensive Check Results")
    print("=" * 60)

    # Conflicts
    conflicts = results.get("conflicts")
    if conflicts:
        print("\nMERGE CONFLICTS")
        print("-" * 60)
        if conflicts.get("error"):
            print(f"Error: {conflicts['error']}")
        elif conflicts.get("hasConflicts"):
            print(f"Status: {conflicts.get('mergeable', 'UNKNOWN')}")
            print(f"Merge State: {conflicts.get('mergeStateStatus', 'UNKNOWN')}")
            print(f"Base: {conflicts.get('baseRefName', '')} <- Head: {conflicts.get('headRefName', '')}")
            print("Action Required: Resolve conflicts before merging")
        else:
            print("No merge conflicts detected.")
            print(f"Mergeable: {conflicts.get('mergeable', 'UNKNOWN')}")

    # Change Requests
    change_requests = results.get("changeRequests")
    if change_requests is not None:
        print("\nCHANGE REQUESTS")
        print("-" * 60)
        if change_requests:
            for cr in change_requests:
                reviewer = cr.get("reviewer", "unknown")
                submitted_at = cr.get("submittedAt", "")[:10] if cr.get("submittedAt") else ""
                body = cr.get("body", "")
                print(f"From @{reviewer} ({submitted_at}):")
                if body:
                    print(indent_block(f'"{body}"', prefix="  "))
                else:
                    print("  (no comment body)")
                print()
        else:
            print("No change requests.")

    # Unresolved Threads
    unresolved_threads = results.get("unresolvedThreads")
    if unresolved_threads is not None:
        print("\nUNRESOLVED REVIEW THREADS")
        print("-" * 60)
        if unresolved_threads:
            for i, thread in enumerate(unresolved_threads, 1):
                path = thread.get("path", "unknown")
                line = thread.get("line")
                location = f"{path}:{line}" if line else path
                outdated = " (outdated)" if thread.get("isOutdated") else ""
                print(f"[{i}] {location}{outdated}")
                print(f"    Thread ID: {thread.get('id', 'unknown')}")
                for comment in thread.get("comments", []):
                    author = comment.get("author", "unknown")
                    body = comment.get("body", "")
                    print(f"    @{author}: {body[:100]}{'...' if len(body) > 100 else ''}")
                print()
        else:
            print("No unresolved review threads.")

    # Resolved threads count
    resolved_count = results.get("resolvedThreadsCount")
    if resolved_count is not None:
        print(f"\nResolved {resolved_count} thread(s).")

    # CI Failures
    ci_failures = results.get("ciFailures")
    if ci_failures is not None:
        print("\nCI FAILURES")
        print("-" * 60)
        if ci_failures:
            render_ci_results(ci_failures)
        else:
            print("No CI failures detected.")

    ci_error = results.get("ciError")
    if ci_error:
        print(f"CI Error: {ci_error}")

    print("=" * 60)


def render_ci_results(results: Iterable[dict[str, Any]]) -> None:
    """Render CI check failure results."""
    results_list = list(results)
    for result in results_list:
        print(f"Check: {result.get('name', '')}")
        if result.get("detailsUrl"):
            print(f"Details: {result['detailsUrl']}")
        run_id = result.get("runId")
        if run_id:
            print(f"Run ID: {run_id}")
        job_id = result.get("jobId")
        if job_id:
            print(f"Job ID: {job_id}")
        status = result.get("status", "unknown")
        print(f"Status: {status}")

        run_meta = result.get("run", {})
        if run_meta:
            branch = run_meta.get("headBranch", "")
            sha = (run_meta.get("headSha") or "")[:12]
            workflow = run_meta.get("workflowName") or run_meta.get("name") or ""
            conclusion = run_meta.get("conclusion") or run_meta.get("status") or ""
            print(f"Workflow: {workflow} ({conclusion})")
            if branch or sha:
                print(f"Branch/SHA: {branch} {sha}")
            if run_meta.get("url"):
                print(f"Run URL: {run_meta['url']}")

        if result.get("note"):
            print(f"Note: {result['note']}")

        if result.get("error"):
            print(f"Error fetching logs: {result['error']}")
            print()
            continue

        snippet = result.get("logSnippet") or ""
        if snippet:
            print("Failure snippet:")
            print(indent_block(snippet, prefix="  "))
        else:
            print("No snippet available.")
        print()


def indent_block(text: str, prefix: str = "  ") -> str:
    return "\n".join(f"{prefix}{line}" for line in text.splitlines())


if __name__ == "__main__":
    raise SystemExit(main())
