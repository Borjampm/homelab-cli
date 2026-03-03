use anyhow::Result;
use tabled::{Table, Tabled};

use crate::tailscale::{self, Node};

#[derive(Tabled)]
struct DeviceRow {
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "IP")]
    tailscale_ip: String,
    #[tabled(rename = "OS")]
    os: String,
    #[tabled(rename = "Status")]
    status: String,
}

fn get_ip_from_tailscale_ips(tailscale_ips: &[String]) -> String {
    tailscale_ips.first().cloned().unwrap_or_default()
}

fn device_row(node: &Node) -> DeviceRow {
    let status = if node.online {
        String::from("online")
    } else {
        String::from("offline")
    };

    DeviceRow {
        name: node.host_name.clone(),
        tailscale_ip: get_ip_from_tailscale_ips(&node.tailscale_ips),
        os: node.os.clone(),
        status,
    }
}

pub fn run() -> Result<()> {
    let status = tailscale::get_status()?;

    let mut devices = vec![&status.self_node];
    devices.extend(status.peer.values());

    let rows: Vec<DeviceRow> = devices.iter().map(|node| device_row(node)).collect();

    let table = Table::new(rows);
    println!("{table}");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_node(online: bool) -> Node {
        Node {
            host_name: "test-host".to_string(),
            tailscale_ips: vec!["100.64.0.1".to_string(), "fd7a::1".to_string()],
            os: "linux".to_string(),
            online,
        }
    }

    #[test]
    fn get_ip_from_tailscale_ips_returns_first() {
        let ips = vec!["100.64.0.1".to_string(), "fd7a::1".to_string()];
        assert_eq!(get_ip_from_tailscale_ips(&ips), "100.64.0.1");
    }

    #[test]
    fn get_ip_from_tailscale_ips_returns_empty_for_empty_list() {
        let ips: Vec<String> = vec![];
        assert_eq!(get_ip_from_tailscale_ips(&ips), "");
    }

    #[test]
    fn device_row_shows_offline_for_offline_node() {
        let node = make_node(false);
        let row = device_row(&node);
        assert_eq!(row.status, "offline");
        assert_eq!(row.name, "test-host");
        assert_eq!(row.tailscale_ip, "100.64.0.1");
    }
}
