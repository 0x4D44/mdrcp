# mdrcp Deployment Tool

`mdrcp` copies release binaries from a Rust project into a user-friendly install location:

- Windows: `c:\apps`
- Linux/macOS: `$HOME/.local/bin`

Run it after `cargo build --release` inside a project directory.

## Quick Start

```bash
cargo build --release
mdrcp
```

The tool detects release executables (workspace aware), copies them to the target directory, and prints colorized status.

## Flags

| Flag | Description |
|------|-------------|
| `--help`, `-h` | Show usage information. |
| `--version`, `-V` | Print version banner (name, version, build timestamp). |
| `--quiet`, `-q` | Suppress banner/progress output (warnings still appear on stderr). |
| `--target <path>`, `-t <path>` | Override the deployment directory (relative paths resolve from the project root). |
| `--summary <format>` | Emit deployment summary in `text`, `json`, or `json-pretty`. Defaults to `text`. |

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
| `No built release executables found` | Release binaries missing. | Run `cargo build --release` and ensure artifacts exist in `target/release/`. |
| JSON summary missing warnings data | `--summary` defaults to `text`; no JSON emitted. | Pass `--summary json` (or `json-pretty`) to request structured output. |
| Override note warns about redundant target | `--target` resolves to the default directory. | Drop the override or point to a different directory. |
