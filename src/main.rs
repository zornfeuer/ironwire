use axum::{
    extract::ws::{
        Message,
        Utf8Bytes,
        WebSocket,
        WebSocketUpgrade,
        CloseFrame,
        close_code
    },
    response::Html,
    routing::{get, post},
    Extension,
    Router
};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, warn};
use tower_http::services::ServeDir;

type UserId = String;
type SharedState = Arc<DashMap<UserId, tokio::sync::mpsc::UnboundedSender<Message>>>;

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", content = "payload")]
enum ClientMessage {
    Auth { token: String },
    // TODO: text, file, etc.
}

#[derive(Debug, Deserialize, Serialize)]
struct AuthOk;

#[tokio::main]
async fn main() -> anyhow::Result<()>{
    tracing_subscriber::fmt::init();

    let state: SharedState = Arc::new(DashMap::new());

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/upload", post(upload_handler))
        .nest_service("/media", ServeDir::new("uploads"))
        .fallback(fallback_handler)
        .layer(Extension(state));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    info!("Server is started on http://{:?}", listener.local_addr()?);

    axum::serve(listener, app).await?;

    Ok(())
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    Extension(state): Extension<SharedState>,
) -> impl axum::response::IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: SharedState) {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Message>();
    // TODO: user_id = Option<UserId>

    loop {
        tokio::select! {
            Some(msg) = socket.recv() => {
                match msg {
                    Ok(Message::Text(text)) => { if let Ok(ClientMessage::Auth { token }) = serde_json::from_str::<ClientMessage>(&text) {
                            if !token.is_empty() {
                                state.insert(token.clone(), tx.clone());

                                let reply = serde_json::json!({ "type": "auth_ok" });
                                let _ = socket.send(Message::Text(Utf8Bytes::from(reply.to_string()))).await;
                            } else {
                                let _ = socket.send(Message::Close(Some(
                                    CloseFrame {
                                        code: close_code::INVALID,
                                        reason: "Empty token".into(),
                                    }
                                ))).await;
                                break;
                            }
                        }
                    },
                    Ok(Message::Close(_)) => break,
                    Ok(_) => {},
                    Err(e) => {
                        warn!("WebSocket receive error: {}", e);
                        break;
                    }
                }
            },
            Some(msg) = rx.recv() => {
                if socket.send(msg).await.is_err() {
                    break;
                }
            }, else => break,
        }
    }

    info!("WebSocket connection closed")
}

async fn upload_handler(
    body: axum::body::Bytes,
) -> Result<axum::Json<serde_json::Value>, axum::http::StatusCode> {
    use std::fs;
    use uuid::Uuid;

    fs::create_dir_all("uploads").map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    let filename = format!("{}.bin", Uuid::new_v4());
    let path = format!("uploads/{}", filename);

    fs::write(&path, &body).map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    let url = format!("/media/{}", filename);
    Ok(axum::Json(serde_json::json!({ "url": url })))
}

async fn fallback_handler() -> Html<&'static str> {
    Html(r#"
        <h1>Messenger Server (MVP)</h1>
        <p>WebSocket: <code>ws://localhost:8080/ws</code></p>
        <p>Upload: POST to <code>/upload</code></p>
        <p>Media: GET <code>/media/...</code></p>
    "#)
}
