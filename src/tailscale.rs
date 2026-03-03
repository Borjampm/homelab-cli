use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::process::Command;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Status {
    #[serde(rename = "Self")]
    pub self_node: Node,
    #[serde(default)]
    pub peer: HashMap<String, Node>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Node {
    pub host_name: String,
    #[serde(rename = "OS")]
    pub os: String,
    #[serde(rename = "TailscaleIPs", default)]
    pub tailscale_ips: Vec<String>,
    pub online: bool,
}

pub fn parse_status_from_json_bytes(json: &[u8]) -> Result<Status> {
    serde_json::from_slice(json).context("failed to parse tailscale JSON output")
}

pub fn get_status() -> Result<Status> {
    let output = Command::new("tailscale")
        .args(["status", "--json"])
        .output()
        .context("failed to run `tailscale` — is it installed and in PATH?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("tailscale exited with {}: {stderr}", output.status);
    }

    parse_status_from_json_bytes(&output.stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load_fixture() -> &'static [u8] {
        include_bytes!("../tests/fixtures/tailscale_status.json")
    }

    #[test]
    fn parses_fixture_without_error() {
        let status = parse_status_from_json_bytes(load_fixture()).expect("failed to parse fixture");
        assert!(!status.self_node.host_name.is_empty());
    }

    #[test]
    fn self_node_has_ip() {
        let status = parse_status_from_json_bytes(load_fixture()).unwrap();
        assert!(
            !status.self_node.tailscale_ips.is_empty(),
            "self node should have at least one IP"
        );
    }

    #[test]
    fn peers_are_populated() {
        let status = parse_status_from_json_bytes(load_fixture()).unwrap();
        assert!(
            !status.peer.is_empty(),
            "fixture should contain at least one peer"
        );
    }

    #[test]
    fn peers_have_host_name() {
        let status = parse_status_from_json_bytes(load_fixture()).unwrap();
        for node in status.peer.values() {
            assert!(!node.host_name.is_empty(), "Node should have a host name")
        }
    }
}
