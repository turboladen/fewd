arm64_target := "aarch64-unknown-linux-gnu"

# Development: run Axum + Vite concurrently (http://localhost:5173)
dev:
    bunx concurrently \
        --names "server,client" \
        --prefix-colors "blue,green" \
        "cd server && cargo run" \
        "bun run dev"

# Build production binary (embeds SPA into single executable)
build:
    bun run build
    cd server && cargo build --release

# Run production binary
run: build
    ./server/target/release/fewd-server

# Cross-compile for Linux ARM64 (e.g., ODroid N2+)
build-arm64:
    bun run build
    cd server && cargo build --release --target {{arm64_target}}

# Deploy to a remote Linux ARM64 host (e.g., just deploy user@192.168.1.50)
deploy host: build-arm64
    ssh {{host}} "sudo systemctl stop fewd || true"
    cat target/{{arm64_target}}/release/fewd-server | ssh {{host}} "sudo tee /opt/fewd/fewd-server > /dev/null && sudo chmod +x /opt/fewd/fewd-server"
    cat deploy/fewd.service | ssh {{host}} "sudo tee /opt/fewd/fewd.service > /dev/null"
    ssh {{host}} "sudo chown -R fewd:fewd /opt/fewd && sudo systemctl daemon-reload && sudo systemctl start fewd"
    @echo ""
    @echo "✅ Deployed to {{host}}. Verify at http://$(echo {{host}} | cut -d@ -f2):3000"

# First-time remote setup: creates fewd user, directories, and installs systemd service
setup-remote host:
    ssh {{host}} "sudo mkdir -p /opt/fewd && sudo chown \$(whoami) /opt/fewd"
    cat deploy/fewd.service | ssh {{host}} "cat > /opt/fewd/fewd.service"
    cat deploy/setup-remote.sh | ssh {{host}} "cat > /tmp/setup-remote.sh"
    ssh {{host}} "bash /tmp/setup-remote.sh"

# Type-check frontend without emitting
check-frontend:
    bunx tsc --noEmit

# Check backend compiles
check-backend:
    cargo check

# Check both
check: check-backend check-frontend

# Run all CI checks locally
ci:
    cd server && cargo fmt --all -- --check
    cd server && cargo clippy --all-targets --all-features -- -D warnings
    cd server && cargo test --all-features
    dprint check
    bun run lint
    bun test
    typos --config .typos.toml

# Reset the dev DB: delete the files, then start the server briefly so
# startup migrations (and seed_if_empty) apply to a fresh database.
# Uses PORT=3099 to avoid colliding with a running `just dev`.
db-reset:
    #!/usr/bin/env bash
    set -euo pipefail
    rm -f server/data/fewd.db server/data/fewd.db-shm server/data/fewd.db-wal
    echo "DB files removed. Building server..."
    (cd server && cargo build --bin fewd-server --quiet)
    echo "Running migrations on fresh DB..."
    LOG=$(mktemp)
    (cd server && PORT=3099 RUST_LOG=info ../target/debug/fewd-server >"$LOG" 2>&1) &
    SERVER_PID=$!
    # Migrations complete before axum binds; wait for the "Server running" log.
    for _ in $(seq 1 100); do
        if grep -q "Server running" "$LOG" 2>/dev/null; then break; fi
        sleep 0.1
    done
    kill "$SERVER_PID" 2>/dev/null || true
    wait "$SERVER_PID" 2>/dev/null || true
    if ! grep -q "Server running" "$LOG"; then
        echo "⚠️  Server did not report ready within 10s. Log:"
        cat "$LOG"
        rm -f "$LOG"
        exit 1
    fi
    rm -f "$LOG"
    echo "✅ Fresh DB at server/data/fewd.db with migrations applied."
