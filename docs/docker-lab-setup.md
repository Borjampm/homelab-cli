# Docker Lab Setup

Steps to set up the Docker-based lab environment for local testing.

## 1. Build the Docker image

The `homelab-node` image is a custom Alpine-based image with sshd and common tools. It must be built locally before starting the lab:

```bash
docker build -t homelab-node -f docker/Dockerfile.node docker/
```

## 2. Fix SSH key permissions

The lab SSH key must have strict permissions (required by the ssh client):

```bash
chmod 600 docker/lab_key
```

## 3. Start the lab containers

```bash
docker compose -f docker/compose.yaml up -d
```

This starts three containers:

| Container | SSH Port (host) | IP (docker network) |
|-----------|----------------|---------------------|
| laptop    | 2210           | 172.20.0.10         |
| server    | 2220           | 172.20.0.20         |
| beast     | 2230           | 172.20.0.30         |

## 4. Configure SSH

Create or update `~/.ssh/config` with entries for each lab node:

```
Host server
    HostName localhost
    Port 2220
    User root
    IdentityFile ~/repositories/homelab-cli/docker/lab_key
    StrictHostKeyChecking accept-new

Host laptop
    HostName localhost
    Port 2210
    User root
    IdentityFile ~/repositories/homelab-cli/docker/lab_key
    StrictHostKeyChecking accept-new

Host beast
    HostName localhost
    Port 2230
    User root
    IdentityFile ~/repositories/homelab-cli/docker/lab_key
    StrictHostKeyChecking accept-new
```

> **Note**: `StrictHostKeyChecking accept-new` auto-accepts host keys on first connection. The `openssh` crate uses `KnownHosts::Strict` which requires keys to be in `~/.ssh/known_hosts`, so we cannot use `UserKnownHostsFile /dev/null`.

## 5. Verify connectivity

```bash
ssh server echo "connection works"
ssh laptop echo "connection works"
ssh beast echo "connection works"
```

## 6. Seed known_hosts (after rebuild)

If you rebuild the Docker image, container host keys change. Clear and re-seed:

```bash
rm -f ~/.ssh/known_hosts
ssh server echo "ok"
ssh laptop echo "ok"
ssh beast echo "ok"
```

## Stopping the lab

```bash
docker compose -f docker/compose.yaml down
```

## Troubleshooting

### SSH "Permission denied (publickey)"

The Dockerfile sets `StrictModes no` in sshd_config because the `authorized_keys` file is bind-mounted from the host with potentially wrong ownership/permissions (Docker bind mounts preserve host UID/GID). Without `StrictModes no`, sshd rejects the key because the file isn't owned by root or has too-open permissions.
