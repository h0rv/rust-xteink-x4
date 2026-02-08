#!/usr/bin/env bash
set -euo pipefail

crate="${1:-xteink-scenario-harness}"
target="${2:-x86_64-unknown-linux-gnu}"
profile_flag="${3:-}"

# Regex used to focus function-level reports on project code.
stack_filter="${STACK_FILTER:-xteink_ui|xteink_scenario_harness|xteink_firmware}"
# Optional hard gate (bytes). If set, script exits non-zero when a matching function exceeds it.
stack_max_bytes="${STACK_MAX_BYTES:-}"

if ! command -v llvm-readobj >/dev/null 2>&1; then
  echo "llvm-readobj is required" >&2
  exit 1
fi

if ! command -v readelf >/dev/null 2>&1; then
  echo "readelf is required" >&2
  exit 1
fi

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo is required" >&2
  exit 1
fi

echo "[stack] building ${crate} (${target}) with -Z emit-stack-sizes..."
build_cmd=(cargo +nightly rustc -p "${crate}" --lib --target "${target}")
if [[ -n "${profile_flag}" ]]; then
  build_cmd+=("${profile_flag}")
fi
RUSTFLAGS="${RUSTFLAGS:-} -Z emit-stack-sizes" "${build_cmd[@]}"

base_dir="target/${target}"
profile_dir="debug"
if [[ "${profile_flag}" == "--release" ]]; then
  profile_dir="release"
fi

mapfile -t objects < <(find "${base_dir}/${profile_dir}" -name '*.o' 2>/dev/null)
if [[ ${#objects[@]} -eq 0 ]]; then
  echo "[stack] no object files found under ${base_dir}/${profile_dir} for ${crate}" >&2
  exit 2
fi

echo "[stack] object-level .stack_sizes section bytes (coarse signal)"

tmp_obj_report="$(mktemp)"
for obj in "${objects[@]}"; do
  readelf -W -S "${obj}" | awk -v obj="${obj}" '
    /\.stack_sizes/ {
      size_hex = $7
      size = strtonum("0x" size_hex)
      total += size
    }
    END {
      if (total > 0) {
        printf "%10d\t%s\n", total, obj
      }
    }
  ' >> "${tmp_obj_report}"
done

if [[ -s "${tmp_obj_report}" ]]; then
  set +o pipefail
  sort -nr "${tmp_obj_report}" | head -n 20
  set -o pipefail
  total_bytes="$(awk -F'\t' '{sum += $1} END {print sum+0}' "${tmp_obj_report}")"
  echo "[stack] aggregate .stack_sizes bytes across objects: ${total_bytes}"
else
  echo "[stack] no .stack_sizes sections found (toolchain may not emit them here)."
fi

echo "[stack] function-level entries from llvm-readobj --stack-sizes"

tmp_fn_report="$(mktemp)"
for obj in "${objects[@]}"; do
  llvm-readobj --stack-sizes "${obj}" 2>/dev/null | awk -v obj="${obj}" '
    /Functions: \[/ {
      line = $0
      sub(/^.*Functions: \[/, "", line)
      sub(/\].*$/, "", line)
      fn = line
    }
    /Size: 0x/ {
      size_hex = $2
      sub(/^0x/, "", size_hex)
      size = strtonum("0x" size_hex)
      if (size > 0 && fn != "") {
        printf "%10d\t%s\t%s\n", size, fn, obj
      }
      fn = ""
    }
  ' >> "${tmp_fn_report}"
done

if [[ ! -s "${tmp_fn_report}" ]]; then
  echo "[stack] no function-level stack entries found."
  rm -f "${tmp_obj_report}" "${tmp_fn_report}"
  exit 0
fi

set +o pipefail
sort -nr "${tmp_fn_report}" | head -n 30 | awk -F'\t' '{printf "%10s  %s\n", $1, $2}'
set -o pipefail

if [[ -n "${stack_filter}" ]]; then
  echo "[stack] top filtered functions (pattern: ${stack_filter})"
  if grep -E "${stack_filter}" "${tmp_fn_report}" >/dev/null 2>&1; then
    set +o pipefail
    grep -E "${stack_filter}" "${tmp_fn_report}" | sort -nr | head -n 30 | awk -F'\t' '{printf "%10s  %s\n", $1, $2}'
    set -o pipefail

    max_filtered="$(grep -E "${stack_filter}" "${tmp_fn_report}" | awk -F'\t' 'BEGIN{m=0} {if ($1>m) m=$1} END {print m+0}')"
    echo "[stack] max filtered function stack bytes: ${max_filtered}"

    if [[ -n "${stack_max_bytes}" ]]; then
      if [[ "${max_filtered}" -gt "${stack_max_bytes}" ]]; then
        echo "[stack] FAIL: max filtered stack ${max_filtered} > threshold ${stack_max_bytes}" >&2
        rm -f "${tmp_obj_report}" "${tmp_fn_report}"
        exit 3
      fi
      echo "[stack] PASS: max filtered stack ${max_filtered} <= threshold ${stack_max_bytes}"
    fi
  else
    echo "[stack] no functions matched filter pattern."
  fi
fi

rm -f "${tmp_obj_report}" "${tmp_fn_report}"
