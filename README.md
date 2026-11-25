# dwm-forge

A monorepo automating upstream suckless `dwm` tracking, deterministic patch application, C verification, Python E2E testing, GHCR container builds, self-hosted release builds, and Gentoo Portage deployment.

## Project Overview

This repository provides a complete automation solution for managing the suckless dwm window manager. It tracks upstream changes, applies patches deterministically, verifies code quality, runs end-to-end tests, and prepares releases for multiple deployment targets including container images and Gentoo Portage.

## Directory Structure

```
dwm-forge/
├── rust-orchestrator/     # Rust CLI with subcommands
├── patches/               # Ordered unified diffs for dwm patches
├── config/                # User configuration files
├── tests/                 # Test suites
├── gentoo/                # Gentoo Portage ebuild template
├── .github/workflows/     # CI/CD workflows
└── README.md              # This file
```

## Components

### Rust CLI (`rust-orchestrator`)
The core automation tool built in Rust with the following subcommands:
- `fetch`: Fetch upstream dwm sources
- `build`: Build dwm with patches applied
- `package`: Create release packages
- `sync-gentoo`: Sync configuration for Gentoo Portage

### Patches (`patches/`)
Ordered unified diffs that are applied deterministically to upstream dwm:
1. `01-alpha.diff` - Alpha patch
2. `02-cfacts.diff` - Cfacts patch  
3. `03-vanitygaps-cfacts.diff` - Vanity gaps and cfacts combined
4. `04-statuscmd.diff` - Status command patch

### Configuration (`config/`)
- `config.h`: User dwm configuration header
- `dwm-lock.toml`: Pinned upstream commit/tag, patch SHA-256 hashes, and build version

### Gentoo Integration
The `gentoo/` directory provides a Portage overlay that uses savedconfig support. To use it:

1. Place this repository at `/var/db/repos/dwm-forge-local`
2. Create `/etc/portage/repos.conf/dwm-forge-local.conf`:
   ```ini
   [dwm-forge-local]
   location = /var/db/repos/dwm-forge-local
   auto-sync = no
   ```
3. Configure `/etc/portage/package.use/dwm` to enable optional Xinerama support and saved configuration:
   ```text
   x11-wm/dwm xinerama savedconfig
   ```
4. Put the dwm configuration to preserve at `/etc/portage/savedconfig/x11-wm/dwm`
5. Run `dwm-forge sync-gentoo` to download latest release artifacts, verify checksums, and update the ebuild

### Tests (`tests/`)
- **C tests** (`tests/c/`): Compilation and symbol tests for various patches
- **Python tests** (`tests/python/`): pytest suite testing the compiled Rust CLI

### Gentoo Portage (`gentoo/`)
Custom Portage ebuild template using `savedconfig` for Gentoo deployment

## Features

- **Deterministic Patch Application**: Ensures consistent patch application across environments
- **C Verification**: Compile-time checks and symbol verification
- **Python E2E Testing**: End-to-end testing of the Rust CLI functionality
- **GHCR Container Builds**: Automated container image builds and publishing
- **Self-hosted Release Builds**: Local build capabilities for custom deployments
- **Gentoo Portage Integration**: Easy deployment to Gentoo systems

## Usage

### Building dwm
```bash
# Fetch upstream sources and apply patches
cargo run --bin dwm-forge fetch

# Build dwm with all patches applied
cargo run --bin dwm-forge build

# Create release packages
cargo run --bin dwm-forge package

# Sync configuration for Gentoo Portage
cargo run --bin dwm-forge sync-gentoo
```

### Development Setup
```bash
# Build the project
cd rust-orchestrator && cargo build

# Run tests
cd tests/python && python -m pytest
cd tests/c && make test

# Check code formatting (Python)
python -m ruff check .
python -m ruff format --check .

# Check Rust code
cd rust-orchestrator && cargo clippy -- -D warnings
```

### Running Tests
```bash
# Run Python tests
cd tests/python && python -m pytest

# Run C tests  
cd tests/c && make test
```

## CI/CD Workflows

The repository includes GitHub Actions workflows for:
- Pull request validation (`pr.yml`)
- Release builds (`release.yml`) 
- Upstream monitoring (`upstream-watch.yml`)

## Development Guidelines

### Rust
- Edition 2021
- Zero `clippy` warnings (`cargo clippy -- -D warnings`)
- Deterministic error exit codes
- Structured error messages using `anyhow`/`thiserror`

### Python
- Target Python 3.11+
- Must pass `ruff check .` and `ruff format --check .` with zero issues

### GitHub Actions
- Top-level `permissions: {}` is mandatory on every workflow
- Only the container publish job may declare `packages: write`
- Image tags MUST use `${{ github.sha }}` (the 40-char SHA-1 commit hash)

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Ensure all tests pass:
   - Rust: `cargo clippy -- -D warnings`
   - Python: `ruff check .` and `ruff format --check .`
   - C tests: Run in `tests/c/`
5. Submit a pull request

## License

This project is licensed under the MIT License - see the LICENSE file for details.