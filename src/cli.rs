use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "homelab", version, about = "Manage your home lab")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    Devices,
    Sync(SyncArgs),
    Exec(ExecArgs),
    Run(RunArgs),
}

#[derive(Args, Debug)]
pub struct SyncArgs {
    #[command(subcommand)]
    pub command: SyncCommand,
}

#[derive(Subcommand, Debug)]
pub enum SyncCommand {
    Push(SyncPushArgs),
    List(SyncListArgs),
    Remove(SyncRemoveArgs),
}

#[derive(Args, Debug)]
pub struct SyncPushArgs {
    #[arg(long = "to")]
    pub to_host: String,

    #[arg(long)]
    pub watch: bool,
}

#[derive(Args, Debug)]
pub struct SyncListArgs {
    #[arg(long = "on")]
    pub on_host: String,
}

#[derive(Args, Debug)]
pub struct SyncRemoveArgs {
    #[arg(long = "on")]
    pub on_host: String,

    pub project: String,
}

#[derive(Args, Debug)]
pub struct RemoteCommandArgs {
    #[arg(long = "on")]
    pub on_host: String,

    #[arg(long = "forward")]
    pub forward: Vec<u16>,

    #[arg(last = true, required = true)]
    pub command: Vec<String>,
}

#[derive(Args, Debug)]
pub struct ExecArgs {
    #[command(flatten)]
    pub remote: RemoteCommandArgs,
}

#[derive(Args, Debug)]
pub struct RunArgs {
    #[command(flatten)]
    pub remote: RemoteCommandArgs,
}
