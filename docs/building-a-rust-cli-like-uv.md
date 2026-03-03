# Building a Rust CLI Tool Like `uv`: A Practical Guide

> **Goal**: Take you from zero to a working, well-structured Rust CLI tool modeled after how `uv` (by Astral) is built. This guide emphasizes idiomatic Rust, real architecture decisions, and the "things you don't know you should know."

---

## 1. Understand What You're Building

`uv` is a Python package manager written in Rust. Its repo ([github.com/astral-sh/uv](https://github.com/astral-sh/uv)) is a **Cargo workspace** with ~50 crates, including:

```
crates/
├── uv-cli/          # Clap-based CLI definitions
├── uv-resolver/     # Dependency resolution logic
├── uv-installer/    # Package installation
├── uv-client/       # HTTP client for registries
├── uv-cache/        # Caching layer
├── uv-fs/           # Filesystem utilities
├── uv-git/          # Git integration
├── uv-logging/      # Structured logging
├── uv-settings/     # Configuration management
├── uv-workspace/    # Workspace/project management
└── ...40+ more
```

**You won't build all of this.** But you will build a CLI tool that follows the same architectural principles, so you learn the patterns that matter.

### What We'll Build: `cask` — A Simplified Package Manager

A CLI tool that can:
- Initialize a project (`cask init`)
- Add/remove dependencies to a manifest (`cask add <pkg>`, `cask remove <pkg>`)
- Resolve and fetch packages from a registry (`cask install`)
- Cache downloads locally
- Show a dependency tree (`cask tree`)

This covers: CLI parsing, file I/O, HTTP, caching, serialization, error handling, async, and testing — the core of what `uv` does.

---

## 2. Set Up the Project as a Cargo Workspace

uv doesn't use a single `src/main.rs`. It uses a **workspace** — multiple crates in one repo. Do the same from day one. This is how serious Rust projects are structured; it enforces separation of concerns at the compiler level.

```bash
mkdir cask && cd cask
cargo init --name cask   # This becomes your binary crate
```

Edit `Cargo.toml` to be a workspace root:

```toml
[workspace]
members = ["crates/*"]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2024"
rust-version = "1.85"

[workspace.dependencies]
# Pin ALL shared dependencies here — this is critical for consistency
clap = { version = "4.5", features = ["derive"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
toml = "1.0"
tokio = { version = "1.49", features = ["full"] }
reqwest = { version = "0.13", features = ["json"] }
anyhow = "1.0"
thiserror = "2.0"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
indicatif = "0.18"
tempfile = "3.26"
dirs = "6.0"
fs-err = "3.3"
walkdir = "2.5"
```

> **Why `workspace.dependencies`?** This is how uv (and every mature Rust project) avoids version drift. Each sub-crate references `dep.workspace = true` instead of specifying its own version. If you skip this, you'll end up with multiple versions of the same crate in your tree — slower builds, bigger binaries, subtle bugs.

Now create sub-crates:

```bash
mkdir -p crates
cargo init --lib crates/cask-cli
cargo init --lib crates/cask-client
cargo init --lib crates/cask-resolver
cargo init --lib crates/cask-cache
cargo init --lib crates/cask-fs
cargo init --lib crates/cask-manifest
cargo init --lib crates/cask-types
```

And keep a thin binary at the root:

```rust
// src/main.rs — this is ALL that goes here
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    cask_cli::run().await
}
```

> **Key lesson from uv**: The binary crate should be almost empty. All logic lives in library crates. This makes everything testable without needing to spawn processes.

---

## 3. The Crate Dependency Graph

Design your crate graph carefully. Dependencies flow **one way** — no cycles.

```
cask (binary)
  └── cask-cli
        ├── cask-client      (HTTP fetching)
        ├── cask-resolver     (dependency resolution)
        ├── cask-cache        (download cache)
        ├── cask-manifest     (reads/writes project files)
        └── cask-fs           (file system helpers)
              └── cask-types  (shared types, no dependencies)
```

`cask-types` is your leaf crate — it defines shared data structures and depends on nothing (except `serde`). Every other crate can depend on it. This is exactly what uv does with `uv-types`, `uv-distribution-types`, etc.

---

## 4. Define Your Types First (cask-types)

```toml
# crates/cask-types/Cargo.toml
[package]
name = "cask-types"
version.workspace = true
edition.workspace = true

[dependencies]
serde = { workspace = true }
```

```rust
// crates/cask-types/src/lib.rs
use serde::{Deserialize, Serialize};
use std::fmt;

/// A package name. Normalized to lowercase.
/// This is a "newtype" — a Rust pattern you should use aggressively.
/// It prevents accidentally passing a raw String where a PackageName is expected.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PackageName(String);

impl PackageName {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into().to_lowercase())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// Implement Display, not ToString. Display gives you ToString for free.
impl fmt::Display for PackageName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A semver-ish version. In a real tool you'd use the `semver` crate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Version(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageSpec {
    pub name: PackageName,
    pub version: Version,
}

/// What goes in cask.toml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub package: ManifestPackage,
    #[serde(default)]
    pub dependencies: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestPackage {
    pub name: String,
    pub version: String,
}
```

> **Newtype pattern**: This is one of the most important Rust idioms. uv uses it extensively (`PackageName`, `Version`, `Url`, etc.). It gives you type safety at zero runtime cost. A function that takes `PackageName` can't accidentally receive a URL string.

---

## 5. Error Handling — Do It Right From the Start

Rust has two schools of error handling. Use **both**, in different places:

| Crate | Use | Why |
|-------|-----|-----|
| `thiserror` | Library crates | Structured, typed errors your callers can match on |
| `anyhow` | Binary/CLI crate | Quick context-adding for user-facing errors |

```rust
// crates/cask-client/src/error.rs
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("package '{0}' not found in registry")]
    PackageNotFound(String),

    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("invalid response from registry: {0}")]
    InvalidResponse(String),
}
```

```rust
// In cask-cli, where you call client code:
use anyhow::{Context, Result};

async fn handle_install(manifest: &Manifest) -> Result<()> {
    let packages = client::fetch_all(&manifest.dependencies)
        .await
        .context("failed to fetch packages")?;  // Adds human-readable context
    // ...
    Ok(())
}
```

> **The rule**: `thiserror` in libraries, `anyhow` at the edges. uv follows this exactly. Don't use `.unwrap()` except in tests. Don't use `panic!()` for recoverable errors.

---

## 6. CLI Parsing with Clap (cask-cli)

uv's `uv-cli` crate defines all commands via Clap's derive API. Do the same:

```toml
# crates/cask-cli/Cargo.toml
[package]
name = "cask-cli"
version.workspace = true
edition.workspace = true

[dependencies]
clap = { workspace = true }
anyhow = { workspace = true }
tokio = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
cask-types = { path = "../cask-types" }
cask-manifest = { path = "../cask-manifest" }
cask-client = { path = "../cask-client" }
cask-cache = { path = "../cask-cache" }
```

```rust
// crates/cask-cli/src/lib.rs
mod commands;

use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "cask", version, about = "A tiny package manager")]
struct Cli {
    #[command(subcommand)]
    command: Command,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Initialize a new project
    Init {
        /// Project name (defaults to directory name)
        #[arg(short, long)]
        name: Option<String>,
    },
    /// Add a dependency
    Add {
        /// Package name
        package: String,
        /// Version requirement (e.g., "1.0", ">=2.3")
        #[arg(short, long, default_value = "*")]
        version: String,
    },
    /// Remove a dependency
    Remove {
        package: String,
    },
    /// Install all dependencies
    Install,
    /// Show the dependency tree
    Tree,
}

pub async fn run() -> Result<()> {
    let cli = Cli::parse();

    // Initialize tracing — uv uses `tracing` throughout, not println!
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| {
                    if cli.verbose { "debug".into() } else { "info".into() }
                }),
        )
        .init();

    match cli.command {
        Command::Init { name } => commands::init::execute(name).await,
        Command::Add { package, version } => commands::add::execute(&package, &version).await,
        Command::Remove { package } => commands::remove::execute(&package).await,
        Command::Install => commands::install::execute().await,
        Command::Tree => commands::tree::execute().await,
    }
}
```

> **Tracing, not println!**: uv uses the `tracing` crate, not `println!` or `eprintln!`. This gives you structured logging with levels (debug, info, warn, error), spans for timing, and can be filtered at runtime via `RUST_LOG=debug cask install`. Adopt this from the start.

---

## 7. Implement Commands One by One

### 7a. `cask init` — File I/O and the `fs-err` crate

```rust
// crates/cask-cli/src/commands/init.rs
use anyhow::{Context, Result};
use cask_manifest::ManifestFile;
use std::env;

pub async fn execute(name: Option<String>) -> Result<()> {
    let project_name = name.unwrap_or_else(|| {
        env::current_dir()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "my-project".to_string())
    });

    let manifest = ManifestFile::create(&project_name)
        .context("failed to create cask.toml")?;

    tracing::info!("Created {} at {}", manifest.filename(), manifest.path().display());
    Ok(())
}
```

```rust
// crates/cask-manifest/src/lib.rs
use anyhow::Result;
use cask_types::Manifest;
use std::path::{Path, PathBuf};

const MANIFEST_FILENAME: &str = "cask.toml";

pub struct ManifestFile {
    path: PathBuf,
    manifest: Manifest,
}

impl ManifestFile {
    pub fn create(name: &str) -> Result<Self> {
        let path = PathBuf::from(MANIFEST_FILENAME);
        let manifest = Manifest {
            package: cask_types::ManifestPackage {
                name: name.to_string(),
                version: "0.1.0".to_string(),
            },
            dependencies: Default::default(),
        };
        let content = toml::to_string_pretty(&manifest)?;
        // fs_err gives you error messages that include the file path.
        // std::fs just says "No such file or directory" — useless.
        fs_err::write(&path, content)?;
        Ok(Self { path, manifest })
    }

    pub fn load() -> Result<Self> {
        let path = PathBuf::from(MANIFEST_FILENAME);
        let content = fs_err::read_to_string(&path)?;
        let manifest: Manifest = toml::from_str(&content)?;
        Ok(Self { path, manifest })
    }

    pub fn save(&self) -> Result<()> {
        let content = toml::to_string_pretty(&self.manifest)?;
        fs_err::write(&self.path, content)?;
        Ok(())
    }

    pub fn manifest(&self) -> &Manifest { &self.manifest }
    pub fn manifest_mut(&mut self) -> &mut Manifest { &mut self.manifest }
    pub fn path(&self) -> &Path { &self.path }
    pub fn filename(&self) -> &str { MANIFEST_FILENAME }
}
```

> **Use `fs-err` instead of `std::fs`**: This is a drop-in replacement that enriches error messages with file paths. uv uses it. Without it, your users get "Permission denied" with no indication of *which file*. This is one of those "things you don't know you should know."

### 7b. `cask add` / `cask remove` — Mutating state

```rust
// crates/cask-cli/src/commands/add.rs
use anyhow::Result;
use cask_manifest::ManifestFile;

pub async fn execute(package: &str, version: &str) -> Result<()> {
    let mut mf = ManifestFile::load()?;
    mf.manifest_mut()
        .dependencies
        .insert(package.to_string(), version.to_string());
    mf.save()?;
    tracing::info!("Added {package}@{version}");
    Ok(())
}
```

### 7c. `cask install` — HTTP, Async, Caching, Progress Bars

This is where it gets interesting. This command needs to:
1. Read the manifest
2. For each dependency, check the cache
3. If not cached, fetch metadata from a registry (we'll use a mock or crates.io)
4. Download the package
5. Show progress

```rust
// crates/cask-client/src/lib.rs
pub mod error;

use cask_types::PackageName;
use error::ClientError;
use reqwest::Client;
use serde::Deserialize;

/// Registry client — wraps reqwest with retry logic and proper User-Agent.
pub struct RegistryClient {
    client: Client,
    base_url: String,
}

#[derive(Debug, Deserialize)]
struct CrateResponse {
    #[serde(rename = "crate")]
    krate: CrateInfo,
}

#[derive(Debug, Deserialize)]
struct CrateInfo {
    max_version: String,
}

impl RegistryClient {
    pub fn new() -> Result<Self, ClientError> {
        // ALWAYS set a User-Agent. Registries (crates.io, PyPI) reject
        // requests without one. uv sets "uv/{version}".
        let client = Client::builder()
            .user_agent(format!("cask/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(ClientError::Network)?;

        Ok(Self {
            client,
            base_url: "https://crates.io/api/v1".to_string(),
        })
    }

    /// Fetch the latest version of a package.
    pub async fn get_latest_version(
        &self,
        name: &PackageName,
    ) -> Result<String, ClientError> {
        let url = format!("{}/crates/{}", self.base_url, name);
        let resp: CrateResponse = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(ClientError::Network)?
            .error_for_status()
            .map_err(|e| {
                if e.status() == Some(reqwest::StatusCode::NOT_FOUND) {
                    ClientError::PackageNotFound(name.to_string())
                } else {
                    ClientError::Network(e)
                }
            })?
            .json()
            .await
            .map_err(ClientError::Network)?;

        Ok(resp.krate.max_version)
    }
}
```

```rust
// crates/cask-cache/src/lib.rs
use anyhow::Result;
use std::path::PathBuf;

/// Cache layout:
///   ~/.cache/cask/
///     packages/
///       <name>-<version>.tar.gz
pub struct Cache {
    root: PathBuf,
}

impl Cache {
    pub fn new() -> Result<Self> {
        // Use the `dirs` crate for platform-correct cache directories.
        // Don't hardcode ~/.cache — it's wrong on macOS and Windows.
        let root = dirs::cache_dir()
            .ok_or_else(|| anyhow::anyhow!("could not determine cache directory"))?
            .join("cask");
        fs_err::create_dir_all(&root)?;
        Ok(Self { root })
    }

    pub fn has(&self, name: &str, version: &str) -> bool {
        self.package_path(name, version).exists()
    }

    pub fn package_path(&self, name: &str, version: &str) -> PathBuf {
        self.root.join("packages").join(format!("{name}-{version}.tar.gz"))
    }

    pub fn store(&self, name: &str, version: &str, data: &[u8]) -> Result<()> {
        let path = self.package_path(name, version);
        if let Some(parent) = path.parent() {
            fs_err::create_dir_all(parent)?;
        }
        fs_err::write(&path, data)?;
        tracing::debug!("Cached {name}@{version} at {}", path.display());
        Ok(())
    }
}
```

Now the install command that ties it together:

```rust
// crates/cask-cli/src/commands/install.rs
use anyhow::{Context, Result};
use cask_cache::Cache;
use cask_client::RegistryClient;
use cask_manifest::ManifestFile;
use cask_types::PackageName;
use indicatif::{ProgressBar, ProgressStyle};

pub async fn execute() -> Result<()> {
    let mf = ManifestFile::load()?;
    let client = RegistryClient::new()?;
    let cache = Cache::new()?;

    let deps = &mf.manifest().dependencies;
    if deps.is_empty() {
        tracing::info!("No dependencies to install.");
        return Ok(());
    }

    let pb = ProgressBar::new(deps.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:30}] {pos}/{len} {msg}")?
    );

    for (name, version_req) in deps {
        let pkg_name = PackageName::new(name);
        pb.set_message(format!("Resolving {name}..."));

        // Check cache first
        if cache.has(name, version_req) {
            pb.set_message(format!("{name}@{version_req} (cached)"));
            pb.inc(1);
            continue;
        }

        // Fetch from registry
        let version = client
            .get_latest_version(&pkg_name)
            .await
            .with_context(|| format!("failed to resolve {name}"))?;

        pb.set_message(format!("Fetched {name}@{version}"));

        // In a real tool, you'd download the actual package here.
        // For now, store a placeholder to demonstrate caching.
        cache.store(name, &version, b"placeholder")?;

        pb.inc(1);
    }

    pb.finish_with_message("Done!");
    Ok(())
}
```

---

## 8. Things You Don't Know You Should Know

### 8a. The `#[must_use]` attribute

```rust
#[must_use = "this `Result` may contain an error that should be handled"]
pub fn validate(&self) -> Result<()> { ... }
```

If a caller ignores the return value, the compiler warns them. Use it on any function where ignoring the result is almost certainly a bug.

### 8b. Builder pattern for configuration

uv uses builders extensively. When a struct has many optional fields:

```rust
pub struct InstallOptions {
    parallel: bool,
    cache_enabled: bool,
    timeout: std::time::Duration,
}

impl InstallOptions {
    pub fn builder() -> InstallOptionsBuilder {
        InstallOptionsBuilder::default()
    }
}

#[derive(Default)]
pub struct InstallOptionsBuilder {
    parallel: bool,
    cache_enabled: bool,
    timeout: Option<std::time::Duration>,
}

impl InstallOptionsBuilder {
    pub fn parallel(mut self, yes: bool) -> Self { self.parallel = yes; self }
    pub fn cache(mut self, yes: bool) -> Self { self.cache_enabled = yes; self }
    pub fn timeout(mut self, d: std::time::Duration) -> Self { self.timeout = Some(d); self }

    pub fn build(self) -> InstallOptions {
        InstallOptions {
            parallel: self.parallel,
            cache_enabled: self.cache_enabled,
            timeout: self.timeout.unwrap_or(std::time::Duration::from_secs(30)),
        }
    }
}
```

### 8c. Use `Cow<'_, str>` when you might or might not need to allocate

```rust
use std::borrow::Cow;

fn normalize_name(name: &str) -> Cow<'_, str> {
    if name.contains('-') {
        Cow::Owned(name.replace('-', "_"))
    } else {
        Cow::Borrowed(name)  // Zero allocation — just borrows the input
    }
}
```

### 8d. `Arc` for sharing data across async tasks

When running parallel downloads with tokio:

```rust
use std::sync::Arc;

let client = Arc::new(RegistryClient::new()?);
let cache = Arc::new(Cache::new()?);

let handles: Vec<_> = deps.iter().map(|(name, ver)| {
    let client = Arc::clone(&client);
    let cache = Arc::clone(&cache);
    let name = name.clone();
    let ver = ver.clone();
    tokio::spawn(async move {
        // Each task gets its own Arc reference
        client.get_latest_version(&PackageName::new(&name)).await
    })
}).collect();

for handle in handles {
    handle.await??;
}
```

### 8e. Feature flags for optional functionality

```toml
[features]
default = ["git"]
git = ["dep:gix"]   # Only compile git support if enabled
```

### 8f. Clippy lints — enforce them in your workspace

Add to your root `Cargo.toml`:

```toml
[workspace.lints.clippy]
# These are what serious Rust projects enable
pedantic = { level = "warn", priority = -1 }
# Allow some pedantic lints that are too noisy
module_name_repetitions = "allow"
must_use_candidate = "allow"

[workspace.lints.rust]
unsafe_code = "deny"
```

Then in each sub-crate:

```toml
[lints]
workspace = true
```

### 8g. Global allocator

uv uses `tikv-jemallocator` on Linux for better performance. You can do the same:

```rust
// src/main.rs
#[cfg(all(not(windows), not(target_os = "openbsd"), not(target_env = "musl")))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;
```

This is an easy 10-20% speedup for allocation-heavy workloads.

---

## 9. Testing

### 9a. Unit tests (inside each crate)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_name_normalizes_to_lowercase() {
        let name = PackageName::new("MyPackage");
        assert_eq!(name.as_str(), "mypackage");
    }
}
```

### 9b. Integration tests for the CLI (using `assert_cmd`)

```toml
# Root Cargo.toml
[dev-dependencies]
assert_cmd = "2.1"
predicates = "3.1"
tempfile = { workspace = true }
```

```rust
// tests/cli_tests.rs
use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

#[test]
fn init_creates_manifest() {
    let dir = TempDir::new().unwrap();

    Command::cargo_bin("cask")
        .unwrap()
        .current_dir(&dir)
        .arg("init")
        .arg("--name")
        .arg("test-project")
        .assert()
        .success();

    let manifest = std::fs::read_to_string(dir.path().join("cask.toml")).unwrap();
    assert!(manifest.contains("test-project"));
}

#[test]
fn add_appends_dependency() {
    let dir = TempDir::new().unwrap();

    // First init
    Command::cargo_bin("cask").unwrap()
        .current_dir(&dir)
        .args(["init", "--name", "test"])
        .assert().success();

    // Then add
    Command::cargo_bin("cask").unwrap()
        .current_dir(&dir)
        .args(["add", "serde", "--version", "1.0"])
        .assert().success();

    let manifest = std::fs::read_to_string(dir.path().join("cask.toml")).unwrap();
    assert!(manifest.contains("serde"));
}

#[test]
fn install_without_manifest_fails() {
    let dir = TempDir::new().unwrap();

    Command::cargo_bin("cask").unwrap()
        .current_dir(&dir)
        .arg("install")
        .assert()
        .failure()
        .stderr(predicate::str::contains("cask.toml"));
}
```

> **This is how uv tests its CLI.** They spawn the actual binary and check stdout/stderr/exit codes. This catches real bugs that unit tests miss (argument parsing, error formatting, file creation).

### 9c. Snapshot testing with `insta`

For complex outputs (like `cask tree`), snapshot tests are invaluable:

```rust
use insta::assert_snapshot;

#[test]
fn tree_output_format() {
    let tree = render_tree(&deps);
    assert_snapshot!(tree);
}
```

First run creates the snapshot file. Subsequent runs compare against it. `cargo insta review` lets you approve changes interactively.

---

## 10. CI/CD: Your `justfile` and GitHub Actions

Use [just](https://github.com/casey/just) as a task runner (the Rust community's `make`):

```just
# justfile

# Run all checks
check:
    cargo clippy --all-targets --all-features -- -D warnings
    cargo fmt --check
    cargo test

# Format code
fmt:
    cargo fmt

# Run with debug logging
run *args:
    RUST_LOG=debug cargo run -- {{args}}

# Build release binary
release:
    cargo build --release
```

For GitHub Actions:

```yaml
# .github/workflows/ci.yml
name: CI
on: [push, pull_request]

jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt
      - uses: Swatinem/rust-cache@v2  # Cache cargo build artifacts
      - run: cargo clippy --all-targets -- -D warnings
      - run: cargo fmt --check
      - run: cargo test
```

---

## 11. Milestone Roadmap

| Milestone | What You Build | Key Rust Concepts |
|-----------|---------------|-------------------|
| **M1** | Workspace + `cask init` + `cask add` | Workspace layout, serde, toml, fs-err, `Result`/`?` |
| **M2** | `cask install` with HTTP + caching | async/await, reqwest, tokio, `Arc`, error handling |
| **M3** | Progress bars + logging | tracing, indicatif, `EnvFilter` |
| **M4** | `cask tree` with resolution | Recursive data structures, `Cow`, iterators |
| **M5** | Parallel downloads | `tokio::spawn`, `Arc`, `JoinSet` |
| **M6** | Integration tests + CI | assert_cmd, tempfile, GitHub Actions |
| **M7** | Config file support (`~/.config/cask/config.toml`) | dirs, builder pattern, feature flags |
| **M8** | Lockfile generation | Deterministic serialization, hashing |

---

## 12. Essential Reading

- **The Rust Book** (free): https://doc.rust-lang.org/book/ — chapters 6 (enums), 9 (errors), 10 (generics/traits), 15 (smart pointers), and 16 (concurrency) are the most relevant.
- **Rust CLI book** (free): https://rust-cli.github.io/book/ — short, practical guide specifically for CLIs.
- **uv source code**: https://github.com/astral-sh/uv — read `crates/uv-cli/src/lib.rs` first to see how they structure Clap commands, then `crates/uv-client/` for HTTP patterns.
- **Effective Rust** by David Drysdale: Modern "Effective C++" equivalent for Rust.
- **Error handling in Rust**: https://nick.groenen.me/posts/rust-error-handling/ — best single article on the topic.

---

## Quick-Start Checklist

```bash
# 1. Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 2. Install helpful tools
cargo install cargo-watch   # Auto-rebuild on file change
cargo install cargo-expand  # See what macros expand to
cargo install just          # Task runner

# 3. Create your project
mkdir cask && cd cask
# Follow Section 2 above

# 4. Development loop
just check                           # Run all checks
cargo watch -x 'test -p cask-types'  # Watch one crate
RUST_LOG=debug cargo run -- init     # Run with debug logging
```
