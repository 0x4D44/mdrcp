# Repository Guidelines

## Project Structure & Module Organization
- `src/` Rust binary crate for the CLI (entry: `src/main.rs`).
- Prefer small, focused modules; as code grows, extract `src/lib.rs` and group by feature (e.g., `src/cli/`, `src/io/`, `src/deploy/`).
- `tests/` for integration tests mirroring `src` paths; unit tests live beside code with `#[cfg(test)]`.
- `assets/` optional fixtures or sample project structures for tests.

## Build, Test, and Development Commands
- `cargo build` / `cargo build --release` compile the binary.
- `cargo run -- [args]` runs the tool in the current directory.
- `cargo test` runs unit and integration tests.
- `cargo fmt --all` formats; `cargo clippy --all-targets -- -D warnings` lints.

What the tool does: run it inside a Rust project directory after building. By default it copies the release (`target/release`) executables to `c:\apps` on Windows or `~/.local/bin` on Linux/macOS (non‑Windows names omit `.exe`); pass `--debug` to instead copy from `target/debug`. Set `MD_TARGET_DIR` to override the default destination (useful for CI/tests).

## Coding Style & Naming Conventions
- Rust 2021; keep lines ≤100 chars and functions short.
- Names: modules/files `snake_case`; types `PascalCase`; CLI flags `kebab-case`.
- Enforce style with rustfmt and clippy; treat clippy warnings as errors.

## Testing Guidelines
- Add fast unit tests per function; keep I/O behind small helpers.
- Use `tempfile` for filesystem tests; avoid touching real user paths.
- Name tests `test_<area>_<behavior>()`. Target ≥80% coverage on changed code.

## Commit & Pull Request Guidelines
- Conventional Commits: `feat|fix|docs|refactor|test|build|ci|chore(scope): summary`.
- Example: `feat(deploy): copy release binaries to c:\apps`.
- PRs: clear description, link issues, include tests/docs, and pass `cargo fmt` + `clippy`.

## Security & Configuration Tips
- Do not commit secrets; prefer config via env or flags.
- Be cautious with paths: the deploy target is `c:\apps`; confirm OS suitability or add a configurable target in changes.
