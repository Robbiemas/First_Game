# Rust Native Parity Ledger Devtool Shell Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the Python state-graph viewer as the human-facing parity ledger surface with a native Rust devtool window that renders the owned `ParityLedgerViewModel` and keeps the CLI/GUI ledger contract in lockstep.

**Architecture:** `mole_ledger` remains the source of truth for the parity ledger registry and artifact shape, `mole_devtool` becomes the GUI-ready Rust consumer and native app shell, and `mole_cli` continues to expose the extraction and inspection surfaces that feed the same artifact. The first GUI slice is read-only and subsystem-based: a registry summary, a tab strip, and a tab body driven entirely by the shared Rust view model. Python is historical/reference-only and must not gain new responsibilities.

**Tech Stack:** Rust 2021, `mole_ledger`, `mole_devtool`, `eframe`/`egui`, `serde_json`, existing workspace CLI utilities.

---

### Task 1: Add a native app state layer in `mole_devtool`

**Files:**
- Create: `D:/Mole Game/First_Game/crates/mole_devtool/src/app.rs`
- Modify: `D:/Mole Game/First_Game/crates/mole_devtool/src/lib.rs`

- [ ] **Step 1: Define the failing contract in a unit test**

```rust
#[test]
fn app_state_exposes_view_model_tabs_and_defaults_to_first_tab() {
    let ledger_map = LedgerMap::from_registry(&LedgerRegistry::roadmap());
    let view_model = ParityLedgerViewModel::from_ledger_map(&ledger_map);
    let app = ParityLedgerApp::from_view_model(view_model);

    assert_eq!(app.title(), "Parity Ledger");
    assert_eq!(app.tab_count(), 10);
    assert_eq!(app.selected_tab_id(), Some("global_values"));
    assert_eq!(app.selected_tab_label(), Some("Global Values"));
}
```

- [ ] **Step 2: Add the minimal Rust implementation to satisfy the contract**

```rust
pub struct ParityLedgerApp {
    view_model: ParityLedgerViewModel,
    selected_tab: usize,
}

impl ParityLedgerApp {
    pub fn from_view_model(view_model: ParityLedgerViewModel) -> Self {
        Self { view_model, selected_tab: 0 }
    }

    pub fn title(&self) -> &'static str {
        "Parity Ledger"
    }

    pub fn tab_count(&self) -> usize {
        self.view_model.tabs.len()
    }

    pub fn selected_tab_id(&self) -> Option<&str> {
        self.view_model.tabs.get(self.selected_tab).map(|tab| tab.id.as_str())
    }

    pub fn selected_tab_label(&self) -> Option<&str> {
        self.view_model.tabs.get(self.selected_tab).map(|tab| tab.label.as_str())
    }
}
```

- [ ] **Step 3: Keep the app-state surface private to the crate except for the public constructor and getters needed by the GUI binary**

- [ ] **Step 4: Update the crate root to expose the new module without moving the existing `ParityLedgerViewModel` API**

### Task 2: Add the Rust-native GUI binary and egui dependency wiring

**Files:**
- Modify: `D:/Mole Game/First_Game/crates/mole_devtool/Cargo.toml`
- Create: `D:/Mole Game/First_Game/crates/mole_devtool/src/main.rs`

- [ ] **Step 1: Add the failing binary-launch contract in a test**

```rust
#[test]
fn native_binary_can_construct_the_parity_ledger_app() {
    let ledger_map = LedgerMap::from_registry(&LedgerRegistry::roadmap());
    let view_model = ParityLedgerViewModel::from_ledger_map(&ledger_map);
    let app = ParityLedgerApp::from_view_model(view_model);

    assert_eq!(app.title(), "Parity Ledger");
}
```

- [ ] **Step 2: Add the GUI dependency block and a minimal binary entrypoint**

```toml
[dependencies]
eframe = { version = "0.33", default-features = true }
mole_ledger = { path = "../mole_ledger" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

```rust
fn main() -> Result<(), eframe::Error> {
    let ledger_map = mole_ledger::LedgerMap::load("docs/state_graphs/parity_ledger_map.json")
        .map_err(|error| eframe::Error::AppCreation(Box::new(std::io::Error::new(std::io::ErrorKind::Other, error))))?;
    let app = mole_devtool::ParityLedgerApp::from_view_model(mole_devtool::ParityLedgerViewModel::from_ledger_map(&ledger_map));

    let options = eframe::NativeOptions::default();
    eframe::run_native(app.title(), options, Box::new(|_cc| Ok(Box::new(app))))
}
```

- [ ] **Step 3: Keep the binary read-only and artifact-driven so it uses the same ledger map the CLI already loads**

### Task 3: Render the subsystem tab strip and selected tab body in egui

**Files:**
- Create: `D:/Mole Game/First_Game/crates/mole_devtool/src/ui.rs`
- Modify: `D:/Mole Game/First_Game/crates/mole_devtool/src/app.rs`

- [ ] **Step 1: Add a rendering contract test for tab order and visible metadata**

```rust
#[test]
fn render_model_preserves_subsystem_tab_order() {
    let ledger_map = LedgerMap::from_registry(&LedgerRegistry::roadmap());
    let view_model = ParityLedgerViewModel::from_ledger_map(&ledger_map);
    let app = ParityLedgerApp::from_view_model(view_model);

    let titles = app.tab_titles();
    assert_eq!(titles[0], "Global Values");
    assert_eq!(titles[4], "Stage Values");
    assert!(titles.iter().any(|title| title == "Combat Physics Values"));
}
```

- [ ] **Step 2: Add the minimal egui UI that renders a registry summary, a tab bar, and the selected tab fields**

```rust
impl eframe::App for ParityLedgerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading(self.title());
            ui.label(format!("Tabs: {}", self.tab_count()));
            ui.horizontal_wrapped(|ui| {
                for (index, tab) in self.view_model.tabs.iter().enumerate() {
                    if ui.selectable_label(self.selected_tab == index, &tab.label).clicked() {
                        self.selected_tab = index;
                    }
                }
            });
            if let Some(tab) = self.view_model.tabs.get(self.selected_tab) {
                ui.separator();
                ui.label(format!("{}", tab.summary));
                ui.label(format!("CLI surface: {:?}", tab.cli_surface));
                ui.label(format!("GUI surface: {:?}", tab.gui_surface));
            }
        });
    }
}
```

- [ ] **Step 3: Keep the UI read-only for this slice; no editing controls until the parity model is fully trusted**

### Task 4: Update docs and handoff notes to point at the Rust GUI path

**Files:**
- Modify: `D:/Mole Game/First_Game/docs/worklogs/turnrun_frame764_parity_notepad.md`
- Modify: `D:/Mole Game/First_Game/docs/superpowers/specs/2026-06-09-future-parity-ledger-map-design.md`
- Modify: `D:/Mole Game/First_Game/crates/mole_cli/src/lib.rs` only if the recommended command list needs a native GUI launch command

- [ ] **Step 1: Record the native Rust GUI shell as the human-facing surface and note that Python is reference-only**
- [ ] **Step 2: Record the first visible tabs and the fact that they are driven by the same owned `ParityLedgerViewModel` used by the CLI**
- [ ] **Step 3: Update the recommended commands so future agents know how to open the native Rust devtool and inspect the ledger artifact**
- [ ] **Step 4: Verify the doc text does not imply the Python viewer is the authoritative UI**

### Task 5: Keep the dual-surface registry invariant intact

**Files:**
- Modify: `D:/Mole Game/First_Game/crates/mole_ledger/src/lib.rs` only if the new GUI surface requires an additional registry field or tab metadata field
- Modify: `D:/Mole Game/First_Game/crates/mole_devtool/src/lib.rs` if the app needs to expose any extra read-only fields for the GUI

- [ ] **Step 1: Confirm the registry and view-model still round-trip without losing tab order, status, or dual-surface alignment**
- [ ] **Step 2: Keep the GUI and CLI consumers on the same artifact file: `docs/state_graphs/parity_ledger_map.json`**
- [ ] **Step 3: Do not introduce separate GUI-only state for things that belong in the owned ledger map**
