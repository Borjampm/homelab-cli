use std::fmt;

use anyhow::{Context, Result};
use openssh::{KnownHosts, Session};

pub struct PortProcess {
    pub pid: u32,
    pub process_name: String,
}

impl fmt::Display for PortProcess {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} (PID {})", self.process_name, self.pid)
    }
}

fn build_port_check_command(port: u16) -> String {
    format!("ss -tlnp 'sport = :{port}'")
}

fn build_port_kill_command(pid: u32) -> String {
    format!("kill {pid}")
}

fn extract_users_block(line: &str) -> Option<&str> {
    let rest = &line[line.find("users:((")? + 8..];
    let end = rest.find("))")?;
    Some(&rest[..end])
}

fn parse_process_entry(entry: &str) -> Option<PortProcess> {
    let fields: Vec<&str> = entry.split(',').collect();
    if fields.len() < 2 {
        return None;
    }

    let process_name = fields[0].trim_matches('"');
    let pid = fields
        .iter()
        .find(|field| field.starts_with("pid="))?
        .strip_prefix("pid=")?
        .parse()
        .ok()?;

    Some(PortProcess {
        pid,
        process_name: process_name.to_string(),
    })
}

fn parse_ss_output(output: &str) -> Vec<PortProcess> {
    output
        .lines()
        .skip(1)
        .filter_map(extract_users_block)
        .flat_map(|block| block.split("),("))
        .filter_map(parse_process_entry)
        .collect()
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

pub async fn kill_processes(session: &Session, processes: &[PortProcess]) -> Result<()> {
    for process in processes {
        let kill_command = build_port_kill_command(process.pid);
        session
            .raw_command(&kill_command)
            .status()
            .await
            .with_context(|| format!("failed to kill PID {}", process.pid))?;
    }
    Ok(())
}

async fn with_session<F, Fut>(host: &str, operation: F) -> Result<()>
where
    F: FnOnce(Session) -> Fut,
    Fut: std::future::Future<Output = Result<()>>,
{
    let session = Session::connect(host, KnownHosts::Strict)
        .await
        .with_context(|| format!("failed to connect to {host}"))?;
    operation(session).await
}

pub async fn run_check(args: &crate::cli::PortTargetArgs) -> Result<()> {
    with_session(&args.on_host, |session| async move {
        let processes = check_port(&session, args.port).await?;
        let _ = tokio::time::timeout(std::time::Duration::from_secs(2), session.close()).await;

        if processes.is_empty() {
            println!("port {} is free on {}", args.port, args.on_host);
        } else {
            for process in &processes {
                println!("port {} is in use by {process}", args.port);
            }
        }
        Ok(())
    })
    .await
}

pub async fn run_kill(args: &crate::cli::PortTargetArgs) -> Result<()> {
    with_session(&args.on_host, |session| async move {
        let processes = check_port(&session, args.port).await?;
        kill_processes(&session, &processes).await?;
        let _ = tokio::time::timeout(std::time::Duration::from_secs(2), session.close()).await;

        if processes.is_empty() {
            println!("port {} is already free on {}", args.port, args.on_host);
        } else {
            for process in &processes {
                println!("killed {process} on port {}", args.port);
            }
        }
        Ok(())
    })
    .await
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
    fn extract_users_block_returns_process_list() {
        let line = "LISTEN 0  128  0.0.0.0:8080  0.0.0.0:*  users:((\"python3\",pid=1234,fd=3))";
        assert_eq!(extract_users_block(line), Some("\"python3\",pid=1234,fd=3"));
    }

    #[test]
    fn extract_users_block_returns_none_without_users() {
        let line = "LISTEN 0  128  0.0.0.0:8080  0.0.0.0:*";
        assert_eq!(extract_users_block(line), None);
    }

    #[test]
    fn parse_process_entry_extracts_name_and_pid() {
        let entry = "\"python3\",pid=1234,fd=3";
        let process = parse_process_entry(entry).unwrap();
        assert_eq!(process.process_name, "python3");
        assert_eq!(process.pid, 1234);
    }

    #[test]
    fn parse_process_entry_returns_none_for_incomplete_entry() {
        assert!(parse_process_entry("\"python3\"").is_none());
    }

    #[test]
    fn parse_process_entry_returns_none_without_pid_field() {
        assert!(parse_process_entry("\"python3\",fd=3").is_none());
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
