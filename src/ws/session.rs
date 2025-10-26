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
        if self.user_id.is_none() {
            return self.handle_auth_message(state, text).await;
        }

        match serde_json::from_str::<ClientMessage>(text) {
            Ok(ClientMessage::Text { to, text: msg_text }) => {
                self.handle_text_message(state, &to, &msg_text).await
            },
            Ok(ClientMessage::Auth { .. }) => {
                warn!("User already authenticated");
                true
            },
            Err(e) => {
                warn!("Failed to parse message: {}", e);
                true
            },
        }
    }

    async fn handle_auth_message(&mut self, state: &SharedState, text: &str) -> bool {
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
                let err = serde_json::json!({ "type": "error", "payload": { "msg": "auth_required" } });
                let _ = self
                    .socket
                    .send(Message::Text(Utf8Bytes::from(err.to_string())))
                    .await;
                true
            }
        }
    }

    async fn handle_text_message(&mut self, state: &SharedState, to: &str, text: &str ) -> bool {
        let from = self.user_id.as_ref().unwrap(); // is called only after authorization so user_id
                                                   // shouldn't be None
        if let Some(sender) = state.get(to) {
            let msg = serde_json::json!({
                "type": "text",
                "payload": {
                    "from": from,
                    "text": text,
                }
            });

            if sender.send(AppMessage::Text(msg.to_string())).is_err() {
                warn!("Failed to send message to user {}", to);
            }
            true
        } else {
            let err = serde_json::json!({
                "type": "error",
                "payload": {
                    "msg": "user_offline",
                    "user": to
                }
            });
            let _ = self
                .socket
                .send(Message::Text(Utf8Bytes::from(err.to_string())))
                .await;
            true
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
