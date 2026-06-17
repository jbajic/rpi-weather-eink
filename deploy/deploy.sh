#!/usr/bin/env bash
# Cross-compile for the Raspberry Pi Zero W and deploy as a systemd timer.
#
# Usage:
#   PI_HOST=pi@raspberrypi.local ./deploy/deploy.sh
#
# Requires `cross` on the dev machine (cargo install cross) and Docker/Podman.
set -euo pipefail

PI_HOST="${PI_HOST:-pi@raspberrypi.local}"
PI_DIR="${PI_DIR:-/home/pi/eink}"
TARGET="arm-unknown-linux-gnueabihf"

cd "$(dirname "$0")/.."

echo ">> Building render-once for ${TARGET} ..."
cross build --release --target "${TARGET}" --no-default-features --features device

BIN="target/${TARGET}/release/render-once"
echo ">> Built ${BIN}"

echo ">> Ensuring ${PI_DIR} exists on ${PI_HOST} ..."
ssh "${PI_HOST}" "mkdir -p '${PI_DIR}'"

echo ">> Copying binary ..."
scp "${BIN}" "${PI_HOST}:${PI_DIR}/render-once"

# Never clobber a config already edited on the device.
if ssh "${PI_HOST}" "test -f '${PI_DIR}/config.toml'"; then
    echo ">> config.toml already present on device, leaving it untouched"
else
    echo ">> Copying default config.toml ..."
    scp config.toml "${PI_HOST}:${PI_DIR}/config.toml"
fi

echo ">> Installing systemd units ..."
scp deploy/eink.service deploy/eink.timer "${PI_HOST}:/tmp/"
ssh "${PI_HOST}" "sudo mv /tmp/eink.service /tmp/eink.timer /etc/systemd/system/ \
    && sudo systemctl daemon-reload \
    && sudo systemctl enable --now eink.timer"

echo ">> Done. Run a render right now with:"
echo "   ssh ${PI_HOST} sudo systemctl start eink.service"
echo "   ssh ${PI_HOST} journalctl -u eink.service -n 30"
