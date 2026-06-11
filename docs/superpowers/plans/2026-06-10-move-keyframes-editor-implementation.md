# Move Keyframes Editor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the Rust `Move Keyframes` tab into a draggable keyframe editor that can edit joints, capsules, ECB/body volumes, save the edited JSON, and keep the renderer modular enough for future animation-set export.

**Architecture:** Keep one editor core in `crates/mole_devtool/src/move_keyframes.rs` and let `ui.rs` be a thin shell that renders it. The editor should load the existing `AttackAirN.json` artifact, normalize all editable geometry into shared handles, and apply direct edits back to the selected frame. Save/export should stay on the same JSON artifact shape for now so the UI remains simple and the eventual compiler step can be added later without redesigning the editor.

**Tech Stack:** Rust, `eframe`/`egui`, `serde_json`, existing `mole_devtool` module layout, existing `resources/melee/frame_data/dolphin_mole/AttackAirN.json`.

---

### Task 1: Add the editor data model and editable-handle inventory

**Files:**
- Modify: `crates/mole_devtool/src/move_keyframes.rs`
- Modify: `crates/mole_devtool/src/lib.rs`
- Modify: `crates/mole_devtool/src/app.rs`
- Test: `crates/mole_devtool/src/move_keyframes.rs` unit tests

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn move_keyframes_editor_exposes_frame_handles_and_dirty_state() {
    let surface = MoveKeyframesSurface::load(workspace_root()).unwrap();
    let editor = MoveKeyframesEditorSurface::from_surface(surface.clone());

    assert_eq!(editor.selected_frame_index(), 0);
    assert_eq!(editor.frame_count(), surface.keyframes.len());

    let handles = editor.handles_for_selected_frame();
    assert!(handles.iter().any(|handle| matches!(handle.kind, MoveKeyframeHandleKind::Joint { .. })));
    assert!(handles.iter().any(|handle| matches!(handle.kind, MoveKeyframeHandleKind::EcbPoint { .. })));
    assert!(handles.iter().any(|handle| matches!(handle.kind, MoveKeyframeHandleKind::HurtboxEndpoint { .. })));
    assert!(handles.iter().any(|handle| matches!(handle.kind, MoveKeyframeHandleKind::HitboxEndpoint { .. })));
    assert!(!editor.is_dirty());
}
```

- [ ] **Step 2: Run the focused test and confirm it fails**

Run:
`cargo test -p mole_devtool move_keyframes_editor_exposes_frame_handles_and_dirty_state -- --nocapture`

Expected: FAIL because `MoveKeyframesEditorSurface`, `MoveKeyframeHandleKind`, and the handle inventory helpers do not exist yet.

- [ ] **Step 3: Implement the minimal editor core**

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MoveKeyframeHandleKind {
    Joint { joint_index: usize },
    HurtboxEndpoint { hurtbox_index: usize, endpoint: MoveKeyframeEndpoint },
    HitboxEndpoint { hitbox_index: usize, endpoint: MoveKeyframeEndpoint },
    EcbPoint { body_volume_index: usize, point: MoveKeyframeBodyPoint },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveKeyframeEndpoint {
    A,
    B,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveKeyframeBodyPoint {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Debug, Clone)]
pub struct MoveKeyframeHandle {
    pub kind: MoveKeyframeHandleKind,
    pub position: [f64; 2],
}

#[derive(Debug, Clone)]
pub struct MoveKeyframesEditorSurface {
    surface: MoveKeyframesSurface,
    selected_frame_index: usize,
    dirty: bool,
}

impl MoveKeyframesEditorSurface {
    pub fn from_surface(surface: MoveKeyframesSurface) -> Self { /* ... */ }
    pub fn selected_frame_index(&self) -> usize { /* ... */ }
    pub fn frame_count(&self) -> usize { /* ... */ }
    pub fn is_dirty(&self) -> bool { /* ... */ }
    pub fn handles_for_selected_frame(&self) -> Vec<MoveKeyframeHandle> { /* ... */ }
}
```

- [ ] **Step 4: Re-run the focused test**

Run:
`cargo test -p mole_devtool move_keyframes_editor_exposes_frame_handles_and_dirty_state -- --nocapture`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/mole_devtool/src/move_keyframes.rs crates/mole_devtool/src/lib.rs crates/mole_devtool/src/app.rs
git commit -m "feat: add move keyframes editor model"
```

### Task 2: Add drag/apply geometry editing and keep the canvas simple

**Files:**
- Modify: `crates/mole_devtool/src/move_keyframes.rs`
- Modify: `crates/mole_devtool/src/ui.rs`
- Test: `crates/mole_devtool/src/move_keyframes.rs` unit tests

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn move_keyframes_dragging_a_hurtbox_endpoint_updates_only_that_endpoint() {
    let surface = MoveKeyframesSurface::load(workspace_root()).unwrap();
    let mut editor = MoveKeyframesEditorSurface::from_surface(surface);

    let before = editor.selected_frame().unwrap().hurtboxes[0].clone();
    editor.drag_handle(
        MoveKeyframeHandleKind::HurtboxEndpoint {
            hurtbox_index: 0,
            endpoint: MoveKeyframeEndpoint::A,
        },
        [1.0, -2.0],
    );
    let after = editor.selected_frame().unwrap().hurtboxes[0].clone();

    assert_ne!(before, after);
    assert!(editor.is_dirty());
}
```

- [ ] **Step 2: Run the focused test and confirm it fails**

Run:
`cargo test -p mole_devtool move_keyframes_dragging_a_hurtbox_endpoint_updates_only_that_endpoint -- --nocapture`

Expected: FAIL because `drag_handle()` and `selected_frame()` do not exist yet.

- [ ] **Step 3: Implement the minimal drag logic**

```rust
impl MoveKeyframesEditorSurface {
    pub fn selected_frame(&self) -> Option<&MoveKeyframe> { /* ... */ }
    pub fn selected_frame_mut(&mut self) -> Option<&mut MoveKeyframe> { /* ... */ }
    pub fn drag_handle(&mut self, kind: MoveKeyframeHandleKind, delta: [f64; 2]) { /* ... */ }
}
```

The drag logic should:
- mutate only the targeted point,
- preserve the rest of the frame data,
- and mark the editor dirty.

In `ui.rs`, keep the canvas simple:
- render the frame preview,
- render handle markers,
- and call the editor drag helpers from egui pointer events.

- [ ] **Step 4: Re-run the focused test**

Run:
`cargo test -p mole_devtool move_keyframes_dragging_a_hurtbox_endpoint_updates_only_that_endpoint -- --nocapture`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/mole_devtool/src/move_keyframes.rs crates/mole_devtool/src/ui.rs
git commit -m "feat: apply draggable move keyframe edits"
```

### Task 3: Add save/export back to the existing JSON artifact

**Files:**
- Modify: `crates/mole_devtool/src/move_keyframes.rs`
- Modify: `crates/mole_devtool/src/ui.rs`
- Modify: `crates/mole_devtool/src/app.rs`
- Test: `crates/mole_devtool/src/move_keyframes.rs` unit tests

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn move_keyframes_save_round_trips_the_existing_json_shape() {
    let root = workspace_root();
    let source_path = root.join("resources/melee/frame_data/dolphin_mole/AttackAirN.json");
    let temp_path = root.join("target").join("move_keyframes_editor_round_trip.json");

    let surface = MoveKeyframesSurface::load(&root).unwrap();
    let mut editor = MoveKeyframesEditorSurface::from_surface(surface);
    editor.save_to(&temp_path).unwrap();

    let round_trip = MoveKeyframesSurface::load_from(&temp_path).unwrap();
    assert_eq!(round_trip.keyframes.len(), editor.frame_count());
    assert!(temp_path.exists());

    std::fs::remove_file(temp_path).unwrap();
}
```

- [ ] **Step 2: Run the focused test and confirm it fails**

Run:
`cargo test -p mole_devtool move_keyframes_save_round_trips_the_existing_json_shape -- --nocapture`

Expected: FAIL because `save_to()` / `load_from()` do not exist yet.

- [ ] **Step 3: Implement save/export**

```rust
impl MoveKeyframesSurface {
    pub fn load_from(path: impl AsRef<Path>) -> Result<Self, String> { /* ... */ }
}

impl MoveKeyframesEditorSurface {
    pub fn save_to(&self, path: impl AsRef<Path>) -> Result<(), String> { /* ... */ }
    pub fn clear_dirty(&mut self) { /* ... */ }
}
```

Add a single `Save` control in the `Move Keyframes` tab that writes the current edited JSON back out through the same path the editor loaded from.

Do not add a new animation-set compiler in this slice. The save path should stay on the current JSON artifact shape so the export pipeline can be introduced later without changing the editor core.

- [ ] **Step 4: Re-run the focused test**

Run:
`cargo test -p mole_devtool move_keyframes_save_round_trips_the_existing_json_shape -- --nocapture`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/mole_devtool/src/move_keyframes.rs crates/mole_devtool/src/ui.rs crates/mole_devtool/src/app.rs
git commit -m "feat: save move keyframes edits"
```

### Task 4: Restore the editor UI around the reusable canvas shell

**Files:**
- Modify: `crates/mole_devtool/src/ui.rs`
- Modify: `crates/mole_devtool/src/move_keyframes.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn move_keyframes_ui_exposes_split_editor_shell() {
    let surface = MoveKeyframesSurface::load(workspace_root()).unwrap();
    let editor = MoveKeyframesEditorSurface::from_surface(surface);

    assert!(editor.handles_for_selected_frame().len() > 0);
}
```

- [ ] **Step 2: Run the focused test and confirm it fails if the shell is missing**

Run:
`cargo test -p mole_devtool move_keyframes_ui_exposes_split_editor_shell -- --nocapture`

Expected: PASS once the editor model exists; use this as the final smoke check after the UI shell lands.

- [ ] **Step 3: Wire the UI shell**

```rust
fn render_move_keyframes(ui: &mut egui::Ui, app: &mut ParityLedgerApp) {
    ui.heading("Move Keyframes");
    ui.label(app.move_keyframes.summary());
    ui.separator();

    ui.columns(2, |columns| {
        render_move_keyframe_list(&mut columns[0], &mut app.move_keyframes_editor);
        render_move_keyframe_preview(&mut columns[1], &app.move_keyframes_editor);
    });
}
```

Keep the split view simple:
- left: frame list,
- right: preview canvas,
- top or bottom: save/export status,
- no extra panes unless they are essential for editing.

- [ ] **Step 4: Re-run the focused test and the devtool compile**

Run:
`cargo test -p mole_devtool move_keyframes_ui_exposes_split_editor_shell -- --nocapture`

Run:
`cargo check -p mole_devtool`

Expected: PASS for both.

- [ ] **Step 5: Commit**

```bash
git add crates/mole_devtool/src/ui.rs crates/mole_devtool/src/move_keyframes.rs
git commit -m "feat: restore move keyframes editor shell"
```

## Coverage Check

- Editor model: Task 1
- Drag/apply geometry edits: Task 2
- Save/export path: Task 3
- UI shell restoration: Task 4

## Notes for Future Expansion

- If later we add animation-set compilation, it should consume the saved JSON artifact rather than replacing the editor core.
- If later we connect live engine snapshots, the editor should treat them as an alternate source for the same geometry handles, not a second UI design.
