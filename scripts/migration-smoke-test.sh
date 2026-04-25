#!/usr/bin/env bash
# Migration drift smoke test. Builds the release server, then boots it
# twice:
#   case A: against a DB pre-seeded from baseline.sql (every shipped
#           migration recorded as applied) — catches in-place edits to
#           existing migrations, because the edited bodies will be skipped
#           and queries against the missing schema will 500.
#   case B: against an empty DB path — catches new-migration regressions
#           in fresh installs. Should be redundant with `cargo test`, but
#           the release binary unifies features differently than the test
#           binary, so this is what surfaces release-only panics like
#           "Sqlite feature is off".
# Probes three representative endpoints in each case (the same ones that
# 500'd during the fewd-nwi incident).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BASELINE="$REPO_ROOT/server/tests/fixtures/schema-snapshots/baseline.sql"
PORT="${SMOKE_TEST_PORT:-3099}"

if [ ! -f "$BASELINE" ]; then
    echo "Baseline fixture not found at $BASELINE" >&2
    echo "Regenerate per server/tests/fixtures/schema-snapshots/README.md" >&2
    exit 1
fi

# Pre-flight: bail if PORT is already bound — better to fail fast than to
# spend ~30s building a release binary only to find we can't start it.
if lsof -nP -iTCP:"$PORT" -sTCP:LISTEN -t >/dev/null 2>&1; then
    LISTENER_PID=$(lsof -nP -iTCP:"$PORT" -sTCP:LISTEN -t | head -1)
    echo "Port $PORT is already bound (PID $LISTENER_PID)." >&2
    echo "Kill it or set SMOKE_TEST_PORT to a free port and rerun." >&2
    exit 1
fi

# RustEmbed compiles ../dist into the binary; create a placeholder so the
# release build doesn't fail when the SPA hasn't been built yet (CI does
# the same dance for clippy/tests in .github/workflows/ci.yml).
mkdir -p "$REPO_ROOT/dist"
[ -f "$REPO_ROOT/dist/.gitkeep" ] || touch "$REPO_ROOT/dist/.gitkeep"

echo "Building release server..."
(cd "$REPO_ROOT/server" && cargo build --release --bin fewd-server --quiet)
BIN="$REPO_ROOT/target/release/fewd-server"

WORK="$(mktemp -d)"
SERVER_PID=""

cleanup() {
    if [ -n "$SERVER_PID" ]; then
        kill "$SERVER_PID" 2>/dev/null || true
        wait "$SERVER_PID" 2>/dev/null || true
    fi
    rm -rf "$WORK"
}
trap cleanup EXIT

run_case() {
    local label="$1"
    local db_path="$2"
    local log="$WORK/$label.log"

    echo
    echo "── case: $label ──"
    RUST_LOG=info DATABASE_PATH="$db_path" PORT="$PORT" "$BIN" >"$log" 2>&1 &
    SERVER_PID=$!

    # Migrations + seed run before axum binds. Poll the readiness log line.
    # Bail early if the server dies (panic during startup).
    for _ in $(seq 1 100); do
        if grep -q "Server running" "$log" 2>/dev/null; then break; fi
        if ! kill -0 "$SERVER_PID" 2>/dev/null; then
            echo "Server died during startup. Log:"
            cat "$log"
            return 1
        fi
        sleep 0.1
    done
    if ! grep -q "Server running" "$log"; then
        echo "Server did not report ready within 10s. Log:"
        cat "$log"
        return 1
    fi

    # `curl -f` exits non-zero on >=400 — the bug-detection mechanism.
    # /api/meals requires query params; use a date range that's guaranteed
    # to parse successfully.
    local fail=0
    curl -fsS "http://localhost:$PORT/api/recipes" >/dev/null \
        || { echo "FAIL: /api/recipes"; fail=1; }
    curl -fsS "http://localhost:$PORT/api/drink-recipes" >/dev/null \
        || { echo "FAIL: /api/drink-recipes"; fail=1; }
    curl -fsS "http://localhost:$PORT/api/meals?start_date=2026-01-01&end_date=2026-12-31" >/dev/null \
        || { echo "FAIL: /api/meals"; fail=1; }

    kill "$SERVER_PID" 2>/dev/null || true
    wait "$SERVER_PID" 2>/dev/null || true
    SERVER_PID=""

    if [ "$fail" -ne 0 ]; then
        echo "Endpoints failed in case '$label'. Server log:"
        cat "$log"
        return 1
    fi
    echo "case '$label' OK"
}

# Case A: pre-seeded baseline (catches in-place edits to applied migrations)
DB_A="$WORK/baseline.db"
sqlite3 "$DB_A" < "$BASELINE"
run_case "baseline-snapshot" "$DB_A"

# Case B: empty DB (catches release-only panics in fresh-install path)
run_case "empty-db" "$WORK/fresh.db"

echo
echo "All smoke-test cases passed."
