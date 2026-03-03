# Simulating Your Tailscale Network with Docker for Offline Rust Development

## The Architecture

Your real setup: Laptop ↔ Tailscale ↔ Server / Powerful PC

Your offline simulation:

```
┌─────────────────────────────────────────────────┐
│  Docker network: "tailnet" (172.20.0.0/16)      │
│                                                  │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐      │
│  │  laptop   │  │  server   │  │  beast    │      │
│  │ .20.0.10  │  │ .20.0.20  │  │ .20.0.30  │      │
│  │  sshd ✓   │  │  sshd ✓   │  │  sshd ✓   │      │
│  │  python ✓  │  │  python ✓  │  │  python ✓  │      │
│  └──────────┘  └──────────┘  └──────────┘      │
│         ▲                                        │
│         │ SSH key auth (no passwords)            │
│         │                                        │
└─────────┼────────────────────────────────────────┘
          │
    Your host machine
    (runs `cask` CLI)
```

You don't need Headscale. Tailscale is just a networking layer — your Rust code
talks SSH, not Tailscale APIs. A Docker bridge network gives you the same
hostname-based connectivity that Tailscale's MagicDNS provides. When you deploy
to your real Tailscale network, the only thing that changes is the hostnames.

---

## Step 1: Pre-pull Everything While Online

```bash
# Pull the base image
docker pull alpine:3.21

# Build your custom SSH node image (see Step 2)
# Then pull any other images you might want
docker pull ubuntu:24.04   # if you want Ubuntu nodes too
```

**Verify it works offline**: disconnect wifi, run `docker images` — your images should still be listed.

---

## Step 2: The SSH Node Image

Create a project directory for your Docker infra:

```bash
mkdir -p ~/cask-lab/docker
cd ~/cask-lab/docker
```

```dockerfile
# ~/cask-lab/docker/Dockerfile.node
FROM alpine:3.21

# Install SSH server + common tools your scripts might need
RUN apk add --no-cache \
    openssh-server \
    openssh-client \
    bash \
    python3 \
    py3-pip \
    curl \
    jq \
    git \
    coreutils \
    procps \
    htop

# Configure sshd
RUN ssh-keygen -A && \
    mkdir -p /root/.ssh && \
    chmod 700 /root/.ssh && \
    sed -i 's/#PermitRootLogin.*/PermitRootLogin prohibit-password/' /etc/ssh/sshd_config && \
    sed -i 's/#PubkeyAuthentication.*/PubkeyAuthentication yes/' /etc/ssh/sshd_config && \
    sed -i 's/#PasswordAuthentication.*/PasswordAuthentication no/' /etc/ssh/sshd_config && \
    # Allow the SSH control master multiplexing that the `openssh` crate uses
    sed -i 's/#MaxSessions.*/MaxSessions 20/' /etc/ssh/sshd_config && \
    echo "StreamLocalBindUnlink yes" >> /etc/ssh/sshd_config

# The authorized_keys file will be mounted at runtime
EXPOSE 22
CMD ["/usr/sbin/sshd", "-D", "-e"]
```

Build it while online (so Alpine packages are downloaded):

```bash
cd ~/cask-lab/docker
docker build -t cask-node -f Dockerfile.node .
```

---

## Step 3: Generate SSH Keys

```bash
# Generate a key pair specifically for your lab
ssh-keygen -t ed25519 -f ~/cask-lab/docker/lab_key -N "" -C "cask-lab"
```

This gives you:
- `lab_key` — private key (your CLI uses this)
- `lab_key.pub` — public key (goes into each container's authorized_keys)

---

## Step 4: Docker Compose

```yaml
# ~/cask-lab/docker/compose.yaml
services:
  laptop:
    image: cask-node
    container_name: laptop
    hostname: laptop
    networks:
      tailnet:
        ipv4_address: 172.20.0.10
    volumes:
      - ./lab_key.pub:/root/.ssh/authorized_keys:ro
      - ./scripts:/opt/scripts:ro    # shared scripts directory
    ports:
      - "2210:22"   # accessible from host as localhost:2210

  server:
    image: cask-node
    container_name: server
    hostname: server
    networks:
      tailnet:
        ipv4_address: 172.20.0.20
    volumes:
      - ./lab_key.pub:/root/.ssh/authorized_keys:ro
      - ./scripts:/opt/scripts:ro
    ports:
      - "2220:22"

  beast:
    image: cask-node
    container_name: beast
    hostname: beast
    networks:
      tailnet:
        ipv4_address: 172.20.0.30
    volumes:
      - ./lab_key.pub:/root/.ssh/authorized_keys:ro
      - ./scripts:/opt/scripts:ro
    ports:
      - "2230:22"

networks:
  tailnet:
    driver: bridge
    ipam:
      config:
        - subnet: 172.20.0.0/16
```

```bash
# Start the lab
cd ~/cask-lab/docker
docker compose up -d

# Verify SSH works from host
ssh -i ./lab_key -o StrictHostKeyChecking=no -p 2210 root@localhost "hostname"
# Should print: laptop

ssh -i ./lab_key -o StrictHostKeyChecking=no -p 2220 root@localhost "hostname"
# Should print: server

# Verify containers can reach each other by hostname
docker exec laptop ssh -o StrictHostKeyChecking=no server hostname
# Should print: server (Docker DNS resolves container names)
```

### Configure your SSH client for convenience

```ssh-config
# ~/.ssh/config  (add this block)

Host lab-laptop
    HostName localhost
    Port 2210
    User root
    IdentityFile ~/cask-lab/docker/lab_key
    StrictHostKeyChecking no
    UserKnownHostsFile /dev/null

Host lab-server
    HostName localhost
    Port 2220
    User root
    IdentityFile ~/cask-lab/docker/lab_key
    StrictHostKeyChecking no
    UserKnownHostsFile /dev/null

Host lab-beast
    HostName localhost
    Port 2230
    User root
    IdentityFile ~/cask-lab/docker/lab_key
    StrictHostKeyChecking no
    UserKnownHostsFile /dev/null
```

Now `ssh lab-server` just works. And critically, the `openssh` crate reads this config automatically.

---

## Step 5: Choosing a Rust SSH Crate

Three real options:

| Crate | Version | Approach | Tradeoff |
|-------|---------|----------|----------|
| `openssh` | 0.11.6 | Wraps system `ssh` binary | Cleanest API, reads `~/.ssh/config`, but requires OpenSSH installed |
| `russh` | 0.57.0 | Pure Rust SSH implementation | No system deps, more control, but more boilerplate |
| `async-ssh2-tokio` | latest | Wraps `russh` with simple API | Easy `execute()`, but no streaming stdout |

**My recommendation: use `openssh`**. Here's why:

1. It reads your `~/.ssh/config` — when you switch from Docker lab to real Tailscale, you just change hostnames. Zero code changes.
2. Its API mirrors `std::process::Command`, which you already know conceptually.
3. It uses SSH's ControlMaster multiplexing — one TCP connection, many commands. This is how Tailscale SSH works too.
4. uv's approach of wrapping system tools rather than reimplementing protocols is the pragmatic choice.

If you later need pure-Rust (e.g., for cross-compilation to a target without OpenSSH), switch to `russh`.

---

## Step 6: Add SSH Crate to Your Workspace

```toml
# Root Cargo.toml — add to [workspace.dependencies]
openssh = { version = "0.11", features = ["process-mux"] }
```

```toml
# crates/cask-remote/Cargo.toml  (new crate)
[package]
name = "cask-remote"
version.workspace = true
edition.workspace = true

[dependencies]
openssh = { workspace = true }
tokio = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
cask-types = { path = "../cask-types" }
```

Then fetch before going offline:
```bash
cargo fetch
cargo build
cargo test
```

---

## Step 7: The Remote Execution Module

```rust
// crates/cask-remote/src/lib.rs
pub mod error;
pub mod node;
pub mod pool;

pub use node::RemoteNode;
pub use pool::NodePool;
```

```rust
// crates/cask-remote/src/error.rs
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RemoteError {
    #[error("failed to connect to {host}: {source}")]
    Connection {
        host: String,
        source: openssh::Error,
    },

    #[error("command failed on {host}: {stderr}")]
    CommandFailed {
        host: String,
        exit_code: i32,
        stderr: String,
    },

    #[error("ssh error: {0}")]
    Ssh(#[from] openssh::Error),
}
```

```rust
// crates/cask-remote/src/node.rs
use crate::error::RemoteError;
use openssh::{KnownHosts, Session, SessionBuilder};
use std::path::Path;
use tracing::{debug, instrument};

/// Represents a single remote machine — analogous to a Tailscale node.
pub struct RemoteNode {
    host: String,
    session: Session,
}

impl RemoteNode {
    /// Connect to a node. If you have ~/.ssh/config entries,
    /// just pass the Host alias (e.g., "lab-server").
    ///
    /// With the openssh crate, your SSH config is respected automatically:
    /// ports, identity files, proxy commands — all of it.
    #[instrument(skip_all, fields(host = %host))]
    pub async fn connect(host: &str) -> Result<Self, RemoteError> {
        debug!("Establishing SSH session");

        let session = Session::connect_mux(host, KnownHosts::Accept)
            .await
            .map_err(|e| RemoteError::Connection {
                host: host.to_string(),
                source: e,
            })?;

        Ok(Self {
            host: host.to_string(),
            session,
        })
    }

    /// Connect with explicit options (useful when not using ssh config).
    pub async fn connect_with(
        host: &str,
        port: u16,
        user: &str,
        key_path: &Path,
    ) -> Result<Self, RemoteError> {
        let session = SessionBuilder::default()
            .port(port)
            .user(user.to_string())
            .keyfile(key_path)
            .known_hosts_check(KnownHosts::Accept)
            .connect(host)
            .await
            .map_err(|e| RemoteError::Connection {
                host: host.to_string(),
                source: e,
            })?;

        Ok(Self {
            host: host.to_string(),
            session,
        })
    }

    /// Execute a command and return stdout.
    #[instrument(skip(self), fields(host = %self.host))]
    pub async fn exec(&self, command: &str) -> Result<String, RemoteError> {
        debug!(command, "Executing remote command");

        let output = self.session.command("bash")
            .arg("-c")
            .arg(command)
            .output()
            .await?;

        if !output.status.success() {
            let code = output.status.code().unwrap_or(-1);
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            return Err(RemoteError::CommandFailed {
                host: self.host.clone(),
                exit_code: code,
                stderr,
            });
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Execute a command, streaming stdout line by line.
    /// This is critical for long-running commands (builds, tests, etc.)
    pub async fn exec_streaming<F>(
        &self,
        command: &str,
        mut on_line: F,
    ) -> Result<(), RemoteError>
    where
        F: FnMut(&str),
    {
        use tokio::io::{AsyncBufReadExt, BufReader};

        let mut child = self.session.command("bash")
            .arg("-c")
            .arg(command)
            .stdout(openssh::Stdio::piped())
            .stderr(openssh::Stdio::piped())
            .spawn()
            .await?;

        let stdout = child.stdout().take().expect("stdout was piped");
        let mut reader = BufReader::new(stdout).lines();

        while let Some(line) = reader.next_line().await.map_err(|e| {
            RemoteError::Ssh(openssh::Error::Disconnected)
        })? {
            on_line(&line);
        }

        let status = child.wait().await?;
        if !status.success() {
            return Err(RemoteError::CommandFailed {
                host: self.host.clone(),
                exit_code: status.code().unwrap_or(-1),
                stderr: "see streaming output".to_string(),
            });
        }

        Ok(())
    }

    /// Upload a file to the remote node via SCP.
    /// The openssh crate doesn't have built-in SCP, so we use the ssh command.
    pub async fn upload(
        &self,
        local_path: &Path,
        remote_path: &str,
    ) -> Result<(), RemoteError> {
        // openssh's subsystem method or we can just use sftp
        let sftp = self.session.sftp();
        // For now, use a command-based approach:
        let content = fs_err::read(local_path)
            .map_err(|e| RemoteError::Ssh(openssh::Error::Disconnected))?;

        let encoded = base64::encode(&content);
        self.exec(&format!(
            "echo '{}' | base64 -d > {}",
            encoded, remote_path
        )).await?;

        Ok(())
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    /// Cleanly close the SSH session.
    pub async fn close(self) -> Result<(), RemoteError> {
        self.session.close().await?;
        Ok(())
    }
}
```

```rust
// crates/cask-remote/src/pool.rs
use crate::error::RemoteError;
use crate::node::RemoteNode;
use std::collections::HashMap;
use tracing::info;

/// A pool of remote nodes — your "tailnet" abstraction.
/// Maps node names to connections.
pub struct NodePool {
    nodes: HashMap<String, RemoteNode>,
}

impl NodePool {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
        }
    }

    /// Connect to multiple nodes in parallel.
    pub async fn connect_all(hosts: &[&str]) -> Result<Self, RemoteError> {
        let mut nodes = HashMap::new();

        // Connect in parallel using tokio::JoinSet
        let mut join_set = tokio::task::JoinSet::new();
        for &host in hosts {
            let host = host.to_string();
            join_set.spawn(async move {
                let node = RemoteNode::connect(&host).await?;
                Ok::<_, RemoteError>((host, node))
            });
        }

        while let Some(result) = join_set.join_next().await {
            let (host, node) = result.expect("task panicked")?;
            info!("Connected to {host}");
            nodes.insert(host, node);
        }

        Ok(Self { nodes })
    }

    pub fn get(&self, name: &str) -> Option<&RemoteNode> {
        self.nodes.get(name)
    }

    /// Run a command on ALL nodes in parallel, return results keyed by host.
    pub async fn exec_all(
        &self,
        command: &str,
    ) -> HashMap<String, Result<String, RemoteError>> {
        let mut join_set = tokio::task::JoinSet::new();

        for (host, node) in &self.nodes {
            let host = host.clone();
            let command = command.to_string();
            // We can't move `node` into the task since we're borrowing self,
            // so we need to use unsafe or restructure. For simplicity,
            // let's collect futures:
            let future = node.exec(&command);
            let host_clone = host.clone();
            join_set.spawn(async move {
                let result = future.await;
                (host_clone, result)
            });
        }

        let mut results = HashMap::new();
        while let Some(result) = join_set.join_next().await {
            let (host, output) = result.expect("task panicked");
            results.insert(host, output);
        }

        results
    }

    /// Gracefully close all connections.
    pub async fn close_all(self) {
        for (host, node) in self.nodes {
            if let Err(e) = node.close().await {
                tracing::warn!("Failed to close session to {host}: {e}");
            }
        }
    }
}
```

---

## Step 8: Wire It Into Your CLI

```rust
// crates/cask-cli/src/commands/remote.rs
use anyhow::{Context, Result};
use cask_remote::{NodePool, RemoteNode};

/// cask remote exec --nodes lab-server,lab-beast "uname -a"
pub async fn exec(nodes: &[String], command: &str) -> Result<()> {
    let hosts: Vec<&str> = nodes.iter().map(|s| s.as_str()).collect();
    let pool = NodePool::connect_all(&hosts)
        .await
        .context("failed to connect to nodes")?;

    let results = pool.exec_all(command).await;

    for (host, result) in &results {
        match result {
            Ok(stdout) => {
                println!("── {host} ──");
                print!("{stdout}");
            }
            Err(e) => {
                eprintln!("── {host} (FAILED) ──");
                eprintln!("{e}");
            }
        }
    }

    pool.close_all().await;
    Ok(())
}

/// cask remote run-script --nodes lab-server ./scripts/deploy.sh
pub async fn run_script(nodes: &[String], script_path: &str) -> Result<()> {
    let script = fs_err::read_to_string(script_path)
        .context("failed to read script")?;

    let hosts: Vec<&str> = nodes.iter().map(|s| s.as_str()).collect();
    let pool = NodePool::connect_all(&hosts).await?;

    let results = pool.exec_all(&script).await;

    for (host, result) in &results {
        match result {
            Ok(stdout) => {
                tracing::info!("{host}: OK");
                print!("{stdout}");
            }
            Err(e) => {
                tracing::error!("{host}: {e}");
            }
        }
    }

    pool.close_all().await;
    Ok(())
}
```

Add to your CLI enum:

```rust
// In cask-cli/src/lib.rs, add to the Command enum:
#[derive(Subcommand)]
enum Command {
    // ... existing commands ...

    /// Execute commands on remote nodes
    Remote {
        #[command(subcommand)]
        action: RemoteAction,
    },
}

#[derive(Subcommand)]
enum RemoteAction {
    /// Run a command on remote nodes
    Exec {
        /// Comma-separated node names (e.g., "lab-server,lab-beast")
        #[arg(short, long, value_delimiter = ',')]
        nodes: Vec<String>,

        /// Command to execute
        command: String,
    },
    /// Run a local script on remote nodes
    RunScript {
        #[arg(short, long, value_delimiter = ',')]
        nodes: Vec<String>,

        /// Path to the script
        script: String,
    },
}
```

---

## Step 9: Integration Tests Against Docker

This is where it all comes together. Your tests spin up the Docker lab and run real SSH commands.

```rust
// tests/remote_tests.rs
use assert_cmd::Command;
use predicates::prelude::*;

/// These tests require `docker compose up -d` in ~/cask-lab/docker/
/// Run them with: cargo test --test remote_tests

#[test]
fn remote_exec_single_node() {
    Command::cargo_bin("cask")
        .unwrap()
        .args(["remote", "exec", "--nodes", "lab-server", "hostname"])
        .assert()
        .success()
        .stdout(predicate::str::contains("server"));
}

#[test]
fn remote_exec_multiple_nodes() {
    Command::cargo_bin("cask")
        .unwrap()
        .args([
            "remote", "exec",
            "--nodes", "lab-server,lab-beast",
            "hostname",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("server"))
        .stdout(predicate::str::contains("beast"));
}

#[test]
fn remote_exec_failing_command() {
    Command::cargo_bin("cask")
        .unwrap()
        .args([
            "remote", "exec",
            "--nodes", "lab-server",
            "exit 1",
        ])
        .assert()
        .failure();
}
```

```rust
// For unit-testing the node module directly (no CLI involved):
// crates/cask-remote/tests/integration.rs

#[tokio::test]
async fn connect_and_exec() {
    // Uses ~/.ssh/config entry "lab-server"
    let node = cask_remote::RemoteNode::connect("lab-server")
        .await
        .expect("failed to connect");

    let output = node.exec("echo hello").await.expect("exec failed");
    assert_eq!(output.trim(), "hello");

    node.close().await.expect("close failed");
}

#[tokio::test]
async fn pool_parallel_execution() {
    let pool = cask_remote::NodePool::connect_all(&[
        "lab-server", "lab-beast",
    ]).await.expect("failed to connect");

    let results = pool.exec_all("hostname").await;

    assert!(results["lab-server"].is_ok());
    assert!(results["lab-beast"].is_ok());
    assert_eq!(results["lab-server"].as_ref().unwrap().trim(), "server");
    assert_eq!(results["lab-beast"].as_ref().unwrap().trim(), "beast");

    pool.close_all().await;
}
```

---

## Step 10: Shared Scripts Directory

Put scripts in `~/cask-lab/docker/scripts/` — they're mounted into every container at `/opt/scripts/`:

```bash
# ~/cask-lab/docker/scripts/sysinfo.sh
#!/bin/bash
echo "=== $(hostname) ==="
echo "OS: $(cat /etc/os-release | grep PRETTY_NAME | cut -d= -f2)"
echo "CPUs: $(nproc)"
echo "Memory: $(free -h | awk '/^Mem:/ {print $2}')"
echo "Uptime: $(uptime -p)"
echo "Python: $(python3 --version)"
```

```bash
chmod +x ~/cask-lab/docker/scripts/sysinfo.sh

# Test it
cask remote exec --nodes lab-server,lab-beast,lab-laptop "/opt/scripts/sysinfo.sh"
```

---

## Step 11: Simulating Different Node Capabilities

Your real Tailscale nodes have different specs. Simulate this with environment variables and constraints:

```yaml
# Updated compose.yaml — add resource constraints + env vars
services:
  laptop:
    image: cask-node
    container_name: laptop
    hostname: laptop
    environment:
      - NODE_ROLE=dev
      - NODE_CPUS=4
    deploy:
      resources:
        limits:
          cpus: "1.0"
          memory: 512M
    # ... rest same as before

  server:
    image: cask-node
    container_name: server
    hostname: server
    environment:
      - NODE_ROLE=server
      - NODE_CPUS=8
    deploy:
      resources:
        limits:
          cpus: "2.0"
          memory: 2G
    # ...

  beast:
    image: cask-node
    container_name: beast
    hostname: beast
    environment:
      - NODE_ROLE=compute
      - NODE_CPUS=16
    deploy:
      resources:
        limits:
          cpus: "4.0"
          memory: 4G
    # ...
```

Your Rust code can query these:

```rust
let role = node.exec("echo $NODE_ROLE").await?;
// Use this to decide which tasks to schedule on which node
```

---

## Step 12: The Transition to Real Tailscale

When you go back online and want to switch to your real network, the *only* change is your `~/.ssh/config`:

```ssh-config
# Replace lab entries with real Tailscale nodes:
Host ts-laptop
    HostName laptop.tail1234.ts.net   # Tailscale MagicDNS name
    User borja

Host ts-server
    HostName server.tail1234.ts.net
    User borja

Host ts-beast
    HostName beast.tail1234.ts.net
    User borja
```

Your Rust code stays identical:
```rust
// During development:
let pool = NodePool::connect_all(&["lab-server", "lab-beast"]).await?;

// In production (just change the names):
let pool = NodePool::connect_all(&["ts-server", "ts-beast"]).await?;
```

This is the whole point of using the `openssh` crate — it delegates to the system SSH which reads your config. No code changes.

---

## Pre-Offline Checklist (Additions)

```bash
# Docker images
docker pull alpine:3.21
docker build -t cask-node -f ~/cask-lab/docker/Dockerfile.node .

# Start once to verify everything works
cd ~/cask-lab/docker
docker compose up -d
ssh lab-server "echo works"
ssh lab-beast "echo works"

# Add openssh crate to workspace, then:
cargo fetch && cargo build && cargo test

# Generate local docs including openssh
cargo doc

# Clone the openssh crate source for reference
git clone https://github.com/openssh-rust/openssh.git ~/references/openssh
```

---

## Quick Management Commands

```bash
# Start the lab
docker compose -f ~/cask-lab/docker/compose.yaml up -d

# Stop (preserves state)
docker compose -f ~/cask-lab/docker/compose.yaml stop

# Destroy and recreate (clean slate)
docker compose -f ~/cask-lab/docker/compose.yaml down
docker compose -f ~/cask-lab/docker/compose.yaml up -d

# See logs from all nodes
docker compose -f ~/cask-lab/docker/compose.yaml logs -f

# Shell into a specific node
docker exec -it server bash

# Check which nodes are running
docker compose -f ~/cask-lab/docker/compose.yaml ps
```

Add these to your `justfile`:

```just
# Lab management
lab-up:
    docker compose -f ~/cask-lab/docker/compose.yaml up -d

lab-down:
    docker compose -f ~/cask-lab/docker/compose.yaml down

lab-status:
    docker compose -f ~/cask-lab/docker/compose.yaml ps

# Run remote tests (requires lab-up)
test-remote:
    cargo test --test remote_tests

# Run everything
test-all: lab-up
    cargo test
    just test-remote
```
