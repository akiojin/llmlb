//! WebSocket endpoint for real-time dashboard updates
//!
//! This module provides `/ws/dashboard` endpoint that streams
//! DashboardEvents to connected clients in real-time.
//!
//! Authentication is required via Bearer token (`Authorization`) or JWT cookie.

use crate::common::auth::UserRole;
use axum::extract::ws::{Message, WebSocket};
use axum::{
    extract::{State, WebSocketUpgrade},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use tracing::{debug, warn};

use crate::events::SharedEventBus;
use crate::AppState;

/// WebSocket upgrade handler for dashboard events
///
/// Clients connect to `/ws/dashboard` to receive real-time updates about:
/// - Node registration/removal
/// - Node status changes
/// - Metrics updates
///
/// Authentication is required unless LLMLB_AUTH_DISABLED (legacy: AUTH_DISABLED) is set.
pub async fn dashboard_ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // Check if auth is disabled
    let auth_disabled = crate::config::is_auth_disabled();

    if !auth_disabled {
        let token = if let Some(auth_header) = headers
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
        {
            auth_header
                .strip_prefix("Bearer ")
                .ok_or_else(|| {
                    (
                        StatusCode::UNAUTHORIZED,
                        "Invalid Authorization header format".to_string(),
                    )
                })?
                .to_string()
        } else {
            crate::auth::middleware::extract_jwt_cookie(&headers)
                .ok_or_else(|| (StatusCode::UNAUTHORIZED, "Missing JWT cookie".to_string()))?
        };

        let claims = crate::auth::jwt::verify_jwt(&token, &state.jwt_secret).map_err(|e| {
            warn!("WebSocket JWT verification failed: {}", e);
            (StatusCode::UNAUTHORIZED, format!("Invalid token: {}", e))
        })?;

        // Only admin users can access the dashboard WebSocket
        if claims.role != UserRole::Admin {
            return Err((StatusCode::FORBIDDEN, "Admin access required".to_string()));
        }

        debug!("WebSocket authenticated for user: {}", claims.sub);
    }

    Ok(ws.on_upgrade(move |socket| handle_socket(socket, state.event_bus.clone())))
}

async fn handle_socket(socket: WebSocket, event_bus: SharedEventBus) {
    let (mut sender, mut receiver) = socket.split();
    let mut event_rx = event_bus.subscribe();

    debug!("Dashboard WebSocket client connected");

    // Send initial connection confirmation
    let welcome = serde_json::json!({
        "type": "connected",
        "message": "Dashboard WebSocket connected"
    });
    if let Err(e) = sender.send(Message::Text(welcome.to_string().into())).await {
        warn!("Failed to send welcome message: {}", e);
        return;
    }

    // Spawn task to handle incoming messages (ping/pong, close)
    let mut recv_task = tokio::spawn(async move {
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(Message::Close(_)) => break,
                Ok(Message::Ping(data)) => {
                    debug!("Received ping, will respond with pong");
                    // Pong is handled automatically by axum
                    let _ = data;
                }
                Err(e) => {
                    warn!("WebSocket receive error: {}", e);
                    break;
                }
                _ => {}
            }
        }
    });

    // Send events to the client
    loop {
        tokio::select! {
            // Check if receive task finished (client disconnected)
            _ = &mut recv_task => {
                debug!("Dashboard WebSocket client disconnected");
                break;
            }
            // Receive events from the event bus
            event_result = event_rx.recv() => {
                match event_result {
                    Ok(event) => {
                        let json = match serde_json::to_string(&event) {
                            Ok(j) => j,
                            Err(e) => {
                                warn!("Failed to serialize event: {}", e);
                                continue;
                            }
                        };
                        if let Err(e) = sender.send(Message::Text(json.into())).await {
                            warn!("Failed to send event: {}", e);
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        warn!("Dashboard WebSocket lagged by {} events", n);
                        // Continue receiving, we just lost some events
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        debug!("Event bus closed");
                        break;
                    }
                }
            }
        }
    }
}
