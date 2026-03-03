# Releasing

## How the pipeline works

```
push to main
  → release-plz opens/updates a Release PR (version bump + CHANGELOG.md)
  → merge the Release PR
    → release-plz publishes to crates.io + creates a git tag
    → git tag triggers cargo-dist
      → builds binaries for macOS (ARM + Intel), Linux (x86_64), Windows (x86_64)
      → creates GitHub Release with binaries + shell/powershell installers
```

Two GitHub Actions workflows power this:

- **release-plz.yml** — runs on every push to `main`. Opens a Release PR and publishes to crates.io when the PR is merged.
- **release.yml** — runs on tag pushes (created by release-plz). Builds cross-platform binaries with cargo-dist and creates the GitHub Release.

## How to trigger a release

1. Push changes to `main` (directly or via PR).
2. release-plz automatically opens a Release PR with a version bump and changelog entry.
3. Review and merge the Release PR.
4. release-plz publishes to crates.io and creates a git tag.
5. The tag triggers cargo-dist, which builds binaries and creates the GitHub Release.

No manual steps required beyond merging the Release PR.

## Testing locally

```bash
# Check that crates.io metadata is valid
cargo publish --dry-run

# Preview what cargo-dist would build (requires cargo-dist installed)
cargo install cargo-dist
dist plan

# Preview what release-plz would do (requires release-plz installed)
cargo install release-plz
release-plz update
```

## Installation methods

### From crates.io

```bash
cargo install homelab
```

### From GitHub Releases (shell installer)

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/borjampm/homelab-cli/releases/latest/download/homelab-installer.sh | sh
```

### From GitHub Releases (PowerShell installer)

```powershell
powershell -ExecutionPolicy ByPass -c "irm https://github.com/borjampm/homelab-cli/releases/latest/download/homelab-installer.ps1 | iex"
```

### Manual download

Download the binary for your platform from [GitHub Releases](https://github.com/borjampm/homelab-cli/releases).

## Updating

```bash
# Via cargo
cargo install homelab

# Via shell installer (always installs latest)
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/borjampm/homelab-cli/releases/latest/download/homelab-installer.sh | sh
```

## Required secrets

Set these in the GitHub repository settings under **Settings → Secrets and variables → Actions**:

| Secret | Purpose | How to get it |
|--------|---------|---------------|
| `CARGO_REGISTRY_TOKEN` | Publish to crates.io | [crates.io → Account Settings → API Tokens](https://crates.io/settings/tokens) — create a scoped token with publish access for the `homelab` crate |
| `RELEASE_APP_ID` | GitHub App ID for release-plz | See GitHub App setup below |
| `RELEASE_APP_PRIVATE_KEY` | GitHub App private key for release-plz | See GitHub App setup below |

`GITHUB_TOKEN` is provided automatically by GitHub Actions and used by cargo-dist in `release.yml`. The release-plz workflow uses a GitHub App token instead, because the default `GITHUB_TOKEN` cannot trigger other workflows (like cargo-dist) from its tag pushes.

### GitHub App setup

1. Go to **Settings → Developer settings → GitHub Apps → New GitHub App**
2. Configure:
   - **Name**: something like `homelab-release`
   - **Homepage URL**: `https://github.com/borjampm/homelab-cli`
   - **Webhook**: uncheck "Active" (not needed)
   - **Permissions → Repository**:
     - `Contents`: Read and write (for tags and pushes)
     - `Pull requests`: Read and write (for opening Release PRs)
3. Click **Create GitHub App**
4. Note the **App ID** — save it as the `RELEASE_APP_ID` secret
5. Scroll down to **Private keys → Generate a private key** — download the `.pem` file and save its contents as the `RELEASE_APP_PRIVATE_KEY` secret
6. Go to **Install App** (left sidebar) → install it on the `borjampm/homelab-cli` repository

## Conventional commits

release-plz uses commit messages to determine the version bump and generate changelog entries. Following [Conventional Commits](https://www.conventionalcommits.org/) is recommended:

| Prefix | Version bump | Example |
|--------|-------------|---------|
| `fix:` | Patch (0.0.x) | `fix: handle missing SSH config gracefully` |
| `feat:` | Minor (0.x.0) | `feat: add sync command for file transfers` |
| `feat!:` or `BREAKING CHANGE:` | Major (x.0.0) | `feat!: redesign node discovery` |
| `chore:`, `docs:`, `ci:`, `refactor:` | No bump | `docs: update installation instructions` |

## License

This project uses the [MIT License](../LICENSE). This means anyone can use, modify, and distribute the software freely, as long as they include the original copyright notice and license text.
