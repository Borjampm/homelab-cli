mod common;

use common::*;
use predicates::prelude::*;

fn start_remote_server(port: u16) {
    ssh_command_on_server(&format!(
        "nohup python3 -m http.server {port} > /dev/null 2>&1 &"
    ));
    std::thread::sleep(std::time::Duration::from_millis(500));
}

fn kill_remote_server(port: u16) {
    ssh_command_on_server(&format!("fuser -k {port}/tcp 2>/dev/null; true"));
    std::thread::sleep(std::time::Duration::from_millis(200));
}

// --- Port Check Tests ---

#[test]
fn port_check_free_port() {
    require_docker_lab!();
    kill_remote_server(9090);

    homelab_command()
        .args(["port", "check", "--on", "server", "9090"])
        .assert()
        .success()
        .stdout(predicate::str::contains("is free"));
}

#[test]
fn port_check_busy_port() {
    require_docker_lab!();
    kill_remote_server(9091);
    start_remote_server(9091);

    homelab_command()
        .args(["port", "check", "--on", "server", "9091"])
        .assert()
        .success()
        .stdout(predicate::str::contains("is in use by"))
        .stdout(predicate::str::contains("python3"))
        .stdout(predicate::str::contains("PID"));

    kill_remote_server(9091);
}

// --- Port Kill Tests ---

#[test]
fn port_kill_busy_port() {
    require_docker_lab!();
    kill_remote_server(9092);
    start_remote_server(9092);

    homelab_command()
        .args(["port", "kill", "--on", "server", "9092"])
        .assert()
        .success()
        .stdout(predicate::str::contains("killed"))
        .stdout(predicate::str::contains("python3"));

    homelab_command()
        .args(["port", "check", "--on", "server", "9092"])
        .assert()
        .success()
        .stdout(predicate::str::contains("is free"));
}

#[test]
fn port_kill_free_port() {
    require_docker_lab!();
    kill_remote_server(9093);

    homelab_command()
        .args(["port", "kill", "--on", "server", "9093"])
        .assert()
        .success()
        .stdout(predicate::str::contains("already free"));
}

// --- Run Auto-Check Tests ---

#[test]
fn run_aborts_when_user_declines_port_kill() {
    require_docker_lab!();
    kill_remote_server(9094);
    start_remote_server(9094);

    let project = create_temp_project(&[("dummy.txt", "placeholder")]);
    let project_name = project
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();

    homelab_command()
        .args([
            "run",
            "--on",
            "server",
            "--forward",
            "9094",
            "--",
            "echo",
            "hello",
        ])
        .current_dir(project.path())
        .write_stdin("n\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains("is in use by"))
        .stderr(predicate::str::contains("kill and continue?"));

    kill_remote_server(9094);
    cleanup_remote_project(&project_name);
}

#[test]
fn run_continues_when_user_accepts_port_kill() {
    require_docker_lab!();
    kill_remote_server(9095);
    start_remote_server(9095);

    let project = create_temp_project(&[("dummy.txt", "placeholder")]);
    let project_name = project
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();

    homelab_command()
        .args([
            "run",
            "--on",
            "server",
            "--forward",
            "9095",
            "--",
            "echo",
            "hello",
        ])
        .current_dir(project.path())
        .write_stdin("y\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("hello"));

    cleanup_remote_project(&project_name);
}

#[test]
fn run_skips_check_when_port_is_free() {
    require_docker_lab!();
    kill_remote_server(9096);

    let project = create_temp_project(&[("dummy.txt", "placeholder")]);
    let project_name = project
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();

    homelab_command()
        .args([
            "run",
            "--on",
            "server",
            "--forward",
            "9096",
            "--",
            "echo",
            "hello",
        ])
        .current_dir(project.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("hello"))
        .stderr(predicate::str::contains("kill and continue?").not());

    cleanup_remote_project(&project_name);
}
