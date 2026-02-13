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

# Deploy to a remote Linux ARM64 host (e.g., just deploy odroid@192.168.1.50)
deploy host: build-arm64
    scp server/target/{{arm64_target}}/release/fewd-server {{host}}:/opt/fewd/fewd-server

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
