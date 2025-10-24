use axum::{
    extract::ws::{WebSocket, WebSocketUpgrade, Message},
    response::Html,
    routing::{get, post},
    Router,
};
use tracing::{info, warn};
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() -> anyhow::Result<()>{
    tracing_subscriber::fmt::init();

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/upload", post(upload_handler))
        .nest_service("/media", ServeDir::new("uploads"))
        .fallback(fallback_handler);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    info!("Server is started on http://0.0.0.0:8080");

    axum::serve(listener, app)
        .await?;

    Ok(())
}

async fn ws_handler(ws: WebSocketUpgrade) -> impl axum::response::IntoResponse {
    ws.on_upgrade(handle_socket)
}

async fn handle_socket(mut socket: WebSocket) {
    info!("New WebSocket connection");
    while let Some(msg) = socket.recv().await {
        match msg {
            Ok(msg) => {
                match msg {
                    Message::Text(_) | Message::Binary(_) => { let _ = socket.send(msg).await; },
                    _ => continue,
                }
            },
            Err(e) => {
                warn!("WebSocket Error: {}", e);
                break;
            }
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
