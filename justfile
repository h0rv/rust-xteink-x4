# Default port (override with: just --set port /dev/ttyUSB0 <command>)
port := "/dev/ttyACM0"
backup_file := "firmware_backup.bin"

# Run all quality checks: format, lint, check
all:
    @echo "Running all quality checks..."
    just fmt
    just lint
    just check
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
    cd crates/xteink-firmware && cargo check

# Build firmware
build-firmware:
    cd crates/xteink-firmware && cargo build --release

# Flash firmware to device (clean rebuild every time)
flash:
    cargo clean -p xteink-firmware -p ssd1677 -p xteink-ui
    cd crates/xteink-firmware && cargo build --release && cargo espflash flash --release --monitor

# Flash and monitor (always rebuilds to ensure latest code)
flash-monitor:
    cd crates/xteink-firmware && rm -f ../../target/riscv32imc-esp-espidf/release/xteink-firmware && cargo build --release && cargo espflash flash --release --monitor

# Just monitor serial output
monitor:
    espflash monitor --port {{port}}

# Backup full flash (16MB, ~25 min) - DO THIS BEFORE FIRST FLASH
backup:
    @echo "Backing up full 16MB flash to {{backup_file}}..."
    @echo "This takes ~25 minutes. Do not disconnect!"
    uvx esptool --chip esp32c3 --port {{port}} read_flash 0x0 0x1000000 {{backup_file}}
    @echo "Backup saved to {{backup_file}}"

# Restore from backup
restore:
    @echo "Restoring from {{backup_file}}..."
    uvx esptool --chip esp32c3 --port {{port}} write_flash 0x0 {{backup_file}}
    @echo "Restore complete"

# Get board info
board-info:
    espflash board-info --port {{port}}

# Erase flash (danger!)
[confirm("This will ERASE ALL FLASH. Are you sure?")]
erase:
    uvx esptool --chip esp32c3 --port {{port}} erase_flash

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
