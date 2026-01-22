use std::{io, net::SocketAddr};

use axum::Router;
use tokio::{net::TcpListener, sync::oneshot, task::JoinHandle};

/// 汎用的にテスト用のHTTPサーバーを起動するためのユーティリティ
#[allow(dead_code)]
pub struct TestServer {
    addr: SocketAddr,
    shutdown: Option<oneshot::Sender<()>>,
    handle: JoinHandle<Result<(), io::Error>>,
}

#[allow(dead_code)]
impl TestServer {
    /// サーバーがバインドしているアドレスを返す
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// サーバーを停止し、バックグラウンドタスクの終了を待つ
    pub async fn stop(mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        let _ = self.handle.await;
    }
}

/// Drop時に自動で停止するテストサーバーガード
#[allow(dead_code)]
pub struct TestServerGuard {
    server: Option<TestServer>,
}

#[allow(dead_code)]
impl TestServerGuard {
    pub fn new(server: TestServer) -> Self {
        Self {
            server: Some(server),
        }
    }

    pub fn addr(&self) -> SocketAddr {
        self.server
            .as_ref()
            .expect("TestServerGuard already stopped")
            .addr()
    }

    pub async fn stop(mut self) {
        if let Some(server) = self.server.take() {
            server.stop().await;
        }
    }
}

impl Drop for TestServerGuard {
    fn drop(&mut self) {
        if let Some(server) = self.server.take() {
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                handle.spawn(async move {
                    server.stop().await;
                });
            }
        }
    }
}

/// 任意のload balancerを実ポートにバインドして起動する
pub async fn spawn_lb(router: Router) -> TestServer {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = oneshot::channel();
    let handle = tokio::spawn(async move {
        axum::serve(
            listener,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(async {
            let _ = rx.await;
        })
        .await
    });

    TestServer {
        addr,
        shutdown: Some(tx),
        handle,
    }
}

/// 任意のロードバランサーをガード付きで起動する
#[allow(dead_code)]
pub async fn spawn_lb_guarded(app: Router) -> TestServerGuard {
    TestServerGuard::new(spawn_lb(app).await)
}
