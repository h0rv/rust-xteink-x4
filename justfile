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

# Flash firmware to device
flash:
    cd crates/xteink-firmware && cargo espflash flash --release --monitor

# Clean all
clean:
    cargo clean

# Format code
fmt:
    cargo fmt --all

# Lint
lint:
    cargo clippy --workspace --exclude xteink-firmware
