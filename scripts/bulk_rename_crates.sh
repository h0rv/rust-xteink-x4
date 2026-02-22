#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/bulk_rename_crates.sh --old-us <name_us> --old-kebab <name-kebab> \
    --new-us <name_us> --new-kebab <name-kebab> [--apply] [--root <path>]

Default mode is dry-run (no file changes).

Examples:
  scripts/bulk_rename_crates.sh \
    --old-us old_core --old-kebab old-core \
    --new-us new_core --new-kebab new-core

  scripts/bulk_rename_crates.sh \
    --old-us old_core --old-kebab old-core \
    --new-us new_core --new-kebab new-core \
    --apply --root .
USAGE
}

apply=0
root="."
old_us=""
old_kebab=""
new_us=""
new_kebab=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --apply)
      apply=1
      shift
      ;;
    --root)
      root="${2:-}"
      [[ -n "$root" ]] || { echo "error: --root requires a path" >&2; exit 2; }
      shift 2
      ;;
    --old-us)
      old_us="${2:-}"
      [[ -n "$old_us" ]] || { echo "error: --old-us requires a value" >&2; exit 2; }
      shift 2
      ;;
    --old-kebab)
      old_kebab="${2:-}"
      [[ -n "$old_kebab" ]] || { echo "error: --old-kebab requires a value" >&2; exit 2; }
      shift 2
      ;;
    --new-us)
      new_us="${2:-}"
      [[ -n "$new_us" ]] || { echo "error: --new-us requires a value" >&2; exit 2; }
      shift 2
      ;;
    --new-kebab)
      new_kebab="${2:-}"
      [[ -n "$new_kebab" ]] || { echo "error: --new-kebab requires a value" >&2; exit 2; }
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "error: unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

[[ -n "$old_us" && -n "$old_kebab" && -n "$new_us" && -n "$new_kebab" ]] || {
  echo "error: required args missing" >&2
  usage >&2
  exit 2
}

command -v rg >/dev/null 2>&1 || { echo "error: ripgrep (rg) is required" >&2; exit 2; }
command -v perl >/dev/null 2>&1 || { echo "error: perl is required" >&2; exit 2; }

pattern="(${old_us}|${old_kebab}|${old_us}_render|${old_kebab}-render|${old_us}_embedded_graphics|${old_kebab}-embedded-graphics|${old_us}_render_web|${old_kebab}-render-web)"

mapfile -t files < <(
  rg -l \
    --hidden \
    --glob '!.git/**' \
    --glob '!**/.git' \
    --glob '!**/.git/**' \
    --glob '!target/**' \
    --glob '!**/*.epub' \
    --glob '!**/*.png' \
    --glob '!**/*.jpg' \
    --glob '!**/*.jpeg' \
    --glob '!**/*.gif' \
    --glob '!**/*.webp' \
    --glob '!**/*.bmp' \
    --glob '!**/*.ico' \
    --glob '!**/*.pdf' \
    --glob '!**/*.zip' \
    --glob '!**/*.bin' \
    --glob '!**/*.wasm' \
    --glob '!**/*.ttf' \
    --glob '!**/*.otf' \
    --glob '!**/*.woff' \
    --glob '!**/*.woff2' \
    --glob '!**/*.mp3' \
    --glob '!**/*.mp4' \
    --glob '!**/*.mov' \
    --glob '!**/*.avi' \
    --glob '!**/*.sqlite*' \
    --glob '!**/*.db' \
    --glob '!**/*.lock' \
    --glob '!scripts/bulk_rename_crates.sh' \
    "$pattern" \
    "$root"
)

if [[ ${#files[@]} -eq 0 ]]; then
  echo "No matching files found under: $root"
  exit 0
fi

echo "Matched ${#files[@]} file(s)."
printf '%s\n' "${files[@]}" || true

if [[ $apply -eq 0 ]]; then
  echo
  echo "Dry run only. No changes made."
  echo "Re-run with --apply to update files in place."
  exit 0
fi

old_us_esc=$(printf '%s' "$old_us" | sed 's/[^^]/[&]/g; s/\^/\\^/g')
new_us_esc="$new_us"
old_kebab_esc=$(printf '%s' "$old_kebab" | sed 's/[^^]/[&]/g; s/\^/\\^/g')
new_kebab_esc="$new_kebab"

for f in "${files[@]}"; do
  perl -0pi -e "
    s/\\b${old_us_esc}_embedded_graphics\\b/${new_us_esc}_embedded_graphics/g;
    s/(?<![A-Za-z0-9_])${old_kebab_esc}-embedded-graphics(?![A-Za-z0-9_])/${new_kebab_esc}-embedded-graphics/g;
    s/\\b${old_us_esc}_render_web\\b/${new_us_esc}_render_web/g;
    s/(?<![A-Za-z0-9_])${old_kebab_esc}-render-web(?![A-Za-z0-9_])/${new_kebab_esc}-render-web/g;
    s/\\b${old_us_esc}_render\\b/${new_us_esc}_render/g;
    s/(?<![A-Za-z0-9_])${old_kebab_esc}-render(?![A-Za-z0-9_])/${new_kebab_esc}-render/g;
    s/\\b${old_us_esc}\\b/${new_us_esc}/g;
    s/(?<![A-Za-z0-9_])${old_kebab_esc}(?![A-Za-z0-9_])/${new_kebab_esc}/g;
  " "$f"
done

echo "Applied replacements to ${#files[@]} file(s)."
