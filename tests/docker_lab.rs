use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::Once;

use predicates::prelude::*;
use tempfile::TempDir;

extern crate libc;

static DOCKER_LAB_STARTED: Once = Once::new();

const COMPOSE_FILE: &str = "docker/compose.yaml";
const LAB_KEY_RELATIVE: &str = "docker/lab_key";
const SSH_PORT_SERVER: u16 = 2220;
const LAB_PORTS: [u16; 3] = [2210, 2220, 2230];

macro_rules! require_docker_lab {
    () => {
        if !docker_compose_is_available() {
            eprintln!("skipping: Docker not available");
            return;
        }
    };
}

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn docker_compose_is_available() -> bool {
    Command::new("docker")
        .args(["compose", "version"])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn wait_for_ssh_ready() {
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

fn update_known_hosts() {
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

fn ensure_docker_lab_running() {
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

fn homelab_command() -> assert_cmd::Command {
    ensure_docker_lab_running();

    assert_cmd::Command::from(std::process::Command::new(assert_cmd::cargo::cargo_bin!(
        "homelab"
    )))
}

fn ssh_command_on_server(remote_command: &str) -> Output {
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

fn cleanup_remote_project(project_name: &str) {
    ssh_command_on_server(&format!("rm -rf ~/remote-synced-projects/{project_name}"));
}

// --- Exec Tests ---

#[test]
fn exec_echo() {
    require_docker_lab!();
    let mut command = homelab_command();

    command
        .args(["exec", "--on", "lab-server", "--", "echo", "hello"])
        .assert()
        .success()
        .stdout(predicate::str::contains("hello"));
}

#[test]
fn exec_hostname() {
    require_docker_lab!();
    let mut command = homelab_command();

    command
        .args(["exec", "--on", "lab-server", "--", "hostname"])
        .assert()
        .success()
        .stdout(predicate::str::contains("server"));
}

#[test]
fn exec_failing_command() {
    require_docker_lab!();
    let mut command = homelab_command();

    command
        .args(["exec", "--on", "lab-server", "--", "false"])
        .assert()
        .failure();
}

#[test]
fn exec_nonexistent_host_fails() {
    require_docker_lab!();
    let mut command = homelab_command();

    command
        .args(["exec", "--on", "nonexistent", "--", "echo", "hi"])
        .assert()
        .failure();
}

// --- Sync Tests ---

fn create_temp_project(files: &[(&str, &str)]) -> TempDir {
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

#[test]
fn sync_push_creates_remote_directory() {
    require_docker_lab!();

    let project = create_temp_project(&[("test.txt", "hello from test")]);
    let project_name = project
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();

    let mut command = homelab_command();
    command
        .args(["sync", "push", "--to", "lab-server"])
        .current_dir(project.path())
        .assert()
        .success();

    let verification = ssh_command_on_server(&format!(
        "test -f ~/remote-synced-projects/{project_name}/test.txt"
    ));
    assert!(verification.status.success(), "remote file should exist");

    cleanup_remote_project(&project_name);
}

#[test]
fn sync_push_transfers_file_contents() {
    require_docker_lab!();

    let expected_content = "integration test content 12345";
    let project = create_temp_project(&[("data.txt", expected_content)]);
    let project_name = project
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();

    let mut command = homelab_command();
    command
        .args(["sync", "push", "--to", "lab-server"])
        .current_dir(project.path())
        .assert()
        .success();

    let cat_output = ssh_command_on_server(&format!(
        "cat ~/remote-synced-projects/{project_name}/data.txt"
    ));
    let remote_content = String::from_utf8_lossy(&cat_output.stdout);
    assert_eq!(remote_content.trim(), expected_content);

    cleanup_remote_project(&project_name);
}

#[test]
fn sync_push_excludes_git_directory() {
    require_docker_lab!();

    let project = create_temp_project(&[("src.txt", "source code")]);
    std::fs::create_dir_all(project.path().join(".git")).expect("failed to create .git dir");
    std::fs::write(project.path().join(".git/HEAD"), "ref: refs/heads/main")
        .expect("failed to write git HEAD");

    let project_name = project
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();

    let mut command = homelab_command();
    command
        .args(["sync", "push", "--to", "lab-server"])
        .current_dir(project.path())
        .assert()
        .success();

    let git_check = ssh_command_on_server(&format!(
        "test -d ~/remote-synced-projects/{project_name}/.git"
    ));
    assert!(
        !git_check.status.success(),
        ".git directory should not be synced"
    );

    cleanup_remote_project(&project_name);
}

#[test]
fn sync_list_shows_pushed_project() {
    require_docker_lab!();

    let project = create_temp_project(&[("file.txt", "listed project")]);
    let project_name = project
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();

    let mut push_command = homelab_command();
    push_command
        .args(["sync", "push", "--to", "lab-server"])
        .current_dir(project.path())
        .assert()
        .success();

    let mut list_command = homelab_command();
    list_command
        .args(["sync", "list", "--on", "lab-server"])
        .assert()
        .success()
        .stdout(predicate::str::contains(&project_name));

    cleanup_remote_project(&project_name);
}

#[test]
fn sync_remove_deletes_project() {
    require_docker_lab!();

    let project = create_temp_project(&[("remove_me.txt", "temporary")]);
    let project_name = project
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();

    let mut push_command = homelab_command();
    push_command
        .args(["sync", "push", "--to", "lab-server"])
        .current_dir(project.path())
        .assert()
        .success();

    let mut remove_command = homelab_command();
    remove_command
        .args(["sync", "remove", "--on", "lab-server", &project_name])
        .assert()
        .success();

    let dir_check =
        ssh_command_on_server(&format!("test -d ~/remote-synced-projects/{project_name}"));
    assert!(
        !dir_check.status.success(),
        "remote project directory should be deleted"
    );
}
