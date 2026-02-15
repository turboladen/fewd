#!/usr/bin/env bash
# One-time setup for fewd on a remote Linux host.
# Usage: just setup-remote user@hostname
set -euo pipefail

echo "==> Creating fewd service user..."
sudo useradd --system --shell /usr/sbin/nologin fewd 2>/dev/null || true

echo "==> Creating directories..."
sudo mkdir -p /opt/fewd/data
sudo chown -R fewd:fewd /opt/fewd

echo "==> Installing systemd service..."
sudo cp /opt/fewd/fewd.service /etc/systemd/system/fewd.service
sudo systemctl daemon-reload
sudo systemctl enable fewd

echo ""
echo "Setup complete. Deploy with:  just deploy user@hostname"
