#!/usr/bin/env bats

# event-publishers（SPEC #582 FR-048e / Issue #781）の判定と配線のテスト
# canonical ダッシュボードイベントのうち 3 種は enum 定義とシリアライズテストだけが存在し、
# production から一度も発行されていなかった。ポーリングで画面が更新されるため症状が出ない。
# 検査が publisher の欠落を検出すること、必須チェックと quality-checks の両方で走ることを保証する。

setup() {
    REPO_ROOT="$BATS_TEST_DIRNAME/../.."
    LINT_WORKFLOW="$REPO_ROOT/.github/workflows/lint.yml"
    MAKEFILE="$REPO_ROOT/Makefile"
    SCRIPT="$REPO_ROOT/scripts/checks/check-event-publishers.sh"
    [ -f "$LINT_WORKFLOW" ]
    [ -f "$MAKEFILE" ]

    SRC="$BATS_TEST_TMPDIR/src"
    ALLOW="$BATS_TEST_TMPDIR/allowlist.txt"
    mkdir -p "$SRC/events" "$SRC/api"
    printf '# empty\n' > "$ALLOW"
    cat > "$SRC/events/bus.rs" <<'EOF'
/// ダッシュボードイベント
#[derive(Debug, Clone, Serialize)]
pub enum DashboardEvent {
    /// ノード登録イベント
    NodeRegistered {
        /// ランタイムID
        runtime_id: Uuid,
    },
    /// アップデート状態変更イベント
    UpdateStateChanged,
}

impl DashboardEventBus {
    pub fn publish(&self, event: DashboardEvent) {}
}

#[cfg(test)]
mod tests {
    #[test]
    fn publishes_in_definition_module_tests() {
        bus.publish(DashboardEvent::NodeRegistered { runtime_id });
        bus.publish(DashboardEvent::UpdateStateChanged);
    }
}
EOF
}

# lint.yml から指定ジョブ（2 スペースインデントのキー）のブロックを抜き出す
job_block() {
    awk -v job="  $1:" '
        $0 == job { in_job = 1; print; next }
        in_job && /^  [A-Za-z0-9_-]+:/ { exit }
        in_job { print }
    ' "$LINT_WORKFLOW"
}

run_check() {
    SRC_DIR="$SRC" ALLOWLIST="$ALLOW" run bash "$SCRIPT"
}

publish_both() {
    cat > "$SRC/api/endpoints.rs" <<'EOF'
pub async fn create_endpoint(state: AppState) {
    state
        .event_bus
        .publish(crate::events::DashboardEvent::NodeRegistered {
            runtime_id: endpoint.id,
        });
    state
        .event_bus
        .publish(crate::events::DashboardEvent::UpdateStateChanged);
}
EOF
}

@test "必須チェック Rust Format & Clippy が event-publishers を実行する" {
    block="$(job_block rust-lint)"
    [[ "$block" == *"name: Rust Format & Clippy"* ]] || false
    [[ "$block" == *"make event-publishers"* ]] || false
}

@test "event-publishers の検査スクリプト変更で Rust lint が起動する" {
    block="$(job_block changes)"
    [[ "$block" == *"'scripts/checks/check-event-publishers.sh'"* ]] || false
}

@test "event-publishers の許可リスト変更で Rust lint が起動する" {
    block="$(job_block changes)"
    [[ "$block" == *"'scripts/checks/event-publishers-allowlist.txt'"* ]] || false
}

@test "make quality-checks が event-publishers を実行する" {
    targets="$(grep -E '^quality-checks:' "$MAKEFILE")"
    [[ " $targets " == *" event-publishers "* ]] || false
    grep -qE '^event-publishers:' "$MAKEFILE"
}

@test "全バリアントに production の publisher があれば成功する" {
    publish_both
    run_check
    [ "$status" -eq 0 ]
}

@test "publisher が無いバリアントがあると失敗する" {
    cat > "$SRC/api/endpoints.rs" <<'EOF'
pub async fn apply(state: AppState) {
    state.event_bus.publish(crate::events::DashboardEvent::UpdateStateChanged);
}
EOF
    run_check
    [ "$status" -eq 1 ]
    [[ "$output" == *"NodeRegistered"* ]] || false
    [[ "$output" != *"UpdateStateChanged に"* ]] || false
}

@test "イベント定義モジュール内の publish は publisher として数えない" {
    cat > "$SRC/events/helper.rs" <<'EOF'
pub fn helper(bus: &DashboardEventBus) {
    bus.publish(DashboardEvent::NodeRegistered { runtime_id });
    bus.publish(DashboardEvent::UpdateStateChanged);
}
EOF
    run_check
    [ "$status" -eq 1 ]
    [[ "$output" == *"NodeRegistered"* ]] || false
    [[ "$output" == *"UpdateStateChanged"* ]] || false
}

@test "tests.rs の publish は publisher として数えない" {
    publish_both
    mkdir -p "$SRC/api/endpoints"
    mv "$SRC/api/endpoints.rs" "$SRC/api/endpoints/tests.rs"
    run_check
    [ "$status" -eq 1 ]
    [[ "$output" == *"NodeRegistered"* ]] || false
}

@test "#[cfg(test)] 以降の publish は publisher として数えない" {
    cat > "$SRC/api/endpoints.rs" <<'EOF'
pub async fn apply(state: AppState) {
    state.event_bus.publish(crate::events::DashboardEvent::UpdateStateChanged);
}

#[cfg(test)]
mod tests {
    #[test]
    fn publishes() {
        bus.publish(crate::events::DashboardEvent::NodeRegistered { runtime_id });
    }
}
EOF
    run_check
    [ "$status" -eq 1 ]
    [[ "$output" == *"NodeRegistered"* ]] || false
}

@test "publish の引数ではない参照（match 等）は publisher として数えない" {
    cat > "$SRC/api/endpoints.rs" <<'EOF'
pub fn describe(event: &DashboardEvent) -> &'static str {
    match event {
        DashboardEvent::NodeRegistered { .. } => "registered",
        DashboardEvent::UpdateStateChanged => "update",
    }
}
EOF
    run_check
    [ "$status" -eq 1 ]
    [[ "$output" == *"NodeRegistered"* ]] || false
    [[ "$output" == *"UpdateStateChanged"* ]] || false
}

@test "publish_coalesced と改行を挟んだ引数も publisher として数える" {
    cat > "$SRC/api/endpoints.rs" <<'EOF'
pub async fn apply(state: AppState) {
    event_bus.publish_coalesced(
        crate::events::DashboardEvent::NodeRegistered {
            runtime_id: endpoint_id,
        },
    );
    bus.publish(DashboardEvent::UpdateStateChanged);
}
EOF
    run_check
    [ "$status" -eq 0 ]
}

@test "前方一致する別名のバリアントを publisher と誤認しない" {
    cat > "$SRC/api/endpoints.rs" <<'EOF'
pub async fn apply(state: AppState) {
    bus.publish(DashboardEvent::NodeRegisteredLegacy { runtime_id });
    bus.publish(DashboardEvent::UpdateStateChanged);
}
EOF
    run_check
    [ "$status" -eq 1 ]
    [[ "$output" == *"NodeRegistered"* ]] || false
}

@test "許可リストにあるバリアントは publisher が無くても成功する" {
    cat > "$SRC/api/endpoints.rs" <<'EOF'
pub async fn apply(state: AppState) {
    state.event_bus.publish(crate::events::DashboardEvent::UpdateStateChanged);
}
EOF
    printf '# 別 Issue の担当\nNodeRegistered\n' > "$ALLOW"
    run_check
    [ "$status" -eq 0 ]
}

@test "publisher があるのに許可リストに残っているエントリは失敗する" {
    publish_both
    printf 'NodeRegistered\n' > "$ALLOW"
    run_check
    [ "$status" -eq 1 ]
    [[ "$output" == *"許可リスト不要"* ]] || false
    [[ "$output" == *"NodeRegistered"* ]] || false
}

@test "enum に存在しないバリアントを指す許可リストエントリは失敗する" {
    publish_both
    printf 'NodeStatusChanged\n' > "$ALLOW"
    run_check
    [ "$status" -eq 1 ]
    [[ "$output" == *"NodeStatusChanged"* ]] || false
}

@test "enum からバリアントを抽出できない場合はエラーにする" {
    publish_both
    printf 'pub struct NotAnEnum;\n' > "$SRC/events/bus.rs"
    run_check
    [ "$status" -eq 2 ]
}

@test "リポジトリの現状は event-publishers を通過する" {
    cd "$REPO_ROOT"
    run bash "$SCRIPT"
    [ "$status" -eq 0 ]
}
