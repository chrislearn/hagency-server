use crate::{assets, command};
use anyhow::Result;
use clap::Args;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicBool, Ordering},
};

#[derive(Args, Default)]
pub struct Options {
    #[arg(long)]
    pub output: Option<PathBuf>,
}

pub fn prepare(root: &Path, options: &Options, stopping: &AtomicBool) -> Result<()> {
    fs::create_dir_all(root.join(".run"))?;
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.join(".run/frontend-build.lock"))?;
    loop {
        match lock.try_lock() {
            Ok(()) => break,
            Err(std::fs::TryLockError::WouldBlock) => {
                anyhow::ensure!(
                    !stopping.load(Ordering::Relaxed),
                    "Frontend build interrupted"
                );
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            Err(std::fs::TryLockError::Error(e)) => return Err(e.into()),
        }
    }
    let dx = assets::dioxus_cli(root, stopping)?;
    let target = root.join(".run/frontend-target");
    command::checked(
        Command::new(dx)
            .current_dir(root)
            .env("CARGO_TARGET_DIR", &target)
            .args([
                "build",
                "--release",
                "--package",
                "hagency-frontend",
                "--base-path",
                "/",
                "--debug-symbols",
                "false",
                "--locked",
            ]),
        stopping,
    )?;
    let source = target.join("dx/hagency-frontend/release/web/public");
    let output = options
        .output
        .clone()
        .unwrap_or_else(|| root.join("resources/frontend/public"));
    // Publish hashed assets before the index so an active server never refers to missing files.
    fs::create_dir_all(&output)?;
    for entry in fs::read_dir(&source)? {
        let entry = entry?;
        if entry.file_name() == "index.html" {
            continue;
        }
        if entry.path().is_dir() {
            assets::copy_tree(&entry.path(), &output.join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), output.join(entry.file_name()))?;
        }
    }
    let index = tempfile::NamedTempFile::new_in(&output)?;
    fs::copy(source.join("index.html"), index.path())?;
    index.persist(output.join("index.html"))?;
    println!("Prepared Hagency frontend: {}", output.display());
    Ok(())
}
