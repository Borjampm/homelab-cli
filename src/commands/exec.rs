use anyhow::{Context, Result};
use openssh::{KnownHosts, Session};
use tracing::info;

pub fn spawn_port_forwards(host: &str, ports: &[u16]) -> Result<Vec<std::process::Child>> {
    let mut tunnels = Vec::new();
    for &port in ports {
        info!(port, "setting up port forward");
        let child = std::process::Command::new("ssh")
            .args(["-N", "-L", &format!("{port}:localhost:{port}"), host])
            .spawn()
            .with_context(|| format!("failed to forward port {port}"))?;
        tunnels.push(child);
    }
    Ok(tunnels)
}

pub fn kill_tunnels(tunnels: Vec<std::process::Child>) {
    for mut tunnel in tunnels {
        let _ = tunnel.kill();
    }
}

pub async fn run(args: &crate::cli::ExecArgs) -> Result<()> {
    info!(host = %args.remote.on_host, "connecting via SSH");
    let session = Session::connect(&args.remote.on_host, KnownHosts::Strict)
        .await
        .with_context(|| format!("failed to connect to {}", args.remote.on_host))?;

    let tunnels = spawn_port_forwards(&args.remote.on_host, &args.remote.forward)?;

    let full_command = args
        .remote
        .command
        .iter()
        .map(|arg| shell_escape::escape(arg.into()))
        .collect::<Vec<_>>()
        .join(" ");
    let status = session.raw_command(&full_command).status().await?;

    kill_tunnels(tunnels);

    if !status.success() {
        anyhow::bail!("remote command exited with {status}");
    }
    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), session.close()).await;
    Ok(())
}
