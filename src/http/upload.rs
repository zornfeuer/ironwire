use axum::{
    body::Bytes,
    http::StatusCode,
    response::Json,
};
use serde_json::json;
use std::fs;
use tracing::error;
use uuid::Uuid;

pub async fn upload_handler(body: Bytes) -> Result<Json<serde_json::Value>, StatusCode> {
    fs::create_dir_all("uploads").map_err(|e| {
        error!("Failed to create uploads dir: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let filename = format!("{}.bin", Uuid::new_v4());
    let path = format!("uploads/{}", filename);

    fs::write(&path, &body).map_err(|e| {
        error!("Failed to write file: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let url = format!("/media/{}", filename);
    Ok(Json(json!({ "url": url })))
}
