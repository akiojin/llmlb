#!/usr/bin/env bash
# migrate-specs-to-issues.sh
# Migrate local specs/SPEC-*/ directories to GitHub Issues with gwt-spec label.
#
# Usage:
#   ./migrate-specs-to-issues.sh [--dry-run] [--specs-dir DIR] [--label LABEL]...
#
# Options:
#   --dry-run       Show what would be done without creating issues
#   --specs-dir     Path to specs/ directory (default: auto-detect from target repository)
#   --label LABEL   Additional label to apply (can be repeated; gwt-spec is always applied)

set -euo pipefail
shopt -s nullglob

DRY_RUN=false
SPECS_DIR=""
REPORT_FILE="migration-report.json"
RATE_LIMIT_BATCH=10
RATE_LIMIT_SLEEP=3
declare -a EXTRA_LABELS=()
REPO_ROOT=""
declare -a SPEC_DIRS=()
declare -a CLEANUP_TARGETS=()
declare -a LEGACY_CLEANUP_ALLOWLIST_RELATIVE=(
  ".specify"
  ".github/spec-kit"
  ".github/spec-kit.yml"
  ".github/spec-kit.yaml"
  ".github/prompts/specify.md"
  ".github/prompts/specify-system.md"
  "scripts/spec-kit.sh"
  "scripts/specify.sh"
  "templates/spec-kit"
  "templates/spec-kit.md"
  "templates/specify"
  "templates/specify.md"
)

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run)
      DRY_RUN=true
      shift
      ;;
    --specs-dir)
      SPECS_DIR="$2"
      shift 2
      ;;
    --label)
      EXTRA_LABELS+=("$2")
      shift 2
      ;;
    *)
      echo "Unknown option: $1" >&2
      exit 1
      ;;
  esac
done

resolve_repo_root() {
  if [[ -n "${GWT_PROJECT_ROOT:-}" ]]; then
    printf '%s\n' "$GWT_PROJECT_ROOT"
    return 0
  fi

  git rev-parse --show-toplevel 2>/dev/null || true
}

append_cleanup_target() {
  local target="$1"
  [[ -e "$target" ]] || return 0
  for existing in "${CLEANUP_TARGETS[@]-}"; do
    [[ -n "$existing" ]] || continue
    [[ "$existing" == "$target" ]] && return 0
  done
  CLEANUP_TARGETS+=("$target")
}

collect_cleanup_targets() {
  CLEANUP_TARGETS=()

  for dir in "${SPEC_DIRS[@]}"; do
    append_cleanup_target "$dir"
  done

  append_cleanup_target "$SPECS_DIR/specs.md"
  append_cleanup_target "$SPECS_DIR/archive"

  for relative_path in "${LEGACY_CLEANUP_ALLOWLIST_RELATIVE[@]}"; do
    append_cleanup_target "$REPO_ROOT/$relative_path"
  done
}

preview_cleanup_targets() {
  collect_cleanup_targets
  echo ""
  echo "Legacy cleanup plan:"
  if [[ ${#CLEANUP_TARGETS[@]:-0} -eq 0 ]]; then
    echo "  (no legacy paths detected beyond migration report handling)"
  else
    for path in "${CLEANUP_TARGETS[@]-}"; do
      [[ -n "$path" ]] || continue
      echo "  - $path"
    done
  fi
  echo "  - $REPORT_FILE (delete after successful cleanup)"
}

cleanup_legacy_sources() {
  collect_cleanup_targets

  echo ""
  echo "Cleaning up legacy local spec artifacts..."
  for path in "${CLEANUP_TARGETS[@]-}"; do
    [[ -n "$path" ]] || continue
    [[ -e "$path" ]] || continue
    echo "  Removing $path"
    rm -rf "$path"
  done

  if [[ -d "$SPECS_DIR" ]] && [[ -z "$(ls -A "$SPECS_DIR" 2>/dev/null)" ]]; then
    echo "  Removing empty $SPECS_DIR"
    rmdir "$SPECS_DIR"
  fi

  if [[ -f "$REPORT_FILE" ]]; then
    echo "  Removing $REPORT_FILE"
    rm -f "$REPORT_FILE"
  fi
}

write_empty_report_and_exit() {
  echo "[]" > "$REPORT_FILE"
  echo ""
  echo "Migration complete: 0 succeeded, 0 failed out of 0 total"
  echo "Report: $REPORT_FILE"
  exit 0
}

if [[ -z "$SPECS_DIR" ]]; then
  REPO_ROOT=$(resolve_repo_root)
  if [[ -z "$REPO_ROOT" ]]; then
    echo "Error: target repository root not found. Set GWT_PROJECT_ROOT or run inside a git repository." >&2
    exit 1
  fi

  for candidate in "$REPO_ROOT/specs"; do
    if [[ -d "$candidate" ]]; then
      SPECS_DIR="$candidate"
      break
    fi
  done

  if [[ -z "$SPECS_DIR" ]]; then
    echo "Specs directory not found under target repository. Nothing to migrate."
    write_empty_report_and_exit
  fi
elif [[ ! -d "$SPECS_DIR" ]]; then
  echo "Error: specs directory not found: $SPECS_DIR" >&2
  exit 1
fi

if [[ -z "$REPO_ROOT" ]]; then
  REPO_ROOT=$(cd "$SPECS_DIR/.." && pwd -P)
fi

echo "Specs directory: $SPECS_DIR"
echo "Dry run: $DRY_RUN"

# Collect SPEC directories (exclude archive/)
for dir in "$SPECS_DIR"/SPEC-*/; do
  [[ -d "$dir" ]] || continue
  SPEC_DIRS+=("$dir")
done

echo "Found ${#SPEC_DIRS[@]} spec directories to migrate"

if [[ ${#SPEC_DIRS[@]} -eq 0 ]]; then
  write_empty_report_and_exit
fi

# Initialize report
echo "[" > "$REPORT_FILE"
FIRST_ENTRY=true
COUNT=0
SUCCESS=0
FAILED=0

read_section() {
  local file="$1"
  if [[ -f "$file" ]]; then
    cat "$file"
  else
    echo "_TODO_"
  fi
}

# Extract title from spec.md (first line starting with #)
extract_title() {
  local spec_file="$1"
  local title=""
  if [[ -f "$spec_file" ]]; then
    title=$(awk '/^#/ {sub(/^#+[[:space:]]*/, ""); print; exit}' "$spec_file" || true)
    if [[ -n "$title" ]]; then
      echo "$title" | head -c 200
    else
      echo "Untitled spec"
    fi
  else
    echo "Untitled spec"
  fi
}

# Build issue body from spec directory files
build_issue_body() {
  local dir="$1"
  local spec_id="$2"
  local spec_content plan_content tasks_content tdd_content
  local research_content data_model_content quickstart_content
  local contracts_note checklists_note

  spec_content=$(read_section "$dir/spec.md")
  plan_content=$(read_section "$dir/plan.md")
  tasks_content=$(read_section "$dir/tasks.md")
  tdd_content=$(read_section "$dir/tdd.md")
  research_content=$(read_section "$dir/research.md")
  data_model_content=$(read_section "$dir/data-model.md")
  quickstart_content=$(read_section "$dir/quickstart.md")

  if [[ -d "$dir/contracts" ]] && [[ -n "$(ls -A "$dir/contracts" 2>/dev/null)" ]]; then
    contracts_note="Migrated from local files. See artifact comments below."
  else
    contracts_note="Artifact files under \`contracts/\` are managed in issue comments with \`contract:<name>\` entries."
  fi

  if [[ -d "$dir/checklists" ]] && [[ -n "$(ls -A "$dir/checklists" 2>/dev/null)" ]]; then
    checklists_note="Migrated from local files. See artifact comments below."
  else
    checklists_note="Artifact files under \`checklists/\` are managed in issue comments with \`checklist:<name>\` entries."
  fi

  cat <<BODY
<!-- GWT_SPEC_ID:${spec_id} -->

## Spec

${spec_content}

## Plan

${plan_content}

## Tasks

${tasks_content}

## TDD

${tdd_content}

## Research

${research_content}

## Data Model

${data_model_content}

## Quickstart

${quickstart_content}

## Contracts

${contracts_note}

## Checklists

${checklists_note}

## Acceptance Checklist

- [ ] Add acceptance checklist
BODY
}

create_artifact_comments() {
  local dir="$1"
  local issue_number="$2"
  local had_error=0

  for subdir in contracts checklists; do
    local artifact_dir="$dir/$subdir"
    [[ -d "$artifact_dir" ]] || continue

    local kind="${subdir%s}"
    for file in "$artifact_dir"/*; do
      [[ -f "$file" ]] || continue
      local name
      name=$(basename "$file")
      local content
      content=$(cat "$file")

      if [[ "$DRY_RUN" == "true" ]]; then
        echo "  [dry-run] Would create $kind artifact comment: $name"
      else
        local comment_body
        comment_body=$(cat <<ARTIFACT
<!-- GWT_SPEC_ARTIFACT:${kind}:${name} -->
${kind}:${name}

${content}
ARTIFACT
)
        if ! gh issue comment "$issue_number" --body "$comment_body" > /dev/null 2>&1; then
          echo "  Warning: Failed to create $kind artifact: $name" >&2
          had_error=1
        fi
      fi
    done
  done

  return "$had_error"
}

add_report_entry() {
  local old_id="$1"
  local issue_number="$2"
  local title="$3"
  local status="$4"

  if [[ "$FIRST_ENTRY" == "true" ]]; then
    FIRST_ENTRY=false
  else
    echo "," >> "$REPORT_FILE"
  fi

  title=$(echo "$title" | sed 's/\\/\\\\/g; s/"/\\"/g; s/\t/\\t/g' | tr -d '\n')

  cat >> "$REPORT_FILE" <<ENTRY
  {"oldSpecId": "${old_id}", "issueNumber": ${issue_number}, "title": "${title}", "status": "${status}"}
ENTRY
}

for dir in "${SPEC_DIRS[@]}"; do
  spec_name=$(basename "$dir")
  spec_file="$dir/spec.md"

  title=$(extract_title "$spec_file")
  if [[ -z "$title" || "$title" == "Untitled spec" ]]; then
    title="$spec_name"
  fi

  COUNT=$((COUNT + 1))

  if [[ "$DRY_RUN" == "true" ]]; then
    echo "[$COUNT] [dry-run] Would create issue: $title (from $spec_name)"
    add_report_entry "$spec_name" 0 "$title" "dry-run"
    continue
  fi

  echo "[$COUNT] Creating issue: $title"

  issue_body=$(build_issue_body "$dir" "$spec_name")

  label_args=("--label" "gwt-spec")
  for label in "${EXTRA_LABELS[@]}"; do
    label_args+=("--label" "$label")
  done

  if issue_url=$(gh issue create --title "$title" --body "$issue_body" "${label_args[@]}" 2>/dev/null); then
    migration_ok=true
    issue_number=$(echo "$issue_url" | sed 's#.*/##')
    echo "  Created issue #$issue_number"

    updated_body=$(build_issue_body "$dir" "#${issue_number}")
    if ! gh issue edit "$issue_number" --body "$updated_body" > /dev/null 2>&1; then
      echo "  Warning: Failed to update issue body for $spec_name" >&2
      migration_ok=false
    fi

    if ! create_artifact_comments "$dir" "$issue_number"; then
      migration_ok=false
    fi

    if [[ "$migration_ok" == "true" ]]; then
      add_report_entry "$spec_name" "$issue_number" "$title" "success"
      SUCCESS=$((SUCCESS + 1))
    else
      add_report_entry "$spec_name" "$issue_number" "$title" "failed"
      FAILED=$((FAILED + 1))
    fi
  else
    echo "  Failed to create issue for $spec_name" >&2
    add_report_entry "$spec_name" 0 "$title" "failed"
    FAILED=$((FAILED + 1))
  fi

  if (( COUNT % RATE_LIMIT_BATCH == 0 )) && (( COUNT < ${#SPEC_DIRS[@]} )); then
    echo "  Rate limit pause: sleeping ${RATE_LIMIT_SLEEP}s..."
    sleep "$RATE_LIMIT_SLEEP"
  fi
done

echo "" >> "$REPORT_FILE"
echo "]" >> "$REPORT_FILE"

echo ""
echo "Migration complete: $SUCCESS succeeded, $FAILED failed out of $COUNT total"

if [[ "$DRY_RUN" == "true" ]]; then
  echo "Report: $REPORT_FILE"
  preview_cleanup_targets
elif (( FAILED > 0 )); then
  echo "Report: $REPORT_FILE"
  echo "Cleanup skipped because some migrations failed."
else
  cleanup_legacy_sources
  echo "Legacy cleanup complete. Report removed: $REPORT_FILE"
fi
