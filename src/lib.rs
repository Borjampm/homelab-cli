mod cli;
mod commands;
mod tailscale;

pub const REMOTE_SYNCED_BASE_PATH: &str = "~/remote-synced-projects/";

use anyhow::Result;
use clap::Parser;

pub async fn run() -> Result<()> {
    let cli = cli::Cli::parse();

    match cli.command {
        cli::Command::Devices => commands::devices::run(),
        cli::Command::Sync(args) => match args.command {
            cli::SyncCommand::Push(push_args) => commands::sync::push(&push_args).await,
            cli::SyncCommand::List(list_args) => commands::sync::list(&list_args).await,
            cli::SyncCommand::Remove(remove_args) => commands::sync::remove(&remove_args).await,
        },
        cli::Command::Exec(args) => commands::exec::run(&args).await,
        cli::Command::Run(args) => commands::run::run(&args).await,
    }
}
