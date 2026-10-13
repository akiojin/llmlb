#!/usr/bin/env bats

setup() {
    ROOT="$BATS_TEST_DIRNAME/../.."
}

@test "HF discovery offline acceptance suite passes" {
    run python3 -m unittest discover -s "$ROOT/tests/models" -v
    [ "$status" -eq 0 ]
}

@test "weekly discovery has schedule, manual trigger, limited permissions and concurrency" {
    workflow="$ROOT/.github/workflows/model-discovery.yml"
    [ -f "$workflow" ]
    # CI does not install ripgrep; keep these assertions on standard tools.
    run grep -E 'cron:.*0 3 \* \* 1' "$workflow"
    [ "$status" -eq 0 ]
    run grep -E 'workflow_dispatch:|contents: read|issues: write|group: hf-model-discovery|ref: develop|discover_hf_models.py --publish' "$workflow"
    [ "$status" -eq 0 ]
    [ "${#lines[@]}" -eq 6 ]
}
