use std::str::FromStr;

use rand_core::{RngCore, OsRng};
use ed25519_dalek::{VerifyingKey, Signature, PUBLIC_KEY_LENGTH, Verifier};
use crate::{
    messages::{AppMessage, ClientMessage},
    state::{SharedState, UserId, MessageSender},
};
use axum::extract::ws::{CloseFrame, Message, Utf8Bytes, WebSocket, close_code};
use tracing::{info, warn};
use hex;

pub struct Session {
    client_id: Option<UserId>,
    pending_verification: Option<PendingVerification>,
    sender: MessageSender,
    socket: WebSocket,
}

struct PendingVerification {
    challenge: [u8; 32],
    public_key: VerifyingKey,
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

    async fn handle_incoming_text(&mut self, state: &SharedState, text: &str) -> bool {
        if self.client_id.is_none() {
            return self.handle_auth_message(state, text).await;
        }

        match serde_json::from_str::<ClientMessage>(text) {
            Ok(ClientMessage::Text { to, text: msg_text }) => {
                self.handle_text_message(state, &to, &msg_text).await
            }
            Ok(ClientMessage::Auth { .. }) | Ok(ClientMessage::Verify { .. }) => {
                warn!("User already authenticated");
                true
            }
            Err(e) => {
                warn!("Failed to parse message: {}", e);
                true
            }
        }
    }

    async fn handle_auth_message(&mut self, state: &SharedState, text: &str) -> bool {
        if self.pending_verification.is_some() {
            // Expecting a Verify message
            return match serde_json::from_str::<ClientMessage>(text) {
                Ok(ClientMessage::Verify { attempt }) => {
                    self.handle_verify(state, &attempt).await
                }
                _ => {
                    self.send_error("expected_verify").await;
                    true
                }
            };
        }

        // First message must be Auth with public key
        match serde_json::from_str::<ClientMessage>(text) {
            Ok(ClientMessage::Auth { token: pubkey_bytes }) => {
                if pubkey_bytes.len() != PUBLIC_KEY_LENGTH {
                    self.send_close("Invalid public key length").await;
                    return false;
                }

                let public_key = match VerifyingKey::from_bytes(pubkey_bytes.as_slice().try_into().unwrap()) {
                    Ok(pk) => pk,
                    Err(_) => {
                        self.send_close("Invalid public key").await;
                        return false;
                    }
                };

                // Generate 32-byte challenge
                let mut challenge = [0u8; 32];
                OsRng.fill_bytes(&mut challenge);

                // Store pending verification
                self.pending_verification = Some(PendingVerification {
                    challenge,
                    public_key,
                });

                // Send challenge
                let msg = serde_json::json!({
                    "type": "auth_challenge",
                    "challenge": hex::encode(challenge)
                });
                if self.socket.send(Message::Text(Utf8Bytes::from(msg.to_string()))).await.is_err() {
                    return false;
                }
                true
            }
            _ => {
                self.send_error("auth_required").await;
                true
            }
        }
    }

    //After first auth request we check proof, so we know this is true private key bearer.
    async fn handle_verify(&mut self, state: &SharedState, sig_bytes: &[u8]) -> bool {
        let pending = self.pending_verification.take().expect("verify called without pending");
        
        if sig_bytes.len() != ed25519_dalek::SIGNATURE_LENGTH {
            self.send_close("Invalid signature length").await;
            return false;
        }

        // Safely convert &[u8] to [u8; 64]
        let sig_array: [u8; 64] = match sig_bytes.try_into() {
            Ok(arr) => arr,
            Err(_) => {
                self.send_close("Invalid signature format").await;
                return false;
            }
        };

        let signature = Signature::from(sig_array);

        if pending.public_key.verify(&pending.challenge, &signature).is_err() {
            self.send_close("Signature verification failed").await;
            return false;
        }

        // We are sure and can proceed.
        let user_id = hex::encode(pending.public_key.as_bytes());
        self.client_id = Some(user_id.clone());
        state.insert(user_id.clone(), self.sender.clone());

        let msg = serde_json::json!({ "type": "auth_ok" });
        if self.socket.send(Message::Text(Utf8Bytes::from(msg.to_string()))).await.is_err() {
            return false;
        }
        
        info!("User authenticated: {}", user_id);
        true
    }

    async fn handle_text_message(&mut self, state: &SharedState, to: &str, text: &str) -> bool {
        let from = self.client_id.as_ref().expect("called only after auth");
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
            let _ = self.socket.send(Message::Text(Utf8Bytes::from(err.to_string()))).await;
        }
        true
    }

    async fn send_error(&mut self, msg: &str) {
        let err = serde_json::json!({ "type": "error", "payload": { "msg": msg } });
        let _ = self.socket.send(Message::Text(Utf8Bytes::from(err.to_string()))).await;
    }

    async fn send_close(&mut self, reason: &str) {
        warn!("Closing connection: {}", reason);
        let _ = self.socket.send(Message::Close(Some(CloseFrame {
            code: 1008,
            reason: Utf8Bytes::from(reason.to_string()),
        }))).await;
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
                        }
                        Ok(Message::Close(_)) => break,
                        Ok(_) => {}
                        Err(e) => {
                            warn!("WebSocket receive error: {}", e);
                            break;
                        }
                    }
                }
                Some(app_msg) = rx.recv() => {
                    let ws_msg: Message = app_msg.into();
                    if self.socket.send(ws_msg).await.is_err() {
                        break;
                    }
                }
                else => break,
            }
        }

        if let Some(id) = self.client_id.take() {
            state.remove(&id);
            info!("User {} disconnected", id);
        }

        info!("WebSocket connection closed");
    }
}