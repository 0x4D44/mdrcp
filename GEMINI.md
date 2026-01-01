# mdrcp - Gemini Context

## Project Overview

`mdrcp` is a Rust CLI tool designed to streamline the deployment of Rust binaries. It automates copying built executables from a project's `target/release` (or `target/debug`) directory to a system-wide binary location (defaulting to `C:\apps` on Windows and `$HOME/.local/bin` on Linux/macOS).

## Key Features

*   **Workspace Aware:** Detects built executables for the selected profile.
*   **Cross-Platform:** Supports Windows, Linux, and macOS.
*   **Configurable:**
    *   `--target`: Override destination directory.
    *   `--debug`: Deploy debug artifacts.
    *   `--summary`: Output deployment results in `text`, `json`, or `json-pretty` formats.
    *   `MD_TARGET_DIR`: Environment variable for global destination override.

## Architecture

*   **Language:** Rust (2021 edition)
*   **Entry Point:** `src/main.rs` - Handles CLI argument parsing and process exit codes.
*   **Core Logic:** `src/lib.rs` - Contains the `run` function and main business logic.
*   **CLI Parsing:** Likely handled in `src/cli/mod.rs` or `src/lib.rs` (uses custom parsing or `clap` - *Note: `Cargo.toml` does not list `clap`, suggests custom or lightweight parsing*).
*   **Dependencies:**
    *   `anyhow`: Error handling.
    *   `toml`: Parsing `Cargo.toml`.
    *   `owo-colors`: Terminal output coloring.
    *   `serde`/`serde_json`: structured output handling.

## Build and Run

### Standard Commands

*   **Build (Release):** `cargo build --release`
*   **Build (Debug):** `cargo build`
*   **Run (Dev):** `cargo run -- [flags]`
*   **Test:** `cargo test`
*   **Format:** `cargo fmt`
*   **Lint:** `cargo clippy --all-targets -- -D warnings`

### Installation (Local)

To install `mdrcp` itself using `mdrcp` (meta!):
```bash
cargo build --release
./target/release/mdrcp
```

## Development Conventions

*   **Code Style:** Standard Rust formatting (`rustfmt`).
*   **Testing:**
    *   Unit tests in `src/`.
    *   Integration tests in `tests/`.
    *   Tests often use `tempfile` to avoid filesystem side effects.
*   **Documentation:**
    *   `README.md` for user-facing docs.
    *   `wrk_docs/` for internal design and decision records.
    *   `AGENTS.md` and `CLAUDE.md` for AI context.

## File Structure Highlights

*   `src/main.rs`: CLI entry point.
*   `src/lib.rs`: Library interface.
*   `src/cli/mod.rs`: CLI argument parsing logic.
*   `tests/integration.rs`: End-to-end integration tests.
*   `AGENTS.md`: Guidelines for AI agents.
