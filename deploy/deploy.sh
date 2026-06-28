#!/usr/bin/env bash
# Cross-compile for the Raspberry Pi Zero W and deploy as a long-running daemon.
#
# Usage:
#   PI_HOST=pi@raspberrypi-weather.home ./deploy/deploy.sh [--overwrite-config]
#
# By default a config.toml already on the device is left untouched. Pass
# --overwrite-config (or OVERWRITE_CONFIG=1) to replace it with the repo copy.
#
# Requires `cross` on the dev machine (cargo install cross) and Docker/Podman.
set -euo pipefail

PI_HOST="${PI_HOST:-pi@raspberrypi-weather.home}"
PI_DIR="${PI_DIR:-/home/pi/eink}"
TARGET="arm-unknown-linux-gnueabihf"
OVERWRITE_CONFIG="${OVERWRITE_CONFIG:-0}"

for arg in "$@"; do
    case "$arg" in
        --overwrite-config) OVERWRITE_CONFIG=1 ;;
        -h|--help)
            echo "Usage: PI_HOST=pi@host ./deploy/deploy.sh [--overwrite-config]"
            exit 0
            ;;
        *)
            echo "unknown argument: $arg" >&2
            exit 1
            ;;
    esac
done

cd "$(dirname "$0")/.."

echo ">> Building for ${TARGET} ..."
cross build --release --target "${TARGET}" --no-default-features --features device

BIN_DIR="target/${TARGET}/release"
echo ">> Built ${BIN_DIR}/{eink-daemon,render-once}"

echo ">> Ensuring ${PI_DIR} exists on ${PI_HOST} ..."
ssh "${PI_HOST}" "mkdir -p '${PI_DIR}'"

# Stop the daemon first: a running executable is locked (ETXTBSY) and can't be
# overwritten. It is restarted by 'enable --now' at the end.
echo ">> Stopping daemon (if running) ..."
ssh "${PI_HOST}" "sudo systemctl stop eink-daemon 2>/dev/null; true"

echo ">> Copying binaries ..."
scp "${BIN_DIR}/eink-daemon" "${PI_HOST}:${PI_DIR}/eink-daemon"
scp "${BIN_DIR}/render-once" "${PI_HOST}:${PI_DIR}/render-once"

# By default never clobber a config already edited on the device; pass
# --overwrite-config to force the repo copy onto the device.
if [[ "${OVERWRITE_CONFIG}" == "1" ]]; then
    echo ">> Overwriting config.toml on device (--overwrite-config) ..."
    scp config.toml "${PI_HOST}:${PI_DIR}/config.toml"
elif ssh "${PI_HOST}" "test -f '${PI_DIR}/config.toml'"; then
    echo ">> config.toml already present on device, leaving it untouched (pass --overwrite-config to replace)"
else
    echo ">> Copying default config.toml ..."
    scp config.toml "${PI_HOST}:${PI_DIR}/config.toml"
fi

echo ">> Removing any old one-shot timer/service ..."
ssh "${PI_HOST}" "sudo systemctl disable --now eink.timer eink.service 2>/dev/null; \
    sudo rm -f /etc/systemd/system/eink.timer /etc/systemd/system/eink.service; true"

echo ">> Installing daemon service ..."
scp deploy/eink-daemon.service "${PI_HOST}:/tmp/eink-daemon.service"
ssh "${PI_HOST}" "sudo mv /tmp/eink-daemon.service /etc/systemd/system/ \
    && sudo systemctl daemon-reload \
    && sudo systemctl enable --now eink-daemon.service"

# Open the firewall for the health endpoint, but only if it is enabled in the
# device's config AND ufw is actually running. Stock Raspberry Pi OS has no
# firewall, so this is a no-op there; the port is read from the device config so
# it always matches what the daemon binds.
echo ">> Configuring firewall for health endpoint (if enabled & ufw active) ..."
ssh "${PI_HOST}" "bash -s '${PI_DIR}'" <<'REMOTE'
set -eu
cfg="$1/config.toml"
grep -qE '^[[:space:]]*enabled[[:space:]]*=[[:space:]]*true' "$cfg" 2>/dev/null || exit 0
port=$(sed -n 's/^[[:space:]]*listen[[:space:]]*=[[:space:]]*"[^:]*:\([0-9]*\)".*/\1/p' "$cfg" | head -n1)
port="${port:-8080}"
if command -v ufw >/dev/null && sudo ufw status | grep -q "Status: active"; then
    sudo ufw allow "${port}/tcp"
    echo "   opened ufw ${port}/tcp"
else
    echo "   no active firewall; port ${port} reachable as-is"
fi
REMOTE

echo ">> Done. The daemon is running. Watch it with:"
echo "   ssh ${PI_HOST} journalctl -u eink-daemon -f"
echo ">> Health check (if enabled in config.toml):"
echo "   curl http://${PI_HOST#*@}:8080/health"
echo ">> After editing config.toml on the device, apply changes with:"
echo "   ssh ${PI_HOST} sudo systemctl restart eink-daemon"
