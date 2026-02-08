#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

cargo fmt --all
cargo clippy -p xteink-ui --features std --target x86_64-unknown-linux-gnu -- -D warnings
cargo test -p xteink-scenario-harness --target x86_64-unknown-linux-gnu
"${ROOT_DIR}/scripts/stack_sizes_report.sh" xteink-scenario-harness x86_64-unknown-linux-gnu
