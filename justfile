arm64_target := "aarch64-unknown-linux-gnu"

# Development: run Axum + Vite concurrently (http://localhost:5173).
# Cargo runs from the workspace root (no `cd server`), so DATABASE_PATH's
# `./data/fewd.db` default resolves against the project root. A server started
# from `server/` would create a parallel `server/data/fewd.db` instead.
#
# RUST_LOG defaults to `info` so server boot, migration application
# (`sea_orm_migration::Migrator::up` logs each applied migration at info
# level), and other tracing::info! lines are visible during dev. Set
# `RUST_LOG` in the shell to override (e.g. `RUST_LOG=debug just dev`).
dev:
    bunx concurrently \
        --names "server,client" \
        --prefix-colors "blue,green" \
        "RUST_LOG=${RUST_LOG:-info} cargo run --bin fewd-server" \
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
#
# The first remote step stops the unit and runs `reset-failed`, which zeroes its
# start-limit counter, so a deploy that fixes a crash loop can start the unit
# after the cap trips.
#
# Pushes the unit to BOTH /opt/fewd/fewd.service (the staging copy
# `just setup-remote` reads on first install) and /etc/systemd/system/
# (where systemd actually loads it from on every reload). Without the
# /etc copy a deploy never propagates unit-file edits to the live
# service — fewd-82e was a 403 regression caused by exactly this gap.
deploy host: build-arm64
    ssh {{host}} "sudo systemctl stop fewd || true; sudo systemctl reset-failed fewd || true"
    cat target/{{arm64_target}}/release/fewd-server | ssh {{host}} "sudo tee /opt/fewd/fewd-server > /dev/null && sudo chmod +x /opt/fewd/fewd-server"
    cat deploy/fewd.service | ssh {{host}} "sudo tee /opt/fewd/fewd.service > /dev/null"
    cat deploy/fewd.service | ssh {{host}} "sudo tee /etc/systemd/system/fewd.service > /dev/null"
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

# This recipe skips the migration drift smoke test and the frozen-lockfile
# check. Clippy and tests run once per package, and the comment on those steps
# in ci.yml explains why. `just --list` shows only the last comment line.
# Run the fail-fast subset of the CI checks locally
ci:
    cargo fmt --all -- --check
    cargo clippy -p fewd-server --all-targets --all-features -- -D warnings
    cargo clippy -p migration --all-targets -- -D warnings
    cargo test -p fewd-server --all-features
    cargo test -p migration
    dprint check
    bun run lint
    bun run test
    typos --config .typos.toml

# Migration drift smoke test: builds the release binary and boots it
# against (a) a snapshot of current prod schema, and (b) a fresh DB.
# Catches in-place edits to already-applied migrations and release-only
# panics that `cargo test` misses (e.g. feature-gated SchemaManager
# helpers like has_column() that need sqlx-sqlite). Run after every
# successful deploy to refresh the baseline; see
# server/tests/fixtures/schema-snapshots/README.md.
smoke-test:
    bash scripts/migration-smoke-test.sh

# Like `just dev`, this recipe runs from the workspace root, so the database it
# resets is `data/fewd.db` unless DATABASE_PATH names another file. It refuses
# to run while a process holds that file open, such as a busy `just dev`, though
# an idle server with no open connections escapes the check. It boots on an
# ephemeral port (PORT=0), so it never competes with `just dev` or the
# migration smoke test for a port.
#
# Delete the dev DB, then boot the server once to migrate and seed a fresh file.
db-reset:
    #!/usr/bin/env bash
    set -euo pipefail
    # An empty DATABASE_PATH usually means a caller's variable failed to expand.
    # Falling back to the default then would wipe the dev database, so refuse.
    if [ -n "${DATABASE_PATH+set}" ] && [ -z "$DATABASE_PATH" ]; then
        echo "DATABASE_PATH is set but empty. Unset it to reset data/fewd.db, or name a file." >&2
        exit 1
    fi
    DB="${DATABASE_PATH:-data/fewd.db}"
    if ! command -v lsof >/dev/null; then
        echo "db-reset needs lsof to check whether a server has $DB open." >&2
        exit 1
    fi
    refuse_if_open() {
        local holders
        if [ -e "$DB" ] && holders=$(lsof -t -- "$DB" 2>/dev/null); then
            echo "$DB is open in PID(s) $(paste -sd' ' - <<<"$holders"). Stop that server (a running just dev?) and rerun." >&2
            exit 1
        fi
    }
    refuse_if_open
    # Build before deleting, so a compile error leaves the existing DB in place.
    echo "Resetting $DB. Building server..."
    cargo build --bin fewd-server --quiet
    # Check again, because a `just dev` that finished compiling during this
    # build can have opened the file since.
    refuse_if_open
    rm -f -- "$DB" "$DB-shm" "$DB-wal" "$DB-journal"
    echo "DB files removed. Running migrations on fresh DB..."
    LOG=$(mktemp)
    SERVER_PID=""
    cleanup() {
        if [ -n "$SERVER_PID" ]; then
            kill "$SERVER_PID" 2>/dev/null || true
            wait "$SERVER_PID" 2>/dev/null || true
        fi
        rm -f "$LOG"
    }
    trap cleanup EXIT
    # `exec` so the subshell *becomes* fewd-server; otherwise $! is the
    # subshell PID and `kill` only signals the wrapper, leaving the server
    # orphaned and still bound to the port.
    (exec env PORT=0 DATABASE_PATH="$DB" RUST_LOG=info ./target/debug/fewd-server >"$LOG" 2>&1) &
    SERVER_PID=$!
    # Migrations complete before axum binds; wait for the "Server running" log,
    # and stop waiting as soon as the server exits.
    for _ in $(seq 1 100); do
        if grep -q "Server running" "$LOG" 2>/dev/null; then break; fi
        if ! kill -0 "$SERVER_PID" 2>/dev/null; then break; fi
        sleep 0.1
    done
    if ! grep -q "Server running" "$LOG"; then
        if kill -0 "$SERVER_PID" 2>/dev/null; then
            echo "⚠️  Server did not report ready within 10s. Log:" >&2
        else
            echo "⚠️  Server exited before reporting ready. Log:" >&2
        fi
        cat "$LOG" >&2
        exit 1
    fi
    echo "✅ Fresh DB at $DB with migrations applied."
