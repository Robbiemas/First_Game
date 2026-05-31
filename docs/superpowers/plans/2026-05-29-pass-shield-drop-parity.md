# Pass Shield Drop Parity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Subagents are disabled for this repository session by the user. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the Melee `Pass` movement state so shield/platform drop-through emerges from the source-shaped down-through-platform gate, with UCF preprocessing translated before core input, without adding a custom `ShieldDrop` or `AxeDrop` state.

**Architecture:** Rust core remains authoritative and vanilla-Melee-shaped. `MeleeCommonData` owns PlCo common fields `x464`, `x468`, `x46C`, and records `x470` for the delayed crouch/pass path. `MotionState::Pass` is entered from `GuardOn`/`Guard` only when the player is grounded on a soft platform, held source L/R shield exists, and the engine-facing pad snapshot satisfies the source down-stick tap window. UCF belongs in `mole_input`/WUP preprocessing before that snapshot reaches the core.

**Tech Stack:** Rust `mole_core`, deterministic 60 Hz simulation, existing Battlefield-style `StageProfile`, native WUP/UCF preprocessing in `mole_input`.

---

## Decomp References

- `D:\Mole Game\.research\doldecomp-melee\src\melee\ft\chara\ftCommon\ftCo_Pass.c`
  - `ftCo_80099F1C`: requires `lstick.y <= -p_ftCommonData->x464`, `x671_timer_lstick_tilt_y < p_ftCommonData->x468`, and `mpColl_IsOnPlatform`.
  - `ftCo_8009A080`: shield path requires held `HSD_PAD_LR` and then enters `ftCo_8009A228`.
  - `ftCo_8009A228`: enters `ftCo_MS_Pass`, sets `self_vel.y = p_ftCommonData->x46C`, calls `mpUpdateFloorSkip`, and expires the y-tilt timer.
  - `ftCo_Pass_Phys`: calls `ft_80084DB0`, the normal airborne fall/fastfall physics helper.
  - `ftCo_Pass_Anim`: animation end enters ordinary `Fall`.
- `D:\Mole Game\.research\doldecomp-melee\src\melee\ft\chara\ftCommon\ftCo_Guard.c`
  - `GuardOn_IASA`, `Guard_IASA`, and `GuardReflect_IASA` call `ftCo_8009A080` after spotdodge, roll, grab, and jump checks.
- `D:\Mole Game\.research\doldecomp-melee\src\melee\ft\types.h`
  - `x464`, `x468`, `x46C`, and `x470` are `float` common-data fields.

## File Map

- Modify `D:\Mole Game\First_Game\crates\mole_core\src\common_data.rs`
  - Add common-data fields for platform pass y threshold, y tap window, pass initial vertical velocity, and platform-drop delay.
  - Extract fields from offsets `0x464`, `0x468`, `0x46c`, and `0x470`.
  - Record source offsets.
- Modify `D:\Mole Game\First_Game\crates\mole_core\src\collision.rs`
  - Add a helper to identify the current support surface for a grounded bottom point.
- Modify `D:\Mole Game\First_Game\crates\mole_core\src\lib.rs`
  - Re-export the support-surface helper for contract tests.
- Modify `D:\Mole Game\First_Game\crates\mole_core\src\state.rs`
  - Add `MotionState::Pass`, checksum id, and stable snapshot hashing.
- Modify `D:\Mole Game\First_Game\crates\mole_core\src\sim.rs`
  - Add `GuardOn`/`Guard` pass checks after spotdodge/roll/grab/jump priority.
  - Enter `Pass` with source vertical velocity, airborne grounded state, and soft-platform skip behavior.
  - Keep `Pass` landing through existing non-special landing path; do not add `ShieldDrop` or `AxeDrop`.
- Modify `D:\Mole Game\First_Game\crates\mole_core\tests\core_contract.rs`
  - Add tests for common-data extraction, support-surface kind, Guard shield-drop entry into `Pass`, hard floor rejection, no invented state names, and deterministic adapter-preprocessed pass.

---

## Implementation Tasks

### Task 1: Common Data For Pass Gate

**Files:**
- Modify: `D:\Mole Game\First_Game\crates\mole_core\src\common_data.rs`
- Modify: `D:\Mole Game\First_Game\crates\mole_core\tests\core_contract.rs`

- [ ] Write failing tests that assert:
  - `MeleeCommonData::provisional_mole().platform_pass_y == 84`
  - `platform_pass_y_tap_window == 3`
  - `pass_initial_y_velocity == -1200`
  - `platform_drop_delay_ticks == 4`
  - source offsets are `x464`, `x468`, `x46C`, and `x470`.
  - synthetic PlCo extraction reads those offsets.
- [ ] Run:
  - `cargo test -p mole_core input_threshold_defaults_come_from_provisional_common_data`
  - Expected: fail because fields do not exist.
- [ ] Add fields, provisional values, extraction, and source entries.
- [ ] Run the three focused common-data tests and commit:
  - `git commit -m "feat: extract pass platform gate data"`

### Task 2: Support Surface Helper

**Files:**
- Modify: `D:\Mole Game\First_Game\crates\mole_core\src\collision.rs`
- Modify: `D:\Mole Game\First_Game\crates\mole_core\src\lib.rs`
- Modify: `D:\Mole Game\First_Game\crates\mole_core\tests\core_contract.rs`

- [ ] Write failing tests that assert `floor_surface_for_bottom` returns the left soft platform for a grounded player on that surface and returns the main floor for the main stage.
- [ ] Run:
  - `cargo test -p mole_core stage_floor_support_reports_surface_kind`
  - Expected: fail because the helper does not exist.
- [ ] Add:
  - `pub fn floor_surface_for_bottom(stage: StageProfile, bottom: Vec2) -> Option<StageSurface>`
  - Export it from `lib.rs`.
- [ ] Run the focused test and commit:
  - `git commit -m "feat: expose grounded support surface"`

### Task 3: Add Melee `Pass` State

**Files:**
- Modify: `D:\Mole Game\First_Game\crates\mole_core\src\state.rs`
- Modify: `D:\Mole Game\First_Game\crates\mole_core\src\sim.rs`
- Modify: `D:\Mole Game\First_Game\crates\mole_core\tests\core_contract.rs`

- [ ] Write failing tests that land player one on the left platform, enter `Guard`, press held L/R plus down, and assert:
  - state becomes `MotionState::Pass`
  - `grounded == false`
  - `velocity.y == MeleeCommonData::provisional_mole().pass_initial_y_velocity`
  - `format!("{:?}", motion_state)` is neither `ShieldDrop` nor `AxeDrop`.
- [ ] Run:
  - `cargo test -p mole_core shield_down_on_soft_platform_enters_pass_not_custom_drop_state`
  - Expected: fail because `Pass` does not exist and Guard currently ignores platform pass.
- [ ] Add `MotionState::Pass`, a checksum id, and `enter_pass`.
- [ ] Add pass logic to `GuardOn` and `Guard` after the existing dodge/roll/grab/jump priority checks.
- [ ] Make airborne contact skip soft platforms while `MotionState::Pass` is active, mirroring `mpUpdateFloorSkip` for this slice.
- [ ] Run the focused test and commit:
  - `git commit -m "feat: enter pass from shield platform drop"`

### Task 4: Reject Hard Floor And Cover Adapter-Preprocessed UCF Path

**Files:**
- Modify: `D:\Mole Game\First_Game\crates\mole_core\tests\core_contract.rs`
- Modify: `D:\Mole Game\First_Game\crates\mole_core\src\sim.rs`

- [ ] Write failing tests that assert:
  - held L/R plus down on the main floor remains `Guard`/`GuardOn`, not `Pass`.
  - with source UCF shield-drop preprocessing enabled in the adapter, the core receives an ordinary vanilla pass-band input and can enter `Pass` on a soft platform.
  - two worlds with identical adapter-preprocessed inputs produce matching checksums through pass entry.
- [ ] Run:
  - `cargo test -p mole_core pass_from_shield_requires_soft_platform_support`
  - `cargo test -p mole_input gamecube_input_mapper_translates_ucf_shield_drop_after_source_two_frame_counter`
  - Expected: fail until adapter preprocessing translates the UCF source condition before core input.
- [ ] Add helper logic:
  - source-shaped non-UCF path: `stick_y <= -platform_pass_y && y_tap < platform_pass_y_tap_window`
  - the spotdodge threshold must stay harder downward than `platform_pass_y`; if the user hits the spotdodge gate, source priority enters `EscapeN` before `Pass`.
  - UCF-assisted path: adapter emits an ordinary pass-band stick value before the core snapshot is derived.
  - core pass entry still requires `facts.source_held.lr()` and `StageSurfaceKind::Soft`.
- [ ] Run focused tests and commit:
  - `git commit -m "test: cover pass platform and ucf gates"`

### Task 5: Verification

**Files:**
- No source changes expected.

- [ ] Run:
  - `cargo test -p mole_core pass`
  - `cargo test -p mole_core shield`
  - `cargo test -p mole_core`
  - `cargo test --workspace`
  - `cargo fmt --all -- --check`
  - `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  - `git diff --check`
  - `rg -n "unsafe\s*\{" crates`
- [ ] Expected:
  - all cargo commands pass.
  - `git diff --check` passes.
  - unsafe scan prints no matches and exits `1`.
- [ ] Push:
  - `git push origin handoff/rust-rollback-architecture`
