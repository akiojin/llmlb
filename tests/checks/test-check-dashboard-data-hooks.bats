#!/usr/bin/env bats

# dashboard-data-hooks（SPEC #821 T016 / Issue #824）の判定と配線のテスト
# `components/` 配下のコンポーネントが React Query のフック（useQuery / useMutation /
# useQueryClient）を直接呼ぶと、データ所有がコンポーネントに散り MVVM 移行が後退する。
# 現違反は許可リストに載せたまま検査を導入し、以降の移行で許可リストを減らしていく
# （ラチェット）。検査が新規の直接呼び出しを検出し、許可リストが単調に縮むことを保証する。

setup() {
    REPO_ROOT="$BATS_TEST_DIRNAME/../.."
    MAKEFILE="$REPO_ROOT/Makefile"
    SCRIPT="$REPO_ROOT/scripts/checks/check-dashboard-data-hooks.sh"
    [ -f "$MAKEFILE" ]

    # 許可リストのエントリは SRC_DIR からの相対パスなので、相対の SRC_DIR で実行する
    cd "$BATS_TEST_TMPDIR"
    SRC="components"
    ALLOW="$BATS_TEST_TMPDIR/allowlist.txt"
    mkdir -p "$SRC/dashboard" "$SRC/users"
    printf '# empty\n' > "$ALLOW"
    cat > "$SRC/dashboard/Clean.tsx" <<'EOF'
import { useModelsViewModel } from '@/viewmodels/useModelsViewModel'

export function Clean() {
  const { models } = useModelsViewModel()
  return <ul>{models.map((m) => <li key={m.id}>{m.id}</li>)}</ul>
}
EOF
}

run_check() {
    SRC_DIR="$SRC" ALLOWLIST="$ALLOW" run bash "$SCRIPT"
}

write_use_query() {
    cat > "$SRC/dashboard/Fetching.tsx" <<'EOF'
import { useQuery } from '@tanstack/react-query'

export function Fetching() {
  const { data } = useQuery({ queryKey: ['models'], queryFn: fetchModels })
  return <div>{data?.length}</div>
}
EOF
}

@test "make quality-checks が dashboard-data-hooks を実行する" {
    targets="$(grep -E '^quality-checks:' "$MAKEFILE")"
    [[ " $targets " == *" dashboard-data-hooks "* ]] || false
    grep -qE '^dashboard-data-hooks:' "$MAKEFILE"
    grep -qE '^\.PHONY:.*\bdashboard-data-hooks\b' "$MAKEFILE"
}

@test "直接呼び出しが無ければ成功する" {
    run_check
    [ "$status" -eq 0 ]
}

@test "許可リストに無いファイルの useQuery 直接呼び出しは失敗する" {
    write_use_query
    run_check
    [ "$status" -eq 1 ]
    [[ "$output" == *"components/dashboard/Fetching.tsx"* ]] || false
    [[ "$output" != *"Clean.tsx"* ]] || false
}

@test "useMutation の直接呼び出しも失敗する" {
    cat > "$SRC/users/Mutating.tsx" <<'EOF'
import { useMutation } from '@tanstack/react-query'

export function Mutating() {
  const mutation = useMutation({ mutationFn: createUser })
  return <button onClick={() => mutation.mutate()}>add</button>
}
EOF
    run_check
    [ "$status" -eq 1 ]
    [[ "$output" == *"components/users/Mutating.tsx"* ]] || false
}

@test "useQueryClient の直接呼び出しも失敗する" {
    cat > "$SRC/users/Invalidating.tsx" <<'EOF'
import { useQueryClient } from '@tanstack/react-query'

export function Invalidating() {
  const queryClient = useQueryClient()
  return <button onClick={() => queryClient.invalidateQueries()}>refresh</button>
}
EOF
    run_check
    [ "$status" -eq 1 ]
    [[ "$output" == *"components/users/Invalidating.tsx"* ]] || false
}

@test "ジェネリクス付きの useQuery<T>( も検出する" {
    cat > "$SRC/dashboard/Generic.tsx" <<'EOF'
export function Generic() {
  const { data } = useQuery<Stats[]>({ queryKey: ['stats'], queryFn: fetchStats })
  return <div>{data?.length}</div>
}
EOF
    run_check
    [ "$status" -eq 1 ]
    [[ "$output" == *"components/dashboard/Generic.tsx"* ]] || false
}

@test "前方一致する別名（useQueryClientRef 等）は検出しない" {
    cat > "$SRC/dashboard/Named.tsx" <<'EOF'
import { useQueryClientRef, useQueryState } from '@/viewmodels/shared'

export function Named() {
  const ref = useQueryClientRef()
  const [state] = useQueryState()
  return <div>{String(ref.current)}{state}</div>
}
EOF
    run_check
    [ "$status" -eq 0 ]
}

@test "テストファイル（*.test.tsx）は検査対象にしない" {
    cat > "$SRC/dashboard/Clean.test.tsx" <<'EOF'
import { useQueryClient } from '@tanstack/react-query'

function Probe() {
  const client = useQueryClient()
  return <span>{client.getQueryCache().getAll().length}</span>
}
EOF
    run_check
    [ "$status" -eq 0 ]
}

@test "許可リストにあるファイルの直接呼び出しは成功する" {
    write_use_query
    printf '# 移行待ち\ndashboard/Fetching.tsx\n' > "$ALLOW"
    run_check
    [ "$status" -eq 0 ]
}

@test "直接呼び出しが無いのに許可リストに残っているエントリは失敗する" {
    printf 'dashboard/Clean.tsx\n' > "$ALLOW"
    run_check
    [ "$status" -eq 1 ]
    [[ "$output" == *"許可リスト不要"* ]] || false
    [[ "$output" == *"dashboard/Clean.tsx"* ]] || false
}

@test "存在しないファイルを指す許可リストエントリは失敗する" {
    printf 'dashboard/Removed.tsx\n' > "$ALLOW"
    run_check
    [ "$status" -eq 1 ]
    [[ "$output" == *"dashboard/Removed.tsx"* ]] || false
}

@test "対象ディレクトリが無い場合はエラーにする" {
    SRC="missing-components"
    run_check
    [ "$status" -eq 2 ]
}

@test "許可リストが無い場合はエラーにする" {
    ALLOW="$BATS_TEST_TMPDIR/missing-allowlist.txt"
    run_check
    [ "$status" -eq 2 ]
}

@test "リポジトリの現状は dashboard-data-hooks を通過する" {
    cd "$REPO_ROOT"
    run bash "$SCRIPT"
    [ "$status" -eq 0 ]
}
