# homelab

A CLI tool for managing computers and devices in a home lab, built in Rust.

## What it does

`homelab` simplifies working with multiple machines across a home network (via Tailscale). Instead of SSH-ing into each machine individually, you can execute commands, run scripts, and manage nodes from a single interface.

## Project structure

```
src/
├── main.rs        # Thin entry point — delegates to lib
├── lib.rs         # CLI parsing and command dispatch
├── cli.rs         # Clap command/argument definitions
├── commands/      # One module per subcommand
└── remote/        # SSH connection and execution logic
```

## Development

### Prerequisites

- [Rust](https://rustup.rs/) (edition 2024)
- Docker (for the local lab environment)
- OpenSSH client (used by the `openssh` crate under the hood)
- [GNU rsync](https://rsync.samba.org/) 3.1+ (required for `sync` and `run` commands — macOS ships with rsync 2.x, install via `brew install rsync`)
- [Tailscale](https://tailscale.com/) (required for the `nodes` command — queries `tailscale status --json`)

### Build and run

```bash
cargo build              # Build debug binary
cargo run -- <args>      # Run the CLI
cargo test               # Run tests
cargo clippy             # Lint
cargo fmt                # Format
```

### Local lab environment

A Docker-based simulation of the home lab lives in `docker/`. It spins up SSH-enabled Alpine containers that mimic real Tailscale nodes. See [docs/docker-lab-setup.md](docs/docker-lab-setup.md) for full setup instructions (building the image, SSH config, known_hosts, etc.).

The CLI resolves host names through `~/.ssh/config`, so you need entries mapping each node to `localhost` with the right port and key. Quick start:

```bash
# Build the image (one time)
docker build -t homelab-node -f docker/Dockerfile.node docker/
chmod 600 docker/lab_key

# Start the lab (3 nodes: laptop, server, beast)
docker compose -f docker/compose.yaml up -d

# Verify connectivity
ssh server echo "ok"

# Stop the lab
docker compose -f docker/compose.yaml down
```

| Node   | Port | IP           |
|--------|------|--------------|
| laptop | 2210 | 172.20.0.10  |
| server | 2220 | 172.20.0.20  |
| beast  | 2230 | 172.20.0.30  |

When moving to the real Tailscale network, only the SSH config hostnames change — the Rust code stays the same.

### Testing

```bash
cargo test --lib                        # Unit tests only
cargo test --test docker_lab            # Integration tests (starts Docker automatically)
cargo test                              # All tests
```

Integration tests start the Docker lab automatically and stop the containers when finished.
