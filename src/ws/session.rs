use crate::{
    messages::{AppMessage, ClientMessage},
    state::{SharedState, UserId, MessageSender},
};
use axum::extract::ws::{
    CloseFrame,
    Message,
    Utf8Bytes,
    WebSocket,
    close_code
};
use tracing::{info, warn};

pub struct Session {
    user_id: Option<UserId>,
    sender: MessageSender,
    socket: WebSocket,
}

impl Session {
    pub async fn new(socket: WebSocket) -> (Self, tokio::sync::mpsc::UnboundedReceiver<AppMessage>) {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        (
            Session {
                user_id: None,
                sender,
                socket,
            },
            receiver,
        )
    }

    async fn handle_incoming_text(&mut self, state: &SharedState, text: &str) -> bool {
        match serde_json::from_str::<ClientMessage>(text) {
            Ok(ClientMessage::Auth { token }) => {
                if token.is_empty() {
                    warn!("Received empty auth token");
                    let _ = self
                        .socket
                        .send(Message::Close(Some(CloseFrame {
                            code: close_code::INVALID,
                            reason: "Empty token".into()
                        })))
                        .await;
                    return false
                }

                self.user_id = Some(token.clone());
                state.insert(token.clone(), self.sender.clone());

                let reply = serde_json::json!({"type": "auth_ok"});
                let _ = self
                    .socket
                    .send(Message::Text(Utf8Bytes::from(reply.to_string())))
                    .await;
                true
            },
            _ => {
                warn!("Received non-auth message before authentication");
                true
            }
        }
    }

    pub async fn run(mut self, state: SharedState, mut rx: tokio::sync::mpsc::UnboundedReceiver<AppMessage>) {
        loop {
            tokio::select! {
                Some(msg) = self.socket.recv() => {
                    match msg {
                        Ok(Message::Text(text)) => {
                            if !self.handle_incoming_text(&state, &text).await {
                                break;
                            }
                        },
                        Ok(Message::Close(_)) => break,
                        Ok(_) => {},
                        Err(e) => {
                            warn!("WebSocket receive error{}", e);
                            break;
                        }
                    }
                },
                Some(app_msg) = rx.recv() => {
                    let ws_msg: Message = app_msg.into();
                    if self.socket.send(ws_msg).await.is_err() {
                        break;
                    }
                },
                else => break,
            }
        }
        
        if let Some(id) = self.user_id.take() {
            state.remove(&id);
            info!("User {} disconnected", id)
        }

        info!("WebSocket connection closed")
    }
}
