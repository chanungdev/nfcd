# Development Environment Setup

## Overview

The Homebrew-installed version and local development builds share the same service label (`io.github.chanungdev.nfcd`), which causes conflicts. When switching between them, always clean up the active one first.

## Switching to Development (brew → dev)

```bash
# 1. Uninstall the Homebrew version (the watcher detects binary removal and auto-cleans the plist)
brew uninstall nfcd

# 2. Build the release binary
cargo build --release

# 3. Run the TUI (automatically generates the plist and registers the LaunchAgent)
cargo run --bin nfcd
```

## Development Workflow

After making code changes:

- **nfcd changes only**: Run `cargo run --bin nfcd` to test immediately.
- **Watcher changes**: Run `cargo build --release` first, then restart the watcher from the TUI or CLI. If the plist is outdated, the TUI automatically restarts the service.

## Switching Back to Homebrew (dev → brew)

```bash
# 1. Start the watcher from the TUI or CLI and confirm it is running

# 2. Remove the development binary (the watcher detects removal and auto-cleans the plist)
cargo clean

# 3. Reinstall the Homebrew version
brew install nfcd
```
