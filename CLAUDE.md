# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

`mdrcp` is a Rust command-line deployment tool that automates copying release executables from Rust projects to a system-wide applications directory (`c:\apps`). It reads the package name from `Cargo.toml` and copies the corresponding release binary from `target/release/` to the target directory.

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
3. **Binary Location**: Constructs path to release executable (handles Windows `.exe` extension)
4. **Directory Creation**: Creates `c:\apps` if it doesn't exist
5. **File Copy**: Copies executable from `target/release/` to `c:\apps`

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

This tool is designed to be run in the root directory of any Rust project after building with `cargo build --release`. It automatically detects the project name and deploys the executable to a centralized location.