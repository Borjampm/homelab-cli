mod common;

use common::*;
use predicates::prelude::*;

// --- Exec Tests ---

#[test]
fn exec_echo() {
    require_docker_lab!();
    let mut command = homelab_command();

    command
        .args(["exec", "--on", "server", "--", "echo", "hello"])
        .assert()
        .success()
        .stdout(predicate::str::contains("hello"));
}

#[test]
fn exec_hostname() {
    require_docker_lab!();
    let mut command = homelab_command();

    command
        .args(["exec", "--on", "server", "--", "hostname"])
        .assert()
        .success()
        .stdout(predicate::str::contains("server"));
}

#[test]
fn exec_failing_command() {
    require_docker_lab!();
    let mut command = homelab_command();

    command
        .args(["exec", "--on", "server", "--", "false"])
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
        .args(["sync", "push", "--to", "server"])
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
        .args(["sync", "push", "--to", "server"])
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
        .args(["sync", "push", "--to", "server"])
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
        .args(["sync", "push", "--to", "server"])
        .current_dir(project.path())
        .assert()
        .success();

    let mut list_command = homelab_command();
    list_command
        .args(["sync", "list", "--on", "server"])
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
        .args(["sync", "push", "--to", "server"])
        .current_dir(project.path())
        .assert()
        .success();

    let mut remove_command = homelab_command();
    remove_command
        .args(["sync", "remove", "--on", "server", &project_name])
        .assert()
        .success();

    let dir_check =
        ssh_command_on_server(&format!("test -d ~/remote-synced-projects/{project_name}"));
    assert!(
        !dir_check.status.success(),
        "remote project directory should be deleted"
    );
}

#[test]
fn sync_push_includes_gitignored_file_when_include_flag_used() {
    require_docker_lab!();

    let project = create_temp_project(&[
        (".gitignore", ".env\n"),
        (".env", "SECRET_KEY=test123"),
        ("app.txt", "main app file"),
    ]);
    let project_name = project
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();

    let mut without_include = homelab_command();
    without_include
        .args(["sync", "push", "--to", "server"])
        .current_dir(project.path())
        .assert()
        .success();

    let env_check = ssh_command_on_server(&format!(
        "test -f ~/remote-synced-projects/{project_name}/.env"
    ));
    assert!(
        !env_check.status.success(),
        ".env should NOT be synced without --include"
    );

    let mut with_include = homelab_command();
    with_include
        .args(["sync", "push", "--to", "server", "--include", ".env"])
        .current_dir(project.path())
        .assert()
        .success();

    let env_check =
        ssh_command_on_server(&format!("cat ~/remote-synced-projects/{project_name}/.env"));
    let remote_content = String::from_utf8_lossy(&env_check.stdout);
    assert_eq!(remote_content.trim(), "SECRET_KEY=test123");

    cleanup_remote_project(&project_name);
}

// --- Run Tests ---

#[test]
fn run_basic_stdout() {
    require_docker_lab!();

    let project = create_temp_project(&[("hello.sh", "#!/bin/sh\necho 'hello from run'")]);
    let project_name = project
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();

    let mut command = homelab_command();
    command
        .args(["run", "--on", "server", "--", "sh", "hello.sh"])
        .current_dir(project.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("hello from run"));

    cleanup_remote_project(&project_name);
}

#[test]
fn run_stderr_output() {
    require_docker_lab!();

    let project = create_temp_project(&[(
        "both.sh",
        "#!/bin/sh\necho 'on stdout'\necho 'on stderr' >&2",
    )]);
    let project_name = project
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();

    let mut command = homelab_command();
    let assert = command
        .args(["run", "--on", "server", "--", "sh", "both.sh"])
        .current_dir(project.path())
        .assert()
        .success();

    let output = assert.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stdout.contains("on stdout") || stderr.contains("on stdout"),
        "stdout content should appear somewhere"
    );
    assert!(
        stderr.contains("on stderr") || stdout.contains("on stderr"),
        "stderr content should appear somewhere"
    );

    cleanup_remote_project(&project_name);
}

#[test]
fn run_nonzero_exit_code() {
    require_docker_lab!();

    let project = create_temp_project(&[(
        "fail.sh",
        "#!/bin/sh\necho 'output before failure'\nexit 42",
    )]);
    let project_name = project
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();

    let mut command = homelab_command();
    command
        .args(["run", "--on", "server", "--", "sh", "fail.sh"])
        .current_dir(project.path())
        .assert()
        .failure()
        .stdout(predicate::str::contains("output before failure"));

    cleanup_remote_project(&project_name);
}

#[test]
fn run_python_script() {
    require_docker_lab!();

    let project = create_temp_project(&[("greet.py", "print('hello from python')")]);
    let project_name = project
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();

    let mut command = homelab_command();
    command
        .args(["run", "--on", "server", "--", "python3", "greet.py"])
        .current_dir(project.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("hello from python"));

    cleanup_remote_project(&project_name);
}

#[test]
fn run_arguments_with_spaces() {
    require_docker_lab!();

    let project = create_temp_project(&[(
        "show_args.py",
        "import sys\nfor arg in sys.argv[1:]:\n    print(arg)",
    )]);
    let project_name = project
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();

    let mut command = homelab_command();
    command
        .args([
            "run",
            "--on",
            "server",
            "--",
            "python3",
            "show_args.py",
            "hello world",
            "foo bar",
        ])
        .current_dir(project.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("hello world"))
        .stdout(predicate::str::contains("foo bar"));

    cleanup_remote_project(&project_name);
}

#[test]
fn run_large_output() {
    require_docker_lab!();

    let project = create_temp_project(&[(
        "big.sh",
        "#!/bin/sh\ni=0\nwhile [ $i -lt 5000 ]; do\n  echo \"line $i\"\n  i=$((i + 1))\ndone",
    )]);
    let project_name = project
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();

    let mut command = homelab_command();
    let assert = command
        .args(["run", "--on", "server", "--", "sh", "big.sh"])
        .current_dir(project.path())
        .assert()
        .success();

    let output = assert.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line_count = stdout.lines().count();
    assert!(
        line_count >= 5000,
        "expected at least 5000 lines, got {line_count}"
    );
    assert!(stdout.contains("line 0"), "should contain first line");
    assert!(stdout.contains("line 4999"), "should contain last line");

    cleanup_remote_project(&project_name);
}

#[test]
fn run_binary_output() {
    require_docker_lab!();

    let project = create_temp_project(&[(
        "binary.sh",
        "#!/bin/sh\nprintf '\\x00\\x01\\x02\\xff\\xfe'\necho done",
    )]);
    let project_name = project
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();

    let mut command = homelab_command();
    command
        .args(["run", "--on", "server", "--", "sh", "binary.sh"])
        .current_dir(project.path())
        .assert()
        .success();

    cleanup_remote_project(&project_name);
}

#[test]
fn run_setup_installs_dependency() {
    require_docker_lab!();

    let project = create_temp_project(&[
        ("requirements.txt", "cowsay"),
        ("app.py", "import cowsay\ncowsay.cow('setup works')"),
    ]);
    let project_name = project
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();

    let mut command = homelab_command();
    command
        .args([
            "run",
            "--on",
            "server",
            "--setup",
            "pip install --break-system-packages -r requirements.txt",
            "--",
            "python3",
            "app.py",
        ])
        .current_dir(project.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("setup works"));

    cleanup_remote_project(&project_name);
}

#[test]
fn run_multiple_setup_commands() {
    require_docker_lab!();

    let project = create_temp_project(&[(
        "check.sh",
        "#!/bin/sh\ncat config.txt && test -d datadir && echo 'both present'",
    )]);
    let project_name = project
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();

    let mut command = homelab_command();
    command
        .args([
            "run",
            "--on",
            "server",
            "--setup",
            "echo 'myconfig' > config.txt",
            "--setup",
            "mkdir -p datadir",
            "--",
            "sh",
            "check.sh",
        ])
        .current_dir(project.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("both present"));

    cleanup_remote_project(&project_name);
}

#[test]
fn run_failing_setup_aborts() {
    require_docker_lab!();

    let project =
        create_temp_project(&[("should_not_run.sh", "#!/bin/sh\necho 'main command ran'")]);
    let project_name = project
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();

    let mut command = homelab_command();
    command
        .args([
            "run",
            "--on",
            "server",
            "--setup",
            "false",
            "--",
            "sh",
            "should_not_run.sh",
        ])
        .current_dir(project.path())
        .assert()
        .failure()
        .stdout(predicate::str::contains("main command ran").not());

    cleanup_remote_project(&project_name);
}

#[test]
fn run_command_not_found() {
    require_docker_lab!();

    let project = create_temp_project(&[("dummy.txt", "placeholder")]);
    let project_name = project
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();

    let mut command = homelab_command();
    command
        .args([
            "run",
            "--on",
            "server",
            "--",
            "nonexistent_command_xyz_12345",
        ])
        .current_dir(project.path())
        .assert()
        .failure();

    cleanup_remote_project(&project_name);
}
