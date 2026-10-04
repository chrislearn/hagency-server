mod assets;
mod command;
mod config;
mod dev;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::{path::PathBuf, sync::Arc, sync::atomic::AtomicBool};

#[derive(Parser)]
#[command(about = "Hagency development tools; use just --list for command shortcuts")]
struct Args {
    #[arg(long, global = true, hide = true)]
    project_root: Option<PathBuf>,
    #[command(subcommand)]
    command: Task,
}

#[derive(Subcommand)]
enum Task {
    /// Generate private component configuration files without starting services.
    InitConfig(config::Options),
    /// Build/copy Pasion resources using the pinned or a local source tree.
    PreparePasion(assets::Options),
    /// Watch changes, compile, validate and gracefully replace the server.
    Dev(dev::Options),
}

fn main() -> Result<()> {
    let args = Args::parse();
    let root = args
        .project_root
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .into()
        })
        .canonicalize()?;
    let stopping = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGINT, stopping.clone())?;
    signal_hook::flag::register(signal_hook::consts::SIGTERM, stopping.clone())?;
    match args.command {
        Task::InitConfig(options) => config::generate(&root, &options),
        Task::PreparePasion(options) => assets::prepare(&root, &options, &stopping),
        Task::Dev(options) => dev::watch(&root, &options, &stopping),
    }
}
