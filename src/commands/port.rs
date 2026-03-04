use anyhow::{Context, Result};
use openssh::{KnownHosts, Session};

pub struct PortProcess {
    pub pid: u32,
    pub process_name: String,
}

fn build_port_check_command(port: u16) -> String {
    format!("ss -tlnp 'sport = :{port}'")
}

fn build_port_kill_command(pid: u32) -> String {
    format!("kill {pid}")
}

fn parse_ss_output(output: &str) -> Vec<PortProcess> {
    let mut processes = Vec::new();

    for line in output.lines().skip(1) {
        let Some(users_start) = line.find("users:((") else {
            continue;
        };
        let rest = &line[users_start + 8..];
        let Some(users_end) = rest.find("))") else {
            continue;
        };
        let users_block = &rest[..users_end];

        for entry in users_block.split("),(") {
            let fields: Vec<&str> = entry.split(',').collect();
            if fields.len() < 2 {
                continue;
            }

            let process_name = fields[0].trim_matches('"');

            let Some(pid_field) = fields.iter().find(|field| field.starts_with("pid=")) else {
                continue;
            };
            let Some(pid) = pid_field
                .strip_prefix("pid=")
                .and_then(|value| value.parse().ok())
            else {
                continue;
            };

            processes.push(PortProcess {
                pid,
                process_name: process_name.to_string(),
            });
        }
    }

    processes
}

pub async fn check_port(session: &Session, port: u16) -> Result<Vec<PortProcess>> {
    let command = build_port_check_command(port);
    let output = session
        .raw_command(&command)
        .output()
        .await
        .with_context(|| format!("failed to check port {port}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_ss_output(&stdout))
}

pub async fn kill_port(session: &Session, port: u16) -> Result<Vec<PortProcess>> {
    let processes = check_port(session, port).await?;
    for process in &processes {
        let kill_command = build_port_kill_command(process.pid);
        session
            .raw_command(&kill_command)
            .status()
            .await
            .with_context(|| format!("failed to kill PID {}", process.pid))?;
    }
    Ok(processes)
}

pub async fn run_check(args: &crate::cli::PortCheckArgs) -> Result<()> {
    let session = Session::connect(&args.on_host, KnownHosts::Strict)
        .await
        .with_context(|| format!("failed to connect to {}", args.on_host))?;

    let processes = check_port(&session, args.port).await?;
    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), session.close()).await;

    if processes.is_empty() {
        println!("port {} is free on {}", args.port, args.on_host);
    } else {
        for process in &processes {
            println!(
                "port {} is in use by {} (PID {})",
                args.port, process.process_name, process.pid
            );
        }
    }

    Ok(())
}

pub async fn run_kill(args: &crate::cli::PortKillArgs) -> Result<()> {
    let session = Session::connect(&args.on_host, KnownHosts::Strict)
        .await
        .with_context(|| format!("failed to connect to {}", args.on_host))?;

    let killed = kill_port(&session, args.port).await?;
    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), session.close()).await;

    if killed.is_empty() {
        println!("port {} is already free on {}", args.port, args.on_host);
    } else {
        for process in &killed {
            println!(
                "killed {} (PID {}) on port {}",
                process.process_name, process.pid, args.port
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_port_check_command_formats_correctly() {
        assert_eq!(build_port_check_command(8080), "ss -tlnp 'sport = :8080'");
    }

    #[test]
    fn build_port_kill_command_formats_correctly() {
        assert_eq!(build_port_kill_command(1234), "kill 1234");
    }

    #[test]
    fn parse_ss_output_extracts_single_process() {
        let output = "\
State  Recv-Q Send-Q Local Address:Port  Peer Address:Port Process
LISTEN 0      128    0.0.0.0:8080       0.0.0.0:*     users:((\"python3\",pid=1234,fd=3))";

        let processes = parse_ss_output(output);
        assert_eq!(processes.len(), 1);
        assert_eq!(processes[0].pid, 1234);
        assert_eq!(processes[0].process_name, "python3");
    }

    #[test]
    fn parse_ss_output_extracts_multiple_processes() {
        let output = "\
State  Recv-Q Send-Q Local Address:Port  Peer Address:Port Process
LISTEN 0      128    0.0.0.0:8080       0.0.0.0:*     users:((\"python3\",pid=1234,fd=3),(\"python3\",pid=5678,fd=4))";

        let processes = parse_ss_output(output);
        assert_eq!(processes.len(), 2);
        assert_eq!(processes[0].pid, 1234);
        assert_eq!(processes[1].pid, 5678);
    }

    #[test]
    fn parse_ss_output_returns_empty_when_port_is_free() {
        let output = "State  Recv-Q Send-Q Local Address:Port  Peer Address:Port Process\n";

        let processes = parse_ss_output(output);
        assert!(processes.is_empty());
    }

    #[test]
    fn parse_ss_output_handles_empty_string() {
        let processes = parse_ss_output("");
        assert!(processes.is_empty());
    }

    #[test]
    fn parse_ss_output_skips_lines_without_users() {
        let output = "\
State  Recv-Q Send-Q Local Address:Port  Peer Address:Port Process
LISTEN 0      128    0.0.0.0:8080       0.0.0.0:*";

        let processes = parse_ss_output(output);
        assert!(processes.is_empty());
    }
}
