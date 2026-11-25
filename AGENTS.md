# AGENTS.md — dwm-forge Rules & Architecture

## Project Goal
Monorepo automating upstream suckless `dwm` tracking, deterministic patch application (`patches/01-*.diff` .. `04-*.diff`), C verification, Python E2E testing, GHCR container builds, self-hosted release builds, and Gentoo Portage (`savedconfig`) deployment.

## Directory Map
- `rust-orchestrator/`: Rust CLI (`dwm-forge`) with subcommands: `fetch`, `build`, `package`, `sync-gentoo`.
- `patches/`: Ordered unified diffs (`01-alpha.diff`, `02-cfacts.diff`, `03-vanitygaps-cfacts.diff`, `04-statuscmd.diff`). Never reorder.
- `config/config.h`: User dwm configuration header.
- `config/dwm-lock.toml`: Pinned upstream commit/tag, patch SHA-256 hashes, and build version.
- `tests/c/`: C compilation & symbol tests for alpha, cfacts, vanitygaps, and statuscmd.
- `tests/python/`: `pytest` + `pytest-cov` suite testing the compiled Rust CLI.
- `gentoo/`: Custom Portage ebuild template using `savedconfig`.
- `.github/workflows/`: CI/CD workflows (`pr.yml`, `release.yml`, `upstream-watch.yml`).

## Strict Coding & CI Rules
1. **Rust**: Edition 2021, zero `clippy` warnings (`cargo clippy -- -D warnings`), deterministic error exit codes, structured error messages (`anyhow`/`thiserror`).
2. **Python**: Target Python 3.11+. Must pass `ruff check .` and `ruff format --check .` with zero issues.
3. **GitHub Actions**: Top-level `permissions: {}` is mandatory on every workflow. Only the container publish job may declare `packages: write`. Image tags MUST use `${{ github.sha }}` (the 40-char SHA-1 commit hash).
4. **Execution Discipline**: Modify only files requested in the current prompt. Always run the prompt's verification command before finishing.
