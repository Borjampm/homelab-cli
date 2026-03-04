test *args:
    #!/usr/bin/env bash
    set -euo pipefail
    trap 'docker compose -f docker/compose.yaml down' EXIT
    cargo test {{args}}
