use anyhow::{Context, Result};
use notify_debouncer_mini::{DebounceEventResult, new_debouncer, notify::RecursiveMode};
use openssh::{KnownHosts, Session};
use std::path::Path;
use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;
use tracing::{error, info};

fn build_rsync_destination(host: &str, remote_base: &str) -> String {
    format!("{host}:{remote_base}")
}

fn build_rsync_command(
    local_dir: &Path,
    destination: &str,
    include_patterns: &[String],
) -> Command {
    let mut cmd = Command::new("rsync");
    cmd.arg(local_dir).arg(destination).args([
        "--archive",
        "--compress",
        "--delete",
        "--info=progress2",
        "--exclude=.git",
    ]);
    for pattern in include_patterns {
        cmd.arg(format!("--include={pattern}"));
    }
    cmd.arg("--filter=:- .gitignore");
    cmd
}

pub fn rsync_to(
    host: &str,
    local_dir: &Path,
    remote_base: &str,
    include_patterns: &[String],
) -> Result<()> {
    let destination = build_rsync_destination(host, remote_base);

    tracing::info!(
        "syncing {} to destination {}",
        local_dir.to_string_lossy(),
        destination
    );

    let status = build_rsync_command(local_dir, &destination, include_patterns)
        .status()
        .context("failed to run rsync")?;

    if !status.success() {
        anyhow::bail!("rsync exited with {status}");
    }

    Ok(())
}

pub fn watch_and_sync(
    host: &str,
    local_dir: &Path,
    remote_base: &str,
    include_patterns: &[String],
) -> Result<()> {
    let (tx, rx) = mpsc::channel::<DebounceEventResult>();

    let mut debouncer =
        new_debouncer(Duration::from_millis(500), tx).context("failed to create file watcher")?;

    debouncer
        .watcher()
        .watch(local_dir, RecursiveMode::Recursive)
        .context("failed to watch directory")?;

    loop {
        match rx.recv() {
            Ok(Ok(_events)) => {
                if let Err(e) = rsync_to(host, local_dir, remote_base, include_patterns) {
                    error!(error = %e, "sync failed");
                }
            }
            Ok(Err(e)) => {
                error!(error = %e, "watcher error");
            }
            Err(_) => break,
        }
    }

    Ok(())
}

pub async fn push(args: &crate::cli::SyncPushArgs) -> Result<()> {
    let local_dir = std::env::current_dir().context("cannot determine current directory")?;
    let remote_base = crate::REMOTE_SYNCED_BASE_PATH;

    rsync_to(
        &args.to_host,
        &local_dir,
        remote_base,
        &args.include_patterns,
    )?;

    if !args.watch {
        return Ok(());
    }

    info!("watching for changes, press Ctrl+C to stop");
    watch_and_sync(
        &args.to_host,
        &local_dir,
        remote_base,
        &args.include_patterns,
    )?;

    Ok(())
}

pub async fn list(args: &crate::cli::SyncListArgs) -> Result<()> {
    let session = Session::connect(&args.on_host, KnownHosts::Strict).await?;
    let output = session
        .raw_command(format!("ls -1 {}", crate::REMOTE_SYNCED_BASE_PATH))
        .output()
        .await?;
    session.close().await?;
    if !output.status.success() {
        anyhow::bail!("no synced projects found on {}", args.on_host);
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    print!("{stdout}");
    Ok(())
}

pub async fn remove(args: &crate::cli::SyncRemoveArgs) -> Result<()> {
    let session = Session::connect(&args.on_host, KnownHosts::Strict)
        .await
        .with_context(|| format!("failed to connect to {}", args.on_host))?;

    let path = format!("{}{}", crate::REMOTE_SYNCED_BASE_PATH, args.project);
    info!(project = %args.project, "removing synced project");

    session
        .raw_command(format!("rm -rf {path}"))
        .status()
        .await
        .context("failed to remove project")?;

    session.close().await?;
    println!("removed {}", args.project);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn build_rsync_destination_formats_correctly() {
        let dest = build_rsync_destination("server", "~/remote-synced-projects/");
        assert_eq!(dest, "server:~/remote-synced-projects/");
    }

    #[test]
    fn build_rsync_destination_with_trailing_slash_in_remote_base() {
        let dest = build_rsync_destination("beast", "/home/user/projects/");
        assert_eq!(dest, "beast:/home/user/projects/");
    }

    #[test]
    fn build_rsync_command_includes_all_required_flags() {
        let local = PathBuf::from("/home/user/myapp");
        let cmd = build_rsync_command(&local, "server:~/projects/", &[]);
        let args: Vec<_> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        assert!(args.contains(&"--archive".to_string()));
        assert!(args.contains(&"--compress".to_string()));
        assert!(args.contains(&"--delete".to_string()));
        assert!(args.contains(&"--info=progress2".to_string()));
        assert!(args.contains(&"--exclude=.git".to_string()));
        assert!(args.contains(&"--filter=:- .gitignore".to_string()));
    }

    #[test]
    fn build_rsync_command_uses_correct_program() {
        let local = PathBuf::from("/home/user/myapp");
        let cmd = build_rsync_command(&local, "server:~/projects/", &[]);
        assert_eq!(cmd.get_program(), "rsync");
    }

    #[test]
    fn build_rsync_command_with_include_patterns() {
        let local = PathBuf::from("/home/user/myapp");
        let include_patterns = vec![".env".to_string(), "secrets.json".to_string()];
        let cmd = build_rsync_command(&local, "server:~/projects/", &include_patterns);
        let args: Vec<_> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();

        let include_env_pos = args.iter().position(|a| a == "--include=.env").unwrap();
        let include_secrets_pos = args
            .iter()
            .position(|a| a == "--include=secrets.json")
            .unwrap();
        let filter_pos = args
            .iter()
            .position(|a| a == "--filter=:- .gitignore")
            .unwrap();

        assert!(
            include_env_pos < filter_pos,
            "--include=.env must come before --filter"
        );
        assert!(
            include_secrets_pos < filter_pos,
            "--include=secrets.json must come before --filter"
        );
    }
}
