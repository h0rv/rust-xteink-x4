# Rust Xteink X4 Sample

## Dev

### Setup
```bash
# Build project
cargo build --release

# Flash to device
cargo espflash flash --release --monitor /dev/ttyUSB0

# Linux: add yourself to dialout/uucp group for serial access
sudo usermod -aG uucp $USER  # Arch
sudo usermod -aG dialout $USER  # Debian/Ubuntu
# Then log out and back in
```

## Resources

[github.com/CidVonHighwind/xteink-x4-sample](https://github.com/CidVonHighwind/xteink-x4-sample)
