use crate::{state::SharedState, ws::session::Session};
use axum::{
    extract::ws::{WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    Extension,
};

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Extension(state): Extension<SharedState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

pub async fn handle_socket(socket: WebSocket, state: SharedState) {
    let (session, rx) = Session::new(socket).await;
    session.run(state, rx).await;
}
