// Load Test を admin ロール限定にするフロント実装のソースレベル回帰テスト。
//
// viewer に Load Test トグルが表示されないこと、admin が実行すると
// `chatApi.completeLoadTest` が呼ばれることは、描画して検証する
// `llmlb/src/web/dashboard/src/pages/LoadBalancerPlayground.test.tsx` が担う。
// ここには、描画では観測できないソースの性質だけを残す。

fn lb_playground_viewmodel_source() -> String {
    super::source::dashboard_file("viewmodels/useLoadBalancerPlaygroundViewModel.ts")
}

fn chat_api_source() -> String {
    super::source::dashboard_file("lib/api/chat.ts")
}

// トグルが非表示の viewer は UI から startLoadTest に到達できないため、
// 多重防御のガードはソースで担保する。
#[test]
fn start_load_test_guards_on_admin() {
    let source = lb_playground_viewmodel_source();
    let guard = super::source::section(
        &source,
        "const startLoadTest = async () => {",
        "const totalRequests",
    );
    let guard = guard.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        guard.contains("if (!isAdmin || !pg.selectedModel || isLoadTesting) return"),
        "startLoadTest must bail out for non-admin users"
    );
}

#[test]
fn load_test_uses_admin_only_endpoint() {
    let chat = chat_api_source();
    let load_test = super::source::section(&chat, "completeLoadTest:", "getModels:");
    let load_test = load_test.split_whitespace().collect::<String>();
    assert!(
        load_test
            .contains("postChatCompletion('/api/dashboard/playground/load-test/chat/completions'"),
        "chat API must expose completeLoadTest targeting the admin-only load-test endpoint"
    );
}

#[test]
fn regular_chat_uses_authenticated_endpoint() {
    let chat = chat_api_source();
    // 通常 Chat は従来の全ユーザー向けエンドポイントのまま。
    // 分離して、Load Test と通常 Chat の dispatch を個別に mutation 確認する。
    let regular_chat = super::source::section(&chat, "complete:", "completeLoadTest:");
    let regular_chat = regular_chat.split_whitespace().collect::<String>();
    assert!(
        regular_chat.contains("postChatCompletion('/api/dashboard/playground/chat/completions'"),
        "regular chat endpoint must remain for all users"
    );
}
