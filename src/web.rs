use std::{sync::Arc, time::Duration};

use axum::{
    Router,
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};

use crate::lifecycle::AppState;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/app.js", get(javascript))
        .route("/style.css", get(stylesheet))
        .route("/api/state", get(snapshot))
        .route("/observe", get(observe))
        .with_state(state)
}

async fn index() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        include_str!("../web/index.html"),
    )
}

async fn javascript() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        include_str!("../web/app.js"),
    )
}

async fn stylesheet() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        include_str!("../web/style.css"),
    )
}

async fn snapshot(State(state): State<Arc<AppState>>) -> Response {
    match state.snapshot().await {
        Ok(snapshot) => axum::Json(snapshot).into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("measurement failed: {error}"),
        )
            .into_response(),
    }
}

async fn observe(
    websocket: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    websocket.on_upgrade(move |socket| observer(socket, state))
}

async fn observer(mut socket: WebSocket, state: Arc<AppState>) {
    let Ok(id) = state.observe().await else {
        let _ = socket
            .send(Message::Text("observation failed".into()))
            .await;
        return;
    };
    let mut updates = state.subscribe();
    let mut expiry = tokio::time::interval(Duration::from_secs(25));
    expiry.tick().await;

    loop {
        tokio::select! {
            message = socket.recv() => {
                match message {
                    Some(Ok(Message::Text(text))) if text == "still-looking" => {
                        expiry.reset();
                    }
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                    _ => {}
                }
            }
            _ = updates.recv() => {
                if let Ok(snapshot) = state.snapshot().await {
                    if socket
                        .send(Message::Text(serde_json::to_string(&snapshot).unwrap().into()))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
            }
            _ = expiry.tick() => break,
        }
    }

    state.stop_observing(id).await;
}
