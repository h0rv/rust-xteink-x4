#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

HOST_TEST_TARGET="${HOST_TEST_TARGET:-$(rustc -vV | awk '/^host: / { print $2 }')}"

cargo fmt --all
cargo clippy -p xteink-ui --features std --target "${HOST_TEST_TARGET}" -- -D warnings
cargo test -p xteink-scenario-harness --target "${HOST_TEST_TARGET}"
"${ROOT_DIR}/scripts/stack_sizes_report.sh" xteink-scenario-harness "${HOST_TEST_TARGET}"
