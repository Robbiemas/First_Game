# State Graph Completeness Audit

Date: 2026-06-21

## Current Finding

The decomp-backed reference graph is not a full Melee common-state graph yet. It
is explicitly scoped as the movement/control path from standstill:

- Reference graph: 53 nodes, 81 edges.
- Rust graph: 58 nodes, 84 edges.
- Rust graph status mix: 24 aligned entries, 3 intentional entries, 115 partial
  entries.
- `graph missing --json` currently reports zero missing entries because every
  reference graph node has a Rust graph counterpart. That is a presence check,
  not a completeness check.

The current reference graph is useful as the Tier 1 movement spine, but it does
not yet represent the full source truth needed for Battlefield plus Captain
Falcon.

## Source Scope Already Available

The source artifacts are ahead of the graph:

- `resources/melee/frame_data/dolphin_mole/source_manifest.json` imports 275
  Captain Falcon source actions.
- 66 imported actions currently bind to a Rust `MotionState`.
- 209 imported actions remain unbound to a current Rust `MotionState`.
- `docs/state_graphs/parity_reports/falcon_ecb_coverage.json` maps 74 sampled
  motion states with exact source action table provenance.
- Battlefield extraction is source-backed for current stage primitives:
  26 collision vertices, 23 collision lines, 2 ledges, 0 dynamic collision
  lines, and `itemdata` still listed as a pending stage layer.

## Graph Gaps By Category

These are the categories that need to become first-class decomp-backed graph
nodes or grouped source ranges before the Rust graph can honestly use the
reference side as a full parity target.

1. Action and motion table surface
   - Promote action-state id, source action key, submotion id, animation length,
     callback names, IASA gates, transition callbacks, and known interrupt
     edges into a generated artifact.
   - Feed this artifact into the State Graphs tab instead of hand-maintaining
     node metadata.

2. Tier 1 movement completion
   - Existing graph covers most movement names but many edges are partial.
   - Current missing Rust edges from the reference include directional fall
     landing edges, aerial-jump-to-fall variants, and guard setoff/reflect
     transitions.
   - Current Rust-only edges include flattened fall and shield-turn shortcuts
     that need either source justification or demotion to implementation notes.

3. Combat state expansion
   - The graph currently omits most common attack variants:
     `Attack12`, `Attack13`, `Attack100Start`, `Attack100Loop`,
     `Attack100End`, angled tilt/smash variants, and source-specific jab
     progression.
   - The graph has sampled Falcon ECB/action data for attack states, but the
     graph does not yet expose that data as decomp-backed nodes.

4. Damage, down, passive, and shield-break expansion
   - The source manifest shows 57 unbound damage/down/passive-related actions.
   - The Rust graph currently compresses these into `SourceDamage`,
     `SourcePassive`, and a small down-state chain. That is acceptable as a
     temporary placeholder, but not complete parity.

5. Grab, throw, capture, and ledge expansion
   - The manifest has unbound `CatchWait`, `CatchAttack`, `CatchCut`, throws,
     capture states, and most ledge/cliff states.
   - The graph only surfaces `Catch`, `CatchDash`, `CliffCatch`, and
     `CliffWait` through coverage, and does not yet include them in the
     reference graph.

6. Captain Falcon special-state expansion
   - The graph includes `SpecialSStart`, `SpecialS`, `SpecialAirSStart`, and
     `SpecialAirS`.
   - Falcon source coverage also includes `SpecialN`, `SpecialAirN`,
     `SpecialHi`, `SpecialAirHi`, `SpecialLw`, `SpecialAirLw`.
   - Unbound Falcon-specific tails include `SpecialHiCatch`, `SpecialHiThrow`,
     `SpecialLwEnd`, `SpecialAirLwEnd`, `SpecialLwEndAir`, and
     `SpecialAirLwEndAir`.

7. Deferred or lower-priority scope
   - Item-mode states, weapon/item spawned entities, and niche object-specific
     content should remain tagged as deferred unless needed by a replay or the
     current character/stage target.
   - Spawned entities and weapons still need their own artifact surface rather
     than being hidden in the state graph.

## Workflow Refresh Needed

The stale part of the workflow is that "missing" currently means "missing from
the current movement reference graph." For this parity effort, the CLI and
devtool need three different checks:

- Reference presence: is a graph node or edge present in both JSON graphs?
- Source completeness: does the reference graph cover every in-scope decomp
  state/action for the selected target set?
- Rust implementation parity: does the Rust engine implement the source-backed
  callbacks, transition gates, timings, and collision side effects for that
  node or edge?

The next concrete CLI/devtool slice should be an `action_motion_tables` artifact
that becomes the State Graphs tab's source authority. After that, `graph missing`
can stay as a structural check, but the primary command should become a
completeness report that ranks source states not yet represented, represented
only as placeholders, or represented without callback/transition parity.

## Recommended Next Order

1. Generate the action/motion table artifact from decomp plus imported Falcon
   action data.
2. Auto-populate the State Graphs reference side from that artifact, grouped by
   source tier and target scope.
3. Reclassify the Rust graph against the generated source artifact:
   `absent`, `placeholder`, `present_partial`, `callback_complete`,
   `transition_complete`, or `aligned`.
4. Promote Tier 1 movement edges from partial to source-backed completeness
   before widening to attacks.
5. Add attack, damage/down/passive, grab/throw/capture, ledge, and Falcon
   special groups in that order.
6. Keep item-mode and spawned-entity work behind explicit deferred tags until
   the current replay or Falcon/Battlefield target requires it.
