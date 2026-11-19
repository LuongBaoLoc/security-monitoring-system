//! 9. KẾT NỐI WEBSOCKET — Kênh /ws/dashboard đẩy sự kiện thời gian thực lên Web.

use axum::{
    extract::{
        ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    response::IntoResponse,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    bao_mat::decode_token_user_id,
    co_so_du_lieu::{map_user, AppState},
};

#[derive(Debug, Deserialize)]
pub struct WsQuery {
    #[serde(default)]
    pub token: String,
}

pub fn broadcast(state: &AppState, event_type: &str, data: Value) {
    let payload = json!({
        "type": event_type,
        "data": data,
    });
    let _ = state.ws_tx.send(payload);
}

pub async fn ws_dashboard(
    ws: WebSocketUpgrade,
    Query(query): Query<WsQuery>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let is_valid = verify_ws_token(&query.token, &state).await;
    ws.on_upgrade(move |socket| handle_socket(socket, state, is_valid))
}

async fn verify_ws_token(token: &str, state: &AppState) -> bool {
    if token.is_empty() {
        return false;
    }
    let Ok(user_id) = decode_token_user_id(token, &state.settings.secret_key) else {
        return false;
    };
    let Ok(Some(row)) = sqlx::query("SELECT * FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(&state.db)
        .await
    else {
        return false;
    };
    let user = map_user(&row);
    user.is_active
}

async fn handle_socket(mut socket: WebSocket, state: AppState, is_valid: bool) {
    if !is_valid {
        let _ = socket
            .send(Message::Close(Some(CloseFrame {
                code: 1008,
                reason: "Invalid or expired token".into(),
            })))
            .await;
        return;
    }

    let mut rx = state.ws_tx.subscribe();
    loop {
        tokio::select! {
            Ok(msg) = rx.recv() => {
                if socket.send(Message::Text(msg.to_string())).await.is_err() {
                    break;
                }
            }
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                    Some(Ok(Message::Ping(p))) => {
                        if socket.send(Message::Pong(p)).await.is_err() {
                            break;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}
