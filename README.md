# Rust Xteink X4 Sample

## Dev

### Setup
```bash
# Build and start container
devcontainer up --workspace-folder .

# Enter container
devcontainer exec --workspace-folder . bash

# Build project
cargo build --release

# Flash to device
cargo espflash flash --release --monitor /dev/ttyUSB0
```

### Rebuild Container
```bash
devcontainer build --workspace-folder . --no-cache
devcontainer up --workspace-folder .
```

### Clean
```bash
devcontainer exec --workspace-folder . cargo clean
```

## Resources

[github.com/CidVonHighwind/xteink-x4-sample](https://github.com/CidVonHighwind/xteink-x4-sample)
