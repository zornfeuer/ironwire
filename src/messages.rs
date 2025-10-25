use serde::{Deserialize, Serialize};
use axum::extract::ws::Message as WsMessage;

#[derive(Debug, Clone)]
pub enum AppMessage {
    Text(String),
    Binary(Vec<u8>),
    Close,
}

impl From<AppMessage> for WsMessage {
    fn from(app_msg: AppMessage) -> Self {
        match app_msg {
            AppMessage::Text(s) => WsMessage::Text(s.into()),
            AppMessage::Binary(b) => WsMessage::Binary(b.into()),
            AppMessage::Close => WsMessage::Close(None),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", content = "payload")]
pub enum ClientMessage {
    Auth { token: String },
    // TODO: Text { to: String, from: String, text: String },
    // TODO: File { to: String, from: String, url: String, mime: String },
}
