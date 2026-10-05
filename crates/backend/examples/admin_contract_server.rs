//! Controlled HTTP contract harness; not a production mode.
use hagency_server::{
    admin::{Admin, Store},
    config::Config,
    frontend::Frontend,
};
use salvo::prelude::*;
use std::sync::Arc;
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .map_err(|_| anyhow::anyhow!("Rustls crypto provider already initialized"))?;
    let conf = Config::load(std::env::args().nth(1).expect("config path"))?;
    let url = std::env::var("FIXTURE_ORIGIN")?;
    let admin = Admin::new(&conf, Arc::new(Store::memory()))
        .await?
        .with_upstream(url.parse()?)?;
    let worker = conf
        .account_config
        .as_ref()
        .map(|_| admin.start_account_worker());
    let acceptor = TcpListener::new(conf.listen).bind().await;
    println!("contract server listening");
    Server::new(acceptor)
        .serve(admin.router().push(Frontend::new(&conf).router()))
        .await;
    if let Some(w) = worker {
        w.abort();
    }
    Ok(())
}
