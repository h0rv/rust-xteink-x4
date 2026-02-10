#!/bin/bash
# Robust font uploader for Xteink X4
# Uploads Bookerly fonts via serial CLI with existence checks

set -e

PORT="${1:-/dev/ttyACM0}"
BAUD="${2:-115200}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FONTS_DIR="${SCRIPT_DIR}/../../crates/xteink-ui/assets/fonts/bookerly"
CLI="${SCRIPT_DIR}/xteink_cli.py"

echo "📤 Uploading Bookerly fonts to device..."
echo "   Port: ${PORT}"
echo "   Baud: ${BAUD}"
echo ""

# Function to run CLI command
cli() {
    uv run "${CLI}" --port "${PORT}" --baud "${BAUD}" "$@"
}

# Check if local font files exist
check_local_fonts() {
    local missing=0
    for font in Bookerly-Regular.ttf Bookerly-Bold.ttf Bookerly-Italic.ttf Bookerly-BoldItalic.ttf; do
        if [[ ! -f "${FONTS_DIR}/${font}" ]]; then
            echo "❌ Missing local font: ${FONTS_DIR}/${font}"
            missing=1
        fi
    done
    if [[ $missing -eq 1 ]]; then
        echo "   Fonts directory: ${FONTS_DIR}"
        exit 1
    fi
    echo "✅ All local font files found"
}

# Create directory if it doesn't exist
ensure_dir() {
    local path="$1"
    if cli exists "${path}" 2>/dev/null | grep -q "^0$"; then
        echo "📁 Creating directory: ${path}"
        cli mkdir "${path}" 2>/dev/null || echo "   (directory may already exist)"
    else
        echo "📁 Directory exists: ${path}"
    fi
}

# Upload font if size differs or doesn't exist
upload_font() {
    local local_path="$1"
    local remote_path="$2"
    local font_name=$(basename "${local_path}")
    local local_size=$(stat -c %s "${local_path}" 2>/dev/null || echo "0")
    
    if [[ "${local_size}" -eq 0 ]]; then
        echo "❌ Cannot read local file: ${local_path}"
        return 1
    fi
    
    # Check remote file size
    local remote_info=$(cli stat "${remote_path}" 2>/dev/null || echo "")
    local remote_size=$(echo "${remote_info}" | awk '{print $2}' || echo "0")
    
    if [[ "${remote_size}" == "${local_size}" ]]; then
        echo "✅ ${font_name} already exists (${local_size} bytes)"
        return 0
    fi
    
    if [[ "${remote_size}" -gt 0 ]]; then
        echo "📝 ${font_name} size mismatch (local: ${local_size}, remote: ${remote_size}), re-uploading..."
    else
        echo "📤 Uploading ${font_name} (${local_size} bytes)..."
    fi
    
    cli put "${local_path}" "${remote_path}"
    echo "✅ ${font_name} uploaded"
}

# Main
check_local_fonts

# Create directories
ensure_dir "/fonts"
ensure_dir "/fonts/bookerly"

# Upload fonts
upload_font "${FONTS_DIR}/Bookerly-Regular.ttf" "/fonts/bookerly/Bookerly-Regular.ttf"
upload_font "${FONTS_DIR}/Bookerly-Bold.ttf" "/fonts/bookerly/Bookerly-Bold.ttf"
upload_font "${FONTS_DIR}/Bookerly-Italic.ttf" "/fonts/bookerly/Bookerly-Italic.ttf"
upload_font "${FONTS_DIR}/Bookerly-BoldItalic.ttf" "/fonts/bookerly/Bookerly-BoldItalic.ttf"

echo ""
echo "✅ Font setup complete!"
echo ""
echo "Fonts on device:"
cli ls /fonts/bookerly/
