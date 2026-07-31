use std::{sync::Arc, time::Duration};

use axum::{
    Router,
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, StatusCode, Uri, header},
    response::{IntoResponse, Response},
    routing::get,
};
use serde_json::json;

use crate::lifecycle::AppState;

#[derive(Clone)]
struct WebState {
    app: Arc<AppState>,
    allowed_origin: Option<String>,
}

pub fn router(state: Arc<AppState>, allowed_origin: Option<String>) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/app.js", get(javascript))
        .route("/style.css", get(stylesheet))
        .route("/api/state", get(snapshot))
        .route("/healthz", get(health))
        .route("/observe", get(observe))
        .with_state(WebState {
            app: state,
            allowed_origin,
        })
}

async fn index() -> Response {
    no_store(
        (
            [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
            include_str!("../web/index.html"),
        )
            .into_response(),
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

async fn snapshot(State(state): State<WebState>) -> Response {
    let response = match state.app.snapshot().await {
        Ok(snapshot) => axum::Json(snapshot).into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("measurement failed: {error}"),
        )
            .into_response(),
    };
    no_store(response)
}

async fn health(State(state): State<WebState>) -> Response {
    match state.app.snapshot().await {
        Ok(_) => axum::Json(json!({ "status": "ok" })).into_response(),
        Err(error) => (
            StatusCode::SERVICE_UNAVAILABLE,
            axum::Json(json!({ "status": "error", "message": error.to_string() })),
        )
            .into_response(),
    }
}

async fn observe(
    websocket: WebSocketUpgrade,
    State(state): State<WebState>,
    headers: HeaderMap,
) -> Response {
    if !origin_is_allowed(&headers, state.allowed_origin.as_deref()) {
        return (StatusCode::FORBIDDEN, "observation origin rejected").into_response();
    }
    websocket
        .on_upgrade(move |socket| observer(socket, state.app))
        .into_response()
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

fn no_store(mut response: Response) -> Response {
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        "no-store, max-age=0".parse().expect("valid cache header"),
    );
    response
}

fn origin_is_allowed(headers: &HeaderMap, configured_origin: Option<&str>) -> bool {
    let Some(origin) = headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
    else {
        return false;
    };

    if let Some(configured_origin) = configured_origin {
        return origin.trim_end_matches('/') == configured_origin;
    }

    let Some(host) = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
    else {
        return false;
    };
    let Ok(origin_uri) = origin.parse::<Uri>() else {
        return false;
    };

    matches!(origin_uri.scheme_str(), Some("http" | "https"))
        && origin_uri
            .authority()
            .is_some_and(|authority| authority.as_str() == host)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(origin: &str, host: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(header::ORIGIN, origin.parse().unwrap());
        headers.insert(header::HOST, host.parse().unwrap());
        headers
    }

    #[test]
    fn same_origin_observation_is_allowed() {
        let headers = headers("https://schrodingers.life", "schrodingers.life");
        assert!(origin_is_allowed(&headers, None));
    }

    #[test]
    fn cross_origin_observation_is_rejected() {
        let headers = headers("https://attacker.example", "schrodingers.life");
        assert!(!origin_is_allowed(&headers, None));
    }

    #[test]
    fn configured_origin_supports_reverse_proxy_deployments() {
        let headers = headers("https://schrodingers.life", "127.0.0.1:3000");
        assert!(origin_is_allowed(
            &headers,
            Some("https://schrodingers.life")
        ));
    }

    #[test]
    fn missing_origin_is_rejected() {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, "schrodingers.life".parse().unwrap());
        assert!(!origin_is_allowed(&headers, None));
    }
}
