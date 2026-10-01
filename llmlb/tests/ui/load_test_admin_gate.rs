// Load Test を admin ロール限定にするフロント実装のソースレベル回帰テスト。
//
// viewer に Load Test トグルが表示されないこと、admin が実行すると
// `chatApi.completeLoadTest` が呼ばれることは、描画して検証する
// `llmlb/src/web/dashboard/src/pages/LoadBalancerPlayground.test.tsx` が担う。
// ここには、描画では観測できないソースの性質だけを残す。

fn lb_playground_source() -> String {
    include_str!("../../src/web/dashboard/src/pages/LoadBalancerPlayground.tsx").to_string()
}

fn chat_api_source() -> String {
    include_str!("../../src/web/dashboard/src/lib/api/chat.ts").to_string()
}

// トグルが非表示の viewer は UI から startLoadTest に到達できないため、
// 多重防御のガードはソースで担保する。
#[test]
fn start_load_test_guards_on_admin() {
    let source = lb_playground_source();
    assert!(
        source.contains("if (!isAdmin"),
        "startLoadTest must bail out for non-admin users"
    );
}

#[test]
fn load_test_uses_admin_only_endpoint() {
    let chat = chat_api_source();
    assert!(
        chat.contains("completeLoadTest")
            && chat.contains("/api/dashboard/playground/load-test/chat/completions"),
        "chat API must expose completeLoadTest targeting the admin-only load-test endpoint"
    );
    // 通常 Chat は従来の全ユーザー向けエンドポイントのまま
    assert!(
        chat.contains("/api/dashboard/playground/chat/completions"),
        "regular chat endpoint must remain for all users"
    );
}
