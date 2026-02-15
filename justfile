# Auto-detect serial port (override with: just --set port /dev/ttyUSB0 <command>)
# Detection order:
# 1) ESPFLASH_PORT env var, if set
# 2) First port that responds to `espflash board-info`
# 3) Linux fallback `/dev/ttyACM0`
port := `if [ -n "${ESPFLASH_PORT:-}" ]; then \
    echo "$ESPFLASH_PORT"; \
elif command -v espflash >/dev/null 2>&1; then \
    for p in /dev/cu.usbmodem* /dev/cu.usbserial* /dev/ttyUSB* /dev/ttyACM*; do \
        [ -e "$p" ] || continue; \
        if espflash board-info --non-interactive --port "$p" >/dev/null 2>&1; then \
            echo "$p"; \
            exit 0; \
        fi; \
    done; \
    echo "/dev/ttyACM0"; \
else \
    echo "/dev/ttyACM0"; \
fi`

# Host target used for desktop tests/simulators.
# Override with HOST_TEST_TARGET when cross-testing.
host_target := `if [ -n "${HOST_TEST_TARGET:-}" ]; then \
    echo "$HOST_TEST_TARGET"; \
else \
    rustc -vV | awk '/^host: /{print $2}'; \
fi`
backup_file := "firmware_backup.bin"
partition_table := "crates/xteink-firmware/partitions.csv"

mod cli "cli/justfile"

# Set ESP-IDF sdkconfig defaults path (MUST be set before any cargo build)

export ESP_IDF_SDKCONFIG_DEFAULTS := "crates/xteink-firmware/sdkconfig.defaults"

# Bootstrap development environment
setup:
    @echo "🔧 Setting up ox4 development environment..."
    @echo ""
    @echo "📦 Installing Rust tools..."
    cargo install espflash espup trunk
    @echo ""
    @echo "🔨 Installing ESP-IDF toolchain..."
    espup install
    @echo ""
    @echo "📚 Initializing git submodules..."
    @if [ -d .git ] && [ -f .gitmodules ]; then \
        git submodule update --init --recursive; \
        echo "✅ Submodules initialized"; \
    else \
        echo "ℹ️  No submodules configured, skipping"; \
    fi
    @echo ""
    @echo "🎨 Setting up Git hooks (optional)..."
    @if [ -d .git ]; then \
        echo "#!/bin/sh" > .git/hooks/pre-commit; \
        echo "just fmt" >> .git/hooks/pre-commit; \
        chmod +x .git/hooks/pre-commit; \
        echo "✅ Pre-commit hook installed (runs 'just fmt')"; \
    else \
        echo "⚠️  Not a git repository, skipping hooks"; \
    fi
    @echo ""
    @echo "🧪 Running initial checks..."
    just check
    @echo ""
    @echo "✅ Setup complete!"
    @echo ""
    @echo "Next steps:"
    @echo "  - Run 'just sim-desktop' to test the desktop simulator"
    @echo "  - Run 'just sim-web' to test the web simulator"
    @echo "  - Run 'just flash' to build and flash firmware to device"
    @echo ""
    @echo "💡 Run 'just --list' to see all available commands"

# Check system dependencies
check-deps:
    @echo "Checking system dependencies..."
    @command -v rustc >/dev/null 2>&1 || (echo "❌ Rust not found. Install from https://rustup.rs" && exit 1)
    @command -v cargo >/dev/null 2>&1 || (echo "❌ Cargo not found" && exit 1)
    @command -v espflash >/dev/null 2>&1 || echo "⚠️  espflash not found (run: cargo install espflash)"
    @command -v espup >/dev/null 2>&1 || echo "⚠️  espup not found (run: cargo install espup)"
    @command -v trunk >/dev/null 2>&1 || echo "⚠️  trunk not found (run: cargo install trunk)"
    @echo "✅ System dependencies check complete"


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
    cargo run -p xteink-sim-desktop --target {{ host_target }}

# Build web simulator
build-web:
    cd crates/xteink-sim-web && trunk build --release

# Check all crates (except firmware - needs esp toolchain)
check:
    cargo check --workspace --exclude xteink-firmware

# Check firmware (requires esp toolchain)
check-firmware:
    cargo check -p xteink-firmware

# Build firmware
build-firmware:
    cargo build -p xteink-firmware --release

# Build firmware and enforce app-partition size gate.
test-firmware-size:
    cargo build -p xteink-firmware --release
    just size-check

# Flash firmware to device (incremental build)
flash:
    cargo build -p xteink-firmware --release
    just size-check
    cd crates/xteink-firmware && cargo espflash flash --release --monitor --non-interactive --port {{ port }} --partition-table partitions.csv --target-app-partition factory 2>&1 | tee ../../flash.log

# Flash and monitor (always rebuilds to ensure latest code)
flash-monitor:
    cargo clean -p xteink-firmware
    cargo build -p xteink-firmware --release
    just size-check
    cd crates/xteink-firmware && cargo espflash flash --release --monitor --non-interactive --port {{ port }} --partition-table partitions.csv --target-app-partition factory 2>&1 | tee ../../flash.log

# Clean flash (full rebuild with sdkconfig regeneration)
flash-clean:
    cargo clean -p xteink-firmware
    rm -rf target/riscv32imc-esp-espidf/release/build/esp-idf-sys-*
    cargo build -p xteink-firmware --release
    just size-check
    cd crates/xteink-firmware && cargo espflash flash --release --monitor --non-interactive --port {{ port }} --partition-table partitions.csv --target-app-partition factory 2>&1 | tee ../../flash.log

# Just monitor serial output
monitor:
    espflash monitor --port {{ port }} 2>&1 | tee flash.log

# Tail the flash log
tail-log:
    tail -f flash.log

# View last 100 lines of flash log
view-log:
    tail -100 flash.log

# Backup full flash (16MB, ~25 min)
backup:
    @echo "Backing up full 16MB flash to {{ backup_file }}..."
    uvx esptool --chip esp32c3 --port {{ port }} read_flash 0x0 0x1000000 {{ backup_file }}

# Restore from backup
restore:
    @echo "Restoring from {{ backup_file }}..."
    uvx esptool --chip esp32c3 --port {{ port }} write_flash 0x0 {{ backup_file }}

# Get board info
board-info:
    espflash board-info --port {{ port }}

# Erase flash (danger!)
[confirm("This will ERASE ALL FLASH. Are you sure?")]
erase:
    uvx esptool --chip esp32c3 --port {{ port }} erase_flash

# Clean all build artifacts
clean:
    cargo clean

# Clean firmware only
clean-firmware:
    cargo clean -p xteink-firmware

# Check firmware binary size against partition limits
size-check:
    python3 scripts/check_binary_size.py {{ partition_table }} target/riscv32imc-esp-espidf/release/xteink-firmware factory

# Format code
fmt:
    cargo fmt --all

# Lint
lint:
    cargo clippy --workspace --exclude xteink-firmware -- -D warnings

# Run tests for UI logic (std feature enabled, host target)
test-ui:
    cargo test -p xteink-ui --features std --target {{ host_target }}

# Run only diff tests (fast, host target)
test-diff:
    cargo test -p xteink-ui --features std --target {{ host_target }} diff

# Run all tests (host target)
test:
    cargo test --workspace --features std --target {{ host_target }}

# Run scripted scenario harness tests (host target)
sim-scenarios:
    cargo test -p xteink-scenario-harness --target {{ host_target }}

# Run scripted scenario harness tests on a deterministic local host target.
# Use this for day-to-day local iteration regardless of auto-detected host target.
sim-scenarios-local:
    cargo test -p xteink-scenario-harness --target x86_64-unknown-linux-gnu -- --nocapture

# Build stack-size report for scenario harness host builds
stack-report:
    ./scripts/stack_sizes_report.sh xteink-scenario-harness {{ host_target }}

# Enforce a max per-function stack threshold for project symbols (host scenario build)
stack-gate max_bytes="100000":
    STACK_MAX_BYTES={{ max_bytes }} ./scripts/stack_sizes_report.sh xteink-scenario-harness {{ host_target }}

# Tight host-side UI reliability loop: fmt + lint + scenarios + stack report
ui-loop:
    HOST_TEST_TARGET={{ host_target }} ./scripts/ui_loop.sh

# Show CLI helpers
cli-help:
    @just --list cli

# Upload Bookerly fonts to SD card via serial CLI (device must be running and SD mounted)
# Usage: just cli-load-fonts
cli-load-fonts:
    @just cli load-fonts
