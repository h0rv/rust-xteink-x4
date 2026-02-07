# Default port (override with: just --set port /dev/ttyUSB0 <command>)

port := "/dev/ttyACM0"
backup_file := "firmware_backup.bin"

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
    cargo run -p xteink-sim-desktop --target x86_64-unknown-linux-gnu

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

# Flash firmware to device (incremental build)
flash:
    cargo build -p xteink-firmware --release
    cd crates/xteink-firmware && cargo espflash flash --release --monitor 2>&1 | tee ../../flash.log

# Flash and monitor (always rebuilds to ensure latest code)
flash-monitor:
    cargo clean -p xteink-firmware
    cargo build -p xteink-firmware --release
    cd crates/xteink-firmware && cargo espflash flash --release --monitor 2>&1 | tee ../../flash.log

# Clean flash (full rebuild with sdkconfig regeneration)
flash-clean:
    cargo clean -p xteink-firmware
    rm -rf target/riscv32imc-esp-espidf/release/build/esp-idf-sys-*
    cargo build -p xteink-firmware --release
    cd crates/xteink-firmware && cargo espflash flash --release --monitor 2>&1 | tee ../../flash.log

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

# Format code
fmt:
    cargo fmt --all

# Lint
lint:
    cargo clippy --workspace --exclude xteink-firmware -- -D warnings

# Run tests for UI logic (std feature enabled, host target)
test-ui:
    cargo test -p xteink-ui --features std --target x86_64-unknown-linux-gnu

# Run only diff tests (fast, host target)
test-diff:
    cargo test -p xteink-ui --features std --target x86_64-unknown-linux-gnu diff

# Run all tests (host target)
test:
    cargo test --workspace --features std --target x86_64-unknown-linux-gnu

# Show CLI helpers
cli-help:
    @just --list cli
