# Homelab CLI

A Rust CLI tool for managing computers and devices in a home lab via SSH.

## Project Overview

- **Binary name**: `homelab`
- **Purpose**: Simplify working with multiple machines in a home lab (execute commands, run scripts, manage nodes)
- **SSH approach**: Uses the `openssh` crate which wraps the system `ssh` binary and reads `~/.ssh/config`
- **Async runtime**: Tokio

## Architecture

Currently a single-crate binary. The plan is to grow into a Cargo workspace following the pattern of mature Rust projects (like `uv`), with library crates for different concerns (CLI parsing, remote execution, types, etc.) and a thin binary at the root.

### Key Dependencies

- `clap` (derive) — CLI argument parsing with subcommands
- `anyhow` — error handling at the binary/CLI level
- `serde` / `serde_json` — JSON deserialization (tailscale status output)
- `tabled` — table formatting for terminal output

## Development

### Commands

```bash
cargo build              # Build debug binary
cargo run -- <args>      # Run the CLI
cargo test               # Run all tests
cargo clippy             # Lint
cargo fmt                # Format
```

### Docker Lab Environment

A local Docker-based simulation of the home lab lives in `docker/`:

- `docker/compose.yaml` — defines SSH-enabled containers (laptop, server, beast)
- `docker/Dockerfile.node` — Alpine-based image with sshd
- `docker/lab_key` / `lab_key.pub` — SSH keypair for lab access
- `docker/scripts/` — shared scripts mounted into containers

```bash
docker compose -f docker/compose.yaml up -d    # Start lab
docker compose -f docker/compose.yaml down      # Stop lab
ssh -i docker/lab_key -p 2220 root@localhost     # Connect to "server" node
```

## Documentation

- When adding a feature that requires a new system dependency (e.g. a binary that gets shelled out to), always update the Prerequisites section of README.md.

## Coding Conventions

- This is a learning project — prefer clarity and idiomatic Rust over cleverness
- Use `thiserror` in library crates, `anyhow` in the binary/CLI crate
- Use `tracing` instead of `println!` for logging
- Follow Rust naming conventions (snake_case for functions/variables, CamelCase for types)
- Use the newtype pattern for domain-specific types
- Prefer `fs-err` over `std::fs` for better error messages
- Do not write comments in the code, it should be readable enough that comments are not needed.
- Use descriptive variable names — avoid short/vague names like `cmd`, `e`, `on`, `to`. Use `command`, `error`, `on_host`, `to_host`, etc.
- For CLI structs, use descriptive field names with explicit `#[arg(long = "...")]` to keep short user-facing flags (e.g. field `on_host` with `#[arg(long = "on")]`)
- Deduplicate shared CLI fields using a common `Args` struct with `#[command(flatten)]`
- Extract pure functions from I/O-heavy async functions to enable unit testing (functional core, imperative shell)
- Unit tests go in `#[cfg(test)] mod tests` blocks in the same file
