//! 運用通知テストの共通ヘルパー

use axum::{
    body::{to_bytes, Body},
    http::{header, Method, Request, StatusCode},
    Router,
};
use chrono::NaiveDateTime;
use llmlb::common::auth::UserRole;
use llmlb::db::settings::SettingsStorage;
use llmlb::notifications::{
    DailyDigestScheduler, Language, MailError, MailMessage, MailTransport, NotificationSettings,
    SmtpCredentials,
};
use llmlb::types::endpoint::{Endpoint, EndpointStatus, EndpointType};
use serde_json::Value;
use sqlx::SqlitePool;
use std::cell::RefCell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

/// テスト用ユーザーのパスワード
pub const TEST_PASSWORD: &str = "password123";

/// ログインしないユーザー用のパスワードハッシュ（bcrypt の計算を省く）
const UNUSED_PASSWORD_HASH: &str = "unused-password-hash";

/// [`TEST_PASSWORD`] でログインできるユーザーを作成する
pub async fn create_login_user(pool: &SqlitePool, username: &str, role: UserRole) {
    let password_hash = llmlb::auth::password::hash_password(TEST_PASSWORD).unwrap();
    llmlb::db::users::create(pool, username, &password_hash, role, false)
        .await
        .expect("create login user");
}

/// ユーザーを作成し、そのユーザーの JWT を返す（パスワードではログインできない）
pub async fn create_user_with_jwt(pool: &SqlitePool, username: &str, role: UserRole) -> String {
    let user = llmlb::db::users::create(pool, username, UNUSED_PASSWORD_HASH, role, false)
        .await
        .expect("create test user");
    llmlb::auth::jwt::create_jwt(
        &user.id.to_string(),
        role,
        &crate::support::lb::test_jwt_secret(),
        false,
        0,
    )
    .expect("create test jwt")
}

/// 管理者 1 名を持つテスト用アプリを構築する
pub async fn build_app_with_admin() -> (Router, SqlitePool, String) {
    let (app, pool) = crate::support::lb::create_test_lb().await;
    let jwt = create_user_with_jwt(&pool, "notify-admin", UserRole::Admin).await;
    (app, pool, jwt)
}

/// JSON リクエストを送り、ステータスと JSON ボディ（空なら Null）を返す
pub async fn send_json(
    app: &Router,
    method: Method,
    uri: &str,
    jwt: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(jwt) = jwt {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {jwt}"));
    }
    let body = match body {
        Some(value) => {
            builder = builder.header(header::CONTENT_TYPE, "application/json");
            Body::from(serde_json::to_vec(&value).unwrap())
        }
        None => Body::empty(),
    };
    let response = app
        .clone()
        .oneshot(builder.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

// ---------------------------------------------------------------------------
// 通知の送信を検証するためのヘルパー
// ---------------------------------------------------------------------------

/// 実際には送信せず、渡されたメールを記録するトランスポート
#[derive(Clone, Default)]
pub struct RecordingTransport {
    sent: Arc<Mutex<Vec<MailMessage>>>,
    failures_remaining: Arc<AtomicUsize>,
}

impl RecordingTransport {
    /// 記録済みのメール
    pub fn sent(&self) -> Vec<MailMessage> {
        self.sent.lock().unwrap().clone()
    }

    /// 次の `count` 回の送信を失敗させる
    pub fn fail_next(&self, count: usize) {
        self.failures_remaining.store(count, Ordering::SeqCst);
    }
}

#[async_trait::async_trait]
impl MailTransport for RecordingTransport {
    async fn send(&self, message: &MailMessage) -> Result<(), MailError> {
        let failing = self
            .failures_remaining
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |remaining| {
                remaining.checked_sub(1)
            })
            .is_ok();
        if failing {
            return Err(MailError::Delivery("simulated SMTP outage".to_string()));
        }
        self.sent.lock().unwrap().push(message.clone());
        Ok(())
    }
}

/// `YYYY-MM-DD HH:MM:SS` をサーバーのローカル時刻として解釈する
pub fn at(value: &str) -> NaiveDateTime {
    NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S").unwrap()
}

/// SMTP の認証情報（環境変数から読まれる想定の値）
pub fn smtp_credentials() -> SmtpCredentials {
    SmtpCredentials::new(Some("mailer".to_string()), Some("s3cret".to_string()))
}

/// 送信に必要な項目がすべて揃った通知設定（09:00 送信・日本語）
pub fn complete_settings() -> NotificationSettings {
    NotificationSettings {
        enabled: true,
        smtp_host: "smtp.example.com".to_string(),
        smtp_port: 587,
        smtp_from: "llmlb@example.com".to_string(),
        daily_digest_time: "09:00".to_string(),
        language: Language::Ja,
    }
}

/// 通知設定を `settings` テーブルへ保存する
pub async fn save_notification_settings(pool: &SqlitePool, settings: &NotificationSettings) {
    llmlb::notifications::save_settings(&SettingsStorage::new(pool.clone()), settings)
        .await
        .expect("save notification settings");
}

/// ユーザーを作成し、通知先メールアドレスを設定する
pub async fn create_user_with_email(
    pool: &SqlitePool,
    username: &str,
    role: UserRole,
    email: &str,
) {
    let user = llmlb::db::users::create(pool, username, UNUSED_PASSWORD_HASH, role, false)
        .await
        .expect("create test user");
    llmlb::db::users::set_email(pool, user.id, Some(email))
        .await
        .expect("set notification email");
}

/// 指定した状態のエンドポイントを DB に登録する
pub async fn register_endpoint(pool: &SqlitePool, name: &str, status: EndpointStatus) {
    let mut endpoint = Endpoint::new(
        name.to_string(),
        format!("http://{name}.example:8080"),
        EndpointType::Xllm,
    );
    endpoint.status = status;
    llmlb::db::endpoints::create_endpoint(pool, &endpoint)
        .await
        .expect("register endpoint");
}

/// 記録用トランスポートを使うスケジューラを作る（プロセス再起動の再現にも使う）
pub fn scheduler(pool: &SqlitePool, transport: &RecordingTransport) -> DailyDigestScheduler {
    DailyDigestScheduler::new(
        pool.clone(),
        Arc::new(transport.clone()),
        smtp_credentials(),
    )
}

/// 通知設定・宛先（管理者 1 名）・エンドポイント 1 件が揃った DB を用意する
pub async fn ready_pool() -> SqlitePool {
    let pool = crate::support::lb::create_test_db_pool().await;
    save_notification_settings(&pool, &complete_settings()).await;
    create_user_with_email(&pool, "ops-admin", UserRole::Admin, "ops@example.com").await;
    register_endpoint(&pool, "gpu-a", EndpointStatus::Online).await;
    pool
}

/// テスト中に出力されたログを溜めるバッファ
#[derive(Clone, Default)]
struct LogBuffer(Arc<Mutex<Vec<u8>>>);

thread_local! {
    /// このスレッドで捕捉中のログの出力先（捕捉していなければ `None`）
    static THREAD_LOGS: RefCell<Option<LogBuffer>> = const { RefCell::new(None) };
}

/// ログを、出力したスレッドが捕捉中のバッファへ書き込む writer
struct ThreadLogWriter;

impl std::io::Write for ThreadLogWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        THREAD_LOGS.with(|logs| {
            if let Some(buffer) = logs.borrow().as_ref() {
                buffer.0.lock().unwrap().extend_from_slice(buf);
            }
        });
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for ThreadLogWriter {
    type Writer = ThreadLogWriter;

    fn make_writer(&'a self) -> Self::Writer {
        ThreadLogWriter
    }
}

/// 現在のスレッドのログの捕捉（破棄すると捕捉をやめる）
pub struct LogCapture {
    buffer: LogBuffer,
}

impl LogCapture {
    /// これまでに出力されたログ
    pub fn contents(&self) -> String {
        String::from_utf8_lossy(&self.buffer.0.lock().unwrap()).into_owned()
    }
}

impl Drop for LogCapture {
    fn drop(&mut self) {
        THREAD_LOGS.with(|logs| *logs.borrow_mut() = None);
    }
}

/// 現在のスレッドが出力するログを捕捉する
///
/// subscriber はプロセス全体に 1 つだけ置き、出力先だけをスレッドごとに分ける。
/// スレッドローカルな subscriber（`set_default`）は使わない: tracing は callsite の
/// 有効／無効をキャッシュするため、subscriber を持たない別スレッドのテストが同じ
/// ログ出力箇所を先に通ると無効として記録され、捕捉側でもログが出なくなる。
pub fn capture_logs() -> LogCapture {
    static INSTALL: std::sync::Once = std::sync::Once::new();
    INSTALL.call_once(|| {
        let subscriber = tracing_subscriber::fmt()
            .with_writer(ThreadLogWriter)
            .with_ansi(false)
            .finish();
        tracing::subscriber::set_global_default(subscriber)
            .expect("no other global subscriber may be installed in this test binary");
    });

    let buffer = LogBuffer::default();
    THREAD_LOGS.with(|logs| *logs.borrow_mut() = Some(buffer.clone()));
    LogCapture { buffer }
}
