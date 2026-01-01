# mdrcp Deployment Tool

`mdrcp` copies built binaries from a Rust project into a user-friendly install location. It reads
from `target/release` by default (matching `cargo build --release`) and can copy from
`target/debug` via `--debug`.

- Windows: `c:\apps`
- Linux/macOS: `$HOME/.local/bin`

Run it after `cargo build --release` inside a project directory (or `cargo build` if you plan to
pass `--debug`).

## Quick Start

```bash
cargo build --release
mdrcp
```

The tool detects built executables for the selected profile (workspace aware), copies them to the target directory, and prints colorized status.

## Flags

| Flag | Description |
|------|-------------|
| `--help`, `-h` | Show usage information. |
| `--version`, `-V` | Print version banner (name, version, build timestamp). |
| `--quiet`, `-q` | Suppress banner/progress output (warnings still appear on stderr). |
| `--target <path>`, `-t <path>` | Override the deployment directory (relative paths resolve from the project root). |
| `--summary <format>` | Emit deployment summary in `text`, `json`, or `json-pretty`. Defaults to `text`. |
| `--release` | Force copying from `target/release` (this is already the default). |
| `--debug` | Copy from `target/debug` artifacts (use after `cargo build`). |
| `--tauri` | Force Tauri project mode (looks for `src-tauri/Cargo.toml`). |
| `--no-tauri` | Disable Tauri auto-detection (force standard mode). |

### Environment Overrides

| Variable | Description |
|----------|-------------|
| `MD_TARGET_DIR` | Absolute path that overrides the default install directory on every platform (handy for CI or tests). When unset, Windows defaults to `c:\apps` and Linux/macOS to `$HOME/.local/bin`. |

## Tauri Support

`mdrcp` automatically detects Tauri projects by checking for `src-tauri/Cargo.toml` and `tauri.conf.json` (or `.json5`) in the project root.

- **Auto-Detection:** When detected, it deploys binaries from `src-tauri/target/...` instead of the root.
- **Product Name:** It reads the `productName` from `tauri.conf.json` and adds it to the list of binaries to deploy (useful if it differs from the Cargo package name).
- **Manual Control:** Use `--tauri` to force this mode or `--no-tauri` to disable it.

### Summary Formats

When `--summary json` or `--summary json-pretty` is used, `mdrcp` writes a single JSON object to stdout. Warnings (for example redundant `--target` overrides) continue to stream to stderr. See `wrk_docs/2025.10.31 - DOC - Deployment Summary Formats.md` for the full schema.

Examples:

```bash
# Compact JSON for CI bots (no color chatter)
mdrcp --quiet --summary json --target dist/bin

# Pretty JSON for easy inspection during manual runs
mdrcp --quiet --summary json-pretty --target dist/bin
```

## Exit Codes

- `0`: Success.
- `1`: Errors (missing `Cargo.toml`, unreadable release artifacts, etc.).

## Development

- `cargo fmt --all`
- `cargo test`
- `cargo clippy --all-targets -- -D warnings`

See `AGENTS.md` for full repository guidelines.

## Troubleshooting

| Symptom | Likely Cause | Fix |
|---------|--------------|-----|
| `No Cargo.toml found` | Tool not run from a Rust project root. | Change into the project directory first. |
| `No built <profile> executables found` | Build artifacts missing for the selected profile. | Run `cargo build --release` for release or `cargo build` for debug, and ensure artifacts exist in the matching `target/<profile>/` directory. |
| JSON summary missing warnings data | `--summary` defaults to `text`; no JSON emitted. | Pass `--summary json` (or `json-pretty`) to request structured output. |
| `Override note warns about redundant target` | `--target` resolves to the default directory. | Drop the override or point to a different directory. |
| `No Cargo.toml found` (Tauri) | Tool run in root but `src-tauri` missing/invalid. | Ensure `src-tauri/Cargo.toml` exists or run in `src-tauri` directly. |
