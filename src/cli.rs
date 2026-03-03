use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "homelab", version, about = "Manage your home lab")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// List all devices in your Tailscale network
    Devices,
    /// Sync local projects to remote hosts via rsync
    Sync(SyncArgs),
    /// Execute a command on a remote host via SSH
    Exec(ExecArgs),
    /// Sync the current project and run a command on a remote host
    Run(RunArgs),
}

#[derive(Args, Debug)]
pub struct SyncArgs {
    #[command(subcommand)]
    pub command: SyncCommand,
}

#[derive(Subcommand, Debug)]
pub enum SyncCommand {
    /// Push the current directory to a remote host
    Push(SyncPushArgs),
    /// List synced projects on a remote host
    List(SyncListArgs),
    /// Remove a synced project from a remote host
    Remove(SyncRemoveArgs),
}

#[derive(Args, Debug)]
pub struct SyncPushArgs {
    /// Target host to push to
    #[arg(long = "to")]
    pub to_host: String,

    /// Watch for file changes and re-sync automatically
    #[arg(long)]
    pub watch: bool,
}

#[derive(Args, Debug)]
pub struct SyncListArgs {
    /// Host to list synced projects on
    #[arg(long = "on")]
    pub on_host: String,
}

#[derive(Args, Debug)]
pub struct SyncRemoveArgs {
    /// Host to remove the project from
    #[arg(long = "on")]
    pub on_host: String,

    /// Name of the project to remove
    pub project: String,
}

#[derive(Args, Debug)]
pub struct RemoteCommandArgs {
    /// Host to run the command on
    #[arg(long = "on")]
    pub on_host: String,

    /// Local ports to forward to the remote host via SSH
    #[arg(long = "forward")]
    pub forward: Vec<u16>,

    /// Command and arguments to execute
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

    /// Commands to run once after sync but before the main command
    #[arg(long = "setup")]
    pub setup_commands: Vec<String>,
}
