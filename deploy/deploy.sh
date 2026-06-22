#!/usr/bin/env bash
# Cross-compile for the Raspberry Pi Zero W and deploy as a long-running daemon.
#
# Usage:
#   PI_HOST=pi@raspberrypi.local ./deploy/deploy.sh [--overwrite-config]
#
# By default a config.toml already on the device is left untouched. Pass
# --overwrite-config (or OVERWRITE_CONFIG=1) to replace it with the repo copy.
#
# Requires `cross` on the dev machine (cargo install cross) and Docker/Podman.
set -euo pipefail

PI_HOST="${PI_HOST:-pi@raspberrypi.local}"
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

echo ">> Done. The daemon is running. Watch it with:"
echo "   ssh ${PI_HOST} journalctl -u eink-daemon -f"
echo ">> After editing config.toml on the device, apply changes with:"
echo "   ssh ${PI_HOST} sudo systemctl restart eink-daemon"
