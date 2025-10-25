use crate::messages::AppMessage;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

pub type UserId = String;
pub type MessageSender = mpsc::UnboundedSender<AppMessage>;
pub type SharedState = Arc<DashMap<UserId, tokio::sync::mpsc::UnboundedSender<AppMessage>>>;

