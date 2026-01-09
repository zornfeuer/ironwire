mod http;
mod messages;
mod state;
mod ws;
mod tls;

use std::sync::Arc;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use axum::{
    Extension, Router, routing::{get, post}
};
use axum_server::tls_rustls::RustlsConfig;
use state::SharedState;
use anyhow::Result;

pub async fn run_server_with_addr(
    addr: &str,
) -> Result<()> {
    let state: SharedState = Default::default();

    let app = Router::new()
        .route("/ws", get(ws::handler::ws_handler))
        .route("/upload", post(http::upload::upload_handler))
        .route("/media/{path}", get(http::media::media_handler))
        .fallback(http::fallback::fallback_handler)
        .layer(Extension(state));

    match tls::load_rustls_config() {
       Ok(tls_config) => {
           tracing::info!("TLS enabled. Listening on https://{}", addr);
           let config = RustlsConfig::from_config(Arc::new(tls_config));
           let socket_addr: SocketAddr = addr.parse()?;
           serve_with_tls(socket_addr, app, config).await
       }
        Err(e) => {
            tracing::warn!("TLS setup failed ({}), falling back to HTTP", e);
            let listener = TcpListener::bind(addr).await?;
            tracing::info!("Server is started on http://{:?}", addr);
            axum::serve(listener, app).await.map_err(Into::into)
        }
    }
}

async fn serve_with_tls(
    addr: SocketAddr,
    app: Router,
    config: RustlsConfig,
) -> Result<()> {
    axum_server::bind_rustls(addr, config)
        .serve(app.into_make_service())
        .await?;
    Ok(())
}
