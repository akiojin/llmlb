#!/usr/bin/env bats

# check-mapping-freshness.sh の契約テスト（Issue #776）
# canonical テーブルの最終確認日が古くなったことを、ネットワークなしで検出できることを確認する。

setup() {
    SCRIPT="$BATS_TEST_DIRNAME/../../scripts/checks/check-mapping-freshness.sh"
    [ -x "$SCRIPT" ]

    export MAPPING_FILE="$BATS_TEST_TMPDIR/mapping.rs"
}

# 今日から N 日ずらした日付（UTC, YYYY-MM-DD）。負数は過去、正数は未来。
# 日付境界をまたいでも結果が変わらないよう、テストは閾値ちょうどの値を使わない。
date_offset() {
    local days="$1"
    if date -u -v+0d +%F >/dev/null 2>&1; then
        if [ "$days" -ge 0 ]; then
            date -u -v+"${days}"d +%F
        else
            date -u -v"${days}"d +%F
        fi
    else
        date -u -d "${days} days" +%F
    fi
}

# write_mapping <canonical>=<last_verified>...
# last_verified が "-" のエントリは last_verified 行を書かない。
write_mapping() {
    {
        echo 'pub static BUILTIN_MAPPINGS: &[ModelMapping] = &['
        local entry
        for entry in "$@"; do
            echo '    ModelMapping {'
            echo "        canonical: \"${entry%%=*}\","
            if [ "${entry#*=}" != "-" ]; then
                echo "        last_verified: \"${entry#*=}\","
            fi
            echo '        aliases: &[],'
            echo '    },'
        done
        echo '];'
        echo
        echo 'const KNOWN_CONTEXT_LENGTHS: &[(&str, u32)] = &[("org/outside-table", 1)];'
    } >"$MAPPING_FILE"
}

@test "全エントリが閾値以内なら成功する" {
    write_mapping "org/model-a=$(date_offset 0)" "org/model-b=$(date_offset -179)"
    run "$SCRIPT"
    [ "$status" -eq 0 ]
    [[ "$output" == *"2 件"* ]] || false
}

@test "閾値を超えたエントリがあると名指しで失敗する" {
    write_mapping "org/model-a=$(date_offset 0)" "org/model-stale=$(date_offset -182)"
    run "$SCRIPT"
    [ "$status" -eq 1 ]
    [[ "$output" == *"org/model-stale"* ]] || false
    [[ "$output" != *"✗ org/model-a"* ]] || false
}

@test "既定の閾値は 180 日である" {
    write_mapping "org/model-a=$(date_offset -179)"
    run "$SCRIPT"
    [ "$status" -eq 0 ]

    write_mapping "org/model-a=$(date_offset -182)"
    run "$SCRIPT"
    [ "$status" -eq 1 ]
}

@test "閾値は MAPPING_MAX_AGE_DAYS で変更できる" {
    write_mapping "org/model-a=$(date_offset -30)"
    MAPPING_MAX_AGE_DAYS=10 run "$SCRIPT"
    [ "$status" -eq 1 ]
    [[ "$output" == *"org/model-a"* ]] || false
}

@test "不正な閾値はエラーになる" {
    write_mapping "org/model-a=$(date_offset 0)"
    MAPPING_MAX_AGE_DAYS=abc run "$SCRIPT"
    [ "$status" -eq 2 ]
}

@test "最終確認日が欠落したエントリは失敗する" {
    write_mapping "org/model-a=$(date_offset 0)" "org/model-missing=-" "org/model-c=$(date_offset 0)"
    run "$SCRIPT"
    [ "$status" -eq 1 ]
    [[ "$output" == *"org/model-missing"* ]] || false
}

@test "日付として不正な最終確認日は失敗する" {
    write_mapping "org/model-a=2026-02-30"
    run "$SCRIPT"
    [ "$status" -eq 1 ]
    [[ "$output" == *"org/model-a"* ]] || false

    write_mapping "org/model-a=2026/01/15"
    run "$SCRIPT"
    [ "$status" -eq 1 ]
}

@test "未来の最終確認日は失敗する" {
    write_mapping "org/model-a=$(date_offset 2)"
    run "$SCRIPT"
    [ "$status" -eq 1 ]
    [[ "$output" == *"org/model-a"* ]] || false
}

@test "テーブル外の記述は検査対象にしない" {
    write_mapping "org/model-a=$(date_offset 0)"
    run "$SCRIPT"
    [ "$status" -eq 0 ]
    [[ "$output" == *"1 件"* ]] || false
}

@test "エントリが 1 件も読み取れない場合はエラーになる" {
    echo 'pub static OTHER: &[u8] = &[];' >"$MAPPING_FILE"
    run "$SCRIPT"
    [ "$status" -eq 2 ]
}

@test "対象ファイルが無い場合はエラーになる" {
    MAPPING_FILE="$BATS_TEST_TMPDIR/missing.rs" run "$SCRIPT"
    [ "$status" -eq 2 ]
}

@test "ネットワークに依存しない" {
    write_mapping "org/model-a=$(date_offset 0)"
    # プロキシを到達不能な宛先へ向けても結果が変わらない
    http_proxy=http://127.0.0.1:1 https_proxy=http://127.0.0.1:1 run "$SCRIPT"
    [ "$status" -eq 0 ]
    ! grep -Eq '\b(curl|wget|nc|ssh|git fetch)\b' "$SCRIPT"
}

@test "リポジトリの実テーブルを読み取れる" {
    # 鮮度そのものは make mapping-freshness が検査する。ここでは実ファイルの書式を
    # スクリプトが解釈できること（エラー終了しないこと）だけを確認する。
    unset MAPPING_FILE
    cd "$BATS_TEST_DIRNAME/../.."
    run "$SCRIPT"
    [ "$status" -ne 2 ]
    [[ "$output" == *"canonical"* ]] || false
}
