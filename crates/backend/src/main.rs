use clap::Parser;
use hagency_server::{
    MatrixServer,
    admin::{Admin, Store},
    config::Config,
    frontend::Frontend,
};
use salvo::prelude::*;
use std::{path::PathBuf, sync::Arc, time::Duration};
use tracing_subscriber::{Layer, layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser)]
#[command(
    version,
    about = "Matrix, OIDC and Hagency administration in one Rust server"
)]
struct Args {
    #[arg(short, long, default_value = "config/dev/hagency.toml")]
    config: PathBuf,
    #[arg(long)]
    check_config: bool,
    #[arg(long)]
    list_config_files: bool,
    #[arg(long, requires = "bootstrap_password_file")]
    bootstrap_admin: Option<String>,
    #[arg(long, requires = "bootstrap_admin")]
    bootstrap_password_file: Option<PathBuf>,
    /// Explicitly link the first Pasion administrator to an existing Matrix administrator.
    #[arg(long, requires = "bootstrap_admin")]
    link_existing_matrix_admin: bool,
}
fn main() -> anyhow::Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(8 * 1024 * 1024)
        .build()?
        .block_on(run())
}
async fn run() -> anyhow::Result<()> {
    let args = Args::parse();
    if args.list_config_files {
        println!(
            "{}",
            serde_json::to_string(&Config::config_files(&args.config)?)?
        );
        return Ok(());
    }
    let mut conf = Config::load(&args.config)?;
    if args.check_config {
        println!("Configuration is valid.");
        return Ok(());
    }
    let filter =
        tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into());
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_filter(filter))
        .with(palpo::logging::capture_layer())
        .with(
            tracing_opentelemetry::layer()
                .with_tracer(opentelemetry::global::tracer("hagency-server")),
        )
        .init();
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .map_err(|_| anyhow::anyhow!("Rustls crypto provider was already initialized"))?;
    conf.prepare_signing_key()?;
    let pasion_config = hagency_server::pasion::prepare(&mut conf).await?;
    let pasion = match &pasion_config {
        Some(figment) => {
            Some(hagency_server::PasionServer::initialize(figment, Default::default()).await?)
        }
        None => None,
    };
    let matrix = MatrixServer::initialize(conf.matrix.clone())
        .await
        .map_err(anyhow::Error::msg)?;
    if let Some(localpart) = &args.bootstrap_admin {
        if conf.pasion.as_ref().is_some_and(|p| p.delegate_matrix_auth) {
            hagency_server::pasion::bootstrap_admin(
                pasion_config.as_ref().unwrap(),
                conf.matrix.server_name.as_str(),
                localpart,
                args.bootstrap_password_file.as_ref().unwrap(),
                args.link_existing_matrix_admin,
            )
            .await?;
        } else {
            anyhow::ensure!(
                !args.link_existing_matrix_admin,
                "linking an existing administrator requires Pasion delegated authentication"
            );
            let user = palpo::core::OwnedUserId::try_from(format!(
                "@{localpart}:{}",
                conf.matrix.server_name
            ))?;
            anyhow::ensure!(user.validate_strict().is_ok(), "invalid bootstrap username");
            anyhow::ensure!(
                !palpo::data::user::user_exists(&user).await?,
                "bootstrap refuses to modify an existing user"
            );
            let password = std::fs::read_to_string(args.bootstrap_password_file.as_ref().unwrap())?;
            let password = password.trim_end_matches(['\r', '\n']);
            anyhow::ensure!(
                password.len() >= 12,
                "bootstrap password must be at least 12 characters"
            );
            palpo::user::create_user(&user, Some(password)).await?;
            palpo::user::make_user_admin(&user).await?;
            tracing::info!(user=%user,"bootstrap administrator created");
        }
    }
    let store = Arc::new(Store::postgres(&conf.database_url).await?);
    let admin = Admin::new(&conf, store).await?;
    let worker = conf
        .account_config
        .as_ref()
        .map(|_| admin.start_account_worker());
    let mut matrix_router = Router::new();
    if conf.pasion.as_ref().is_some_and(|p| p.delegate_matrix_auth) {
        matrix_router =
            matrix_router.hoop(hagency_server::pasion::DelegatedAdminGuard::new(&conf)?);
    }
    matrix_router = matrix_router.push(matrix.router());
    let mut router = admin
        .router()
        .push(matrix_router)
        .push(Router::with_path("healthz").get(health))
        .push(Frontend::new(&conf).router());
    if let Some(pasion) = &pasion {
        router = Router::new()
            .push(pasion.router(
                &conf.pasion.as_ref().unwrap().resources()?,
                Some(hagency_server::pasion::MOUNT),
            ))
            .push(router);
    }
    let service = matrix.service(router);
    let acceptor = TcpListener::new(conf.listen).try_bind().await?;
    let server = Server::new(acceptor);
    let handle = server.handle();
    tokio::spawn(async move {
        let ctrl_c = tokio::signal::ctrl_c();
        #[cfg(unix)]
        {
            let mut terminate =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .expect("SIGTERM handler");
            tokio::select! {_=ctrl_c=>(),_=terminate.recv()=>()}
        }
        #[cfg(not(unix))]
        ctrl_c.await.expect("shutdown signal");
        handle.stop_graceful(Some(Duration::from_secs(15)));
    });
    tracing::info!(listen=%conf.listen,public_origin=%conf.public_origin,"hagency-server started");
    server.serve(service).await;
    if let Some(worker) = worker {
        worker.abort();
        let _ = worker.await;
    }
    if let Some(pasion) = pasion {
        tokio::time::timeout(Duration::from_secs(15), pasion.shutdown())
            .await
            .map_err(|_| anyhow::anyhow!("Pasion workers did not stop within 15 seconds"))?;
    }
    Ok(())
}
#[handler]
async fn health() -> &'static str {
    "ok"
}
