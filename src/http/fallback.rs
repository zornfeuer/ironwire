use axum::response::Html;

pub async fn fallback_handler() -> Html<&'static str> {
    Html(r#"
        <h1>IronWire</h1>
        <p>WebSocket: <code>ws://localhost:8080/ws</code></p>
        <p>Upload: POST to <code>/upload</code></p>
        <p>Media: GET <code>/media/...</code></p>
    "#)
}
