# Dual-Surface Parity Ledger Framework Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the parity ledger a Rust-first, subsystem-based registry where every agentic/CLI capability has a matching GUI surface and every GUI surface has a matching agentic/CLI surface.

**Architecture:** The `mole_ledger` crate owns the canonical tab registry and encodes each tab’s subsystem ownership, status, and dual-surface access rule. `mole_cli` exposes that registry as a machine-readable command/output so automation and future dev-tool code can consume one source of truth. The old Python viewer is historical/reference-only; new renderer and tab work belongs in the Rust dev tool.

**Tech Stack:** Rust 2021, `serde`, `serde_json`, workspace crates `mole_ledger` and `mole_cli`, existing Markdown handoff/spec docs.

**Status:** Implemented in Rust for the registry, CLI writer, CLI parity/snapshot consumers, the shared `mole_devtool` view-model crate, and the checked-in ledger-map artifact. Later work moved the human-facing renderer into the native Rust dev tool; Python should not receive new ledger behavior.

---

### Task 1: Harden the Rust ledger registry as the canonical dual-surface map

**Files:**
- Modify: `crates/mole_ledger/src/lib.rs`
- Test: `crates/mole_ledger/src/lib.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn roadmap_registry_declares_matching_cli_and_gui_surfaces() {
    let registry = LedgerRegistry::roadmap();
    assert!(registry.is_dual_surface());
}
```

- [ ] **Step 2: Run the focused test to verify it fails if the contract is broken**

Run: `cargo test -p mole_ledger roadmap_registry_declares_matching_cli_and_gui_surfaces -- --nocapture`
Expected: PASS only after the registry enforces the dual-surface rule for every tab.

- [ ] **Step 3: Write minimal implementation**

```rust
pub fn is_dual_surface(&self) -> bool {
    self.tabs
        .iter()
        .all(|tab| tab.access.cli == tab.access.gui)
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p mole_ledger roadmap_registry_declares_matching_cli_and_gui_surfaces -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/mole_ledger/src/lib.rs
git commit -m "feat: encode dual-surface parity ledger registry"
```

### Task 2: Expose the registry through the Rust CLI as the agentic surface

**Files:**
- Modify: `crates/mole_cli/src/lib.rs`
- Modify: `crates/mole_cli/src/generated.rs` if the command catalog needs to advertise the new output
- Create: `crates/mole_cli/src/ledger_registry.rs` if a dedicated module keeps the CLI boundary clean
- Modify: `crates/mole_cli/tests/cli_contract.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn generated_ledger_registry_emits_the_roadmap_map() {
    let output = generated::ledger_registry_json();
    assert!(output.get("tabs").is_some());
    assert_eq!(output["tabs"].as_array().unwrap().len(), 10);
}
```

- [ ] **Step 2: Run the test to verify it fails before the command exists**

Run: `cargo test -p mole_cli generated_ledger_registry_emits_the_roadmap_map -- --nocapture`
Expected: FAIL until the CLI exposes the registry output.

- [ ] **Step 3: Write minimal implementation**

```rust
pub fn ledger_registry_json() -> serde_json::Value {
    mole_ledger::registry_json()
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p mole_cli generated_ledger_registry_emits_the_roadmap_map -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/mole_cli/src/lib.rs crates/mole_cli/src/generated.rs crates/mole_cli/src/ledger_registry.rs crates/mole_cli/tests/cli_contract.rs
git commit -m "feat: expose parity ledger registry through mole cli"
```

### Task 3: Update the handoff notes so future agents preserve the duality rule

**Files:**
- Modify: `docs/worklogs/turnrun_frame764_parity_notepad.md`
- Modify: `docs/superpowers/specs/2026-06-09-future-parity-ledger-map-design.md`

- [ ] **Step 1: Write the failing expectation**

```markdown
- Every ledger surface must have both CLI and GUI access declared.
- Python is historical/reference-only; new ledger work lands in Rust.
```

- [ ] **Step 2: Update the docs to include the rule and the current Rust registry path**

```markdown
The parity ledger is now governed by `crates/mole_ledger`, which declares
each tab with matching `cli` and `gui` surface states.
```

- [ ] **Step 3: Verify the updated notes are coherent**

Run: inspect the edited Markdown and confirm the registry path, tab order, and duality rule are consistent.

- [ ] **Step 4: Commit**

```bash
git add docs/worklogs/turnrun_frame764_parity_notepad.md docs/superpowers/specs/2026-06-09-future-parity-ledger-map-design.md
git commit -m "docs: record dual-surface parity ledger rule"
```

## Self-Review

1. **Spec coverage:** The plan covers the current registry, the CLI surface, and the documentation handoff. The Python viewer is intentionally left as historical reference, not a surface to extend.
2. **Placeholder scan:** No TBD or vague implementation steps remain.
3. **Type consistency:** The plan uses the same `LedgerRegistry`, `LedgerAccess`, `LedgerSurfaceState`, and `registry_json()` names defined in `crates/mole_ledger/src/lib.rs`.
