# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

`mdrcp` is a Rust command-line deployment tool that automates copying built executables from Rust projects to a system-wide applications directory (`c:\apps`). It reads the package/workspace metadata from `Cargo.toml` and copies the corresponding binaries from `target/release/` by default or from `target/debug/` when the `--debug` flag is supplied. You can override the default install location (useful for CI/tests) by setting the `MD_TARGET_DIR` environment variable.

## Build Commands

- `cargo build` - Build debug version
- `cargo build --release` - Build release version (required before deployment)
- `cargo run` - Run the deployment tool in current directory
- `cargo test` - Run unit tests
- `cargo clippy` - Run linting
- `cargo fmt` - Format code

## Architecture

### Core Functionality
- **Main Entry Point**: `src/main.rs:7` - Entry point that calls `run()` with current directory
- **Core Logic**: `src/main.rs:17` - `run()` function accepts a directory path for testability
- **TOML Parsing**: Uses `toml` crate to extract package name from `Cargo.toml`
- **Error Handling**: Uses `anyhow` crate for structured error handling with context

### Key Operations
1. **Validation**: Checks for existence of `Cargo.toml` in target directory
2. **Package Discovery**: Parses `Cargo.toml` to extract package name
3. **Binary Location**: Constructs the path to the selected build profile's executable (handles Windows `.exe` extension)
4. **Directory Creation**: Creates `c:\apps` if it doesn't exist
5. **File Copy**: Copies executables from `target/<profile>/` to `c:\apps` (or the path in `MD_TARGET_DIR` when set)

### Testing Strategy
- **Unit Tests**: Located in `src/main.rs:63-118`
- **Test Coverage**: Missing `Cargo.toml`, invalid TOML, missing release binary
- **Test Dependencies**: Uses `tempfile` crate for isolated filesystem testing
- **Helper Functions**: `create_and_write_file()` for test file creation

## Dependencies

### Runtime Dependencies
- `toml = "0.7"` - TOML parsing for Cargo.toml
- `anyhow = "1.0"` - Enhanced error handling

### Development Dependencies  
- `tempfile = "3.8"` - Temporary directory creation for tests

## Usage Pattern

This tool is designed to be run in the root directory of any Rust project after building with `cargo build --release` (or `cargo build` if you plan to pass `--debug`). It automatically detects the project name and deploys the executable to a centralized location.
