# Lessons Learnt

- 2026-06-14: Windows UAC "Installer Detection" forces an elevation prompt (spawn
  fails with os error 740, ERROR_ELEVATION_REQUIRED) for any *unmanifested* exe
  whose filename contains keywords like `update`/`setup`/`install`/`patch`. Fix by
  embedding an `asInvoker` manifest (`embed-manifest` in build.rs) AND avoiding
  those keywords in spawned binary names. Note `embed-manifest` only covers `bin`
  targets (`rustc-link-arg-bins`), NOT integration-test binaries — so a test file
  named `self_update.rs` still breaks `cargo test`; rename it.
