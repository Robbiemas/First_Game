# Agent Workspace Notes

- The Git repository root is `D:\Mole Game\First_Game`.
- In the Codex desktop app, some file-edit tools may be rooted at `D:\Mole Game`, one level above the repository.
- When using `apply_patch`, target repository files with the `First_Game/` prefix, for example `First_Game/crates/mole_core/src/state.rs`.
- Verify repository state with `git -C "D:\Mole Game\First_Game" status -sb` before staging or committing.
- For any workflow involving decomp extraction, Mole CLI, the Rust dev tool GUI, or Rust engine import, read `docs/architecture/rust-devtool-lossless-middleware.md` first.
- For runtime data, generated artifacts, rollback snapshots, hot render paths, or extraction-to-engine baking, read `docs/architecture/runtime-baked-data-performance-contract.md` before editing.
- Before adding or copying Rust dev tool layout/UI behavior, read `docs/architecture/rust-devtool-ui-primitives.md` and reuse the existing primitives in `crates/mole_devtool/src/layout.rs`, `template.rs`, and `theme.rs`.
- Treat Python and Pygame files as historical/reference surfaces only. New durable dev-tool behavior belongs in Rust.
- The CLI and GUI are dual surfaces: anything added to one side must be represented on the other side through the same Rust-owned artifact or view model.
