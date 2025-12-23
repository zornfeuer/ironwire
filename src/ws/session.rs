use crate::{
    messages::{AppMessage, ClientMessage},
    state::{SharedState, UserId, MessageSender},
    ws::auth::AuthChallenge,
};
use axum::extract::ws::{
    CloseFrame,
    Message,
    Utf8Bytes,
    WebSocket,
    close_code
};
use tracing::{info, warn};
use tokio::time::Duration;

const AUTH_TIMEOUT: u64 = 10;

enum HandleResult {
    Continue,
    Close
}

pub struct Session {
    client_id: Option<UserId>,
    pending_verification: Option<AuthChallenge>,
    sender: MessageSender,
    socket: WebSocket
}

impl Session {
    pub async fn new(socket: WebSocket) -> (Self, tokio::sync::mpsc::UnboundedReceiver<AppMessage>) {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        (
            Session {
                client_id: None,
                pending_verification: None,
                sender,
                socket,
            },
            receiver,
        )
    }

    pub async fn run(
        mut self,
        state: SharedState,
        mut rx: tokio::sync::mpsc::UnboundedReceiver<AppMessage>
    ) {
        let auth_timeout = Duration::from_secs(AUTH_TIMEOUT);
        let mut auth_timer = Some(Box::pin(tokio::time::sleep(auth_timeout)));

        loop {
            tokio::select! {
                Some(msg) = self.socket.recv() => {
                    match msg {
                        Ok(Message::Text(text)) => {
                            match self.handle_incoming_message(&state, &text).await {
                                HandleResult::Close => break,
                                _ => (),
                            }
                            if self.client_id.is_some() && auth_timer.is_some() {
                                auth_timer = None;
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
                _ = async {
                    if let Some(timer) = &mut auth_timer {
                        timer.await;
                    }
                }, if auth_timer.is_some() => {
                    let _ = self.socket.send(Message::Close(Some(CloseFrame {
                        code: close_code::PROTOCOL,
                        reason: "Authentication timeout".into(),
                    }))).await;
                    break;
                },
                else => break,
            }
        }
        
        if let Some(id) = self.client_id.take() {
            state.remove(&id);
            info!("User {} disconnected", id)
        }

        info!("WebSocket connection closed")
    }
}

impl Session {
    async fn handle_incoming_message(&mut self, state: &SharedState, text: &str) -> HandleResult {
        if self.client_id.is_none() {
            return self.handle_auth_message(state, text).await;
        }

        match serde_json::from_str::<ClientMessage>(text) {
            Ok(ClientMessage::Text { to, text: msg_text }) => {
                self.handle_text_message(state, &to, &msg_text).await
            },
            Ok(ClientMessage::File { to, url }) => {
                self.handle_file_message(state, &to, &url).await
            },
            Ok(ClientMessage::Auth { .. }) | Ok(ClientMessage::Verify { .. }) => {
                warn!("User already authenticated");
                HandleResult::Continue
            },
            Err(e) => {
                warn!("Failed to parse message: {}", e);
                HandleResult::Continue
            },
        }
    }

    async fn handle_auth_message(&mut self, state: &SharedState, text: &str) -> HandleResult {
        if self.pending_verification.is_some() {
            return match serde_json::from_str::<ClientMessage>(text) {
                Ok(ClientMessage::Verify { attempt }) => {
                    self.handle_verify(state, &attempt).await
                }
                _ => {
                    warn!("Expected verify message during auth challenge");
                    self.send_close("expected_verify").await;
                    HandleResult::Close
                }
            };
        }

        match serde_json::from_str::<ClientMessage>(text) {
            Ok(ClientMessage::Auth { token: pubkey_bytes }) => {
                match AuthChallenge::new(&pubkey_bytes) {
                    Ok((auth_challenge, challenge_msg)) => {
                        self.pending_verification = Some(auth_challenge);
                        if self.socket.send(challenge_msg).await.is_err() {
                            return HandleResult::Close;
                        }
                        HandleResult::Continue
                    }
                    Err(_) => {
                        self.send_close("Invalid public key").await;
                        HandleResult::Close
                    }
                }
            }
            _ => {
                warn!("Expected auth message before authentication");
                self.send_close("auth_required").await;
                HandleResult::Close
            }
        }
    }

    async fn handle_verify(&mut self, state: &SharedState, sig_bytes: &[u8]) -> HandleResult {
        let pending = match self.pending_verification.take() {
            Some(p) => p,
            None => {
                warn!("Verify called without pending challenge");
                self.send_close("No pending authentication challenge").await;
                return HandleResult::Close;
            }
        };
        
        match pending.verify(sig_bytes) {
            Ok(()) => {
                let client_id = pending.client_id();
                self.client_id = Some(client_id.clone());
                state.insert(client_id.clone(), self.sender.clone());

                let msg = serde_json::json!({ "type": "auth_ok" });
                if self.socket.send(Message::Text(Utf8Bytes::from(msg.to_string()))).await.is_err() {
                    return HandleResult::Close;
                }
                
                info!("User authenticated: {}", client_id);
                HandleResult::Continue
            }
            Err(e) => {
                self.send_close(e.into()).await;
                HandleResult::Close
            }
        }
    }

    async fn handle_text_message(&mut self, state: &SharedState, to: &str, text: &str ) -> HandleResult {
        let from = match self.client_id.as_ref() {
            Some(id) => id,
            None => {
                warn!("Text message attempted without authentication");
                self.send_close("Not authenticated").await;
                return HandleResult::Close;
            }
        };

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
        }
        HandleResult::Continue
    }

    async fn handle_file_message(&mut self, state: &SharedState, to: &str, url: &str) -> HandleResult {
        let from = match self.client_id.as_ref() {
            Some(id) => id,
            None => {
                warn!("File message attempted without authentication");
                self.send_close("Not authenticated").await;
                return HandleResult::Close;
            }
        };

        if let Some(sender) = state.get(to) {
            let msg = serde_json::json!({
                "type": "file",
                "payload": {
                    "from": from,
                    "url": url,
                }
            });

            if sender.send(AppMessage::Text(msg.to_string())).is_err() {
                warn!("Failed to send message to user {}", to);
            }
        } else {
            self.send_error("user_offline", to).await;
        }
        HandleResult::Continue
    }

    async fn send_error(&mut self, msg: &str, to: &str) {
        let err = serde_json::json!({ "type": "error", "payload": { "msg": msg, "user": to } });
        let _ = self.socket.send(Message::Text(Utf8Bytes::from(err.to_string()))).await;
    }

    async fn send_close(&mut self, reason: &str) {
        warn!("Closing connection: {}", reason);
        let _ = self.socket.send(Message::Close(Some(CloseFrame {
            code: 1008,
            reason: Utf8Bytes::from(reason.to_string()),
        }))).await;
    }
}
