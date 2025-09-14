# Repository Guidelines

## Project Structure & Module Organization
- Expected layout:
  - `src/` core libraries and shared utilities.
  - `apps/` runnable services/CLIs; thin wrappers around `src`.
  - `tests/` unit and integration tests mirroring `src` paths.
  - `assets/` fixtures, sample data, and static files.
  - `scripts/` local dev tasks (`*.ps1` Windows, `*.sh` Unix).
- Keep modules small; group by feature (e.g., `src/parser/`, `src/io/`, `src/cli/`).

## Build, Test, and Development Commands
- Prefer script wrappers first:
  - `./scripts/setup.(ps1|sh)` install toolchains and deps.
  - `./scripts/dev.(ps1|sh)` run the main app with auto‑reload.
  - `./scripts/test.(ps1|sh)` run the test suite with coverage.
  - `./scripts/lint.(ps1|sh)` format + static checks.
- If wrappers are absent, use stack defaults:
  - Node: `npm ci`, `npm test`, `npm run build`, `npm run dev`
  - Python: `python -m venv .venv; . .venv/Scripts/Activate.ps1; pip install -e .[dev]; pytest -q`
  - Rust: `cargo build --release; cargo test`

## Coding Style & Naming Conventions
- Indentation: 2 spaces (JS/TS), 4 spaces (Python), default (Rust/Go).
- Max line length 100; wrap thoughtfully.
- Names: `snake_case` files and functions; `PascalCase` types/classes; `kebab-case` CLI names.
- Use formatters: Prettier (JS/TS), Black + Ruff (Python), rustfmt + clippy (Rust).

## Testing Guidelines
- Place tests in `tests/` with names like `test_<module>_<behavior>.py|ts|rs`.
- Aim for ≥80% coverage on changed code.
- Prefer fast, isolated unit tests; use fixtures for I/O; mark slow/integration.

## Commit & Pull Request Guidelines
- Conventional Commits: `feat|fix|docs|refactor|test|build|ci|chore(scope): summary`.
  - Example: `feat(parser): support fenced code blocks`.
- PRs: concise description, link issues, include tests and docs updates, add screenshots for UI, pass CI.

## Security & Configuration Tips
- Do not commit secrets; use `.env.example`.
- Validate inputs; avoid panics/exceptions leaking stack traces in CLIs.
- Keep dependencies minimal; run `scripts/audit.(ps1|sh)` or stack equivalents regularly.

