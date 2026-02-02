# Default port (override with: just --set port /dev/ttyUSB0 <command>)

port := "/dev/ttyACM0"
backup_file := "firmware_backup.bin"

# Set ESP-IDF sdkconfig defaults path (MUST be set before any cargo build)
# This ensures our stack size and other settings are actually applied
export ESP_IDF_SDKCONFIG_DEFAULTS := "crates/xteink-firmware/sdkconfig.defaults"

# Run all quality checks: format, lint, check
all:
    @echo "Running all quality checks..."
    just fmt
    just lint
    just check
    just check-firmware
    @echo "✅ All checks passed!"

# Run all checks including firmware (requires ESP toolchain)
all-full:
    @echo "Running all quality checks including firmware..."
    just fmt
    just lint
    just check
    just check-firmware
    @echo "✅ All checks passed (including firmware)!"

# Run web simulator
sim-web:
    cd crates/xteink-sim-web && trunk serve --release

# Run desktop simulator
sim-desktop:
    cargo run -p xteink-sim-desktop

# Build web simulator
build-web:
    cd crates/xteink-sim-web && trunk build --release

# Check all crates (except firmware - needs esp toolchain)
check:
    cargo check --workspace --exclude xteink-firmware

# Check firmware (requires esp toolchain)
check-firmware:
    cd crates/xteink-firmware && \
        ESP_IDF_SDKCONFIG_DEFAULTS="$PWD/sdkconfig.defaults" \
        cargo check

# Build firmware (uses sdkconfig.defaults via environment variable)
build-firmware:
    cd crates/xteink-firmware && \
        ESP_IDF_SDKCONFIG_DEFAULTS="$PWD/sdkconfig.defaults" \
        cargo build --release

# Check if sdkconfig needs regeneration (returns 0 if clean needed)
_needs-clean:
    #!/usr/bin/env bash
    # Check if sdkconfig.defaults is newer than the generated sdkconfig
    SDKCONFIG=$(find crates/xteink-firmware/target -name "sdkconfig" -path "*/esp-idf-sys*/out/esp-idf/sdkconfig" 2>/dev/null | head -1)
    if [ -z "$SDKCONFIG" ]; then
        # No sdkconfig exists yet, need clean build
        exit 0
    fi
    if [ crates/xteink-firmware/sdkconfig.defaults -nt "$SDKCONFIG" ]; then
        echo "⚠️  sdkconfig.defaults changed - clean build required"
        exit 0
    fi
    exit 1

# Smart flash - detects sdkconfig changes and regenerates if necessary (fast)
flash:
    #!/usr/bin/env bash
    SDKCONFIG=$(find crates/xteink-firmware/target -name "sdkconfig" -path "*/esp-idf-sys*/out/esp-idf/sdkconfig" 2>/dev/null | head -1)
    if [ -n "$SDKCONFIG" ] && [ crates/xteink-firmware/sdkconfig.defaults -nt "$SDKCONFIG" ]; then
    	echo "📝 sdkconfig.defaults changed - regenerating sdkconfig (fast)..."
    	just regenerate-sdkconfig
    fi
    echo "⚡ Building and flashing..."
    cd crates/xteink-firmware && \
        ESP_IDF_SDKCONFIG_DEFAULTS="$PWD/sdkconfig.defaults" \
        cargo build --release && \
        cargo espflash flash --release --monitor 2>&1 | tee ../../flash.log

# Flash and monitor (always rebuilds to ensure latest code, logs to flash.log)
flash-monitor:
    cd crates/xteink-firmware && rm -f ../../target/riscv32imc-esp-espidf/release/xteink-firmware && cargo build --release && cargo espflash flash --release --monitor 2>&1 | tee ../../flash.log

# Clean flash (use after sdkconfig.defaults changes, logs to flash.log)
# This does a full clean rebuild - use 'just regenerate-sdkconfig' for faster rebuilds
flash-clean:
    #!/usr/bin/env bash
    cd crates/xteink-firmware && cargo clean
    just regenerate-sdkconfig
    cd crates/xteink-firmware && \
        ESP_IDF_SDKCONFIG_DEFAULTS="$PWD/sdkconfig.defaults" \
        cargo build --release && \
        cargo espflash flash --release --monitor 2>&1 | tee ../../flash.log

# Just monitor serial output (logs to flash.log)
monitor:
    espflash monitor --port {{ port }} 2>&1 | tee flash.log

# Tail the flash log file
tail-log:
    tail -f flash.log

# View last 100 lines of flash log
view-log:
    tail -100 flash.log

# Backup full flash (16MB, ~25 min) - DO THIS BEFORE FIRST FLASH
backup:
    @echo "Backing up full 16MB flash to {{ backup_file }}..."
    @echo "This takes ~25 minutes. Do not disconnect!"
    uvx esptool --chip esp32c3 --port {{ port }} read_flash 0x0 0x1000000 {{ backup_file }}
    @echo "Backup saved to {{ backup_file }}"

# Restore from backup
restore:
    @echo "Restoring from {{ backup_file }}..."
    uvx esptool --chip esp32c3 --port {{ port }} write_flash 0x0 {{ backup_file }}
    @echo "Restore complete"

# Get board info
board-info:
    espflash board-info --port {{ port }}

# Erase flash (danger!)
[confirm("This will ERASE ALL FLASH. Are you sure?")]
erase:
    uvx esptool --chip esp32c3 --port {{ port }} erase_flash

# Clean all
clean:
    cargo clean

# Format code
fmt:
    cargo fmt --all

# Lint
lint:
    cargo clippy --workspace --exclude xteink-firmware -- -D warnings

# Check and lint combined (useful for CI)
check-lint:
    cargo check --workspace --exclude xteink-firmware
    cargo clippy --workspace --exclude xteink-firmware -- -D warnings

# Regenerate only sdkconfig (much faster than full clean)
# CRITICAL: Must use this after changing sdkconfig.defaults
regenerate-sdkconfig:
    #!/usr/bin/env bash
    echo "🧹 Forcing complete sdkconfig regeneration..."
    # Remove sdkconfig from firmware crate (if it exists)
    rm -f crates/xteink-firmware/sdkconfig 2>/dev/null
    rm -f crates/xteink-firmware/sdkconfig.old 2>/dev/null
    
    # Remove all esp-idf-sys build artifacts
    SDKCONFIG_DIR=$(find crates/xteink-firmware/target -type d -name "esp-idf-sys*" -path "*/build/*" 2>/dev/null | head -1)
    if [ -n "$SDKCONFIG_DIR" ]; then
    	rm -rf "$SDKCONFIG_DIR" 2>/dev/null
    fi
    
    # Remove the .bin file to force relinking
    rm -f crates/xteink-firmware/target/riscv32imc-esp-espidf/release/xteink-firmware 2>/dev/null
    rm -f crates/xteink-firmware/target/riscv32imc-esp-espidf/release/xteink-firmware.bin 2>/dev/null
    
    echo "✅ Sdkconfig cache cleared - ready for rebuild with new settings"
    echo "⚠️  IMPORTANT: Build with 'just build-firmware' or 'just flash' now"

# Build firmware with proper sdkconfig environment
# This ensures sdkconfig.defaults is used correctly
build-firmware-clean:
    #!/usr/bin/env bash
    echo "🔧 Building firmware with forced sdkconfig regeneration..."
    just regenerate-sdkconfig
    cd crates/xteink-firmware && \
        ESP_IDF_SDKCONFIG_DEFAULTS="$PWD/sdkconfig.defaults" \
        cargo build --release
