#![allow(dead_code)]

use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::Once;

use tempfile::TempDir;

extern crate libc;

pub static DOCKER_LAB_STARTED: Once = Once::new();

pub const COMPOSE_FILE: &str = "docker/compose.yaml";
pub const LAB_KEY_RELATIVE: &str = "docker/lab_key";
pub const SSH_PORT_SERVER: u16 = 2220;
pub const LAB_PORTS: [u16; 3] = [2210, 2220, 2230];

macro_rules! require_docker_lab {
    () => {
        if !common::docker_compose_is_available() {
            eprintln!("skipping: Docker not available");
            return;
        }
    };
}

pub(crate) use require_docker_lab;

pub fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

pub fn docker_compose_is_available() -> bool {
    Command::new("docker")
        .args(["compose", "version"])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

pub fn wait_for_ssh_ready() {
    let lab_key_path = project_root().join(LAB_KEY_RELATIVE);
    for _ in 0..10 {
        let output = Command::new("ssh")
            .args([
                "-o",
                "StrictHostKeyChecking=no",
                "-o",
                "UserKnownHostsFile=/dev/null",
                "-o",
                "ConnectTimeout=2",
                "-i",
                &lab_key_path.to_string_lossy(),
                "-p",
                &SSH_PORT_SERVER.to_string(),
                "root@localhost",
                "true",
            ])
            .output()
            .expect("failed to run ssh command");
        if output.status.success() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
    panic!("SSH not ready on port {SSH_PORT_SERVER} after 10 retries");
}

pub fn update_known_hosts() {
    let home_dir = std::env::var("HOME").expect("HOME not set");
    let known_hosts_path = PathBuf::from(&home_dir).join(".ssh/known_hosts");

    let existing_content = std::fs::read_to_string(&known_hosts_path).unwrap_or_default();

    let filtered_lines: Vec<&str> = existing_content
        .lines()
        .filter(|line| {
            !LAB_PORTS
                .iter()
                .any(|port| line.contains(&format!("[localhost]:{port}")))
        })
        .collect();

    let mut new_keys = String::new();
    for port in LAB_PORTS {
        let output = Command::new("ssh-keyscan")
            .args(["-p", &port.to_string(), "localhost"])
            .output()
            .expect("failed to run ssh-keyscan");
        new_keys.push_str(&String::from_utf8_lossy(&output.stdout));
    }

    let mut final_content = filtered_lines.join("\n");
    if !final_content.is_empty() && !final_content.ends_with('\n') {
        final_content.push('\n');
    }
    final_content.push_str(&new_keys);

    std::fs::write(&known_hosts_path, final_content).expect("failed to write known_hosts");
}

extern "C" fn shutdown_docker_lab() {
    let _ = Command::new("docker")
        .args(["compose", "-f", COMPOSE_FILE, "down"])
        .current_dir(project_root())
        .status();
}

pub fn ensure_docker_lab_running() {
    DOCKER_LAB_STARTED.call_once(|| {
        let compose_status = Command::new("docker")
            .args(["compose", "-f", COMPOSE_FILE, "up", "-d", "--wait"])
            .current_dir(project_root())
            .status()
            .expect("failed to start docker compose");

        assert!(compose_status.success(), "docker compose up failed");

        wait_for_ssh_ready();
        update_known_hosts();

        unsafe {
            libc::atexit(shutdown_docker_lab);
        }
    });
}

pub fn homelab_command() -> assert_cmd::Command {
    ensure_docker_lab_running();

    assert_cmd::Command::from(std::process::Command::new(assert_cmd::cargo::cargo_bin!(
        "homelab"
    )))
}

pub fn ssh_command_on_server(remote_command: &str) -> Output {
    let lab_key_path = project_root().join(LAB_KEY_RELATIVE);
    Command::new("ssh")
        .args([
            "-o",
            "StrictHostKeyChecking=no",
            "-o",
            "UserKnownHostsFile=/dev/null",
            "-i",
            &lab_key_path.to_string_lossy(),
            "-p",
            &SSH_PORT_SERVER.to_string(),
            "root@localhost",
            remote_command,
        ])
        .output()
        .expect("failed to run ssh command")
}

pub fn cleanup_remote_project(project_name: &str) {
    ssh_command_on_server(&format!("rm -rf ~/remote-synced-projects/{project_name}"));
}

pub fn create_temp_project(files: &[(&str, &str)]) -> TempDir {
    let temp_dir = tempfile::Builder::new()
        .prefix("test-project-")
        .tempdir()
        .expect("failed to create temp project dir");
    for (name, content) in files {
        let file_path = temp_dir.path().join(name);
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent).expect("failed to create parent dirs");
        }
        std::fs::write(&file_path, content).expect("failed to write test file");
    }
    temp_dir
}
