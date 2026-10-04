//! Entry point for the headless daemon. The work lives in the library so the
//! daemon can be started by tests with an injected shutdown future.

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    limedl_server::main_entry().await
}
