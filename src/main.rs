#[tokio::main]
async fn main() -> anyhow::Result<()>{
    tracing_subscriber::fmt::init();
    let addr = "0.0.0.0:8080";
    ironwire::run_server_with_addr(addr).await
}
