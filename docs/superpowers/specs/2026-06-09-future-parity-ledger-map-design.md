# Future Parity Ledger Map Design

Date: 2026-06-09

## Purpose

Define the next-generation parity ledger as a model-based, subsystem-oriented
dev-tool foundation for Melee-to-Rust parity work.

The goal is to give future agents a stable map for what still needs to exist
before we can say, "the game is fully lined up, and we only compare and fix
diffs now." This ledger must support the current Rust engine path, the
decompilation reference path, and the eventual editing path after parity is
fully established.

This is not a character-editor spec yet. It is the ledger and extraction basis
that the editor will later consume.

## Current State

The project already has a meaningful base:

- Rust-owned generated value sheets exist for global, character, physics,
  combat, and stage data.
- Battlefield has a dedicated stage asset shape.
- Rust now owns a parity ledger map artifact and a shared `mole_devtool`
  view-model crate that turns the ledger map into a GUI-ready model.
- The native Rust GUI shell now exists in `crates/mole_devtool` and should be
  treated as the human-facing ledger path going forward.
- The old Python viewer is historical/reference-only. The Rust dev tool is the
  current human-facing path and must consume Rust-owned artifacts directly.
- Source-to-sheet coverage for the currently tracked extracted fields is
  complete.

Current generated ledger outputs:

- `docs/state_graphs/value_sheets/global_common_values.json`
- `docs/state_graphs/value_sheets/captain_falcon_values.json`
- `docs/state_graphs/value_sheets/physics_engine_values.json`
- `docs/state_graphs/value_sheets/combat_physics_values.json`
- `docs/state_graphs/value_sheets/battlefield_stage_values.json`
- `docs/state_graphs/parity_ledger_map.json`

Current stage asset output:

- `resources/melee/extracted/stages/battlefield_stage.json`

## Ledger Principles

The parity ledger must obey these rules:

- Model-based first, not character-based first.
- Rust is the durable authority for generated artifacts and runtime behavior.
- Decompilation is the authoring reference, not a runtime dependency.
- Python may exist only as historical reference or temporary legacy extraction
  support. New dev-tool work belongs in Rust.
- Tabs should represent engine subsystems, not convenience buckets.
- Any capability exposed to a CLI or agentic workflow must have a matching GUI
  surface, and any GUI surface must have a matching CLI or agentic surface.
- The parity ledger map must be loadable as a typed Rust artifact so the future
  GUI can consume the same contract without reinterpreting raw JSON.
- Global values belong in one shared sheet.
- Character values belong in per-character profile sheets.
- Stage values belong in per-stage sheets.
- Combat, movement, collision, and entity-spawn systems should be separable.
- No party-item, trophy, or irrelevant collectable surface should be added.
- Spawned move objects that affect gameplay, like Peach turnips or equivalent
  future move-spawned entities, are in scope.

## Tabs We Have Now

1. `Global Values`
2. `Character Values`
3. `Physics Engine Values`
4. `Combat Physics Values`
5. `Stage Values`

These are the current baseline tabs. They are enough to keep the data surface
organized, but they are not the final ledger shape.

## Tabs To Add Next

### `Action / Motion Tables`

This tab should cover motion-state and action-table parity:

- action state to source binding
- motion state timing
- callback order
- IASA / interrupt / autocancel timing
- action transition windows
- per-state animation and callback provenance

This is the tab that explains why a state changes, not just what the current
numbers are.

### `Collision Volumes`

This tab should cover all gameplay-relevant geometry:

- hitboxes
- hurtboxes
- ECB / body volumes
- push / jostle surfaces
- ledge-grab / ledge-affecting volumes if they become explicit
- frame-specific volume changes

This tab should keep geometry separate from motion timing so later agents can
audit collision parity without mixing it with animation timing.

### `Spawned Entities / Weapons`

This tab should cover move-generated gameplay objects:

- projectiles
- summons
- throw-spawned objects
- move-spawned items like Peach turnips
- state-linked entity lifecycles

This is in scope because those objects are part of combat parity and character
behavior. It is not a home for item mode, trophy mode, or unrelated party
content.

### `Shield / Grab / Tech / Ledge Interactions`

This tab should capture interaction systems that are often split across
multiple subsystems in the decomp:

- shield math
- grab and throw hooks
- tech / knockdown recovery behavior
- ledge interaction behavior
- defensive interaction timers and windows

If this surface grows too large, it can later be split into smaller tabs, but
the first implementation should keep it cohesive enough that parity review is
still easy.

### `Character Special State Values`

This tab is optional and should only exist if the general character sheet
becomes too noisy for special mechanics:

- charge systems
- transformation timers
- stance toggles
- unique per-character resources
- other character-only state that does not fit the standard profile sheet

This tab should stay minimal. Only create it when a value set cannot be cleanly
owned by `Character Values` or by one of the subsystem tabs above.

## Ownership Model

The ledger should stay split by responsibility:

- `Global Values` owns shared tuning values from the common data source.
- `Character Values` owns per-character profile data.
- `Physics Engine Values` owns locomotion, gravity, jump, air, and movement
  physics that apply across the engine.
- `Combat Physics Values` owns knockback, hitlag, damage-response, and related
  combat math.
- `Stage Values` owns stage geometry and stage-specific collision data.
- `Action / Motion Tables` owns state-timing and callback identity.
- `Collision Volumes` owns the actual frames and shapes that collide.
- `Spawned Entities / Weapons` owns gameplay objects created by actions.
- `Shield / Grab / Tech / Ledge Interactions` owns the interaction layer.
- `Character Special State Values` only exists for mechanics that do not fit
  elsewhere.

The key rule is that the ledger must remain subsystem-based. We should not add
tabs just because a value is character-specific if the real ownership is the
engine subsystem.

## How We Tackle It

### Phase 1: Stabilize The Rust Ledger Registry

Make Rust the source of truth for which ledger tabs exist and what they load.

This phase should:

- keep the current Rust-generated sheets as the canonical output surface
- define a Rust-side registry for tabs and categories
- emit `docs/state_graphs/parity_ledger_map.json` from Rust as the shared
  parity-ledger contract for both CLI and GUI consumers
- provide a typed Rust consumer for that artifact so GUI work can parse and
  validate the ledger map without touching Python
- surface the owned ledger map in Rust CLI parity reports so the agentic
  workflow and future GUI both read the same contract
- build the Rust `mole_devtool` view-model layer on top of the owned ledger map
  so the future GUI has a typed, render-ready contract
- make the dev tool consume the registry rather than a hardcoded tab list
- keep the Python viewer as historical reference only while Rust replaces any
  remaining useful affordances

### Phase 2: Expand The Rust Data Schemas

Add the missing subsystem artifact shapes in Rust:

- action / motion table artifacts
- collision volume artifacts
- spawned entity artifacts
- interaction system artifacts

The schema should be explicit and future-proof, but not overengineered. Only
include the fields needed for parity review, extraction, and later editing.

### Phase 3: Surface The New Tabs In The Dev Tool

Add the new tabs to the live dev tool using the Rust registry as the driving
source.

The Rust-native view model should show:

- tab names
- category counts
- field counts
- source provenance
- current Rust value vs decomp value where that comparison makes sense
- clear empty states for tabs that are not yet populated

### Phase 4: Keep The Dev Tool Rust-Native

As new ledger work happens, implement the dev-tool side in Rust first and keep
Python out of the durable path.

Python may still be read as a historical reference for visual behavior that has
not yet been translated, but it should not be extended as a bridge or authority.
Every new amendment should make the Rust CLI/devtool/engine contract more
complete.

### Phase 5: Treat The Ledger As A Parity Contract

Once the major tabs are populated, the ledger should become the contract we use
for parity review:

- compare source and Rust values
- identify missing fields
- identify behavior mismatches
- drive extraction or fix-up work
- avoid guessing at mechanics that are not yet proven

## Suggested Rollout Order

The next practical order should be:

1. Keep the existing Rust-generated sheets as the baseline.
2. Add `Action / Motion Tables`.
3. Add `Collision Volumes`.
4. Add `Spawned Entities / Weapons`.
5. Add `Shield / Grab / Tech / Ledge Interactions`.
6. Add `Character Special State Values` only if the first five surfaces show a
   real need.
7. Continue moving any remaining historical Python-only affordances into the
   Rust dev tool once the data model is stable enough for that surface.

## Testing Expectations

Each new tab or ledger surface should come with tests that prove:

- the sheet or artifact is generated from the expected source
- the tab is visible in the dev tool
- the field counts are stable
- the coverage gap list is explicit
- the UI does not silently omit the new surface

The tests should continue to allow the staged transition from Rust-owned data
with Python rendering to fully Rust-owned tooling.

## Success Criteria

We should consider the ledger foundation successful when:

- the subsystem tabs exist in a stable registry
- the dev tool can show every relevant parity surface without character-based
  special casing
- the current Rust engine path can compare against decomp-backed artifacts
- the future editing path can reuse the same ledger data without schema churn
- future agents can pick up from this document and know what to build next
