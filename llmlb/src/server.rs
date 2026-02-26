//! axumサーバー起動・シャットダウンハンドリング

use crate::shutdown::ShutdownController;
use crate::AppState;
use std::net::SocketAddr;
use tracing::info;

/// axumサーバーを起動し、シャットダウンシグナルを待機する
pub async fn run(state: AppState, bind_addr: &str) {
    let shutdown = state.shutdown.clone();

    let app = crate::api::create_app(state);

    let listener = tokio::net::TcpListener::bind(bind_addr)
        .await
        .expect("Failed to bind to address");

    info!("LLM Load Balancer server listening on {}", bind_addr);

    let shutdown_signal = shutdown_signal(shutdown);

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal)
    .await
    .expect("Server error");

    info!("Server shutdown complete");
}

/// シャットダウンシグナルを待機
async fn shutdown_signal(shutdown: ShutdownController) {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C, shutting down...");
        }
        _ = terminate => {
            info!("Received SIGTERM, shutting down...");
        }
        _ = shutdown.wait() => {
            info!("Shutdown requested, shutting down...");
        }
    }
}
