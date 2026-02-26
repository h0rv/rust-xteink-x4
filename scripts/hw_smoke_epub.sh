#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOG_PATH="${LOG_PATH:-${ROOT_DIR}/flash.log}"
RUN_LOG="${RUN_LOG:-${ROOT_DIR}/target/hw_smoke_epub.run.log}"
PORT="${XTEINK_PORT:-${ESPFLASH_PORT:-/dev/ttyACM0}}"
FLASH_TIMEOUT_SECS="${FLASH_TIMEOUT_SECS:-180}"
BOOT_WAIT_SECS="${BOOT_WAIT_SECS:-40}"
OPEN_WAIT_SECS="${OPEN_WAIT_SECS:-20}"

mkdir -p "${ROOT_DIR}/target"
rm -f "${LOG_PATH}" "${RUN_LOG}"

echo "[smoke] flashing + monitoring on ${PORT}"
(
    cd "${ROOT_DIR}"
    RUSTC_WRAPPER= timeout "${FLASH_TIMEOUT_SECS}"s just --set port "${PORT}" flash
) >"${RUN_LOG}" 2>&1 &
FLASH_PID=$!

cleanup() {
    if kill -0 "${FLASH_PID}" >/dev/null 2>&1; then
        kill "${FLASH_PID}" >/dev/null 2>&1 || true
        wait "${FLASH_PID}" 2>/dev/null || true
    fi
}
trap cleanup EXIT

wait_for_log() {
    local pattern="$1"
    local timeout_secs="$2"
    local start
    start="$(date +%s)"
    while true; do
        if [[ -f "${LOG_PATH}" ]] && grep -Fq "${pattern}" "${LOG_PATH}"; then
            return 0
        fi
        if (( "$(date +%s)" - start >= timeout_secs )); then
            return 1
        fi
        sleep 1
    done
}

if ! wait_for_log "[BOOT:22] entering main event loop" "${BOOT_WAIT_SECS}"; then
    echo "[smoke] boot marker not reached; see ${RUN_LOG}"
    exit 1
fi

echo "[smoke] booted; uploading samples"
(cd "${ROOT_DIR}" && XTEINK_PORT="${PORT}" just cli load-samples) >>"${RUN_LOG}" 2>&1

echo "[smoke] driving UI: open + page nav"
(cd "${ROOT_DIR}" && XTEINK_PORT="${PORT}" just cli btn confirm) >>"${RUN_LOG}" 2>&1
sleep 1
(cd "${ROOT_DIR}" && XTEINK_PORT="${PORT}" just cli btn confirm) >>"${RUN_LOG}" 2>&1
sleep 2
(cd "${ROOT_DIR}" && XTEINK_PORT="${PORT}" just cli btn right) >>"${RUN_LOG}" 2>&1
sleep 1
(cd "${ROOT_DIR}" && XTEINK_PORT="${PORT}" just cli btn left) >>"${RUN_LOG}" 2>&1

if ! wait_for_log "[EINKED][EPUB] open start path=" "${OPEN_WAIT_SECS}"; then
    echo "[smoke] EPUB open marker missing in ${LOG_PATH}"
    exit 1
fi

if grep -Eq "memory allocation of .* failed|abort\\(\\) was called|Guru Meditation|panicked at" "${LOG_PATH}"; then
    echo "[smoke] crash/OOM marker found in ${LOG_PATH}"
    exit 1
fi

echo "[smoke] PASS: boot + epub open + page nav markers present"
