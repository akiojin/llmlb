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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn shutdown_signal_completes_when_controller_requests_shutdown() {
        let shutdown = ShutdownController::default();
        let wait_task = tokio::spawn(shutdown_signal(shutdown.clone()));

        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        shutdown.request_shutdown();

        tokio::time::timeout(std::time::Duration::from_secs(2), wait_task)
            .await
            .expect("shutdown signal task timed out")
            .expect("shutdown signal task panicked");
    }
}
