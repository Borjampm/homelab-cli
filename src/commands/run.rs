use anyhow::{Context, Result};
use openssh::{KnownHosts, Session};
use std::path::Path;
use tracing::info;

fn project_name_from_path(path: &Path) -> Result<String> {
    path.file_name()
        .context("current directory has no name")
        .map(|name| name.to_string_lossy().into_owned())
}

fn shell_escape_command(command: &[String]) -> String {
    command
        .iter()
        .map(|arg| shell_escape::escape(arg.into()))
        .collect::<Vec<_>>()
        .join(" ")
}

fn build_remote_command(remote_base: &str, project_name: &str, command: &[String]) -> String {
    format!(
        "cd {remote_base}{project_name} && {}",
        shell_escape_command(command)
    )
}

fn build_setup_command(remote_base: &str, project_name: &str, setup_command: &str) -> String {
    format!("cd {remote_base}{project_name} && {setup_command}")
}

fn build_kill_command(command: &[String]) -> String {
    let pattern = command.join(" ");
    format!("pkill -f '{pattern}' 2>/dev/null; true")
}

pub async fn run(args: &crate::cli::RunArgs) -> Result<()> {
    let local_dir = std::env::current_dir().context("cannot determine current directory")?;
    let remote_base = crate::REMOTE_SYNCED_BASE_PATH;
    let host = &args.remote.on_host;

    info!("initial sync");
    crate::commands::sync::rsync_to(host, &local_dir, remote_base, &args.include_patterns)?;

    info!(host = %host, "connecting via SSH");
    let session = Session::connect(host, KnownHosts::Strict)
        .await
        .with_context(|| format!("failed to connect to {host}"))?;

    let tunnels = crate::commands::exec::spawn_port_forwards(host, &args.remote.forward)?;

    let project_name = project_name_from_path(&local_dir)?;

    for setup_command in &args.setup_commands {
        let full_setup = build_setup_command(remote_base, &project_name, setup_command);
        info!(command = %full_setup, "running setup command");
        let status = session
            .raw_command(&full_setup)
            .status()
            .await
            .with_context(|| format!("setup command failed to execute: {setup_command}"));
        match status {
            Err(error) => {
                crate::commands::exec::kill_tunnels(tunnels);
                return Err(error);
            }
            Ok(exit_status) if !exit_status.success() => {
                crate::commands::exec::kill_tunnels(tunnels);
                anyhow::bail!("setup command failed: {setup_command}");
            }
            Ok(_) => {}
        }
    }

    let full_command = build_remote_command(remote_base, &project_name, &args.remote.command);

    let watcher_host = host.to_owned();
    let watcher_dir = local_dir.clone();
    let watcher_base = remote_base.to_owned();
    let watcher_include_patterns = args.include_patterns.clone();
    std::thread::spawn(move || {
        if let Err(e) = crate::commands::sync::watch_and_sync(
            &watcher_host,
            &watcher_dir,
            &watcher_base,
            &watcher_include_patterns,
        ) {
            tracing::error!(error = %e, "watcher stopped with error");
        }
    });

    info!(command = %full_command, "starting remote command");

    let mut cmd = session.raw_command(&full_command);
    let status = tokio::select! {
        status = cmd.status() => Some(status),
        _ = tokio::signal::ctrl_c() => {
            info!("received Ctrl+C, shutting down");
            None
        },
    };

    drop(cmd);

    crate::commands::exec::kill_tunnels(tunnels);

    if status.is_none() {
        let kill_cmd = build_kill_command(&args.remote.command);
        let _ = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            session.raw_command(&kill_cmd).status(),
        )
        .await;
        std::process::exit(0);
    }
    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), session.close()).await;

    if let Some(result) = status
        && !result.context("remote command failed")?.success()
    {
        anyhow::bail!("remote command exited with a non-zero status");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn build_remote_command_with_multiple_args() {
        let cmd = build_remote_command(
            "~/projects/",
            "myapp",
            &["cargo".into(), "run".into(), "--release".into()],
        );
        assert_eq!(cmd, "cd ~/projects/myapp && cargo run --release");
    }

    #[test]
    fn build_remote_command_with_single_arg() {
        let cmd = build_remote_command("~/projects/", "myapp", &["ls".into()]);
        assert_eq!(cmd, "cd ~/projects/myapp && ls");
    }

    #[test]
    fn build_remote_command_preserves_argument_order() {
        let cmd = build_remote_command(
            "~/base/",
            "proj",
            &["echo".into(), "a".into(), "b".into(), "c".into()],
        );
        assert_eq!(cmd, "cd ~/base/proj && echo a b c");
    }

    #[test]
    fn build_kill_command_wraps_correctly() {
        let cmd = build_kill_command(&["cargo".into(), "run".into(), "--release".into()]);
        assert_eq!(cmd, "pkill -f 'cargo run --release' 2>/dev/null; true");
    }

    #[test]
    fn build_kill_command_with_single_word() {
        let cmd = build_kill_command(&["python3".into()]);
        assert_eq!(cmd, "pkill -f 'python3' 2>/dev/null; true");
    }

    #[test]
    fn build_remote_command_escapes_arguments_with_spaces() {
        let cmd = build_remote_command(
            "~/projects/",
            "myapp",
            &[
                "python3".into(),
                "-c".into(),
                "import sys; print(sys.argv)".into(),
                "arg with spaces".into(),
            ],
        );
        assert_eq!(
            cmd,
            "cd ~/projects/myapp && python3 -c 'import sys; print(sys.argv)' 'arg with spaces'"
        );
    }

    #[test]
    fn shell_escape_command_handles_special_characters() {
        let escaped = shell_escape_command(&["echo".into(), "hello'world".into()]);
        assert_eq!(escaped, "echo 'hello'\\''world'");
    }

    #[test]
    fn project_name_from_path_extracts_last_component() {
        let path = PathBuf::from("/home/user/projects/my-cool-app");
        let name = project_name_from_path(&path).unwrap();
        assert_eq!(name, "my-cool-app");
    }

    #[test]
    fn project_name_from_path_returns_error_for_root() {
        let path = PathBuf::from("/");
        assert!(project_name_from_path(&path).is_err());
    }

    #[test]
    fn build_setup_command_produces_cd_and_command() {
        let cmd = build_setup_command("~/projects/", "myapp", "yarn install");
        assert_eq!(cmd, "cd ~/projects/myapp && yarn install");
    }

    #[test]
    fn build_setup_command_preserves_complex_shell_expression() {
        let cmd = build_setup_command(
            "~/projects/",
            "myapp",
            "pip install -r requirements.txt && echo done",
        );
        assert_eq!(
            cmd,
            "cd ~/projects/myapp && pip install -r requirements.txt && echo done"
        );
    }
}
