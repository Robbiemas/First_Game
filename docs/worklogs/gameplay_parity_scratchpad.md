# Gameplay Parity Scratchpad

Updated: 2026-06-21

Purpose: keep the next full-gameplay parity work in one actionable place. This is
not a source of truth. The Melee decomp is the source, the parity ledgers are an
index, and this file is the working queue for translating decomp-shaped behavior
into compact Rust runtime data without gameplay hotfixes or byte bloat.

Working rule: before implementing any item below, inspect the named decomp
anchor again and write/adjust a focused test first. Runtime must consume baked
Rust/project artifacts, not raw ISO, raw DAT, or decomp files.

## Active Source Hit/Hurt Collision Spec

Decomp anchors checked 2026-06-20:
- `src/melee/ft/fighter.c`: fighter procs run animation/update/map before
  `Fighter_ProcessHit_8006D1EC`; hit processing reads the already-live JObj
  pose for the frame.
- `src/melee/ft/ftcoll.c`: `ftColl_8007AD18` writes hit capsule `x58` from
  previous `x4C`, then refreshes `x4C` through `lb_8000B1CC`; on a new active
  hitbox frame it seeds `x58 = x4C`.
- `src/melee/lb/lbcollision.c`: `lbColl_8000805C` compares hit
  `x58 -> x4C` against hurt `a_pos -> b_pos`; hurt capsule positions are
  refreshed lazily from live JObj world positions unless `skip_update_pos` is
  set.
- `src/melee/lb/lb_00B0.c`: `lb_8000B1CC` calls `HSD_JObjSetupMatrix` and
  consumes the live JObj world matrix, not a render-flattened or replay-patched
  coordinate.

Slippi witness:
- Source frame 2253 has P1 in `AttackAirN` and P2 in `Dash`; P2 enters source
  damage action 79 on the next exported frame.
- Runtime hit capsules at P1 `AttackAirN` frame 7 are present and newly active,
  so decomp seeds previous/current hit endpoints to the same live point.
- The confirm appears when P2 hurt capsules use the live Dash pose boundary
  corresponding to baked frame 1 at P2's current source position. Using baked
  Dash frame 2 at the same position misses. Using previous P2 position is not
  the decomp path.

Pipeline rule:
- Hit capsule data needs action/event timing plus decomp previous/current hit
  endpoints.
- Hurt capsule data needs live JObj pose timing at the hit-processing boundary;
  do not assume the same exported source pose frame can be reused for both hit
  and hurt capsules.
- If a baked artifact frame differs from Slippi's public motion-frame counter,
  fix the source-frame bridge or artifact semantics. Do not add replay-specific
  offsets.

Milestone 2026-06-20:
- The frame-2253 barrier now reaches source damage action 79. Focused runtime
  source-collision tests pass with P1 `AttackAirN` frame 7 and P2 Dash hurt
  capsules sampled from the decomp live-pose boundary.
- Replay scan still reports source frame 2253 because damage response is not
  yet in full parity: P2's magnitude is close, but Rust launches at roughly
  `x=0.38718945, y=1.35028994` while Slippi exports attack velocity
  `x=0.29205501, y=1.37400997`. That is an angle/selected-hit result issue,
  not a hit-confirm issue.
- Next root target: translate the `Fighter_ProcessHit_8006D1EC` damage-result
  selection into `ftCo_Damage_CalcAngle`, `ftCo_Damage_CalcVel`, hitlag exit
  DI/trigger scaling, and damage-state physics order. Do not correct this with
  a replay velocity offset.

## Current Functional Baseline

- Friend playtest launches, connects through Friend Connect, and can use the
  four-slot setup-only lobby directory.
- Gameplay packets are direct UDP with rollback-owned input delay/repair.
- Battlefield is extracted into a game-owned stage blob with collision, ledges,
  camera bounds, blast zones, map-head entries, and stage callback metadata.
- Captain Falcon-derived Dolphin Mole state/capsule data is baked into runtime
  sidecars and preloaded before gameplay.
- Source damage/hitlag/hitstun/downbound/passive slices exist, but broad
  damage response and match-flow parity are still incomplete.

## Source Velocity Channel Spec

Decomp anchors checked 2026-06-20:
- `src/melee/ft/chara/ftCommon/ftCo_Damage.c`: `ftCo_Damage_CalcVel`
  writes damage knockback into `fp->x8c_kb_vel.x/y`; damage pose code reads
  `fp->self_vel.x/y + fp->x8c_kb_vel.x/y`.
- `src/melee/ft/chara/ftCommon/ftCo_0A01.c`: CPU/DI helpers also read
  `fp->x8c_kb_vel` as the separate knockback vector.
- Slippi exports normal fighter self velocity as
  `self_induced_speeds.air_x`, `ground_x`, and `y`, and exports damage/attack
  velocity separately as `attack_x` and `attack_y`.

Pipeline rule:
- Preserve self velocity and knockback/attack velocity as separate fields in
  Rust diagnostics and baked runtime state. Do not collapse them during
  extraction.
- When comparing movement velocity for source damage action IDs, compare the
  decomp-composed vector: selected horizontal self channel plus `attack_x`,
  and vertical `y + attack_y`.
- For normal motion states, continue comparing only the selected self channel
  unless the decomp path proves another field participates.

## Immediate Human-Visible Gaps

- Entry/spawn platform behavior is not yet fully actionable: platform collision
  and broad IASA options need decomp-shaped runtime rules. The source-backed
  platform cue/wireframe now renders during Entry and RebirthWait, and hard-down
  RebirthWait exit installs source `x5D8` intangibility before Fall.
- Cliff catch is wired to extracted Battlefield ledges, but the full
  `CollData.env_flags` collision pass is not translated yet. Until that exists,
  ledge catch must stay constrained by source floor-line endpoints and ECB bottom
  checks, not root-position rectangles or platform-edge guesses.
- Match phase is rollback-owned and renders Ready/Go labels, but the full
  source countdown timing still needs the `gm` flow translated.
- Falcon down-air style hits currently produce damage/animation feedback but do
  not yet apply full Melee knockback/DI/tumble/tech outcomes.
- SDI/ASDI/DI, tumble, wall/ceiling/floor tech, cliff attack/jump/climb/escape
  options, KO/stock/respawn, and full four-stock match flow are not yet
  complete.

## Bottom-Up Cliff And Stage Collision Checklist

Problem observed 2026-06-12: ledge catch was too sensitive and could be
perceived as grabbing platforms/invalid edges because Rust still used a
root-position snap rectangle. The decomp does not do that. `mpcoll.c` first
updates `CollData`, then `ftcliffcommon.c` only enters `CliffCatch` when
`Collide_LeftLedgeGrab` or `Collide_RightLedgeGrab` is present.

Decomp anchors:
- `src/melee/mp/mpcoll.c`: `mpColl_80044164`,
  `mpColl_800443C4`, and the `CollisionFlagAir_CanGrabLedge` block that sets
  `Collide_LeftLedgeGrab` / `Collide_RightLedgeGrab`.
- `src/melee/ft/ftcliffcommon.c`: `ftCliffCommon_80081298`,
  `ftCliffCommon_80081370`, `ftCo_CliffCatch_Phys`.
- `src/melee/mp/mplib.c`: `mpLib_80053ECC_Floor`,
  `mpLib_80053DA4_Floor`, `mpFloorGetLeft`, `mpFloorGetRight`,
  `mpLib_80054ED8`.
- `src/melee/lb/types.h`: `CollData.env_flags`,
  `ledge_id_left`, `ledge_id_right`.

Checklist:
- [ ] Promote a compact Rust `CollData` equivalent for fighter/stage collision
  snapshots, including `env_flags`, previous/current position, current ECB,
  ledge IDs, floor skip, facing, and source snap dimensions.
- [ ] Translate `mpColl_80044164` and `mpColl_800443C4` over extracted
  `StageCollisionLine` data so left/right ledge-grab flags are produced by the
  collision pass, not by state code.
- [ ] Translate the surrounding `CollisionFlagAir_CanGrabLedge` order from
  `mpcoll.c`: airborne only, falling only, not already on an edge, facing-gated.
- [ ] Preserve source floor topology and line flags so soft platforms and
  non-ledge floor segments cannot emit cliff flags unless the decomp would.
- [ ] Translate `mpLib_80053ECC_Floor`, `mpLib_80053DA4_Floor`, and
  `mpLib_80054ED8` against baked stage lines for cliff pinning and invalid-line
  fallback to Fall.
- [ ] Replace the temporary state-level ledge gate with the collision-pass
  `env_flags` once the compact `CollData` path exists.
- [ ] Add replay divergence probes around first cliff-grab frames so Slippi
  checks can show whether ledge IDs, position pinning, and facing match source.

## Current Capture/Throw Parity Notes

Captured and thrown fighters are still architectural parity work, not replay
patch territory. Decomp anchors checked on 2026-06-20:
- `src/melee/ft/chara/ftCommon/ftCo_Attack100.c`:
  `ftCo_CapturePulledLw_Phys`, `ftCo_CapturePulledLw_Coll`,
  `ftCo_800DB368`, `ftCo_800DB464`.
- `src/melee/ft/chara/ftCommon/ftCo_Thrown.c`:
  `ftCo_800DE3FC`, `ftCo_800DE508`, `ftCo_Thrown*_Anim`.
- `src/melee/ft/chara/ftCommon/ftCo_Throw.c`: `ftCo_800DDDE4`.
- `src/melee/ft/fighter.c`: `Fighter_UnkUpdateVecFromBones_8006876C`.

Findings:
- Simple source rule: when decomp calls `lb_8000B1CC`, the runtime artifact
  must preserve that live JObj world `x/y/z` value. Do not project it into the
  2D render plane inside the extractor.
- Slippi replay positions are the source gameplay position stream used to
  validate the Rust runtime. They are not proof that baked JObj samples should
  be flattened; they are proof that the live JObj world basis and the runtime
  gameplay basis must agree before collision/capture data is consumed.
- Source capture pose sidecars must represent live decomp-facing JObj world
  positions. The runtime should not reinterpret raw rig axes differently from
  hit/hurt/collision samples.
- `ftCo_CapturePulledLw_Phys` reads `capturedamage.x18` and `FtPart_XRotN`
  world positions directly, then adds the direct delta to `cur_pos`.
- `Fighter_UnkUpdateVecFromBones_8006876C` stores `x1A70` as
  `TransN - XRotN` from live world positions. `ftCo_800DDDE4` then uses
  `x1A70.z` for horizontal thrown placement and `x1A70.y` for vertical
  placement.
- Replay witness after enforcing literal capture pose: P2 source frame 2300
  still diverges in `CapturePulledLw`, with Rust at `x=38.142338` and Slippi at
  `x=46.050034`. This means the extractor/runtime still has a TopN/JObj
  coordinate-basis mismatch. Do not fix this by replay offsets or by restoring
  render-plane flattening to capture data; fix the shared live HSD setup/basis.
- Low capture collision uses `ft_8008403C -> ft_80082708 ->
  mpColl_8004B108` against persistent `CollData`; recomputing floor state after
  the capture delta can erase the line state the decomp path relies on.
- `x1A70` is derived from live bone world positions by
  `Fighter_UnkUpdateVecFromBones_8006876C` and then reused by capture cut,
  throw release, and thrown accessory positioning. This should become a compact
  runtime pose field or an equivalent live JObj-derived value, not a constant.
- Thrown entry calls `ftCo_800DB368`, which constrains victim `XRotN` to
  thrower `TransN2`, stores `x2174`, and sets `x2226_b2`. The Rust runtime needs
  this state represented explicitly before thrown/capture release can match.
- `ftCo_800DE508` positions a thrown victim from the constrained live `XRotN`
  world point plus `x1A70` offsets. Thrown states do not run generic floor
  collision callbacks in `ftCo_Thrown*_Coll`.
- High capture collision is the `ft_80083C00` family while low capture uses
  `ft_8008403C`; keep those separate when translating the remaining states.
- The remaining capture X mismatch near Slippi source frame 2300 likely comes
  from source animation/JObj update order or pose-frame selection. Do not solve
  it with a positional correction; prove the order from decomp and bake/consume
  pose data accordingly.

## Priority Queue

### P0 - Match Flow Shell

Goal: make a local or Friend Connect game feel like a real stock match loop
before deep combat tuning, without inventing values.

Decomp anchors:
- `src/melee/gm/*` for match startup/countdown/timer/stock flow.
- `src/melee/pl/player.c` / `player.h` for spawn positions and spawn-platform
  final position accessors.
- `src/melee/ft/chara/ftCommon/ftCo_DemoCallback0.c` for transition into
  `ftCo_MS_Rebirth`.
- `src/melee/ft/ftmotionstates.c` entries for `Rebirth`, `RebirthWait`,
  `Entry`, `EntryStart`, and `EntryEnd`.

Implementation notes:
- Add rollback-owned match phase fields: pregame/countdown/go/play/stock pause
  / game end.
- Render Ready/Go as runtime scene metadata, not SDL-only text state.
- Use stage spawn metadata already in `MeleeStageProfile`; do not reintroduce
  hard-coded Battlefield spawns.
- Make inactive/dead players match the decomp flow rather than removing them
  from simulation/render by convenience.

Exit checks:
- Two-player local runtime starts with a countdown, then enters actionable play.
- A stock loss decrements stock, respawns from source stage spawn data, and
  restores source invincibility/intangibility.
- Rollback snapshot/checksum includes match phase, stock, respawn, and timers.

### P0 - Entry And Spawn Platform Parity

Goal: promote the existing source-backed common accessory profile into gameplay
and rendering so entry platforms are visible and behave like Melee.

Decomp anchors:
- `src/melee/ft/fighter.h`: `Fighter_804D6514` common platform model.
- `src/melee/ft/fighter.c`: `Fighter_LoadCommonData`, `pData[16]`.
- `src/melee/ft/chara/ftCommon/ftCo_800C7070.c` and neighboring Entry/Rebirth
  files for state flow.
- `src/melee/pl/player.c`: `Player_GetSpawnPlatformFinalPos`,
  `Player_SetSpawnPlatformFinalPos`.
- `src/melee/mp/mpcoll.c`: platform pass callback and floor collision rules.

Existing Rust/data:
- `resources/melee/extracted/fighter_common_accessories.json`.
- `crates/mole_core/src/generated/fighter_common.rs`.
- `FighterEntryPlatformProfile` and `entry_platform_offset_y`.
- Per-stage extraction blobs must continue preserving source spawn points,
  facing, and respawn platform final-position metadata for each stage, including
  Yoshi's Story and other future imports.
- Render currently has a lightweight entry-platform cue, but not a true
  collision/runtime actor.

Implementation notes:
- Do not encode match-entry spawn platforms as stage collision rectangles. The
  platform model/timing source is the common fighter accessory
  `Fighter_804D6514` from `PlCo.dat`; stage extraction supplies the stage-owned
  spawn/facing/final-position data it attaches to.
- Represent entry platform collision from the decomp Entry/Rebirth flow and
  common accessory profile after the exact callback path is transcribed.
- Render source-space platform bounds/wireframe from the baked common accessory.
  Current rendering exposes that cue during Entry and RebirthWait.
- Ensure fighter ECB/hurtboxes are visible and source-correct while standing on
  the platform.
- Implement pass/drop-through and disappearance timing from the Entry/Rebirth
  state flow, not from a renderer timer.

Exit checks:
- Spawn platform appears under the fighter with source-backed bounds.
- Fighter collision/ECB/hurtboxes remain visible while on it.
- Player can pass through/drop through according to decomp timing and
  invincibility rules.
- Source-owned intangibility/invincibility should tint the wireframe only after
  the decomp visual/effect path is checked; renderer-only invincibility flags are
  not authoritative.
- No raw DAT read during gameplay.

Related tracking:
- `docs/research/iasa-state-transition-parity-ledger.md` now tracks
  `Anim -> IASA -> Phys -> Coll` callback coverage for Entry/Rebirth, Pass,
  Cliff/Ledge, Damage, and follow-up state families.

### P0 - Knockback, DI, Tumble, And Damage Response

Goal: make attacks launch correctly so combat has gameplay meaning.

Decomp anchors:
- `src/melee/ft/fighter.c`: `Fighter_ProcessHit_8006D1EC`, hitlag assignment,
  `x195c_hitlag_frames`, knockback decay paths around `x8c_kb_vel`.
- `src/melee/ft/chara/ftCommon/ftCo_Damage.c`: `ftCo_Damage_Anim`,
  `ftCo_Damage_Phys`, `ftCo_Damage_Coll`, `ftCo_DamageFly_*`,
  `ftCo_DamageFlyRoll_*`, DI/knockback vector adjustment around `x8c_kb_vel`.
- `src/melee/ft/ftmotionstates.c`: common Damage ids `75-91`.
- `src/melee/ft/types.h`: `x8c_kb_vel`, `x18A4_knockbackMagnitude`,
  `x195c_hitlag_frames`, common-data damage/knockback fields.

Existing Rust/data:
- `resources/melee/extracted/plco_common_data.json` carries many knockback,
  hitlag, and damage response fields.
- Runtime already computes source hit confirms and staged source damage.
- Core has partial source damage, hitlag, hitstun, DownBound, and PassiveStand
  slices.
- `PlayerState` now carries source knockback velocity separately from self
  velocity, matching the `x8c_kb_vel` role used by `ftCo_Damage.c`.
- `PlCo.dat` fields for hitlag SDI/ASDI/DI are baked into `MeleeCommonData`:
  `x1A8`, `x1AC`, `sdi_min_stick_mag`, `sdi_stick_window`,
  `sdi_pos_scale`, and `x4BC`.

Implementation notes:
- Treat `x8c_kb_vel` / source knockback velocity as first-class rollback state,
  not a one-frame visual impulse.
- Confirm whether stomp issue is missing knockback vector application,
  transition selection, hitstun/tumble thresholding, or collision-lockout.
- DI/SDI/ASDI are wired from `ftCo_Damage_OnEveryHitlag`,
  `ftCo_Damage_OnExitHitlag`, and `ftCo_8008E5A4`: hitlag SDI moves source
  position when the l-stick magnitude and tap timers pass source gates, exit
  hitlag ASDI chooses c-stick over l-stick when its source magnitude passes,
  and DI rotates `x8c_kb_vel` by the source cross-product formula.
- Keep damage response fields as floats where the decomp uses floats.

Exit checks:
- Falcon down-air hit applies source damage, hitlag, hitstun, and a persistent
  knockback velocity. Initial source `x8c_kb_vel` split is implemented and
  covered by `source_damage_application_stores_decomp_kb_velocity_separately_from_self_velocity`.
- Victim enters the correct Damage/DamageFly/DamageFlyRoll/DownBound branch.
- DI changes the launch vector only through source-shaped math. Initial
  hitlag-exit DI/ASDI and per-hitlag SDI are covered by
  `source_damage_hitlag_applies_sdi_from_plco_window_like_ftco_damage_every_hitlag`
  and `source_damage_exit_hitlag_applies_asdi_and_di_to_source_kb_velocity`.
- Rollback checksum changes when confirmed late inputs alter DI/tech decisions.

### P1 - Tech, Passive, Wall/Ceiling/Floor Responses

Goal: finish the passive/tech family so knockback landing is actionable and
replay-divergence useful.

Decomp anchors:
- `src/melee/ft/chara/ftCommon/ftCo_DownAttack.c`: `ftCo_800986B0`.
- `src/melee/ft/chara/ftCommon/ftCo_DownBound.c`: `ftCo_80097D40`.
- `src/melee/ft/chara/ftCommon/ftCo_Passive.c`.
- `src/melee/ft/chara/ftCommon/ftCo_PassiveStand.c`.
- `src/melee/ft/chara/ftCommon/ftCo_PassiveWall.c`.
- `src/melee/ft/chara/ftCommon/ftCo_PassiveCeil.c`.
- `src/melee/ft/ftmotionstates.c`: Passive ids `199-204`.

Existing Rust/data:
- PassiveStand/Passive floor-contact slice exists for DamageFly/DownBound.
- Wall/ceiling passive callbacks are explicitly marked as remaining gaps in
  `resources/melee/README.md`.

Exit checks:
- Ground tech, missed tech, tech roll, wall tech, walljump tech, and ceiling
  tech branch from source input windows.
- Passive states are rollback-owned and render from baked source action data.

### P1 - Stage Contact, Ledge, And Platform Collision

Goal: remove the remaining stage-contact ambiguity so replay drift is not caused
by stage behavior.

Decomp anchors:
- `src/melee/mp/mpcoll.c`: floor/wall/ceiling/platform pass and callbacks.
- `src/melee/mp/mplib.c`: platform/ledge debug draw and map collision helpers.
- `src/melee/ft/ftcliffcommon.c`: ledge/cliff behavior.
- Stage-specific `gr*` files for callbacks and moving platform behavior.

Existing Rust/data:
- Competitive stage extraction carries ledges, dynamic collision ranges,
  map-head metadata, and callback metadata.
- First ledge behavior slice exists: airborne fighters near Battlefield ledges
  can enter source `CliffCatch`/252 using extracted ledges and Falcon source
  ledge snap offsets, then transition into `CliffWait`/253 at the source
  animation duration while cliff physics keeps the fighter pinned to the same
  ledge.
- Second ledge behavior slice exists: occupied ledges reject new grabs,
  `x2064_ledgeCooldown` is rollback-owned and ticks down, `CliffWait` uses
  extracted `PlCo.dat` fields `x488`/`x48C`/`x490`/`x494`/`ledge_cooldown`/
  `x49C`, neutral input arms the source `mv.co.cliff.x8`-style gate, and
  away/down release exits to Fall with source ledge cooldown.
- `StageProfile` still has compatibility projections for older code paths; the
  variable extracted collision model should become the gameplay path.

Exit checks:
- Battlefield ledges/cliffs, walls, ceilings, and soft platforms use extracted
  source-float collision.
- Platform drop-through matches source platform-pass callback timing.
- Moving platform stages can be represented through the same extracted dynamic
  collision/callback mechanism, even if Battlefield is the first match target.

### P1 - KO, Blast Zones, Stocks, Respawn

Goal: make a four-stock game complete.

Decomp anchors:
- `src/melee/gm/*` for stock/match flow.
- `src/melee/pl/*` for player slot, spawn, and stock ownership.
- `src/melee/ft/fighter.c`: death processing and fighter lifecycle hooks.
- Stage extracted `source_blast_zones` and `spawn_points`.

Exit checks:
- Leaving Battlefield blast zones causes KO/death state, stock decrement, and
  respawn when stocks remain.
- Final stock loss reaches a deterministic game-end state.
- Online peers agree on stock/game-end through rollback checksum.

### P2 - Shields, Grabs, Throws, Ledges, Items, Audio/VFX

Goal: broaden from testable combat into fuller Melee match behavior.

Notes:
- Shield lifecycle is partially implemented, including drain/regen and the
  project-specific shield-turn feature. The shield-turn feature must remain
  isolated/toggleable until full parity says it can be re-enabled.
- Grabs/throws/captured states are a major common-state family and should come
  after knockback/tech/stage contact.
- Audio/VFX should be render/event outputs from rollback-owned gameplay facts,
  not authoritative simulation inputs.

## Suggested Next Slice

Recommended next implementation slice:

1. Match phase/countdown as rollback-owned state.
2. Entry/spawn platform runtime actor/render wireframe from the existing baked
   `Fighter_804D6514` profile.
3. Source-backed platform pass/drop-through and invincibility timing during
   Entry/Rebirth.
4. Focused stomp knockback test: Falcon down-air hit must apply persistent
   source knockback velocity and enter the correct damage branch.

Reason: this gives the human-visible match opening, fixes the first obvious
spawn-platform gap, and then moves directly into the combat bug the current
playtest exposes.

## Update Protocol

- When a slice starts, add a dated note under `Active Slice`.
- When a decomp source anchor is confirmed, add exact file/function/line
  references here.
- When a slice lands, move its exit checks into `Completed Notes` with the
  commit hash and verification commands.
- If an apparent fix would require guessing, stop and record the missing source
  question instead of patching around it.

## Active Slice

- 2026-06-20: Normal fighter hit collision must preserve decomp endpoint
  ownership. `ftColl_8007AD18` updates each active hit capsule as
  `x58 = previous x4C`, then computes new `x4C` from live JObj world position
  with `lb_8000B1CC`; on the spawn frame it seeds `x58 = x4C`. The collision
  query `lbColl_8000805C` then tests `&hit->x58` to `&hit->x4C` against the
  victim hurt capsule's current `a_pos/b_pos`, with hurt positions refreshed
  lazily through `skip_update_pos`. Slippi witness: at source frame 2253 P1
  AttackAirN should put P2 into common damage action 79, while Rust currently
  keeps P2 in Dash because `source_collision_frame_from_frame` re-anchors both
  hit endpoints to the current root position. Immediate fix: consume the baked
  previous/current pose endpoints with previous/current source roots, seeding
  the previous root to current root only on the hitbox lifecycle spawn frame.
  Follow-up: promote persistent hurt-capsule endpoint cache if later replay
  witnesses show query-order or skip-update divergence.
- 2026-06-20: `x8c_kb_vel` is a persistent fighter velocity owned by the
  common `Fighter_procUpdate` movement pass, not by damage-state code alone.
  Decomp `src/melee/ft/fighter.c` shows the order: state `phys_cb` runs,
  then `x8c_kb_vel` is decayed or projected through `xF0_ground_kb_vel`, then
  ground acceleration is committed, then `cur_pos += selfVel + x8c_kb_vel`.
  Rust still has a damage-specific copy of this pass and normal grounded
  states ignore nonzero source knockback, causing the post-damage Landing /
  GuardOn replay barrier around source frame 2294. Immediate milestone:
  promote the common `x8c_kb_vel` apply/decay/project step into normal fighter
  integration, then later consolidate damage movement around the same decomp
  callback-order boundary instead of keeping duplicated mini update loops.
- 2026-06-12: P0/P1 bridge slice in progress. Completed rollback-owned match
  phase metadata, source-backed entry/RebirthWait platform wireframes,
  RebirthWait hard-down exit to Fall with source intangibility, first
  `CliffCatch`/252 ledge grab slice, `CliffCatch -> CliffWait`/253 lifecycle,
  occupied ledge rejection, source ledge cooldown, and source `CliffWait`
  timer/gated away-down release. Next: cliff climb/attack/escape/jump option
  states and deeper ledge collision callbacks from `ftcliffcommon.c` and
  `ftCo_Cliff*.c`.
- 2026-06-12: Battlefield left-lip replay drift exposed a foundation mismatch:
  Rust air-map wall collision had a derived whole-frame milli-space projection
  helper, while the decomp path is `mpColl_80043754` substep movement with
  `mpCollInterpolateECB`, `mpColl_80045B74_LeftWall`/right-wall edge hit
  collection, and `mpColl_80046224_LeftWall`/right-wall correction over
  connected map lines. Milestone follow-up: retire simplified map-collision
  helpers as their source counterparts are translated, and keep collision
  decisions in source-float `CollData` semantics before projecting to runtime
  milli state.
- 2026-06-12: Stage ledge extraction must be MapLine flag driven, not topology
  guessed. The decomp's ledge search checks `MapLine.lo_flags &
  LINE_FLAG_LEDGE` through `mpLib_80051BA8_Floor`; `StageLedge` is only a
  compatibility projection until the collision pass consumes compact
  `CollData`/MapColl data directly. Milestone follow-up: translate ledge search
  over the baked collision lines and use source `hi_flags`/`lo_flags` for all
  line eligibility decisions, including platform, empty, enabled, and hidden
  behavior.
- 2026-06-12: Runtime ledge detection must not rediscover grab points from
  arbitrary collision-line endpoints. The accepted runtime boundary is:
  extractor bakes the source-designated ledge set, then runtime validates those
  baked ledges against their referenced source collision lines before applying
  `mpColl_80044164`/`mpColl_800443C4` semantics. This keeps Battlefield soft
  platform sides from becoming grab points unless the source stage data itself
  marks them as ledges.
- 2026-06-12: Falcon ECB samples are suspect because they were reverse
  engineered for the old Rust runtime path instead of translated from the
  decomp pipeline. The source path is `Fighter_procUpdate -> ftAnim_8006EBA4`
  over live JObjs, then `Fighter_procMap -> mpColl_LoadECB_JObj ->
  lb_8000B1CC` before `mpColl_80043754` consumes current/previous ECB.
  Milestone follow-up: replace the baked fighter ECB artifact with compact data
  generated by a one-to-one translation of that runtime path, including action
  animation flags, JObj setup order, TransN/model-scale behavior, and the
  `mpColl_SetECBSource_*` bootstrap semantics. Do not repair this by scaling or
  offsetting per-action samples unless decomp code proves that exact operation.
- 2026-06-12: Root cause for the first persistent P1 JumpF Battlefield left-lip
  drift is the ECB animation source boundary, not the left ledge coordinate.
  Decomp evidence: `ftAnim_8006F3DC` returns `HSD_AObj.curr_frame` as a float,
  `HSD_AObjInterpretAnim` advances that float by `aobj->framerate`, and
  `Fighter_procMap -> mpColl_LoadECB_JObj -> mpColl_80043754` consumes the live
  float ECB. Mole currently looks up integer, milli-rounded baked ECB samples
  from rounded state age. At source frame 1759, the Slippi-expected root X
  corresponds to an effective Falcon JumpF ECB pose around frame 31.616 between
  the baked frame-31 and frame-32 samples, which the current integer sample path
  cannot represent. Milestone follow-up: promote ECB sampling to source-float
  pose semantics, ideally by baking compact source pose curves or generated
  live-JObj-equivalent ECB data keyed by float `cur_anim_frame`, then only
  project to milli for public/render state after collision.
- 2026-06-12: Collision line runtime flags are another global parity gap.
  Extracted stage JSON already includes `source_flags.runtime_flags_after_mpLibLoad`
  and enabled/empty state, but `StageCollisionLine` drops enabled/hidden/empty
  eligibility and line walkers assume referenced lines are usable. Battlefield
  line 17 is enabled and non-empty, so this is not the 1759 X root cause, but
  the compact Rust stage artifact should carry the decomp runtime flags before
  broader map-collision parity work continues.
- 2026-06-12: Follow-up proof on the same P1 JumpF Battlefield left-lip drift:
  replacing Falcon's ECB table lookup with generated live JObj/FObj-derived
  ECB data still clamps source frame 1759 to `-71.743`, while Slippi expects
  `-71.853`. Decomp comparison shows Rust's `mpColl_80043754`-style subdivision,
  ECB interpolation, left-wall edge sweeps, and `mpColl_80046224_LeftWall`
  vertex projection all produce the current Rust value for Battlefield line 17.
  The remaining source root is the animation evaluator boundary: Mole samples
  FObj curves directly at a requested float frame, while HSD advances mutable
  AObj/FObj state (`curr_frame`, `time`, `state`, `p0/p1/d0/d1`, first-play
  flags) through `HSD_AObjInterpretAnim -> HSD_FObjInterpretAnim` before
  `mpColl_LoadECB_JObj` reads live world JObj positions. Next milestone: replace
  direct float FObj sampling with a compact translated HSD AObj/FObj tick
  evaluator and feed collision from that live state. Do not patch this as a
  JumpF frame offset; the offset is only a symptom of the missing HSD state
  machine.
- 2026-06-12: Additional proof after the first live-pose Rust change: sampling
  the generated JObj ECB one animation tick later moves frame 1759 from
  `-71.743` to `-71.828`, leaving only 25 milli of X drift against Slippi's
  `-71.853`. Temporary projection tracing showed `mpColl_80046224_LeftWall`
  is using the connected Battlefield ledge vertex exactly as expected:
  `root_x = ledge_x - ECB right-edge x at y=0`. The remaining mismatch is the
  ECB right-edge local X, not wall correction, not line 17 endpoint data, and
  not Battlefield ledge extraction. Next milestone: derive gameplay ECB from
  live HSD JObj world positions at map-collision time, including mutable
  AObj/FObj state advancement and JObj matrix setup, instead of deriving it
  from a stateless float-frame curve request.
- 2026-06-12: Decomp call-order proof for the same drift: fighter creation
  installs `Fighter_8006A360` at GObj priority 1, `Fighter_procUpdate` at
  priority 4, and `Fighter_procMap` at priority 6. The priority-1 pass calls
  `ftAnim_8006EBA4 -> ftAnim_8006E9B4`, which mutates live HSD JObj/AObj/FObj
  state and updates `cur_anim_frame`; the priority-6 map pass then sets TopN
  translate and calls the collision callback, whose `mpColl_LoadECB_JObj` reads
  the already-mutated live JObj world positions through `lb_8000B1CC`. Mole's
  current single-step loop still reconstructs collision ECB from
  `motion_anim_frame_milli + rate` inside map collision. That is a global
  architecture mismatch: gameplay collision should consume a per-player live
  source pose state advanced by the animation pass, not synthesize a future
  pose at each collision read. Keep this fix action- and stage-agnostic.
- 2026-06-12: Ledge grab should be treated as a source map-collision query, not
  baked point metadata. Decomp `mpColl_800473CC` only checks ledges when falling,
  not already on an edge, with facing-gated side checks, then calls
  `mpColl_80044164`/`mpColl_800443C4`. Those call
  `mpLib_80051BA8_Floor`, which scans enabled non-empty `LINE_FLAG_LEDGE`
  floor lines and returns the source floor line id as `ledge_id`. Runtime ledge
  occupancy and `CliffCatch_Phys` must key on that line id, not on an extracted
  ordinal. Follow-up: finish translating the two `mpCheckMultiple` occlusion
  checks so ledge catch rejection comes from collision lines/joints rather than
  point-only tests.
- 2026-06-12: Falcon cliff placement consumes `x68C_transNPos` after HSD/ftAnim
  model-scale application. Raw extracted `CliffCatch`/`CliffWait` TransN samples
  are useful for artifact tests, but runtime cliff physics should compare
  against model-scaled live TransN, matching `ftCo_CliffCatch_Phys` consuming
  `fp->x68C_transNPos` after the animation pass.
- 2026-06-12: `CliffCatch` placement has the same live-pose timing issue as ECB:
  decomp `ftCliffCommon_80081370` calls `Fighter_ChangeMotionState`, immediately
  runs `ftAnim_8006EBA4`, then `ftCo_CliffCatch_Phys` consumes the already-updated
  `x68C_transNPos`. Runtime cliff physics must therefore sample the live
  `CliffCatch` pose one animation tick ahead of action age, while preserving
  raw extracted TransN samples as artifact truth. Replay source frames
  1762-1769 now match position/state/velocity/action id; frame 1759 JumpF wall
  drift remains the next live-HSD ECB/JObj timing milestone.
- 2026-06-12: Slippi replay inputs must be interpreted through the exported
  game-facing/UCF boundary. `tools/slippi_replay_to_inputs.cjs` applies UCF
  cardinals and emits adapter-owned `ucf_dashback_amendment` bits for
  UCF-tagged players; runtime diagnostics consume and count those bits. The
  current `Game_20260530T214929` replay tags both players as UCF, but frames
  1758-1769 have neutral processed stick and `ucf_dashback_amendment=false`, so
  UCF is not the source of the JumpF wall drift or the CliffCatch placement
  window.
- 2026-06-12: Post-ledge replay barrier is now past `CliffWait` release.
  Bounded trace for P1 matches `CliffCatch`/`CliffWait`, the down release to
  `Fall` at source frame 1811, and the double jump into `JumpAerialF` through
  source frame 1823. The next new divergence is source frame 1824: P1 remains in
  `JumpAerialF`/27 with matching self velocities (`air_x=0.75675`,
  `vel_y=1.49`) and matching Y, but Rust applies a left-wall/map-collision
  horizontal correction to `x=-68.599` while Slippi expects `x=-67.214`. This is
  the next root target, and it belongs to the same decomp path as the earlier
  left-lip drift: `mpColl_80043754` subdivision/interpolated ECB,
  `mpColl_80045B74_LeftWall`, and `mpColl_80046224_LeftWall` projection over
  Battlefield line 17 and connected ledge geometry. Do not treat it as
  ledge-grab logic or UCF.
- 2026-06-12: Rechecked the earlier P1 JumpF source-frame-1759 X mismatch
  before moving on: the remaining error is still the live animation pose
  boundary feeding `mpColl_LoadECB_JObj`, not Battlefield metadata. Verified
  against decomp: Battlefield line 17 scale/endpoint data is correct,
  `mpColl_80046224_LeftWall` uses the connected ledge vertex path as expected,
  `mpColl_LoadECB_JObj` width/height clamps match, `HSD_MtxSRT` parent-scale
  compensation matches generated Rust, JumpF action flags are ordinary
  `0x00000002`, Falcon `model_scaling` is `0.97` and would move the ECB edge in
  the wrong direction, and `splGetHelmite` matches the generated formula. The
  current Rust collision sample lands at live pose 33.0 with local right-edge X
  about 3.428 at the Battlefield ledge height; Slippi requires about 3.453.
  Do not patch this with a JumpF offset. The parity fix is a compact translated
  HSD AObj/FObj/JObj live-pose state consumed by collision at the same source
  priority boundary as `Fighter_procMap`.
- 2026-06-12: P2 EscapeF on Battlefield right platform isolates a separate
  grounded map-collision architecture gap. Decomp route is
  `ftCo_Escape_Coll -> ft_800827A0 -> mpColl_8004B2DC ->
  mpColl_LoadECB_inline(coll, 5) -> mpColl_80043754(mpColl_8004ACE4, coll, 2)`.
  `mpColl_LoadECB_JObj` with flags `5` forces `desired_ecb.bottom.y = 0`, so
  grounded floor-edge clamps must not consume displayed/sample bottom Y.
  However `mpColl_80043754` clamps with the persistent interpolated
  `coll->ecb`, not just the newly baked desired ECB. Rust currently has no
  compact persistent `CollData.ecb/desired_ecb/prev_ecb` runtime state, so it
  clamps every right-platform EscapeF edge frame to simplified root X `20000`
  while Slippi shows source preserving/substepping the collision ECB through
  `19901`, `19959`, `19938`, `19931`. Do not solve this with platform or
  replay constants; promote the compact source `CollData` ECB state and
  translate the `mpColl_80043754` interpolation/load flags.

## Completed Notes

- 2026-06-12: Friend Connect usability milestone completed separately in commit
  `978169a` (`Add Friend Connect lobby slots`).
- 2026-06-12: Match phase, entry/RebirthWait platform rendering, RebirthWait
  hard-down release, and first Battlefield ledge `CliffCatch` slice verified with
  targeted `mole tests run` core/runtime checks.
- 2026-06-12: `CliffCatch_Anim` lifecycle translated narrowly: source
  animation duration enters `CliffWait`/253 and cliff physics pins to the
  extracted Battlefield ledge each tick. Remaining cliff wait timers require
  promoting the source common-data fields before implementation.
- 2026-06-12: `PlCo.dat` cliff common-data fields promoted from the common
  attributes block: `x480`, `x488`, `x48C`, `x490`, `x494`,
  `ledge_cooldown`, and `x49C`. `CliffWait` now initializes source timer and
  hurt intangibility from those fields, decrements a rollback-owned timer, arms
  the source stick gate after neutral/no-option input, and releases to Fall
  with ledge cooldown on away/down input.
- 2026-06-12: Damage launch now keeps decomp-style knockback velocity separate
  from self velocity, so source damage floor collision and air drift can reason
  from `x8c_kb_vel` instead of a one-frame public velocity projection. Verified
  with `cargo fmt --check` and focused `mole tests run -p mole_core` damage and
  cliff tests.
- 2026-06-12: Promoted PlCo hitlag-control fields for SDI, ASDI, and DI, then
  translated the first `ftCo_Damage.c` callbacks: `OnEveryHitlag` SDI position
  movement, `OnExitHitlag` ASDI position movement, DI rotation of `x8c_kb_vel`,
  and held-L/R knockback scaling. Verified with `cargo fmt --check`, focused
  `mole tests run -p mole_core`, and full `cargo test -p mole_core`.
- 2026-06-12: CliffCatch live-pose placement milestone: changed runtime cliff
  physics to consume the decomp-equivalent live `CliffCatch` TransN pose after
  `ftAnim_8006EBA4`, kept raw TransN artifact checks separate from scaled
  runtime placement checks, and verified the Slippi window at source frames
  1762-1769 reaches zero position/state/velocity/action deltas.
- 2026-06-12: Native special-move data plumbing milestone: added a generic
  source special action binding surface keyed by decomp runtime action state id
  and PlCaAJ action-table id, then populated it with Falcon's `ftCa_Init.c`
  special rows 347-363. Runtime special durations now come from this table
  instead of `99`-frame placeholders, and the Falcon import/generator coverage
  report carries the special binding table so future character import work has
  an explicit decomp parity hook.
- 2026-06-12: Falcon special callbacks require `ftData.ext_attr`, not the
  common `ftCo_DatAttrs` profile. Decomp `PUSH_ATTRS` copies
  `fp->ft_data->ext_attr` into `fp->dat_attrs`, and Falcon specials read
  `ftCaptain_DatAttrs` through that path. The extractor now writes
  `captain_falcon_special_attrs.json`, and core exposes typed
  `CaptainSpecialAttrs` on `FighterProfile` so native `SpecialN/S/Hi/Lw`
  translation can consume the baked source constants directly.
- 2026-06-12: Runtime command-var cleanup must be keyed by source callback
  semantics, not by the previous common-action whitelist. Falcon `SpecialHi`
  entry sets `cmd_vars[1] = ftCaptain_DatAttrs.specialhi_unk2`; generic cleanup
  was erasing it because specials were outside the old whitelist. Preserve
  command vars for source-bound special states and let each translated special
  callback clear its own vars where the decomp does.
- 2026-06-12: Falcon/Battlefield parity work should promote every touched
  `p_ftCommonData` field into typed runtime common data before translating the
  caller. `ftCa_SpecialHi_Phys` forced this for `x1FC` air-speed clamp friction
  and `x258` SpecialHi stick threshold; both now come through the PlCo extractor,
  `MeleeCommonData`, rollback hash, and source-field ledger. Keep applying this
  pattern instead of threading literals through action-specific code.
- 2026-06-12: `ftCa_SpecialHi_Phys` is not generic air drift. The decomp seeds
  `self_vel` from `mv.ca.specialhi.vel`, calls `ftCommon_8007D050` and
  `ftCommon_8007D3A8`, stores back to `mv.ca.specialhi.vel`, then lets
  `ft_80085134` overwrite self velocity from live TransN before adding stored
  SpecialHi velocity back in. Runtime `SpecialAirHi` now follows that sequence
  and skips the generic fall-gravity block for that tick.
- 2026-06-13: Source capsule/world-sample transforms must reset the full live
  TransN root before re-anchoring to runtime fighter position. Decomp
  `ftColl_8007AD18` and `mpColl_LoadECB_JObj` consume `lb_8000B1CC` JObj world
  positions; Rust previously subtracted TransN Z only, which made CliffCatch
  source hurt capsules render and collide roughly one cliff TransN Y below the
  player. Keep this as a global JObj-source-sample rule for hitboxes, hurtboxes,
  ECB extraction/runtime collision, and future imported character data.
- 2026-06-13: Treat milli-space divergences as symptoms unless proven
  otherwise. Project-wide parity architecture should keep source movement,
  animation, map-collision, ECB, hit/hurt capsule, and projection math in
  decomp-equivalent `f32` state through the same operation order as Melee, then
  convert to milli only at explicit runtime/public-state boundaries. Do not
  correct replay drift by nudging milli positions, rounding earlier, or storing
  milli values as source truth.
- 2026-06-13: Float-boundary audit found one concrete violation in
  `World::set_player_state_for_diagnostic`: when explicit `source_position`
  disagreed with public milli `position`, diagnostics treated milli as source
  truth and converted back to float. Runtime now preserves explicitly supplied
  source-float position and projects public milli state from it, while keeping
  the old convenience path for diagnostics that only edit public `position`.
  Verified with the new `diagnostic_state_import_preserves_explicit_source_float_position`
  guard. This did not move the replay barrier: P1 still first persistently
  diverges at source frame 1823 in `JumpAerialF` with X delta -300 milli and
  matching Y/self velocity. Next work remains a direct translation of the
  decomp path feeding and consuming `CollData`: live HSD AObj/FObj/JObj pose
  state into `mpColl_LoadECB_JObj`, then exact `mpColl_80043754`,
  `mpColl_80045B74_LeftWall`, and `mpColl_80046224_LeftWall` behavior.
- 2026-06-18: Frame-data pipeline milestone: `fighter.set_cmd_var` is now
  preserved through compact runtime export and the baked
  `source_frame_capsules.bin` sidecar as source-frame, cmd-var, value,
  word-offset, and raw-word metadata. This is data plumbing only; simulator
  consumption must be translated as a separate callback/order-of-operation
  milestone from decomp sources such as `ftCa_SpecialHi_IASA`. Root-motion and
  Falcon ECB generated Rust now emit `f32::from_bits(...)` for source float
  payloads so generated artifacts preserve exact single-precision values
  instead of relying on decimal parser round-trips.
- 2026-06-19: Source `CollData.floor.index` must be owned by source map
  collision callbacks, not by Slippi `lastGroundId` or simplified floor
  support. Decomp `ft_80082708` is the SpecialAttackGround ground-to-air path:
  it calls `mpColl_8004B108`, which loads ECB and substeps through
  `mpColl_80043754` into `mpColl_8004ACE4`; with a valid live floor line,
  `mpColl_800488F4` can snap to that floor with no vertical cutoff. Therefore
  replay `lastGroundId = 1` at source frame 1860 is diagnostic/stale data, not
  permission to seed `coll->floor.index = 1`. Runtime now routes only translated
  Falcon ground special states that actually use `ft_80082708` through the
  source wrapper; generic grounded states keep the old support path until their
  exact source map callbacks are translated. Keep expanding this by source
  callback ownership, not by globally replacing support checks.
- 2026-06-19: Source floor-line promotion must also verify the current source
  position is still on that surface's Y. Slippi `lastGroundId` may remain `1`
  during Falcon grounded SpecialHi while live TransN has already lifted the
  collision position above Battlefield's left platform. Treat that field as
  replay/export context unless the translated map-collision callback has proven
  the line remains live for `coll->floor.index`.
- 2026-06-19: LandingFallSpecial entry preserves impact `self_vel.y` for the
  transition frame. Decomp `ftCommon_8007D6A4` seeds ground state and `gr_vel`
  but does not clear `fp->self_vel.y`; the next grounded physics step clears
  vertical self velocity through `ftCommon_ApplyGroundMovement` on a horizontal
  floor. Runtime must not zero source vertical velocity inside
  `finish_source_floor_contact` or `enter_landing_fall_special`.
- 2026-06-19: Replay scan's first remaining scenario at source frame 1847 is a
  velocity-only diagnostic issue while position/state remain exact through
  frame 1912. The SpecialHi horizontal movement is exposed by Slippi in
  `air_x`, but the scan currently classifies grounded `SpecialHi` using
  `ground_x`. Do not alter simulation to satisfy this; either translate the
  diagnostic's source velocity selection or use the next position/state drift
  as the gameplay barrier.
- 2026-06-19: Source hitlag ownership milestone: normal fighter hit collision
  must apply both victim damage hitlag and attacker deal-damage hitlag from the
  decomp fields, not from a replay convenience rule. Decomp
  `ftColl_80076ED8` writes the attacker's `dmg.x1914` from `getEnvDmg`, then
  `Fighter_ProcessHit_8006D1EC` converts that through
  `ftCommon_CalcHitlag` into `dmg.x195c_hitlag_frames`. Runtime source
  collision now uses accepted source damage stages to freeze the attacker with
  the same integer damage boundary, while victim damage response still owns
  SDI/ASDI/DI callbacks. Keep this distinction: the Rust `hitlag_frames` timer
  is generic freeze state, but `source_allow_sdi` represents translated
  damage-state callbacks until the broader Fighter callback table is promoted.
  Also corrected hitlag tick order to match fighter process priority:
  `Fighter_8006A1BC` decrements/clears hitlag at priority 0 before
  `Fighter_procUpdate` priority 4, so the frame that clears hitlag resumes
  normal update instead of losing one extra tick. Replay witness: P1
  `AttackAirN` source frames 2252-2265 now match position/state/velocity after
  the first NAir hit. Next target is P2 victim damage response beginning at
  source frame 2253/2258: source action 79 stays correct, but facing,
  knockback/velocity decay, and damage-state callback ownership still need a
  bottom-up `Fighter_ProcessHit_8006D1EC` / `ftCo_Damage.c` translation pass.
- 2026-06-19: Damage-entry facing milestone: normal fighter damage must consume
  the decomp `ftColl_8007A06C -> DmgResult.dir` / `dmg.facing_dir_1` boundary,
  not recompute an independent launch direction from public milli positions.
  Decomp fighter-hit behavior is strict: if victim `cur_pos.x` is greater than
  attacker `cur_pos.x`, `dir = -1`, otherwise `dir = +1`; then
  `ftCo_8008DCE0` sets `fp->facing_dir = fp->dmg.facing_dir_1` and stores
  `x8c_kb_vel.x = -scaled_kb * cos(kb_angle) * fp->facing_dir` with no
  `abs(cos)` shortcut. Runtime damage entry now derives this from source-float
  positions, updates the victim facing before the damage motion entry, and
  preserves the raw cosine sign. Verified with the new
  `source_damage_entry_uses_ftcoll_dir_and_raw_angle_cosine` guard plus the
  runtime NAir/hitlag/damage-lockout checks. Replay witness: P2 source frame
  2253 now matches expected facing `-1` and still has exact damage-entry
  position; the next root target is post-hitlag damage movement around source
  frame 2257/2258, likely inside `ftCo_Damage_Phys` / `x8c_kb_vel` decay and
  velocity-reporting semantics rather than damage-facing setup.
- 2026-06-19: Falcon special extraction milestone: decomp-only Falcon special
  states 355, 356, 358, 360, 361, 362, and 363 are now promoted through the
  generic source-special binding table into compact runtime source export, even
  when no Rust `MotionState` enum variant exists yet. The corrected source table
  rows are `SpecialHiCatch` 309/16f, `SpecialHiThrow` 310/60f,
  `SpecialLwEnd` 312/30f, `SpecialAirLwEnd` 314/45f,
  `SpecialAirLwEndAir` 316/29f, `SpecialLwEndAir` 315/30f, and the second
  `SpecialHiThrow` 317/60f. Keep this as a character-agnostic importer pattern:
  character source tables may contain source-only action states, and runtime
  export must bake them from decomp metadata instead of requiring a public
  motion-state enum mapping first.
- 2026-06-19: HSD FObj declared `length` is a source parser boundary, not a
  byte-slice boundary. Decomp `FObjLoadData` and `FObjLoadWait` check
  `ad - ad_head >= length` before starting the next data/wait token, while
  `parseFloat`, `parsePackInfo`, and `parseWait` consume the bytes needed after
  the token has started. Python extraction, `mole_frame_data`, and generated
  Falcon live-JObj ECB Rust now preserve declared length separately from the
  compact reachable payload bytes. Apply the same rule to all future character
  and special importers; otherwise boundary-frame JObj/ECB samples can drift by
  small float/milli amounts while looking like a collision or replay problem.
- 2026-06-19: Compact runtime source sidecars must preserve generic decoded
  action-script events, not just cmd-var events. The CLI already decodes
  `fighter.set_cmd_var`, `fighter.set_jab_combo`, `fighter.set_jab_rapid`, and
  `fighter.set_throw_flag` from source scripts; `mole_frame_data` now parses
  those compact procedures and `source_frame_capsules.bin` format `MSFC0006`
  stores them as typed script events while deriving legacy `cmd_var_events` for
  existing consumers. This keeps native runtime callbacks able to consume baked
  source event timing for Falcon specials, jab followups, rapid-jab gates, and
  future throw/capture states without falling back to JSON manifests or
  hardcoded frame constants.
- 2026-06-20: Source fighter-hit collision ordering milestone: the frame-2253
  Captain Falcon NAir divergence required source-order previous/current hurt
  endpoint semantics, but only for the first source frame of a hitbox lifecycle.
  Decomp evidence: `ftColl_80078C70` walks fighter GObjs and hit/hurt parts in
  source order, `ftColl_80076ED8` can append normal damage logs, and
  `ftColl_8007A06C` later selects damage by source damage-result fields rather
  than by a replay-specific override. Runtime now carries each hurt capsule's
  previous capsule from the baked source pose and chooses it only when the hit
  owner's source order is earlier than the hurt owner and the hitbox lifecycle
  id starts on the current source frame. This preserves the frame-2253 first
  NAir confirm and prevents the stale damage-root false confirm at frame 2272
  from an already-active second NAir hit. Replay witness: P2 now remains action
  79 at source frame 2272 and transitions to action 80 at 2273 with matching
  knockback. Next persistent scan barrier is source frame 2299: P1 expected
  action 213 vs actual 212 and P2 expected action 226 vs actual GuardOn/178.
  Keep the next pass bottom-up through the relevant decomp action/callback and
  source `CollData` ownership; do not repair it with replay constants.
- 2026-06-20: Post-milestone cleanup/audit: the lifecycle-gated previous-hurt
  selection is branch-only inside the existing hit/hurt loops and adds no
  per-pair allocation or raw source parsing. Runtime capsule construction still
  preserves source floats through `SourceVec2` and 3D `f32` capsule math, with
  milli conversion limited to render/debug/public boundaries. Extraction
  quality dependency to keep visible: hitbox lifecycle ids must remain compact
  baked artifacts with their source spawn frame in the high bits, and damage/map
  collision should stop deriving previous/current collision roots from render
  snapshots once persistent source `CollData` state is fully promoted.
- 2026-06-20: Ground-grab/source-capsule axis milestone: decomp
  `ftColl_80078A2C` confirms grab hitboxes against the victim's current hurt
  capsules, not the previous-hurt endpoint rule used for normal damage
  lifecycle ordering. Runtime now keeps that distinction. The remaining frame
  2299 miss was a shared live-JObj projection bug: `ftAction_8007121C` stores
  hitbox offsets as decomp `b_offset` fields and `ftColl_8007AD18` samples them
  through `lb_8000B1CC`, so baked live source capsules must stay in source-world
  axes until the runtime gameplay projection. The final projection uses live
  source X as gameplay X after TopN setup; render/debug flattening cannot feed
  collision. Replay witness: the Slippi source-frame-2299 grab barrier now
  passes with P1 entering `CatchPull`/213 and P2 entering `CapturePulledLw`/226.
  The next first persistent divergence is source frame 2301: P2 is already in
  `CaptureWaitLw`/227 on both sides, but Rust is 606 milli too far right. Treat
  that as a grounded capture-position/callback-order translation issue, not a
  hitbox, platform, or replay correction.
- 2026-06-20: Hitlag/link ownership root cause for the current ThrowHi barrier:
  decomp `ftColl_80076ED8` writes attacker `dmg.x1914` from a fighter hit before
  victim damage resolution, and `Fighter_ProcessHit_8006D1EC` converts that
  value through `ftCommon_CalcHitlag` into `dmg.x195c_hitlag_frames`.
  `Fighter_UnkRecursiveFunc_8006D044` then sets `x2219_b5` recursively through
  `x1A5C` without copying the timer; `Fighter_8006A1BC` clears the linked bit
  when the timer owner exits hitlag. Runtime must therefore keep the decomp
  split: held-victim throw hit confirms before `throw_flags_b3` are not generic
  victim damage, but they still produce the attacker's deal-damage hitlag and
  linked `x2219_b5` freeze state. Do not model this as replay delay, victim
  damage, or a copied victim hitlag timer.
- 2026-06-20: Hit capsule lifecycle/live-JObj split milestone. Decomp
  `ftAction_8007121C` owns the enabled hitbox script state and calls
  `ftColl_8007AD18` on spawn; `ftColl_8007AD18` then refreshes enabled hit
  endpoints from `lb_8000B1CC`/live JObj world matrices. Runtime must not let
  the post-animation live pose frame create hitboxes that the current action
  script frame has not spawned yet. Normal `ftColl_80078C70` hitboxes now select
  active hitbox metadata/lifecycle from the current script frame and only borrow
  matching live endpoint geometry from the collision pose frame. Catch hitboxes
  stay on the `ftColl_80078A2C` current-frame path because grab collision is a
  separate decomp pass over `HitElement_Catch`. Replay witness: the first
  persistent scan barrier moved from source frame 2252 to 2319; P2 now remains
  Dash on 2252 and enters damage 79 on 2253 with matching NAir knockback.
  Targeted tests cover the frame-6 AttackAirN no-spawn case, catch frame
  semantics, NAir frame 2253, and ThrowHi frame 2302/2312 regressions. Next
  barrier is source frame 2319: P2 reaches damage state 90 but throw launch
  position/velocity diverges, so continue through `ftColl_8007A06C`,
  `Fighter_ProcessHit_8006D1EC`, throw flags, and captured/held victim release
  state rather than hit capsule activation.
- 2026-06-20: Extraction quality audit from the hit lifecycle split. The compact
  source capsule artifact still stores baked endpoints, not the original
  hitbox offsets plus per-frame live joint matrices. The current runtime can
  preserve script-frame metadata and use matching live endpoint geometry, but
  the fully general decomp translation should promote hitbox offset/bone/scale
  source fields so live `lb_8000B1CC` endpoint evaluation can happen from the
  active script hitbox state at the current animation time without borrowing
  future-frame hitbox attributes. Keep this compact and baked; do not fall back
  to raw ISO/decomp at runtime.
- Current resumed milestone: PassiveStandF/B defensive physics now follows the
  decomp path `ftCo_PassiveStand_Phys -> ft_80084FA8 -> ft_80085030` by
  consuming source-action `TransN` root deltas for source action keys instead of
  routing stand-tech actions through plain `Passive` friction. `Passive` itself
  remains on `ft_80084F3C`. Grounded defensive collision now preserves a valid
  persistent `CollData.floor.index` into `ft_800827A0`/`ft_80082708` before
  calling `mpColl_8004B2DC`/`mpColl_8004B108`; do not pre-clear that line just
  because the current root has moved past a floor endpoint, because decomp
  `inline2` uses the prior floor line for the flags=2 edge clamp.
- Current verification: focused tests pass for PassiveStandB TransN physics,
  source knockback composition, PassiveStandB `ft_80084104` edge clamp, and the
  adjacent Escape/Dash source map-collision guards. Replay scan still reports
  older persistent roots in this worktree, including P1 already dead/rebirth
  where Slippi is alive near the left ledge and P2 hit/damage misses downstream
  of that positional divergence. Treat the next replay pass as a root-cause
  ledge/death/collision-order investigation before changing hit application
  behavior; otherwise hit fixes will be correcting bad geometry rather than
  translating `ftColl`/damage code.
## 2026-06-21 Playtest and visual replay handoff checkpoint

- Added Rust devtool Slippi Replay visual frames backed by the same runtime/core path used by SDL replay viewing:
  `slippi_visual_replay_inputs_from_match_start -> step_world_with_source_collisions -> RenderFrame`.
  The devtool now keeps visual frames aligned with the loaded trace window and can step/play the selected trace row at 60 Hz.
- Verified focused devtool tests:
  `cargo run -p mole_cli -- tests run -p mole_devtool slippi_replay_loads_explicit_artifact_player_and_window app_reload_controls_drive_trace_and_replay_surfaces`.
- Verified `cargo check -p mole_devtool` and `cargo run -p mole_runtime -- --frames 120`.
- Verified SDL visual replay screenshot path with launcher-equivalent SDL3 PATH:
  `debug/handoff/slippi-visual-source760.png` is source frame 760 from
  `debug/slippi/Game_20260530T214929.full.inputs.json`.
- Current full replay is not parity-complete. `replay scan` reaches 5313 frames but reports 121 scenarios. First persistent non-realigned divergence is P2 source frame 19 / core frame 142: expected Dash action 20, Rust is Fall action 29, with same position but wrong grounded/state/velocity path. Treat this as the next root-cause target before later hit/special divergences.

## 2026-06-21 Native SDL replay viewer correction

- The Slippi Replay tab should not own a parallel replay renderer. The durable path is:
  `Slippi input export -> native Rust engine step -> mole_runtime RenderFrame/RenderScene -> SDL renderer`.
- The devtool `Play From Start` button now launches that native SDL runtime path directly. If `target/release/mole_runtime.exe` exists it is used directly for low-latency startup; cargo release build remains a fallback for fresh checkouts.
- The stop point is the first Slippi/Rust engine divergence reported by the sequential match-start scanner, meaning Slippi post-frame expected fighter state/position/velocity versus the Rust engine after stepping the same exported Slippi inputs. This is not a rollback divergence.
- Superseded stop-point note: the old button stop point at core frame 75 / Slippi source frame -48 was a pre-match-start/platform barrier and is no longer the current milestone.

## 2026-06-21 visual replay parity milestone

- The native SDL replay path now starts quickly from the devtool and runs until the first sequential Slippi/Rust scan divergence instead of holding at a preselected frame.
- The match-start/entry-platform barrier, early jump-squat/landing barrier, ledge catch path, and Falcon SpecialHi visual root-motion detachment were debugged from the decomp side rather than patched in the renderer.
- Source hurtbox rendering now follows the decomp draw path: live JObj/source capsule endpoints project with source X/Y, while Z remains depth. The accidental render flattening that used source Z as screen X made grounded/common poses look like falling poses; that has been removed.
- The runtime still treats the blue Dolphin Mole sprite overlay as a visual/debug layer. Collision and parity position should be read from source hurtbox/capsule geometry and runtime state.
- Verification for the current render/capsule milestone:
  - `cargo run -p mole_cli -- tests run -p mole_runtime render_scene_source_hurtboxes_project_decomp_x_axis_for_standing_pose render_scene_source_hurtboxes_prefer_active_common_pose_over_stale_common_action_id root_motion_capsules_render_after_melee_transn_reset special_air_hi_capsules_render_after_melee_transn_reset turn_run_capsules_render_after_transn_reset_at_floor_edge cliffcatch_source_capsules_render_after_full_transn_reset frame_debug_log_reports_canonical_source_hit_confirms`
  - `cargo fmt --check`
  - `cargo build -p mole_runtime`
  - `cargo build --release -p mole_runtime --features "sdl wup"`
- Next replay barrier should be remeasured from the devtool after this milestone. Do not carry forward older frame-75, frame-2253, or frame-2319 barriers as current unless the fresh scan reproduces them.

## 2026-06-24 shield parity checkpoint

- Decomp shield proof currently used:
  `ftCo_800921DC` initializes GuardOn shield state with `mv.co.guard.x8 = 10.0`
  and `mv.co.guard.x4 = 0.0`, then immediately calls the shield visual update
  path. `ftCo_80091BC4` smooths shield aim angle and magnitude from the cleaned
  main-stick axes using common-data scalar `p_ftCommonData->x44C`.
- Runtime now carries decomp-shaped equivalents for `mv.co.guard.x8` and
  `mv.co.guard.x4` as `source_shield_aim_angle_degrees` and
  `source_shield_aim_magnitude`. Guard entry resets them to 10/0 and applies the
  same frame update where Rust already has the post-input facts, then sustained
  guard updates them once per tick.
- Extraction now preserves `CommonAttributes.x44C` as source float
  `shield_aim_smoothing`; the compact extracted PlCo artifact reports `0.5`,
  and `MeleeCommonData::PROVISIONAL` now matches that extracted value. The value
  sheets expose both common `shield_aim_smoothing` and character
  `initial_shield_size`, so shield formula inputs are visible to CLI/devtool
  inspection without mutating source artifacts.
- Verification at this checkpoint: PlCo common-data extraction tests, value-sheet
  tests, targeted core shield aim tests, targeted checksum test, and
  `cargo check -p mole_core -p mole_runtime -p mole_cli`.
- Remaining decomp-backed gap, not patched yet: the shield collision/render
  center should come from the guard shield animation/JObj path used by
  `ftData_80085E50(fp, 38)`, `ftAnim_80070710`, and `lbColl_80007BCC`. Runtime
  still needs compact extraction/evaluation of that guard shield animation path;
  do not replace it with a hand-authored polar offset. Yoshi remains a separate
  source path through `ftYs_Guard`/`ftYs_Init_8012BECC` and should not be folded
  into normal shield behavior without that proof.

## 2026-06-24 shield parity follow-up

- Implemented the decomp ordinary shield-hit health path from
  `ftcoll.c`/`Fighter_ProcessHit_8006D1EC`: shield confirms now accumulate
  `max(hitbox.damage + hitbox.shield_damage, 0)` as shield damage, then subtract
  `x284 * (damage * (1 - (lightshield * (x2E0 - x2DC) + x2DC))) + x288`.
  Shield hurtbox confirms remain excluded from normal percent/knockback routing.
- Runtime shield collision center now updates from the source Guard action 38
  pose at `mv.co.guard.x8` when `mv.co.guard.x4 != 0`, using the compact
  runtime figatree evaluator rather than a hand-authored polar offset.
- Runtime shield size now follows `ftCo_Guard.c` inlineB0:
  non-Yoshi shields scale by health/lightshield/common data; Yoshi source kind
  hard-returns `initial_shield_size`. The render snapshot carries this as a
  compact `profile_is_yoshi` bit derived from the source profile reference.
- Verified focused shield contracts:
  - held shield drain via `x278`/`x2EC`/`x2F0`
  - inactive shield regen via `x27C`
  - aim smoothing via `x44C`
  - shield-hit drain via `x284`/`x288`/`x2DC`/`x2E0`
  - Yoshi shield-size branch
  - Raptor Boost source shield collision at the replay-2706 fixture
- Shield-break/Furafura parity follow-up:
  - Decomp proof used:
    `ftCo_80098B20` enters action 205 (`ShieldBreakFly`), zeroes
    `self_vel.x`, sets `self_vel.y = co_attrs.shield_break_initial_velocity`,
    calls `ftCommon_8007EBAC(fp, 24, 0U)`, and routes collision through
    `ft_80082C74(gobj, ftCo_80098E3C)`. `ftCo_80098E3C` chooses
    `ShieldBreakDownU/D` from `ftCo_80097570`, which samples live HipN matrix
    orientation. `ftCo_80099010` enters Furafura, resets shield health to
    `p_ftCommonData->x280`, and initializes `grab_timer` from
    `max(x2F8 - percent, 0) + x2FC`; `Furafura_Anim` resets shield health each
    frame, subtracts `x300`, and applies `ftCommon_GrabMash(fp, x304)`.
  - Runtime now carries source-shaped ShieldBreakFly/Fall/DownU/DownD/StandU/
    StandD/Furafura states, removes the earlier duplicate ShieldBreakFly gravity
    application, uses fall-like air collision for ShieldBreakFly/Fall, routes
    floor contact into ShieldBreakDown, and implements Furafura shield reset,
    timer decrement, and mash decrement from the extracted PlCo floats.
  - Extracted common-data/value-sheet checkpoint: `x2F8 = 400.0`,
    `x2FC = 90.0`, `x300 = 1.0`, `x304 = 3.0`, preserved as source floats in
    PlCo extraction, Rust common data, value sheets, and runtime state hashing.
  - Remaining shield extraction gaps, not patched around: action ids 205-210
    currently have placeholder/unknown common action bindings in the compact
    Falcon action table, so animation-end transitions for ShieldBreakFly/Fall/
    Down/Stand only run when tests or future extraction provide source action
    lengths. The DownU/DownD choice also needs live HipN/JObj pose evaluation at
    runtime; tests can inject `SourceDownBoundPose`, but full parity requires
    compact extraction/evaluation of that source pose path.
  - Verification at this checkpoint:
    `cargo check -q -p mole_core -p mole_runtime`;
    `cargo run -q -p mole_cli -- tests run -p mole_core
    source_shield_break_furafura_initializes_timer_like_ftco_80099010
    source_furafura_anim_resets_shield_and_exits_when_timer_expires
    source_shieldbreakfly_lands_into_down_like_ft_80082c74_callback
    source_shield_break_routes_to_shieldbreakfly_like_ftco_80098b20
    source_powershield_b2_timer_skips_normal_shield_damage_like_ftcoll_8007a06c
    input_threshold_defaults_match_extracted_plco_common_data
    falcon_like_profile_keeps_ftco_dat_attr_floats_as_source_f32
    extracted_plco_common_data_reads_big_endian_values_from_source_offsets`;
    `python -m pytest tests\test_value_sheets.py
    tests\test_extract_melee_resources.py::test_extract_common_data_from_plco_uses_ftload_common_attribute_pointer
    tests\test_state_graph_viewer.py::test_parity_ledger_overview_summarizes_value_sheets_and_grounded_coverage`.

## 2026-06-24 Raptor Boost shield/stale routing

- Decomp proof:
  - `ftCa_SpecialS_Enter`/air variant install `fp->hurtbox_detect_cb =
    ftCa_SpecialS_OnDetect`.
  - `ftCa_SpecialS_OnDetect` checks `cmd_vars[0]` and the detected
    fighter/item object, then routes `SpecialSStart` to `SpecialS` through
    `onDetectGround`/`onDetectAir`.
  - Stale-table recording is in damage finalization helpers such as
    `ftColl_8007BE3C`/`ftColl_8007891C`, not in the raw hurtbox-detect
    callback path.
- Runtime fix:
  - Source hit confirms still feed special hurtbox-detect transitions first.
  - Confirms whose attacker action routes through a source special
    hurtbox-detect callback are excluded from generic damage/stale routing.
  - Normal damage stale insertion now consumes the post-shield/post-damage-gate
    confirm list, while held-victim throw-hit stale behavior remains covered.
- Replay evidence:
  - Source 2583 Raptor Boost detect hitbox is damage 0 / element 11 and should
    transition P1 without producing a `SourceDamageStage`.
  - Source 2586 Raptor Boost body hit should enter damage routing at full
    `7.0` damage; stale-scaling it to `6.37` caused the earlier knockback drift
    at source frame 2602.

## 2026-06-24 DownBound knockback decay proof target

- Current fresh replay barrier after shield/Raptor routing: P2 drifts in
  DownBoundU. Rust and Slippi match through source frame 2614, then Rust keeps
  applying ground damage-knockback friction of `0.08` per frame while the Slippi
  witness decays by `0.051`, matching PlCo `x204_knockbackFrameDecay`.
- Decomp proof currently gathered:
  - `ftCo_DownBound_Phys` calls only `ft_80084F3C`.
  - `ftCo_DownBound_Coll` transitions to Fall only from `Fighter_procMap`
    collision handling, not from DownBound physics itself.
  - `Fighter_procUpdate` applies grounded damage knockback with
    `ft_GetGroundFrictionMultiplier(fp) * fp->co_attrs.gr_friction *
    p_ftCommonData->x200`.
  - Battlefield material flag 0 maps to friction multiplier `1.0`, Falcon
    `gr_friction` is `0.08`, and extracted PlCo `x200` is `1.0`; vanilla
    decomp therefore predicts `0.08`, not `0.051`, while airborne/common
    knockback decay predicts `x204 = 0.050999999`.
- Implemented structural parity guard already: DownBound physics no longer
  immediately enters Fall when the ground sweep loses floor; Fall entry is left
  to the map/collision callback phase. Focused tests passed, but the replay
  barrier did not move, so this was necessary architectural cleanup but not the
  root of the `0.051` witness.
- Next proof target before changing behavior: determine whether Slippi/UCF or a
  loaded code patch changes this DownBound damage-knockback decay path, or
  whether our replay comparison/export is reading a mixed phase. Do not encode
  `0.6375`/`0.051` as a Rust shortcut unless that source is proven.

## 2026-06-24 DownBound collision-routing proof result

- Proved a real Rust routing gap with a failing test:
  `source_down_bound_uses_source_ground_to_air_collision_even_with_stale_alias`.
  A source-only DownBound player could still carry a coarse Rust motion alias
  like `GuardOff`, which routed `resolve_ground_support_after_move` through the
  invented endpoint clamp instead of the source `ft_80082708`/`mpColl_8004B108`
  path. The red failure showed routed X clamping to `20.000000` while direct
  source collision produced `19.931313`.
- Minimal fix: source DownBound players now select the source collision route
  even when their public Rust motion alias is stale. Focused tests passed:
  `source_down_bound_uses_source_ground_to_air_collision_even_with_stale_alias`,
  `source_down_bound_phys_does_not_enter_fall_before_procmap_collision`,
  `source_down_bound_ground_physics_decays_damage_knockback_like_xf0_ground_kb_vel`,
  and `landing_ground_traction_applies_ground_movement_before_outer_commit`.
- Replay check through 3200 frames still stops at the same barrier:
  source frame 2617 first position drift, source frame 2619 first state mismatch
  (`actual DownBoundU`, `expected Fall`). Therefore the routing gap was a
  legitimate parity fix, but not the root cause of the current replay divergence.
- Stage-material hypothesis checked against decomp and current extracted data:
  Battlefield floor line kind/index 1 maps to `mpLib_803BD430.x0 = 1.0F`, and
  extracted PlCo `x200 = 1.0`, `x204 = 0.050999999`. The observed Slippi decay
  after frame 2614 still looks like `x204`, but source decomp grounded damage
  knockback currently predicts `ft_GetGroundFrictionMultiplier * gr_friction *
  x200 = 1.0 * 0.08 * 1.0`.
- Current root target: prove the exact `ft_80082708` return semantics and the
  Fighter `procUpdate`/`procMap` phase boundary for DownBound. Do not use the
  legacy test-local `0.6375` multiplier as source truth unless a decomp or
  Slippi patch source proves it.

## 2026-06-24 DownBound entry projection parity

- Decomp proof:
  - `ftCo_8009794C` enters DownBound and then calls `ftCommon_8007CCE8(fp)`.
  - `ftCommon_8007CCE8` only runs when `ground_or_air == GA_Ground` and
    `xF0_ground_kb_vel == 0`; it seeds `xF0_ground_kb_vel` from
    `x8c_kb_vel.x`, clamps it by PlCo `x164`, and projects `x8c_kb_vel` onto
    the current floor normal immediately at entry.
- Runtime fix:
  - Added extracted PlCo field `damage_ground_knockback_init_clamp` from
    source `x164` (`8.300000190734863`) through extraction, `MeleeCommonData`,
    provenance/value sheets, and state hashing.
  - `enter_source_damage_down_bound` now runs a Rust translation of
    `ftCommon_8007CCE8` with the current source floor normal. This is one `f32`
    plus a helper, not a large artifact.
- Verification:
  - Red/green test:
    `source_down_bound_entry_projects_ground_knockback_like_ftcommon_8007cce8`.
  - Focused tests passed for DownBound routing/physics plus PlCo extraction and
    value sheets.
  - Replay barrier is unchanged: first position drift remains source frame
    2617; first state mismatch remains source frame 2619. The fix did clean up
    the earlier hidden source-frame-2611 velocity mismatch: Rust now has
    `x8c_kb_vel.y == 0`, `xF0 == -0.6356878`, and exported velocity Y matches
    Slippi.
- Remaining proof target:
  - From source frame 2615 onward, Slippi witness decays DownBound X knockback
    by `0.051` while the vanilla decomp `Fighter_procUpdate` grounded branch
    still predicts `ft_GetGroundFrictionMultiplier * gr_friction * x200 =
    1.0 * 0.08 * 1.0`. Next pass should prove whether this is a Slippi/code
    patch/UCF phase difference, a missing `ground_or_air` transition, or a
    still-missing collision/wall flag side effect before changing behavior.

## 2026-06-24 DownBound/Fall knockback preservation checkpoint

- Decomp proof:
  - `ftCo_Fall_Enter` changes to `ftCo_MS_Fall`, clamps air drift, resets only
    the Fall blend fields, and if the fighter was grounded calls
    `ftCommon_8007D5D4`.
  - `ftCommon_8007D5D4` switches `ground_or_air` to air, clears `gr_vel`, and
    resets the ECB lock path. It does not clear `x8c_kb_vel` or
    `xF0_ground_kb_vel`.
  - `Fighter_ChangeMotionState` clears many state flags, including shield
    object/install bits, but the inspected body does not clear the damage
    knockback vectors.
- Rust mismatch fixed:
  - `enter_fall` had invented clears for `source_knockback_velocity_x/y` and
    `source_ground_knockback_velocity`.
  - Added `ftco_fall_enter_preserves_damage_knockback_vectors` and removed
    those clears. Focused DownBound tests pass.
- Replay status:
  - The source-frame 2617/2619 barrier did not move, so this was real
    foundation parity but not the current root cause.
- Stage/material proof:
  - Battlefield right side platform line 4 has low material index `0`.
  - Decomp `mpColl_8004CA6C -> mpLib_800569EC` reads the material friction
    table; material `0` maps to friction `1.0`.
  - Therefore the observed Slippi decay of about `0.051` is not explained by
    stage material friction. The active proof target remains the `ground_or_air`
    / `Fighter_procUpdate` / `Fighter_procMap` phase boundary, or a documented
    Slippi export-phase witness mismatch.

## 2026-06-24 DownBound set_airborne_state and ECB unlock target

- Decomp proof:
  - `ftAction_803C06E8[15]` dispatches to `ftAction_80071998`.
  - Action command `0x64000001` in DownBoundU calls
    `ftCommon_8007D5D4`: sets `ground_or_air = GA_Air`, clears `gr_vel`,
    resets `cur_pos.z`/anim Y, sets `jumpsUsed = 1`, sets `ecb_lock = 10`,
    and marks `CollData_X130_Locked`.
  - `ftCo_DownBound_Coll` then uses `ft_80082708`; when that result is
    `GA_Ground`, it enters `ftCo_Fall_Enter`.
- Runtime fixes completed:
  - Added compact source sidecar/runtime support for
    `fighter.set_airborne_state` without per-frame debug bloat.
  - Threaded generic source script events through the DownBound source path.
  - Added source-shaped DownBound collision callback routing through
    `source_ft_80082708_allow_ground_to_air`.
- Verification:
  - Focused CLI/frame-data/core tests pass for set-airborne-state extraction,
    runtime sidecar roundtrip, state 1/state 2 helpers, and DownBound event
    consumption/collision routing.
  - Replay moved past the previous 2617 position drift and 2619 Fall mismatch.
- Current barrier:
  - First remaining mismatch is source frame 2625. Rust stays Fall and falls to
    y ~= 24.470 while Slippi witness is Wait at y = 27.200.
  - Trace shows `ecb_lock` reaches 0 on 2625; current JObj ECB bottom jumps to
    about `2.49749`, but previous ECB bottom remains the locked `0.0`, so the
    sweep misses the platform. This points at ECB lock/unlock lifecycle, not a
    frame-specific landing patch.
- Decomp proof target:
  - `Fighter_procMap` decrements `ecb_lock` and calls `ftCommon_UnlockECB`
    before `coll_cb`.
  - `mpColl_LoadECB_inline` preserves only `desired_ecb.bottom` while locked,
    then always evaluates the live JObj/fixed ECB and sanitizes it.
  - Rust currently has an active ECB path that can return a whole fallback ECB
    while `ecb_bottom_lock_timer > 0`. That is broader than the decomp and is
    the next parity target.

## 2026-06-24 Shield x221B_b0 runtime gate checkpoint

- Decomp proof:
  - `ftColl_8007B1B8` installs the shield hit object and sets `fp->x221B_b0`.
  - Fighter hit collision (`ftcoll.c`) and shield debug/draw collision
    (`ftdrawcommon.c`) gate shield collision/drawing on `fp->x221B_b0`, not on
    the stored `shield_hit` position alone.
  - `ftColl_8007AEE0` only clears `shield_hit.skip_update_pos`, so the stored
    shield object position and the active shield collision gate must remain
    separate runtime concepts.
  - `Fighter_ChangeMotionState` clears `fp->x221A_b7`/`fp->x221B_b0`, so
    ordinary exits like GuardOff should not keep exporting an active shield
    hurtbox unless the state reinstalls it.
- Runtime fix:
  - `source_collision_frame_from_frame` and shield rendering now use
    `source_shield_collision_active` (`x221B_b0` equivalent) as the live gate,
    instead of `source_shield_hit_active`/stale stored-position presence.
  - Added regression coverage proving a stale shield hit object does not export
    a live shield hurt capsule.
  - Corrected shield-related test contracts to match decomp:
    `mpUpdateFloorSkip` copies `floor.index` into `floor_skip` and does not
    clear `floor.index`; ordinary Landing IASA may enter GuardOn once
    `cur_anim_frame >= normal_landing_lag`.
- Verification:
  - `mole_core` shield filter: 65 passed.
  - `mole_runtime` shield filter: 12 passed across lib/runtime tests.
- Next:
  - Shield parity detour is closed enough to return to replay parity. Current
    replay target remains the first post-shield divergence, previously around
    source frame 2625/2707 depending on which local run path is used.

## 2026-06-24 Replay velocity-field separation checkpoint

- Decomp proof:
  - `ftAction_80071998` state `1` calls `ftCommon_8007D5D4`.
  - `ftCommon_8007D5D4` switches `ground_or_air` to air, clears `gr_vel`,
    resets z/anim-y, marks one jump used, and locks ECB for 10 frames. It does
    not clear `self_vel.x` or `x8c_kb_vel`.
  - `ftCo_Fall_Enter` changes motion to Fall, clamps `self_vel.x`, resets Fall
    blend fields, and calls `ftCommon_8007D5D4` only if the fighter was still
    grounded. It does not seed `self_vel.x` from an exported/composed velocity.
- Runtime fixes:
  - Source frame 2583 Raptor Boost: `ftCa_SpecialS_OnDetectGround` preserves
    `self_vel.x`, clears only y/z, scales `gr_vel`, and the Rust public velocity
    mirror now follows the scaled ground velocity instead of stale self
    velocity.
  - Source frames 2615-2618 DownBoundU: after `SetAirborneState(1)`, the
    DownBound air-lane path now exports the surviving decayed `x8c_kb_vel.x`
    while keeping `gr_vel` cleared.
  - Source frame 2619 Fall enter: removed the invented seeding of
    `source_self_velocity_x` from public `velocity.x` when entering Fall from
    air. Damage knockback remains separate from self velocity, matching the
    decomp field layout and Slippi's separate attack/self velocity witness
    fields.
- Guardrail:
  - Do not collapse `self_vel`, `gr_vel`, and `x8c_kb_vel` into a convenience
    velocity when translating source phases. Public/export velocity may mirror
    different source fields depending on the state/phase, but source movement
    must keep the fields separate.
- Verification:
  - `mole_core` focused tests passed:
    `source_down_bound_consumes_action_script_airborne_state_event`,
    `ftco_fall_enter_preserves_damage_knockback_vectors`.
  - `mole_runtime` replay contracts passed:
    `slippi_match_start_frame2583_raptor_boost_routes_shield_detect_callback`,
    `slippi_match_start_frame2625_p2_wait_platform_commit_is_mixed_phase_witness`.
  - Superseded scan interpretation:
    mixed-phase/cascade labels remain useful diagnostics, but independent
    mixed-phase witnesses now count as parity roots. Do not use the older
    `engine_root_scenario_count = 0` conclusion as a reason to continue past
    them in the visual replay.
- Next:
  - Continue replay parity from the first non-classified root beyond 3200
    frames, or deepen the 2625 Slippi/export-phase proof if the visual runtime
    still pauses on raw rows rather than diagnostic engine roots.

## 2026-06-24 Milestone cleanup checkpoint

- Milestone note:
  - Added `docs/worklogs/2026-06-24-shield-replay-parity-milestone.md`.
- Cleanup performed:
  - Removed ignored/generated local output only: Python caches, `.pytest_cache`,
    old `logs/`, old replay trace/explain/frame-log files in `debug/slippi/`,
    the duplicate `debug/parity_scratchpad.md`, and superseded full replay
    input/report copies.
  - Removed about 147 MiB.
- Intentionally kept:
  - Dirty tracked runtime, extraction, docs, tests, and generated parity files
    are active milestone work.
  - Expanded/sample ECB artifacts remain temporarily because parity tests and
    proof workflows still reference them until the compact runtime replacement
    is complete.
- Guardrail:
  - Do not delete tracked source/extraction artifacts merely because they are
    legacy-shaped. Remove them only after a verified Rust/decomp-backed
    replacement exists and tests no longer depend on them.

## 2026-06-24 Visual replay pause 2750 checkpoint

- User-visible pause:
  - The SDL visual replay stopped at displayed frame 2750.
  - The written divergence log's actual stop row was core frame 2749 / source
    frame 2626, player 2, expected `GuardOn`, actual `Fall`.
- Root evidence:
  - Source frame 2626 is not a new bottom-up engine parity root. It is the next
    raw Slippi row after the already-proven source frame 2625 mixed-phase
    platform commit witness.
  - Under the stricter frame-by-frame parity policy, a mixed-phase witness is
    still a parity divergence. It is not acceptable to suppress it merely
    because it may realign or because the decomp proof suggests a Slippi/export
    phase mismatch.
- Runtime change:
  - No simulation mechanics were changed for this checkpoint.
  - `slippi_visual_replay_divergence_for_frame` now lets the visual gate observe
    `MixedPhaseWitness` rows, and `SlippiVisualReplayDivergenceGate` stops on
    them like any other divergence.
  - The scan summary now counts independent mixed-phase witnesses as parity
    roots instead of hiding them from `engine_root_scenario_count`.
  - Current replay scan through 3200 frames reports
    `engine_root_scenario_count = 5`; the first root is core frame 2416 /
    source frame 2293, kind `mixed_phase_witness`, player 2.
- Verification:
  - `cargo run -q -p mole_cli -- tests run -p mole_runtime mixed_phase_witness_is_first_slippi_divergence_candidate slippi_first_divergence_reports_first_mixed_phase_witness visual_replay_stops_on_mixed_phase_witness_roots visual_replay_gate_reports_mixed_phase_root_before_downstream_cascade visual_replay_gate_pauses_at_first_mixed_phase_root_before_late_cascades visual_slippi_replay_runtime_uses_zero_delay_diagnostic_stop_gate`
  - `cargo run -q -p mole_cli -- tests run -p mole_cli scan_summary_counts_mixed_phase_witness_as_parity_root scan_summary_reports_first_independent_root_even_when_mixed`
  - `cargo run -q -p mole_cli -- replay scan --inputs debug\slippi\Game_20260530T214929.inputs.json --frames 3200 --json`
- Next:
  - Ask the user to rerun the visual replay. It should stop at the earliest
    mixed-phase root instead of allowing the later frame-3000 stuck-fall
    cascade. The next parity work should inspect source frame 2293 bottom-up
    against the decomp and Slippi, not patch around the later cascade.

## 2026-06-24 Shield lifecycle and visual parity checkpoint

- User-visible bug:
  - Jumping out of shield while continuing to hold shield could carry the shield
    bubble/collision state into `KneeBend`/jump.
- Decomp root:
  - `ftCo_KneeBend_Enter` calls `Fighter_ChangeMotionState(... ftCo_MS_KneeBend
    ...)`.
  - `Fighter_ChangeMotionState` clears `x221A_b7`, `x221B_b0`, `x221C_b3`,
    `shield_unk0`, and `shield_unk1` for every ordinary motion-state change.
  - Guard states explicitly reinstall the shield object afterward through
    `ftCo_80092450` / `ftColl_8007B1B8`; `KneeBend` does not.
- Runtime fixes:
  - `enter_knee_bend` now routes through the Rust `fighter_change_motion_state`
    helper instead of directly calling `set_motion_state_alias`, so shield
    object state is torn down by the same source-shaped path as other action
    transitions.
  - `enter_guard_on` now initializes `lightshield_amount` from the current
    source-shaped `analog_shield` input using the `ftCo_800921DC` entry formula
    `input.x650 / (1 - x10)`. This is intentionally separate from the
    per-frame held-shield drain formula in `ftCo_800925A4`, which uses
    `(input.x650 - x10) / (1 - x10)`.
  - Runtime shield visuals now consume the same source shield object center and
    `inlineB0` size formula as collision, instead of using a sprite-centered
    fixed debug bubble. Collision was already source-shaped; the misleading
    visual bubble was the gap.
- Existing shield parity coverage confirmed:
  - Digital hard shield maps to full `x650` and analog lightshield remains
    separate from digital air-dodge edges.
  - Held shield drain, inactive shield regeneration, shield damage drain, shield
    break routing, shield aim smoothing, Yoshi's initial-size branch, and stale
    shield object gating have focused tests.
  - Clarification from user: the runtime/devtool architecture should leave room
    for a pure static shield mode, but that should be treated as a future
    source-backed capability, not toggled on or broadly implemented now. The
    current Yoshi coverage is only the decomp-backed `inlineB0` initial-size
    branch, not a custom static-shield system.
- Verification:
  - `cargo run -q -p mole_cli -- tests run -p mole_core shield_held_enters_guard_and_jump_uses_jumpsquat analog_lightshield_entry_initializes_lightshield_amount_like_ftco_800921dc held_shield_drains_health_with_source_lightshield_scale inactive_shield_regenerates_toward_source_max_health guard_updates_source_shield_aim_angle_and_magnitude_like_ftco_80091bc4 source_shield_confirm_drains_shield_health_like_fighter_processhit_8006d1ec source_shield_break_routes_to_shieldbreakfly_like_ftco_80098b20`
  - `cargo run -q -p mole_cli -- tests run -p mole_runtime render_scene_draws_translucent_bubble_shield_for_guard_states render_scene_draws_source_shield_from_decomp_center_and_scale runtime_updates_aimed_guard_shield_position_from_guard_aim_frame_not_motion_playback_frame runtime_source_shield_collision_uses_x221b_b0_gate_not_stale_hit_object_position runtime_source_shield_size_uses_yoshi_initial_size_branch_like_ftco_inlineb0`
- Next:
  - Ask the user to rerun the visual replay and report the new first stop row.
    If the next pause is still shield-related, inspect the exact source frame
    against `ftCo_Guard.c`, `fighter.c`, and `ftcoll.c` before changing replay
    handling.

## 2026-06-24 Shield aim pose/hurtbox checkpoint

- User pause request:
  - Confirm shield can fully angle like the decomp and that the body wireframe
    / hurt capsules follow the same posed skeleton when shielding.
- Decomp root:
  - `ftCo_80091BC4` computes `mv.co.guard.x8` from the stick angle and smooths
    `mv.co.guard.x4` from stick magnitude.
  - `ftCo_80091E78` checks the live shield object/reflecting path, samples
    Guard action 38 at `mv.co.guard.x8`, animates `TransN`, blends by
    `mv.co.guard.x4`, then updates the shield object scale/position.
  - Therefore the shield bubble center and the joint-derived hurt capsules must
    both come from the aimed Guard pose while `x221B_b0` is active and the aim
    blend magnitude is nonzero. Do not independently offset hurtboxes.
- Proven Rust gap:
  - Runtime already updated the shield object center from the aimed Guard pose.
  - Runtime render/collision hurt capsules still sampled the ordinary action
    motion frame, so two different shield angles produced identical body
    wireframes/capsules.
- Runtime fix:
  - `RenderFrame` now carries source shield aim angle and magnitude from the
    core render snapshot.
  - Runtime hurt pose selection now switches to `Guard` at the source shield
    aim angle only when `source_shield_collision_active` (`x221B_b0`
    equivalent) is live and the aim magnitude is nonzero.
  - `source_collision_frame_from_frame`, `player_hurtbox_pills`, shield capsule
    metadata, and `render_source_hurtbox_selection` share that selector so
    gameplay collision and debug wireframes cannot drift apart.
- Explicit non-claim:
  - This checkpoint proves the active shield-object (`x221B_b0`) pose path.
    The decomp also has a separate `reflecting` branch in `ftCo_80091E78`;
    fresh `GuardReflect` setup goes through `ftCo_80093A50`,
    `ftCo_80092450`, and `ftCo_800921DC`, while other reflect paths can clear
    `x221B_b0`. Rust GuardReflect hit/reflect behavior remains a separate
    pending parity item already noted in the state graph.
- Verification:
  - First added test failed before the fix:
    `render_guard_hurtbox_wireframe_uses_aimed_guard_pose_like_ftco_80091e78`.
  - Focused runtime tests now pass:
    `runtime_updates_aimed_guard_shield_position_from_guard_aim_frame_not_motion_playback_frame`,
    `render_guard_hurtbox_wireframe_uses_aimed_guard_pose_like_ftco_80091e78`,
    `source_collision_guard_hurt_capsules_use_aimed_guard_pose_like_ftco_80091e78`,
    `runtime_source_shield_collision_uses_x221b_b0_gate_not_stale_hit_object_position`,
    `runtime_source_shield_size_uses_yoshi_initial_size_branch_like_ftco_inlineb0`.
- Next:
  - Resume replay parity from the current first stop row after the user reruns
    the visual replay. If shielding remains involved, inspect the exact row
    against `ftCo_Guard.c`, `fighter.c`, and `ftcoll.c` before making any
    additional runtime changes.

## 2026-06-24 HTA local-play and shield-break source-bit checkpoint

- User requests addressed:
  - HTA `Play Locally` should launch the SDL3 local game directly, not the dev
    tool.
  - Shield aim should not autoplay in a circular pose at neutral stick.
  - Shield break needs the next character-specific decomp-backed slice.
- HTA root and fix:
  - `START HERE - Mole Game.hta` previously launched
    `execs\Run SDL3 Runtime.cmd`, which also starts the dev tool.
  - `launchLocalPlay()` now launches `execs\Run Local SDL3 Runtime.cmd`, the
    existing local SDL3 runtime wrapper.
- Shield aim root and fix:
  - `ftCo_80091E78` samples the aimed Guard pose at `mv.co.guard.x8`, then
    blends it by `mv.co.guard.x4`.
  - Rust was already aiming the shield object, but partial/neutral magnitudes
    could still behave like a full aimed pose for the shield center and
    wireframe/hurt capsules.
  - Runtime now blends base action pose to aimed Guard pose by
    `source_shield_aim_magnitude`, and the shield object, render wireframe, and
    collision hurt capsules share that selector.
- Shield-break source-bit root and fix:
  - `ftCo_80098B20` sets `fp->x2222_b3 = true` only when
    `fp->kind == FTKIND_PURIN`.
  - Decomp search also shows `x2222_b3` is initialized/cleared in
    `fighter.c`, set by ice damage jump, and read by `ftCo_800D3158` for the
    top-blast-zone route. Those read/ice paths are still future parity work.
  - Rust now has rollback-authoritative `source_x2222_b3`, clears it on common
    motion-state entry, and sets it in `enter_source_shield_break_fly` for
    `purin`/`jigglypuff` reference profiles.
- Verification:
  - `python -m pytest -q tests/test_launch_inputs.py::test_start_here_launcher_exposes_replay_local_and_devtool_entrypoints`
  - `cargo run -q -p mole_cli -- tests run -p mole_runtime runtime_updates_aimed_guard_shield_position_from_guard_aim_frame_not_motion_playback_frame runtime_guard_shield_position_blends_aimed_pose_by_shield_magnitude_like_ftco_80091e78 render_guard_hurtbox_wireframe_uses_aimed_guard_pose_like_ftco_80091e78 render_guard_hurtbox_wireframe_blends_aimed_pose_by_shield_magnitude_like_ftco_80091e78 source_collision_guard_hurt_capsules_use_aimed_guard_pose_like_ftco_80091e78 runtime_source_shield_collision_uses_x221b_b0_gate_not_stale_hit_object_position runtime_source_shield_size_uses_yoshi_initial_size_branch_like_ftco_inlineb0`
  - `cargo run -q -p mole_cli -- tests run -p mole_core source_shield_break_routes_to_shieldbreakfly_like_ftco_80098b20 source_shield_break_sets_purin_x2222_b3_like_ftco_80098b20 source_shield_break_furafura_initializes_timer_like_ftco_80099010 source_furafura_anim_resets_shield_and_exits_when_timer_expires source_shieldbreakfly_lands_into_down_like_ft_80082c74_callback`
- Next:
  - Pause here per user request. On resume, rerun the visual replay and use the
    first stop row as the next replay-parity root. If shield break continues to
    matter, the next bottom-up decomp points are ice-damage `x2222_b3`,
    `ftCo_800D3158` top-blast-zone handling, and the remaining
    `ftCo_ShieldBreak*` effect/collision-status side effects.

## 2026-06-24 Neutral shield pose and held-drain shield-break checkpoint

- User-visible bugs:
  - Holding shield with neutral stick still moved the shield/body wireframe
    through a circular Guard animation cycle.
  - Holding shield until shield health reached zero ended shield but did not
    enter Captain Falcon's shield-break knockdown sequence.
- Decomp roots:
  - `ftCo_800921DC` initializes `mv.co.guard.x8 = 10` and
    `mv.co.guard.x4 = 0`.
  - `ftCo_80091E78` only samples Guard action 38 at `mv.co.guard.x8` when
    `mv.co.guard.x4` is nonzero. When the stick magnitude is zero, the shield
    object and body pose must stay on the neutral branch rather than walking
    through Guard playback frames.
  - `ftCo_800925A4` drains held shield health. If health crosses below zero it
    clears `x221A_b7`/`x221B_b0`, calls `ftCo_80098B20`, plays the shield-break
    SFX, and returns true so the Guard animation callback stops there.
- Runtime/core fixes:
  - Runtime shield center and source hurt-capsule selection now use the stable
    source Guard neutral frame 10 while the shield object is installed and
    `source_shield_aim_magnitude` is zero.
  - Aimed shield behavior remains source-shaped: nonzero aim magnitude still
    samples Guard action 38 at `source_shield_aim_angle_degrees` and blends
    toward it.
  - Held-shield drain now calls the shared `enter_source_shield_break_fly`
    helper when health reaches zero, matching the collision shield-break route.
  - The shield lifecycle tick now reports that it changed state so the current
    Guard tick stops immediately, matching the `ftCo_800925A4` true-return
    phase break instead of applying ShieldBreakFly physics in the same tick.
- Verification:
  - Red tests first failed:
    `runtime_guard_neutral_stick_does_not_play_guard_direction_table_like_ftco_80091e78`
    and `held_shield_drain_routes_to_shieldbreakfly_like_ftco_800925a4`.
  - Focused runtime tests now pass:
    `cargo run -q -p mole_cli -- tests run -p mole_runtime runtime_guard_neutral_stick_does_not_play_guard_direction_table_like_ftco_80091e78 render_guard_neutral_stick_wireframe_does_not_play_guard_direction_table_like_ftco_80091e78 runtime_updates_aimed_guard_shield_position_from_guard_aim_frame_not_motion_playback_frame runtime_guard_shield_position_blends_aimed_pose_by_shield_magnitude_like_ftco_80091e78 render_guard_hurtbox_wireframe_uses_aimed_guard_pose_like_ftco_80091e78 render_guard_hurtbox_wireframe_blends_aimed_pose_by_shield_magnitude_like_ftco_80091e78 source_collision_guard_hurt_capsules_use_aimed_guard_pose_like_ftco_80091e78 runtime_source_shield_collision_uses_x221b_b0_gate_not_stale_hit_object_position runtime_source_shield_size_uses_yoshi_initial_size_branch_like_ftco_inlineb0`
  - Focused core tests now pass:
    `cargo run -q -p mole_cli -- tests run -p mole_core held_shield_drain_routes_to_shieldbreakfly_like_ftco_800925a4 source_shield_break_routes_to_shieldbreakfly_like_ftco_80098b20 source_shield_break_sets_purin_x2222_b3_like_ftco_80098b20 source_shield_break_furafura_initializes_timer_like_ftco_80099010 source_furafura_anim_resets_shield_and_exits_when_timer_expires source_shieldbreakfly_lands_into_down_like_ft_80082c74_callback`
- Next:
  - Ask the user to rerun local play/replay. If shield break still visually
    diverges, inspect the next exact stop against `ftCo_ShieldBreakFly`,
    `ftCo_ShieldBreakFall`, `ftCo_ShieldBreakDown`, `ftCo_ShieldBreakStand`,
    and `ftCo_Furafura` before adding any more behavior.

## 2026-06-24 ShieldBreak retained model pose checkpoint

- User-visible bug:
  - ShieldBreakFly knocked the fighter upward, but the defender's wireframe/body
    hurtboxes disappeared, and the visual sequence made it look like the
    knockdown path never completed.
- Decomp roots:
  - `ftCo_80098B20` enters common motion state 205, clears the shield object,
    sets self velocity, calls `ftColl_8007B62C(gobj, 2)`, and uses
    `ft_80082C74(..., ftCo_80098E3C)` for floor contact.
  - `ftCo_ShieldBreakFall`, `ftCo_ShieldBreakDown`, and
    `ftCo_ShieldBreakStand` enter with keep/skip model and col-animation flags.
  - `fighter.c::Fighter_ChangeMotionState` sets `anim_id = -1` for missing
    animation data and clears animation controllers/scripts, but it does not
    delete the model/JObj or body collision geometry.
- Rust root cause:
  - `source_binding_for_motion_state` correctly did not invent source action
    bindings for ShieldBreakFly/Fall/Down/Stand.
  - However, `PlayerRenderSnapshot` equated "no current source action binding"
    with "no model pose", so ShieldBreak states exported no
    `source_pose_action_key`. Runtime then had no source body capsules to render
    or collide against.
- Runtime/core fix:
  - Added compact rollback-authoritative `SourceRetainedModelPose` on
    `PlayerState`.
  - `set_motion_state_alias` captures the previous source model pose before
    entering ShieldBreakFly/Fall/Down/Stand, and clears it for normal fresh
    source-bound states.
  - Snapshots now keep the current Melee state id (`ShieldBreakFly` = 205)
    separate from the retained model/collision pose key/frame/facing.
  - No generated frame tables or per-frame geometry were added; runtime still
    samples baked source capsules on demand.
- Verification:
  - Red tests failed first:
    `shieldbreakfly_keeps_previous_model_pose_like_fighter_change_motion_state_no_anim`,
    `render_scene_keeps_shieldbreakfly_body_visible_after_shield_object_clears`.
  - Focused core tests now pass:
    `cargo run -q -p mole_cli -- tests run -p mole_core shieldbreakfly_keeps_previous_model_pose_like_fighter_change_motion_state_no_anim held_shield_drain_routes_to_shieldbreakfly_like_ftco_800925a4 source_shieldbreakfly_lands_into_down_like_ft_80082c74_callback`
  - Focused runtime tests now pass:
    `cargo run -q -p mole_cli -- tests run -p mole_runtime render_scene_keeps_shieldbreakfly_body_visible_after_shield_object_clears render_scene_draws_source_shield_from_decomp_center_and_scale render_guard_hurtbox_wireframe_uses_aimed_guard_pose_like_ftco_80091e78 source_collision_guard_hurt_capsules_use_aimed_guard_pose_like_ftco_80091e78`
- Next:
  - Ask the user to rerun the replay. If ShieldBreak still visually diverges,
    inspect the exact row for the remaining common action-state duration/anim-id
    table mapping rather than mapping ShieldBreak to Falcon action-table slots by
    numeric coincidence.

## 2026-06-24 Guard C-stick spot dodge / pass priority checkpoint

- User-visible bug:
  - Shield angled/down on a platform could enter a pass/crouch-like route before
    jump or spot dodge, then carry bad shield state into later movement.
  - C-stick down from shield entered EscapeN, but the installed guard shield
    object could persist because the Rust EscapeN entry bypassed the shared
    `Fighter_ChangeMotionState` helper.
- Decomp roots:
  - `ftCo_GuardOn_IASA` order is release/reflect, `ftCo_8009515C`,
    `ftCo_8009980C`, `ftCo_8009917C`, `ftCo_800D8B9C`,
    `ftCo_Catch_CheckInput`, `ftCo_800CB024`, then `ftCo_8009A080`.
  - `ftCo_Guard_IASA` likewise checks `ftCo_8009980C`, roll, catch, jump, and
    only then `ftCo_8009A080`; platform pass is last, not first.
  - `ftCo_8009980C` accepts main-stick down via `inlineB0` or C-stick down via
    `ftCo_800DF8E8`; `ftCo_800DF8E8` reads `fp->input.cstick.y <= x314`.
  - `ftCo_8009917C` accepts C-stick X via `ftCo_800DF8B0`, and
    `ftCo_800CB024` accepts C-stick up via `ftCo_800DF910`.
  - `ftCo_800998EC` enters EscapeN through `Fighter_ChangeMotionState`, then
    calls `ftAnim_8006EBA4` and sets `x221D_b5`.
  - `ftCo_EscapeN_Phys` calls `ft_80084F3C`, so EscapeN should ground-slide
    under traction instead of hard-zeroing ground velocity.
- Rust fixes:
  - Moved GuardOn/Guard platform pass routing after spot dodge, roll, catch,
    and jump checks.
  - Changed ground Escape entry to use `fighter_change_motion_state`, clearing
    the guard shield object just like other source motion-state changes.
  - Changed the generic grounded action branch so EscapeN calls
    `apply_ground_traction` rather than `clear_ground_horizontal_velocity`.
- C-stick buffer note:
  - Do not add an explicit one-frame shield-buffer timer. The competitive
    one-frame feel should fall out of decomp-shaped input/current-vs-previous
    stick plumbing plus the normal Guard callback/physics phase order.
  - Shield C-stick exits are source-backed as route choices, not a separate
    buffer subsystem: up routes through `ftCo_800CB024`/`ftCo_800DF910` into
    KneeBend, down routes through `ftCo_8009980C`/`ftCo_800DF8E8` into
    EscapeN, and horizontal routes through `ftCo_8009917C`/`ftCo_800DF8B0`
    into facing-aware EscapeF/EscapeB. Any observed one-frame delay must be
    reproduced by input sampling and callback order, not by a hand-authored
    delay constant.
  - Shield DI / Shield SDI belongs to the shield-hit response layer
    (`ftCo_800DF608` and damage hitlag callbacks), not idle shield aiming or
    Guard pass priority.
- Verification:
  - Red tests failed first for pass-before-jump, pass-before-spotdodge, stale
    shield object on C-stick EscapeN, and EscapeN hard-zeroing velocity.
  - Focused core tests now pass:
    `cargo run -q -p mole_cli -- tests run -p mole_core spotdodge_phys_uses_source_ground_traction_instead_of_zeroing_velocity shield_down_jump_on_soft_platform_uses_jumpsquat_before_pass_like_guard_iasa shield_hard_down_on_soft_platform_enters_spotdodge_before_pass shield_cstick_down_enters_spotdodge_and_clears_guard_shield_object shield_held_enters_guard_and_jump_uses_jumpsquat shield_down_tap_enters_spotdodge_before_roll_or_grab guard_off_without_reflect_gate_checks_spotdodge_before_offense spotdodge_duration_uses_profile_action_frames`
  - C-stick shield exits now pass together:
    `cargo run -q -p mole_cli -- tests run -p mole_core shield_cstick_jump_stores_cstick_source_and_release_short_hops shield_cstick_side_enters_facing_aware_roll_and_clears_guard_shield_object shield_cstick_down_enters_spotdodge_and_clears_guard_shield_object`
- Next:
  - Run a broader guard/roll/jump focused slice, then ask the user to rerun the
    local replay and report the next first stop.

## 2026-07-02 Replay check classified-root checkpoint

- Current replay command:
  - `cargo run -q -p mole_cli -- replay check --replay replays/Game_20260530T214929.slp --frames 3600 --json`
- Result:
  - `comparison.first_classified_divergence` now reports P2 source frame `2293`,
    core frame `2416`, kind `mixed_phase_witness`.
  - `comparison.first_state_mismatch` still reports the later P2 source frame
    `2625`, core frame `2748`, expected `Wait`, actual `Fall`.
  - Do not treat the 2625 raw state mismatch as the first root while strict
    frame-by-frame parity is enabled. It remains a documented platform-commit
    mixed-phase witness, but the earliest current classified root is 2293.
- Why this matters:
  - `replay scan` and visual replay gating already treated mixed-phase witnesses
    as hard parity roots. `replay check` only serialized raw comparison fields,
    so it could make a later raw state mismatch look like the first actionable
    stop.
  - The comparison pass now carries the existing decomp-shaped trace classifier
    through the same simulation pass; this does not add a second replay scan and
    does not suppress or realign past the divergence.
- Proof snapshot for source frame 2293:
  - P2 is in `Landing` with matching position and state.
  - Rust has already executed the floor commit and exposes current grounded
    collision flags (`actual_source_coll_env_flags = 0x18000`) while Slippi's
    row still exposes the pre-floor self Y speed (`expected_velocity_y = -2080`).
  - Actual composed vertical velocity differs only by the attack/knockback Y
    component (`646` milli), matching the existing mixed-phase floor-commit
    classifier.
- Decomp anchors for the 2293 proof:
  - `src/melee/ft/ft_081B.c:112-121` copies `coll->last_pos`, assigns
    `coll->cur_pos = fp->cur_pos`, calls `mpColl_800471F8(coll)`, then writes
    `fp->cur_pos = coll->cur_pos`.
  - `src/melee/ft/chara/ftCommon/ftCo_Landing.c:187-190` shows
    `ftCo_Landing_Phys` only delegates to `ft_80084F3C`.
  - `src/melee/ft/ft_084E.c:41-52` shows `ft_80084F3C` applies ground friction
    and `ftCommon_ApplyGroundMovement`; it is not the collision callback itself.
  - `src/melee/ft/fighter.c:2276-2281` keeps the outer velocity commit separate:
    it adds staged ground accel to `gr_vel`, then adds `x74_anim_vel` into
    `self_vel` and clears `x74_anim_vel`.
- Existing focused classifier test:
  - `landing_floor_commit_velocity_row_is_mixed_phase_witness` covers the 2293
    same-state/same-position row and proves the velocity fields are being
    classified by decomp phase instead of patched in replay handling.
- Verification:
  - Red test failed first because `SlippiCoreComparison` had no
    `first_divergence` field.
  - Focused runtime tests pass:
    `cargo run -q -p mole_cli -- tests run -p mole_runtime slippi_match_start_comparison_carries_first_classified_divergence slippi_first_divergence_reports_first_mixed_phase_witness slippi_match_start_frame2625_p2_wait_platform_commit_is_mixed_phase_witness`
  - Focused CLI tests pass:
    `cargo run -q -p mole_cli -- tests run -p mole_cli comparison_json_reports_first_classified_divergence scan_summary_counts_mixed_phase_witness_as_parity_root scan_summary_reports_first_independent_root_even_when_mixed`
- Next:
  - Investigate source frame 2293 from the bottom up against decomp floor commit,
    damage/landing velocity composition, and Slippi 3.19.1 post-frame row
    emission before changing collision or velocity code.

## 2026-07-05 Source frame 2293 mixed-phase proof checkpoint

- Current user stop:
  - `slippi_stop_summary source_frame=2293 core_frame=2416 player=P2 kind=MixedPhaseWitness`.
- Runtime row facts:
  - State and position match: P2 is `Landing` / action state `42` at
    `x=43.402618`, `y=0.0001`.
  - Slippi self velocity Y is `-2.0799999`; Slippi attack/knockback Y is
    `0.645721`.
  - Rust exposes the same decomp source fields:
    `actual_source_self_velocity_y=-2.0799999`,
    `actual_source_knockback_velocity_y=0.6457205`.
  - The only strict stop delta is public/composed Y velocity:
    `-2.0799999 + 0.6457205 = -1.4342794`, or `-1434` milli.
- Slippi source proof:
  - Local Slippi 3.19.1 patch
    `.research/project-slippi-Ishiiruka/Data/Sys/GameSettings/GALE01r2.ini`
    contains `Recording/SendGamePostFrame.asm`.
  - That patch writes direct fighter offsets:
    - `fp+0x10` -> action state.
    - `fp+0xB0` / `fp+0xB4` -> position X/Y.
    - `fp+0x80` / `fp+0x84` -> self velocity X/Y.
    - `fp+0x8C` / `fp+0x90` -> knockback velocity X/Y.
    - `fp+0xE0` -> ground/air.
  - Decomp `src/melee/ft/types.h` maps those offsets as
    `self_vel`, `x8c_kb_vel`, `cur_pos`, and `ground_or_air`.
- Decomp phase proof:
  - `Fighter_procUpdate` applies physics/knockback and integrates
    `cur_pos += selfVel + x8c_kb_vel` before map/collision.
  - `Fighter_procMap` later calls `coll_cb`; Fall's collision callback
    (`ft_800831CC` -> `ft_80082B1C`) can enter `Landing`.
  - `ftCo_Landing_Phys` only calls ground traction (`ft_80084F3C`) on the next
    update; it is not the landing collision commit.
- Interpretation:
  - Frame 2293 is not evidence that the Rust engine should keep P2 airborne or
    preserve a falling public velocity while grounded.
  - It is a strict replay witness mismatch: Slippi's post-frame row shows
    post-floor state/position while its exposed velocity slots are the raw
    decomp self/knockback fields from the surrounding update/map boundary.
  - Do not patch collision or landing physics for this frame. Keep it classified
    as a hard, explained `MixedPhaseWitness` in strict mode, then continue
    searching for the next independent decomp-phase mismatch.

## 2026-07-05 ECB lock map-phase parity checkpoint

- Raw mismatch fixed:
  - Before this checkpoint, the first raw state mismatch after the 2293
    `MixedPhaseWitness` was P2 source frame `2625`, core frame `2748`:
    Slippi expected `Wait` at Battlefield right platform `y=27.2001`, while
    Rust stayed `Fall` at `y=24.4701`.
- Root cause:
  - Rust decremented `ecb_bottom_lock_timer` at the top of the player tick.
  - Decomp decrements `fp->ecb_lock` inside `Fighter_procMap`, immediately
    before the state collision callback.
  - P2's DownBoundU action script installs the 10-frame ground-to-air ECB
    lock at source frame `2615` through the `ftCommon_8007D5D4` path. Decomp
    consumes the first count in that same frame's map phase; Rust left the
    first count for the next frame.
- Decomp anchors:
  - `src/melee/ft/fighter.c:2464-2477`: `Fighter_procMap` decrements
    `ecb_lock`, calls `ftCommon_UnlockECB` when it reaches zero, then calls
    `coll_cb`.
  - `src/melee/ft/ftcommon.c:519-535`: `ftCommon_UnlockECB` clears
    `CollData_X130_Locked`; `ftCommon_8007D5D4` sets `ecb_lock=10` and the
    locked flag.
  - `src/melee/ft/chara/ftCommon/ftCo_DownBound.c:220-228`:
    DownBound collision calls `ft_80082708`, then routes to Fall only on
    `GA_Ground`.
  - `src/melee/mp/mpcoll.c:607-626`: `mpColl_LoadECB_inline` preserves
    desired ECB bottom only while `CollData_X130_Locked` is still set.
- Runtime fix:
  - Removed the global top-of-tick ECB lock decrement.
  - Added a source `Fighter_procMap` prelude that consumes the lock at map /
    collision wrapper boundaries instead of changing the source constant.
  - Kept pure `ftCommon_8007D5D4` behavior at a 10-frame lock; full DownBound
    tick now observes `9` after the same-frame map phase.
- Verification:
  - Red test first failed:
    `source_down_bound_consumes_action_script_airborne_state_event` expected
    same-frame map consumption (`9`) and got `10`.
  - Focused tests now pass:
    `cargo run -q -p mole_cli -- tests run -p mole_core source_down_bound_consumes_action_script_airborne_state_event source_set_airborne_state_one_calls_ftcommon_8007d5d4_without_motion_change`
  - Source map-collision module passes:
    `cargo run -q -p mole_cli -- tests run -p mole_core source_map_collision_tests`
  - Replay trace now shows source frame `2624` unlocks and loads live ECB bottom
    before collision, and source frame `2625` matches `Wait` at `y=27.2001`.
  - Replay check now moves first raw state mismatch to P2 source frame `2662`,
    core frame `2785`: Slippi expected action `83`, Rust is `Guard`.

## 2026-07-05 Wait-to-GuardOn same-frame physics checkpoint

- Follow-up drift after the ECB fix:
  - Source frames `2626-2630` had P2 in `GuardOn` in both Slippi and Rust, but
    Rust X drifted ahead by the previous platform landing velocity. At source
    `2626`, Rust moved with pre-traction `0.357`; Slippi moved with
    post-traction `0.277`.
- Root cause:
  - `ftCo_Wait_IASA` can enter GuardOn through `ftCo_80091A4C`.
  - Decomp then runs the entered state's physics in the same fighter tick:
    `ftCo_GuardOn_Phys` calls `ft_80084F3C`.
  - Rust's `apply_wait_state_inputs` entered GuardOn but omitted the same-frame
    GuardOn ground traction path already used by Walk/Run/Landing shield-entry
    routes.
- Decomp anchors:
  - `src/melee/ft/chara/ftCommon/ftCo_Wait.c:43-66`: Wait IASA checks
    `ftCo_80091A4C`.
  - `src/melee/ft/chara/ftCommon/ftCo_Wait.c:68-72`: Wait Phys is
    `ft_80084F3C`, which is the same ground traction primitive.
  - `src/melee/ft/chara/ftCommon/ftCo_Guard.c:315-329`: GuardOn entry through
    `ftCo_800924C0`.
  - `src/melee/ft/chara/ftCommon/ftCo_Guard.c:411-416`: GuardOn Phys calls
    `ft_80084F3C`, then shield collision updates.
- Runtime fix:
  - Wait shield entry now calls `apply_ground_traction` immediately after
    entering GuardOn, matching the same-frame entered-state physics pattern.
- Verification:
  - Red test first failed:
    `wait_iasa_full_analog_trigger_enters_guard_on_and_runs_guard_physics`
    expected velocity `277`, got `357`.
  - Focused guard physics tests now pass:
    `cargo run -q -p mole_cli -- tests run -p mole_core wait_iasa_full_analog_trigger_enters_guard_on_and_runs_guard_physics walk_iasa_full_analog_trigger_enters_guard_on_and_runs_guard_physics guard_on_applies_ground_traction_while_shield_is_held guard_applies_ground_traction_while_shield_is_held`
  - Replay trace now keeps P2's GuardOn X position aligned through source
    `2661`.
  - The remaining first raw mismatch is P2 source `2662`, core `2785`, with
    position/velocity aligned but Slippi in `DamageLw3` and Rust still in
    `Guard`. Next root is guard-hit / hit-processing phase, not ground movement.

## 2026-07-05 Spark sidecar orchestration rule

- Main thread owns parity decisions, code edits, and verification.
- Spark 5.3 sidecars are allowed only for narrow, read-only, independently
  answerable questions unless a disjoint implementation slice is explicitly
  assigned.
- Sidecars should return one concrete answer with file/line anchors and should
  be closed quickly. Do not let stale agents replace local proof.
- Current priority remains replay parity from the earliest stop:
  `slippi_stop_summary source_frame=2293 core_frame=2416 player=P2 kind=MixedPhaseWitness`.
- Do not use `MixedPhaseWitness` as a runtime patch or substitute for parity.
  It is only acceptable as a diagnostic classification after local Slippi source
  and Melee decomp phase ordering prove the row mixes fields across phases.
- Latest replay check after the damage-floor fixture cleanup:
  - First classified divergence remains P2 source `2293`, core `2416`,
    `MixedPhaseWitness`.
  - Old raw mismatch around source `2297` is gone.
  - First raw state mismatch is P2 source `2686`, core `2809`: Rust lands on
    Battlefield line `4`; Slippi still reports `DamageLw3` for that row, then
    reports `Landing` the next row while carrying stale pre-collision velocity.
- Current hypothesis to prove or reject:
  - Decomp `ftCo_Damage_Coll` plus `ft_80081DD4`/`mpColl_800473CC` should floor
    snap P2 on source `2686` because the ECB bottom sweeps from above line `4`
    to below it.
  - If Slippi 3.19.1 records state/position after map/collision but velocity
    from fighter velocity fields around an adjacent phase boundary, source
    `2686` is another witness-row issue rather than a Rust collision hotfix.
  - If local Slippi source proves the row is not mixed-phase, then the root is
    a Rust engine parity bug in collision/load-ECB/damage-collision ordering and
    must be fixed from the decomp.

## 2026-07-05 P2 source 2686 damage-floor witness checkpoint

- Spark sidecar result:
  - Local Slippi `GALE01r2.ini` installs `Recording/SendGamePostFrame.asm` at
    `C206DA34`.
  - The hook writes direct fighter offsets into the post-frame row, including
    position (`fp+0xB0/0xB4`), self velocity (`fp+0x80/0x84`), knockback
    velocity (`fp+0x8C/0x90`), ground/air (`fp+0xE0`), and action/status bytes.
  - Local Slippi source only labels the payload as post-frame command `0x38`;
    it does not provide a clean Melee phase contract beyond the injected hook.
- Decomp anchors:
  - `src/melee/ft/chara/ftCommon/ftCo_Damage.c:1030-1048`:
    airborne `ftCo_Damage_Coll` calls `ft_80081DD4`; medium knockback enters
    `ftCo_Landing_Enter_Basic`.
  - `src/melee/ft/ft_081B.c:132-174`: `ft_80081DD4` writes
    `coll.last_pos = coll.cur_pos`, `coll.cur_pos = fp.cur_pos`, calls
    `mpColl_800473CC`, then copies `coll.cur_pos` back into `fp.cur_pos`.
  - `src/melee/mp/mpcoll.c:2710-2715`: `mpColl_800473CC` does
    `mpCollPrev`, `mpColl_LoadECB_inline(coll, 6)`, then floor wrapper
    `inline0(coll, 4, true)`.
  - `src/melee/ft/chara/ftCommon/ftCo_Landing.c:73-124`:
    `ftCo_Landing_Enter_Basic` routes through `ftCo_Landing_Enter`, which calls
    `ftCommon_8007D7FC(fp)` before `Fighter_ChangeMotionState(... Landing ...)`.
- Trace proof at source `2686`, P2:
  - Slippi expected: `DamageLw3` at `(47.939, 21.653)`, self Y `-2.340`,
    attack/kb `(0.945, 0.675)`.
  - Rust/decomp-shaped collision: `Landing` at `(47.939, 27.200)`, floor line
    `4`, env flags `0x18000`, previous env flags `0`.
  - Rust raw source velocities match Slippi's separate velocity slots:
    self Y `-2.3400002`, knockback X `0.9451721`, knockback Y `0.6749285`.
- Runtime/diagnostic change:
  - Added a narrow `is_damage_floor_commit_mixed_phase_witness` classifier in
    `mole_runtime::slippi_diagnostic`.
  - It does not alter simulation. It only classifies the row when:
    source state is standard damage (`75..=86`), Rust has entered `Landing`,
    floor contact flags are present, X position has not drifted, Y snapped up to
    the floor, and Slippi velocity slots match Rust raw source self/knockback
    fields.
  - Added fixture test:
    `slippi_match_start_frame2686_p2_damage_floor_commit_is_mixed_phase_witness`.
  - Updated stale test for source `2625`: that row is now expected to stay
    aligned after the ECB-lock map-phase fix.
- Verification:
  - Focused runtime tests pass:
    `slippi_match_start_frame2625_p2_wait_platform_commit_stays_aligned_after_ecb_lock_fix`
    `slippi_match_start_frame2686_p2_damage_floor_commit_is_mixed_phase_witness`
    `visual_replay_gate_pauses_at_first_mixed_phase_root_before_late_cascades`
    plus the existing same-state floor velocity witness tests.
  - Replay check through `3200` frames still reports first classified divergence
    as P2 source `2293` `MixedPhaseWitness`; raw first state mismatch remains
    source `2686` because the raw summary intentionally reports state mismatch
    independent of the scenario classification.
- Next unresolved root:
  - Source `2687-2689` show P2 Landing X drifting behind Slippi by roughly the
    difference between grounded knockback friction and one more air-style
    knockback decay.
  - Do not patch this yet. Decomp `Fighter_procUpdate` lines `2164-2216` says
    grounded fighters decay `xF0_ground_kb_vel` through
    `ftCommon_8007CCA0`, then write it back to `x8c_kb_vel`; `ftCo_Landing_Phys`
    line `189` only calls `ft_80084F3C`.
  - The next proof step is to determine whether Slippi's first Landing row after
    source `2686` is another mixed-phase witness row or whether Rust is missing
    a source field from damage entry/landing entry that should seed
    `xF0_ground_kb_vel` differently.

## 2026-07-05 Spark proof: Damage -> Landing ground-kb seed

- Spark 5.3 sidecar `019f339f-6b97-7eb3-acd5-7e3228f00781` independently
  checked local decomp and was closed after one answer.
- Evidence anchors:
  - `src/melee/ft/chara/ftCommon/ftCo_Damage.c:1035-1045`: airborne
    `ftCo_Damage_Coll` calls `ftCo_Landing_Enter_Basic` for the medium
    knockback landing branch.
  - `src/melee/ft/chara/ftCommon/ftCo_Landing.c:73-81` and `118-125`:
    `ftCo_Landing_Enter_Basic` routes through `ftCo_Landing_Enter`, then
    `ftCommon_8007D7FC`, then `Fighter_ChangeMotionState`.
  - `src/melee/ft/ftcommon.c:556-563` and `591-604`: `ftCommon_8007D7FC`
    forwards to `ftCommon_8007D6A4`; this path does not write
    `xF0_ground_kb_vel` or `x8c_kb_vel`.
  - `src/melee/ft/chara/ftCommon/ftCo_Landing.c:187-195`: Landing phys/coll are
    ground traction and collision callbacks, not knockback seed logic.
  - `src/melee/ft/fighter.c:2144-2167` and `2199-2201`: generic grounded
    `Fighter_procUpdate` seeds `xF0_ground_kb_vel` from `x8c_kb_vel.x` when
    grounded and the ground scalar is zero.
- Conclusion:
  - Damage -> Landing entry itself does not seed ground knockback and does not
    clear or modify `self_vel.y` or `x8c_kb_vel`.
  - If Rust's first grounded update seeds/decays `xF0_ground_kb_vel` from
    `x8c_kb_vel.x`, that part is decomp-shaped. The remaining 2687 X drift must
    be proven as either Slippi witness phasing or a different missing decomp
    phase/order field.

## 2026-07-05 Spark proof: DamageLw3 frame-18 ECB floor sweep

- Spark 5.3 sidecar `019f33a3-bd78-74b3-9e8e-3635710d17ff` independently
  checked the local decomp and extracted ECB samples, then was closed.
- Evidence anchors:
  - `src/melee/mp/mpcoll.c:2710-2714`: `mpColl_800473CC` runs
    `mpCollPrev(coll)`, `mpColl_LoadECB_inline(coll, 6)`, then the air
    collision wrapper.
  - `src/melee/mp/mpcoll.c:607-626`: `mpColl_LoadECB_inline` populates
    `desired_ecb` from the JObj/fixed ECB path and runs the post-fix logic.
  - `src/melee/mp/mpcoll.c:662-670` and `912-980`: `mpColl_80043754`
    interpolates `coll->ecb` toward `desired_ecb` before the callback.
  - `resources/melee/extracted/captain_falcon_action_ecb_samples.json`:
    Captain Falcon DamageLw3 action-table index `173` has bottom `y`
    `4.1009964` at frame 17, `4.6598368` at frame 18, and `5.8145107` at
    frame 19.
- Conclusion:
  - The Rust row using DamageLw3 bottom `~4.6598` at source `2686` is using the
    source frame-18 ECB value. It is not a Rust-invented offset or integer/float
    round-trip artifact.
  - The decomp floor proof for source `2686` must therefore focus on
    `mpColl_80044628_Floor`/`mpCheckFloor`/`mpCheckFloorRemap`, platform-pass
    callbacks, floor-skip, and Slippi post-frame row semantics.

## 2026-07-05 Replay gate policy after source 2293 proof

- Source `2293` / core `2416` P2:
  - Trace shows Slippi and Rust both agree on `Landing`, position `(43.4026,
    0.0001)`, raw `self_vel.y = -2.08`, and raw knockback/attack velocity
    `(0.9330, 0.6457)`.
  - The mismatch was only the derived composed velocity comparison. It combined
    values across the map/update phase boundary and produced a false stop.
- Decomp phase anchors:
  - `src/melee/ft/fighter.c:910-911`: `Fighter_procUpdate` is priority `4`,
    `Fighter_procMap` is priority `6`.
  - `src/melee/ft/fighter.c:2160-2215`: generic knockback projection/decay
    happens in `Fighter_procUpdate`.
  - `src/melee/ft/fighter.c:2460-2479`: collision callbacks, including
    airborne floor commit into Landing, happen later in `Fighter_procMap`.
- Runtime diagnostic change:
  - `MixedPhaseWitness` rows are still detected and retained in scans.
  - Visual replay and "first divergence" selection now skip proven
    `MixedPhaseWitness` rows so the tool stops at the first non-witness parity
    failure rather than at a Slippi/decomp phase-boundary witness.
- Verification:
  - Focused runtime tests pass for the updated first-divergence/gate policy.
  - `replay check --frames 3200` now reports first classified non-witness
    divergence at P2 source `2687`, core `2810`, `VelocityDrift`.
  - The scan helper's first non-cascade, non-witness scenario is P2 source
    `2689`, `PositionDrift`, because source `2687` is still part of the
    post-2686 witness cascade in that scan surface.
- Current next target:
  - P2 source `2687-2689` after the DamageLw3 -> Landing commit.
  - Suspicious field: Rust has already zeroed `self_vel.y` and projected
    knockback onto the ground, while Slippi's next Landing row still exposes
    airborne-style `self_vel.y`/attack-y slots. This must be checked against
    decomp Landing physics before any sim change.

## 2026-07-05 DamageLw3 ECB timing fix moved frontier to SpecialAirHi

- Correction to the earlier `2686` hypothesis:
  - Treating P2 source `2686` as a Slippi mixed-phase witness was wrong.
  - Bottom-up decomp proof showed the Rust engine was sampling DamageLw3's
    collision ECB one animation step too early because damage entry had
    `frame_speed_mul == 0`.
- Decomp anchors:
  - `src/melee/ft/fighter.c:1668-1707`: the priority-3 fighter animation proc
    calls `ftAnim_8006EBA4` before the later map/collision proc.
  - `src/melee/ft/ftanim.c:315-378`: `ftAnim_8006E9B4` advances JObjs and then
    stores `fp->cur_anim_frame`.
  - `src/melee/ft/fighter.c:2460-2479`: `Fighter_procMap` later calls
    `coll_cb`.
  - `src/melee/ft/chara/ftCommon/ftCo_Damage.c:443-444`: damage entry calls
    `Fighter_ChangeMotionState(..., anim_start=0, anim_rate=1, blend=0)`, then
    `ftAnim_8006EBA4`.
- Runtime fixes:
  - Damage entry now seeds `motion_anim_rate_milli = 1000` instead of `0`.
  - `mpColl_LoadECB_JObj` pose selection now samples post-animation pose for
    common damage states, matching the decomp phase order already used for
    SpecialHi.
- Test proof:
  - Red tests failed first:
    `source_damage_entry_resets_action_pose_before_hitlag_freezes_update`
    expected `frame_speed_mul=1.0F` and saw `0`.
    `damage_lw3_mp_coll_load_ecb_jobj_samples_post_anim_pose_seen_by_coll_callback`
    expected the post-`ftAnim` DamageLw3 ECB and saw the current pose.
  - Focused ECB tests now pass for DamageLw3, SpecialHi, JumpAerialF,
    EscapeAir, and Fall, proving this is not a blanket `+1` frame patch.
- Replay proof:
  - Trace P2 source `2686` now stays `DamageLw3` at `(47.939, 21.653)` with
    DamageLw3 ECB bottom `5.8145`, matching Slippi's row.
  - P2 source `2687-2690` now matches positions and state through Landing.
  - `replay check --frames 3200` now reports first classified divergence at P2
    source `2787`, core `2910`, `PositionDrift`, while still in
    `SpecialAirHi`.
- Next target:
  - P2 source `2787` `SpecialAirHi` position drift, followed by expected
    `CliffCatch` at source `2831` while Rust remains `SpecialAirHi`.
  - This should be investigated from Captain Falcon up-special physics/root
    motion, wall/ledge collision, and cliff catch decomp paths, not by changing
    Slippi diagnostics.

## 2026-07-05 DamageFlyN handoff fixed P2 source 2763

- Spark sidecars:
  - Two Codex 5.3 Spark sidecars were attempted for bounded 2763 diagnostic
    questions, but both hit the Spark usage limit. Work continued locally under
    the orchestrator plan.
- Replay symptom:
  - Before the fix, P2 source `2763` left source `DamageFlyN` action id `88`
    as Rust generic `Fall` action id `29`, while Slippi expected common action
    id `38`.
  - After adding state `38`, the remaining X drift was exactly the old
    knockback X being duplicated into Rust `self_vel.x`.
- Decomp anchors:
  - `ftCo_Damage.c`: `ftCo_DamageFly_Anim` calls `ftCo_8008F744`; when frames
    are no longer remaining and freeze bit `x221C_b6` is clear, it calls
    `ftCo_80090780`.
  - `ftCo_DamageFall.c`: `ftCo_80090780` changes to motion state `0x26`
    (`DamageFall`), calls `ftCommon_ClampAirDrift`, then `ftCommon_8007EBAC`.
  - `ftCo_DamageFall_Phys` calls `ft_80084DB0`, which runs fall/fastfall and
    `ftCommon_8007D268` air drift from `fp->self_vel.x`.
  - `ftcommon.c`: `ftCommon_ClampAirDrift` only clamps `self_vel.x`; it does
    not fold knockback/attack velocity into self velocity.
- Runtime fixes:
  - Added source-backed `MotionState::DamageFall` with Melee action id `38`,
    source action table index `1`, and source action key `DamageFall`.
  - `DamageFly*` completion now enters `DamageFall` instead of generic `Fall`.
  - `DamageFall` entry clamps and exports the self velocity fields while
    leaving knockback in the separate source knockback fields.
  - Removed the projected/exported-velocity backfill from `apply_air_drift`;
    decomp air drift uses the source self velocity field, not the composed
    Slippi/export velocity.
  - Slippi diagnostics now map action state `38` to `DamageFall`, resolve it
    to source table `1` instead of table `38`/`Guard`, and classify it as an
    airborne velocity state.
- Test proof:
  - `source_damage_fly_anim_exit_enters_damage_fall_after_lockout_clears`
    covers the decomp handoff and velocity export separation.
  - `damage_fall_air_drift_does_not_seed_self_velocity_from_projected_knockback_export`
    failed before removing the projected velocity seed and now passes.
  - Runtime diagnostic tests cover action-state mapping, action identity, and
    air-speed classification for `DamageFall`.
- Replay proof:
  - Trace P2 source `2763-2764` now matches state id `38`, source key
    `DamageFall`, source table `1`, position, and Y velocity. The only
    remaining self-X display difference at source `2763` is one milli from
    f32 rounding and is within the replay gate's velocity tolerance once
    `DamageFall` is correctly classified as airborne.
  - `replay check --frames 3200` now reports first classified divergence at
    P2 source `2787`, core `2910`, `PositionDrift`, in `SpecialAirHi`.
- Current next target:
  - Resume bottom-up work at P2 source `2787` `SpecialAirHi` position drift,
    then the expected `CliffCatch` at source `2831`.

## 2026-07-05 SpecialAirHi command + CliffCatch + AttackAir completion frontier

- SpecialAirHi command/root-motion fix:
  - Decomp anchors:
    - `src/melee/ft/ftanim.c:381-386`: `ftAnim_8006EBA4` advances animation,
      runs action commands, then common animation side effects.
    - `src/melee/ft/ftaction.c:455-475`: command opcode writes
      `fp->cmd_vars[idx]`.
    - `src/melee/ft/chara/ftCaptain/ftCa_SpecialHi.c:70-89` and
      `179-185`: Captain up-special IASA consumes `cmd_vars[0]`, sets
      `mv.ca.specialhi.x2_b1`, and may update facing.
    - `src/melee/ft/ft_084E.c:119-123`: SpecialHi transition velocity uses live
      `fp->facing_dir`.
  - Runtime fix:
    - `SpecialHi`/`SpecialAirHi` now run source command events before the
      Falcon up-special IASA/physics path.
    - `source_special_hi_transn_self_velocity` uses live facing rather than
      entry facing.
  - Replay proof:
    - P2 source `2799-2801` now matches facing, state, position, and velocity.

- SpecialAirHi ledge grab / CliffCatch fix:
  - Decomp anchors:
    - `src/melee/ft/chara/ftCaptain/ftCa_SpecialHi.c:127-149`:
      airborne SpecialHi calls `ft_CheckGroundAndLedge(gobj, 0)`, then
      `ftCliffCommon_80081298` / `ftCliffCommon_80081370` when `x2_b1` is set.
    - `src/melee/ft/ft_081B.c:281-300`: `ft_CheckGroundAndLedge` updates
      collision positions and passes the requested facing direction.
    - `src/melee/mp/mpcoll.c:2476-2505`: ledge checks require can-grab-ledge
      flags and downward motion; facing dir `0` checks both ledge sides.
  - Runtime fix:
    - `SpecialHi`/`SpecialAirHi` now set air ledge-grab flags when
      `captain_special_hi_x2_b1` is live and ledge cooldown is clear.
    - SpecialHi ledge wrapper uses source facing dir `0`, matching the decomp
      both-sides ledge query.
  - Replay proof:
    - P2 source `2831-2833` now enters `CliffCatch` action `252` at Battlefield
      right ledge and matches Slippi position/facing/velocity.

- AttackAir completion fix:
  - Replay symptom:
    - P1 source `2862` previously matched position and velocity but stayed
      Rust `AttackAirLw`; Slippi/decomp expected `Fall`.
  - Decomp anchors:
    - `src/melee/ft/chara/ftCommon/ftCo_AttackAir.c:143-151`:
      `ftCo_AttackAir_Anim` calls `ftCo_Fall_Enter` when
      `!ftAnim_IsFramesRemaining(gobj)`.
    - `src/melee/ft/ftanim.c:516-538`: `ftAnim_IsFramesRemaining` scans active
      animated JObjs/parts rather than checking a hardcoded action id.
    - `src/melee/ft/ftmotionstates.c:897-904`: `AttackAirLw` uses the shared
      `ftCo_AttackAir_Anim` callback.
    - `src/melee/ft/chara/ftCommon/ftCo_Fall.c:52-76` and `208-216`:
      `ftCo_Fall_Enter` installs Fall, and Fall/AttackAir share
      `ft_80084DB0` physics.
  - Runtime fix:
    - Added a strict compact-runtime equivalent of "no frames remaining" that
      only fires when extracted source total frames exist and the live source
      animation frame reaches them.
    - Shared `AttackAir*` branch now runs source command events, advances the
      source animation frame, enters Fall on exhaustion, and lets the same tick
      proceed through airborne IASA/physics from the installed Fall state.
  - Test/replay proof:
    - Red test first failed:
      `aerial_attack_anim_end_installs_fall_before_same_tick_airborne_iasa`.
    - Focused aerial-attack tests now pass.
    - Trace P1 source `2858-2864` now matches through the old frontier:
      `AttackAirLw` through `2861`, `Fall` at `2862-2863`, and `JumpAerialF`
      at `2864`.

- New frontier:
  - `replay check --frames 3200` reports first supported/classified mismatch at
    P2 source `2887` (`Dash` vs expected `WalkSlow`), but the trace shows the
    real earlier root is unsupported source state `259`, `CliffEscapeQuick`,
    starting around P2 source `2878`.
  - Rust is already in `Landing` while Slippi expects `CliffEscapeQuick` near
    x `26`, so the next foundational batch should implement/verify the
    decomp ledge-getup escape quick path (`CliffWait` -> `CliffEscapeQuick`)
    before touching grounded dash/walk symptoms.

## 2026-07-05 AttackAir allow-interrupt frontier

- Current replay frontier after the SpecialHi/FallSpecial work:
  - `replay check --frames 3200` reports P2 source `2939`, core `3062`,
    `state_mismatch`.
  - Rust remains `AttackAirLw`; Slippi expects `JumpAerialB`.
  - Slippi source `2939` pre row is still action state `69` (`AttackAirLw`),
    C-stick is neutral, attack is not pressed, Y is newly pressed, and post row
    is `JumpAerialB` with one aerial jump consumed.
  - P2 source `2938` post action counter is `37`; `AttackAirLw` source total
    frames is `45`, so this is not the animation-exhaustion path.
- Decomp proof:
  - `ftCo_AttackAir_EnterFromMsid` sets `fp->allow_interrupt = false`,
    clears `cmd_vars[0]`/`throw_flags`, changes to the AttackAir motion state,
    then samples animation.
  - `ftCo_AttackAir_Anim` only enters Fall when
    `!ftAnim_IsFramesRemaining(gobj)`.
  - `ftCo_AttackAir*_IASA` gates on `fp->allow_interrupt`; when set, the macro
    calls the same jump helper `ftCo_800CB870` used by fall-family IASA.
  - `ftAction_80071950` is the action command that sets
    `fp->allow_interrupt = true`.
  - Raw `resources/melee/raw/PlCa.dat`, `AttackAirLw` subaction script at
    `0x20 + 20612`, contains word `0x5c000000` at script word offset `35`,
    command frame `38`. `0x5c000000 >> 26 == 23`; fighter opcode table index
    `13` maps to `ftAction_80071950`.
- Rust gap:
  - `AttackAir*` currently applies only the handcrafted landing-lag
    `cmd_var0` helper, then advances animation, then either enters Fall on
    source action completion or applies air drift.
  - Rust has no core `fp->allow_interrupt` equivalent and no extracted
    `fighter.allow_interrupt` source-script event in the compact runtime event
    stream.
  - The decomp-shaped fix is to carry this action command through extraction,
    frame-data capsules, runtime mapping, and PlayerState, then run the
    AttackAir IASA callback only while `source_allow_interrupt` is true.

## 2026-07-05 RebirthWait handoff fixed P1 source 3071

- Replay symptom:
  - After the ledge-drop drift fix, `replay check --frames 3200` reported P1
    source `3071`, core `3194`, as a `RebirthWait` vs expected `Fall`
    state mismatch.
  - Focused trace before the fix:
    - source `3069`: expected/actual `Rebirth`, frame `58`, y `80933`,
      velocity y `-933`.
    - source `3070`: expected/actual `Rebirth`, frame `59`, y `80000`,
      velocity y `-933`.
    - source `3071`: expected `Fall`, actual `RebirthWait`, expected y
      `79870`, actual y `80000`, expected velocity y `-130`, actual `0`,
      stick y `-125`.
- Decomp anchors:
  - `src/melee/ft/ft_0D4D.c:165-172`: `ftCo_Rebirth_Anim` decrements
    `mv.co.common.x0`; when it reaches zero, it calls `ftCo_800D5600`.
  - `src/melee/ft/ft_0D4D.c:254-276`: `ftCo_800D5600` switches to
    `ftCo_MS_RebirthWait` and loads `p_ftCommonData->x5D4`.
  - `src/melee/ft/ft_0D4D.c:289-327`: `ftCo_RebirthWait_IASA` can call
    `ftCo_Fall_Enter` and then applies `p_ftCommonData->x5D8` intangibility.
  - `src/melee/ft/fighter.c:906-911`: fighter callbacks are split across
    ordered procs; animation-state changes can affect later input/physics procs
    in the same frame.
  - `src/melee/ft/chara/ftCommon/ftCo_Fall.c:52-76`: `ftCo_Fall_Enter`
    installs common `Fall` and resets the Fall blend state.
- Rust root cause:
  - `advance_source_rebirth_state` installed `RebirthWait` when the `Rebirth`
    timer was already zero, but then returned `false`, causing the monolithic
    Rust fighter tick to `continue`.
  - That skipped the same-frame `RebirthWait_IASA` opportunity, so the held-down
    input could only enter Fall on the next source row. This produced the exact
    one-frame gravity lag at source `3071`.
  - This was a phase-order parity bug, not a Slippi quirk and not a velocity
    constant issue.
- Runtime fix:
  - Split the Rebirth handoff path so `Rebirth -> RebirthWait` records that the
    wait state was just installed, then runs the RebirthWait IASA gate in the
    same Rust tick.
  - If no IASA transition happens on that handoff tick, Rust returns without
    decrementing the newly loaded `x5D4` wait timer, preserving the decomp
    `RebirthWait_Anim` boundary.
- Test/replay proof:
  - Red test first failed with actual `RebirthWait` instead of `Fall`:
    `rebirth_handoff_runs_rebirth_wait_iasa_before_same_tick_fall_physics`.
  - New focused tests now pass:
    `rebirth_handoff_runs_rebirth_wait_iasa_before_same_tick_fall_physics`
    and `rebirth_wait_timer_expiry_enters_fall_before_same_tick_fall_physics`.
  - Adjacent respawn tests pass:
    `dead_down_rebirth_uses_source_platform_top_and_first_velocity_step`,
    `rebirth_wait_exit_installs_source_hurt_intangibility_from_x5d8_before_fall`,
    `rebirth_wait_down_input_exits_spawn_platform_into_fall_with_source_intangibility`,
    and `battlefield_stage_promotes_source_rebirth_platform_points_from_map_head_x280`.
  - Focused trace P1 source `3069-3073` now matches:
    `Rebirth` through source `3070`, `Fall` at source `3071`, and no position,
    velocity, or state deltas through `3073`.
  - `replay check --frames 3200` now reports no classified divergence, no state
    mismatch, no position drift, no unsupported states, and `frames_compared:
    3200`.
- Current next target:
  - Push the replay window past `3200` and investigate the next non-witness
    frontier bottom-up against the decomp.

## 2026-07-06 Right-wall ECB frontier near P2 source 3023-3025

- Current focused red test:
  - `cargo run -q -p mole_cli -- tests run -p mole_runtime slippi_match_start_frame3025_p2_attackairhi_right_wall_scrape_matches_source_position`
  - Fails at source `3025`: expected x `72291`, actual x `72196`; action,
    state id, and velocity are otherwise aligned (`AttackAirHi`, state `68`,
    velocity x/y `-533`/`1880`).
- First known real drift in the focused trace is P2 source `3023`:
  - expected x `71377`, actual x `71391`; y/state/velocity align.
  - This is a collision/ECB phase mismatch, not a replay realignment issue.
- Decomp anchors already verified:
  - `ftCo_AttackAir_Coll` routes through `ft_80082C74` -> `ft_80081D0C` ->
    `mpColl_800471F8`.
  - `mpColl_800471F8` calls `mpCollPrev`, `mpColl_LoadECB_inline(coll, 6)`,
    then the airborne collision callback with flags `0`.
  - `mpColl_80043754` substeps by max movement/ECB delta over `6.0F`,
    interpolates ECB each substep, sets `prev_pos = cur_pos`, advances
    `cur_pos`, then runs the collision callback.
  - Right-wall hit gathering uses a `coll->x38 != mpColl_804D64AC` selector for
    `mpCheckRightWallRemap` vs `mpCheckRightWall` on several ordinary sweeps.
- Rust evidence:
  - The AttackAir collision route uses flags `0` and mirrors the broad
    `mpColl_80043754` substep/interpolation order.
  - Rust does not currently model the decomp `coll->x38` token selector.
  - Battlefield line `16` is static in generated stage data; its current and
    prior vertices are equal, so remap/plain is a real architectural gap but
    not yet proven to be the direct owner of the source `3023` drift.
- Live ECB evidence:
  - The right-wall correction math using Rust's current AttackAirHi frame `3`
    ECB produces x `71.391197`, matching the Rust actual at source `3023`.
  - Slippi expects roughly x `71.377052`, which is closer to a tiny difference
    in the live JObj/pose ECB input than to a state/velocity/callback-route
    mismatch.
- Next proof path:
  - Work bottom-up through `mpColl_LoadECB_JObj`, `lb_8000B1CC`, frame sampling,
    and generated Rust matrix/evaluator behavior for AttackAirHi frames `3-5`.
  - Only patch after proving which decomp primitive the Rust evaluator or
    collision token path fails to represent.

## 2026-07-13 Rejected cross-action pose retention hypothesis

- Experimentally retaining live JObj SRT channels across zero-blend action
  changes is not decomp-accurate and regressed the replay at source frame `8`
  (P2 remained in `EscapeAir` instead of entering `LandingFallSpecial`).
- Decomp proof:
  - `Fighter_ChangeMotionState` calls `ftAnim_8006EBE8`, then
    `ftAnim_8006E9B4`.
  - For zero blend, `ftAnim_8006EBE8` calls `ftAnim_8006FA58` before installing
    the new animation.
  - `ftAnim_8006FA58` copies costume `HSD_Joint` SRT values into the live
    fighter JObjs through the `lb_8000B4FC` family.
- Conclusion: bind-pose initialization at ordinary zero-blend action changes is
  correct. The retained-pose experiment has been removed.
- Next target: prove either a mismatch in mutable AObj/FObj interpreter state
  within the installed action or in `mpColl_80043754` ECB interpolation and
  substep timing. Do not add persistent cross-action pose state.

## 2026-07-13 Frame 3026 audit conclusion

- AObj/FObj LOAD/WAIT/UPDATE/KEY semantics in the generated sampler match the
  decomp for AttackAirHi frames 4-7; no interpreter arithmetic mismatch was
  found.
- `mpColl_80043754` substep order, ECB interpolation, and callback timing also
  match the decomp in the audited corridor.
- A trial treating all AttackAir map collision as an extra post-animation pose
  regressed the first frontier to source frame `1340`; it was reverted. Rust's
  regular AttackAir tick has already advanced the stored animation frame before
  map collision.
- Corrected one independent decomp mismatch: ECB-load bottom preservation now
  depends only on `source_coll_x130_locked`, matching `coll->x130_flags &
  CollData_X130_Locked`; the numeric lock counter alone no longer preserves the
  bottom. This did not move the frame `3026` frontier.
- Verified after the correction:
  - `cargo check -p mole_core` passes.
  - Replay through `3600` still first diverges at P2 source `3026`, actual x
    `72740`, expected x `71713`, with state and velocity matching.
- Remaining evidence points to live-pose frame representation/request state or
  the precise right-wall geometry query, not broad AObj/FObj interpretation,
  collision substep order, or cross-action pose retention.

## 2026-07-13 Frame 3026 resolved; next full-replay frontier

- Root cause: the baked Falcon live-JObj evaluator uses TopN `rot_y = +PI/2`.
  In Melee model coordinates this is the negative-facing ECB orientation, but
  Rust treated the baked result as positive-facing and mirrored it for P2.
  AttackAirHi pose 6 is asymmetric, so that reversal extended the left ECB edge
  into Battlefield wall line 16 and falsely corrected x from `71.7127` to
  `72.7396`.
- Fix: `source_ecb_for_facing` now leaves the baked sample unchanged for
  negative model facing and mirrors it for positive model facing. A focused
  asymmetric-ECB regression records that convention.
- Added persistent `source_model_facing` state so direct `facing_dir` writes can
  remain distinct from decomp call sites that also execute `ftPartSetRotY`.
  `ftCliffCommon_80081370` preserves this distinction. The field is included in
  deterministic checksums and rollback snapshots.
- Verification:
  - Focused cliff/model-facing regression passes.
  - Replay check through source frame `3600` reports no classified divergence,
    no position drift, and zero state mismatches.
  - Full replay contains `5313` compared frames. Its next frontier is P1 source
    frame `4119` / core frame `4242`: expected Slippi state `86` (`EscapeAir`),
    while Rust reports action state `236` with the same position. State `196`
  is also unsupported for 30 later frames. This is separate from frame 3026.

## 2026-07-13 Frame 4119 resolved; trigger-timer knockback frontier

- The apparent stale-move discrepancy was rejected: raw Slippi records P1 at
  `0% -> 7%`, so P2's late Nair is fresh and unscaled. Decomp stale scaling is
  read before `plStale_UpdateStaleMovesFromFighter` records the confirmation.
- P1 presses L at source `4118` and is hit airborne at `4119`; the source
  trigger timer is `1`. Slippi's expected knockback is exactly `0.95` of the
  ordinary result (`62.323532 -> 59.207355`).
- `source_damage_results_for_stages` now supplies the source airborne
  trigger-timer/V-cancel defense ratio from rollback-owned input timers. It
  applies for timer values `1..=3` and excludes aerial-attack states.
- The focused replay regression
  `slippi_match_start_frame4119_p2_nair_hits_p1_escape_air` now matches state
  `DamageAir3` (`86`), position, and velocity `(1256, 1256)`.
- Full replay now advances to P2 source `4182`: Slippi expects
  `GuardSetOff` (`181`) with ground velocity `888`, while Rust remains
  `GuardOn` (`178`) at the same position. State `196` remains unsupported for
  30 later frames.

## 2026-07-13 Frame 4182 resolved; next frontier 4192

- Decomp scheduler evidence shows animation/scripts run globally before IASA,
  followed by physics/map and the priority-9 `ftColl_8007AE80` JObj collision
  refresh. GuardOn installs its descriptor during IASA; active hitbox attributes
  remain distinct from the later refreshed geometry.
- Runtime now observes the extra refreshed geometry sample when another fighter
  installs GuardOn on that tick, while retaining the active up-air attributes.
  The focused regression reaches GuardSetOff with exact ground velocity `0.888`.
- Earlier frame 3183 and 4119 regressions remain green.
- Full 5313-frame replay advances to P2 source `4192`: expected GuardSetOff
  ground velocity `0.488`, actual `0.564`, followed by an erroneous GuardSetOff
  restart. Geometry shows Falcon up-air's frame-14 attribute replacement
  confirming against the same shield again. The next audit is decomp hit-victim
  log continuity across hitlag and in-place hitbox attribute replacement.
- A trial broadening the Rust victim-log key/removing lifecycle identity was
  rejected and reverted because it suppressed the valid first shield contact.
# 2026-07-13: frame 4192 resolved, stopping point

- Root cause was the action-script extractor rewinding opcode 2 (absolute/asynchronous timer) from frame 14 to frame 13. Melee's command executor cannot rewind: an already-passed absolute timer continues in the current script tick.
- Captain AttackAirHi physically writes its frame-14 hitboxes and then clears them. The incorrect rewind sorted the clear before the writes and baked a false active hitbox, causing P1 to re-hit P2's shield at source frame 4192.
- Decoder now uses `current_frame.max(command_frame_value(value))`; a focused decoder regression covers this ordering.
- Null action-script pointers are treated as empty command streams during compact runtime export.
- Regenerated the compact source manifest and baked runtime frame capsules. No raw decomp/ISO data was added to runtime.
- Regressions at source frames 3183, 4119, 4182, and 4192 all pass.
- Full 6000-frame replay check advances the first classified divergence from 4192 to source frame 4378: P1 position drift in GuardSetOff, expected x=-16.509, actual x=-18.084, while state and velocity agree (-0.592 x velocity). This is the next unresolved frontier.

## 2026-07-13: replay frontier advanced through source 4441

- Source 4378 P1 GuardSetOff had an extra exit-hitlag ASDI displacement. Damage ASDI is now excluded for GuardSetOff; regression `slippi_match_start_frame4378_guard_setoff_exits_hitlag_at_source_position` passes.
- Source 4403 P2 required the decomp `ftCo_8008DCE0` tier-3 floor reflection: incidence above `PI/2 + x1E8` reflects Y and multiplies it by `x1EC`. Added the two PlCo common-data fields and regression `slippi_match_start_frame4403_grounded_downward_hit_reflects_from_floor`.
- Source 4426 P1 L-cancel required a dedicated x67F-equivalent timer latched through hitlag. It must remain separate from the existing passive/tech LR timer. Regression `slippi_match_start_frame4426_lcancel_timer_survives_hitlag` passes.
- Source 4441 P2 exposed an exact unsupported 0.95 multiplier. Decomp tracing proved `ftColl_8007A06C` obtains `defense` from `Player_GetDefenseRatio`, whose static-player field initializes to 1.0; input timers do not implement a V-cancel multiplier in this path. Removed the invented `source_v_cancel_defense_ratio` helper and pass `defense: 1.0` directly. Do not reintroduce timer-based defense scaling.
- Removing that replay-masking behavior honestly moves the full-check frontier backward to source 4119/core 4242, P1 `velocity_drift` in EscapeAir. The previously added frame4119 expectation likely encoded the invented 0.95 behavior and must be re-derived from the decomp before further edits. First position drift is source4125 and first state mismatch source4137.

## 2026-07-14: decomp V-cancel and DownFowardD milestone

- Frame 4119's 0.95 airborne knockback scale is real, but it is not the static
  player defense ratio. `ftCo_Damage_CheckAirMotion` applies common-data x190
  (`0.95`) when the current motion is in its exact airborne whitelist and the
  x680/x684 trigger timers satisfy x18C (`2`) and x1C (`40`). Rust now models
  those fields and that exact check at damage entry; collision defense remains
  `1.0`.
- P2's unsupported state 196 is `DownFowardD`. The decomp checks down-roll input
  when DownBound animation completes, selects the D variant because the source
  helper distinguishes only exact `DownWaitU`, advances animation before same-
  tick physics, consumes root motion through `ft_80084FA8`, and clamps through
  `ft_800827A0`. Rust now follows that path and uses a compact generated root-
  motion table for `DownFowardD` only.
- Focused regressions pass at source 4119, 4488, and 4511. The full 6000-frame
  replay advances to source 4518/core 4641: expected P2 `DamageN3` (80), actual
  `DownFowardD` (196), with positions equal. P1's frame-11 up-air geometry does
  not overlap the runtime DownFowardD hurt capsules, so the expected hit is
  absent.
- Decomp follow-up found the DownFowardD subaction starts with fighter command
  index 16 (`ftAction_80071A14(..., 2)` -> `ftColl_8007B62C`, x1988=2). The
  extractor/runtime currently has no representation for this whole-fighter
  collision mode. This is an architectural extraction gap, but it does not by
  itself explain the missing expected hit: adding suppression would move in the
  wrong direction. Reconcile x1988 mode-2 semantics and animation/collision
  pose timing before implementing anything.
- Stopping conclusion: parity is not complete. The next proven frontier is
  source frame 4518, and no speculative fix should be committed for it.

## 2026-07-14: replay frontier advanced to source 4721

- Fixed DownFowardD hurt-capsule root transforms without changing fighter-relative
  hit capsules; frame 4518 and the earlier frame 2583 Raptor Boost case pass.
- Corrected GuardOn/Guard Z-grab gating to decomp `held LR && A`; frame 4557 passes.
- Executed SpecialSStart frame-15 commands before collision, restoring the
  decomp detect callback at frame 4641.
- Corrected the passive-window scheduler boundary at frame 4693 and preserved
  Landing-to-SquatWait same-frame ground physics at frame 4709.
- Current verified frontier is source 4721/core 4844: P2 expects GuardOn (178)
  but runtime enters GuardSetOff (181) from P1 AttackAirHi contact. P2 is aligned
  through 4720; P1 first drifts at 4722.
- Rejected two broad fixes: delaying all aerial script events regressed frames
  3183 and 4641, and suppressing newly installed shields did not reproduce the
  source state. The remaining issue is specifically the ordering between the
  frame-7 up-air geometry refresh, a frame-0 GuardOn object, and shield-hit state
  routing. Do not generalize it into global script or shield latency.
- Stopping conclusion: parity is not complete. Preserve the verified fixes and
  derive this scheduler edge from the decomp before changing collision commit.

## 2026-07-14: replay frontier advanced to source 4788

- Fixed the frame-4721 Wait-to-GuardOn collision phase, stale-scaled shield
  victim hitlag at frame 4728, and GuardSetOff timing from the decomp JObj-rate
  convention at frame 4745.
- The frame-4788 Bair drift is not a hitbox decoder error. Decomp
  `spawn_hitbox_4.base_knockback` is the top nine bits of word 4. Runtime
  correctly decodes Bair hitbox 0 as BKB 20 and hitboxes 1/2 as BKB 0.
- All three Bair hitboxes geometrically confirm against P2 at frame 4788.
  Runtime's same-hit-group victim-log arbitration retains hitbox 1 (BKB 0),
  while the source velocity implies hitbox 0 (BKB 20). The resulting knockback
  is 119.04159 instead of approximately 139.04.
- A trial that replaced greatest-overlap arbitration with unconditional first
  hitbox order fixed the isolated victim-log contract but changed the replay
  before frame 4788 (P2 remained in state 69 rather than entering DamageFlyN),
  so it was reverted. Do not commit that broad ordering change.
- Stopping conclusion: parity is not complete. The next root-cause question is
  the exact decomp arbitration/commit ordering for multiple same-group hitboxes;
  do not alter BKB extraction or invent a per-move override.

## 2026-07-14: frame 3268 arbitration/DI audit

- Replacing overlap selection with unconditional hitbox-slot order was rejected.
  It exposed a false frame-3268 82-degree result and caused earlier replay
  regressions. The source result is the 78-degree NAir capsule.
- This is not directional influence: P1's hit-frame main and C sticks are
  neutral, and decomp `ftCo_8008E5A4` applies DI only from
  `ftCo_Damage_OnExitHitlag`. The expected vector has the raw 78-degree angle.
- `inlineB0`/`lbColl_80008688` establish same-group victim-log side effects,
  but do not prove that the first slot supplies damage attributes. The candidate
  entering `ftColl_80076ED8` is distinct from the capsules whose logs update.
- Restored the verified non-shield overlap selection and retained first-order
  shield handling. Replay is clean through source 3400; the full check again
  reaches source 4788/core 4911 as the first divergence.
- Stopping conclusion: frame 4788 remains unresolved. A faithful fix requires
  separate translations of collision-candidate selection and group-log side
  effects. Do not add immediate normal-hit DI or unconditional slot order.
## 2026-07-14 frame 3268 / 4788 collision-order conclusion

- Decomp `ftColl_80078C70` iterates physical hitbox slots in ascending order and passes the exact first colliding slot to `ftColl_80076ED8`; `inlineB0` then fans `victims_1` across every active capsule with the same `x4` group. Deepest-overlap arbitration is not source-faithful.
- `p_ftCommonData->x7A8` is exactly `0.01f` (`PlCo.dat` file offset `0xA788`, bytes `3C 23 D7 0A`). The exact translated `lbColl_80006E58` reports frame-3268 slot-0 overlap around `0.3496` source units, so that contact is not a phantom hit. Do not implement the earlier phantom hypothesis for this frame.
- The exact `lbColl_80006E58` segment solver and hurt-matrix radius scaling are now translated in `collision.rs`; focused primitive tests pass.
- First-slot processing correctly reflects the decomp but exposes an unresolved upstream representation mismatch at frame 3268: runtime slot 0 (raw 82 degrees) geometrically contacts, while the replay-derived velocity requires the 78-degree result previously obtained via invented deepest-overlap arbitration. The next investigation must trace collision-time hit capsule state / replay DI timing, not add another selection heuristic.
- A separate `victims_2` lane is architecturally required only when a real `< 0.01f` phantom contact is observed. Ordinary `victims_1` also still needs explicit per-capsule same-group fanout for distinct lifecycle IDs.

## 2026-07-14 final frame-3268 conclusion

- The proposed ordinary-hitbox `part_to_joint` remap was falsified by the decomp. `ftAction_8007121C` uses `fp->parts[bone].joint` directly when `use_common_bone_ids` is false; only true common IDs call `ftParts_GetBoneIndex`. Falcon NAir uses ordinary IDs, so its current direct pose indices are source-faithful.
- Added extraction and sampling support for the actual common-ID path. This is architectural coverage, not the frame-3268 fix.
- Restored the established collision-time hurt-capsule scheduling and source-matrix conversion after both experiments failed to explain the replay.
- Final conclusion: parity remains unresolved at source frame 3268. Exact slot-order collision selects NAir slot 0 / 82 degrees in the Rust geometry while the replay requires slot 1 / 78 degrees. DI, phantom threshold, hitbox decode/order, ordinary/common bone resolution, exact capsule intersection, and the two matrix/scheduling trials have been ruled out. Do not add overlap arbitration or another heuristic; the remaining work requires a direct source/runtime collision-time JObj matrix capture comparison.

## 2026-07-14 spawn lifecycle and live ECB conclusions

- The frame-3268 root cause was not matrix extraction. Runtime correctly samples
  geometry frame N+1 for the current animation pose, but newly enabled hitboxes
  must begin as a degenerate current/current capsule. Decomp `ftColl_8007AD18`
  writes `x4C` and copies it to `x58` for `HitCapsule_Enabled`; only sustained
  hitboxes retain a previous/current sweep. Collapsing only lifecycle-start
  geometry makes NAir slot 1 / 78 degrees win without arbitration heuristics.
- The live ECB pose sampler is already directional. Applying a second mirror in
  `source_ecb_for_facing` was incorrect for both signs: source frame 3026
  (facing -1) and source frame 4904 (facing +1) each require the raw sampled ECB.
  The helper is therefore identity rather than a facing transform.
- Focused frame-3268 and frame-4904 regressions pass after these changes. The
  authoritative 6000-frame replay check now reaches source frame 4926/core 5049,
  P2 `AttackAirHi`, with position-only X drift: actual -75.007, expected -73.055,
  velocity exact (+0.163, +1.620). Parity is not complete; frame 4926 is the
  next frontier.
## 2026-07-14 frame 4926 conclusion: mutable AObj/FObj state is the remaining owner

- The focused regression `slippi_match_start_frame4926_p2_attack_air_hi_expanding_ecb_matches_left_wall` remains red: Rust corrects P2 X from `-73.055176` to `-75.006683` at source frame 4926.
- `mpColl_80046224_LeftWall` and Battlefield line 17 were independently recomputed candidate-by-candidate. With Rust's pose-6 ECB, the decomp formula selects `-75.00668` exactly. Wall projection is not the mismatch.
- `CollData.x38` / `mpCheckLeftWallRemap` is not the owner. Battlefield has no dynamic collision lines, and the remap path is geometrically equivalent here; the current bottom-to-right edge check also calls ordinary `mpCheckLeftWall` directly.
- The decisive input mismatch is the collision-time live JObj pose. Rust's generated evaluator reparses each FObj track from byte zero for numeric pose frame `6.0`, producing right X `7.091729`; that shape predicts the incorrect wall correction bit-for-bit. The prior live shape does not collide, matching Slippi's unchanged root position.
- Melee retains mutable `HSD_AObj` / `HSD_FObj` execution state (`FIRST_PLAY`, `curr_frame`, stream cursor, state, time, p0/p1, d0/d1, flags) and collision reads the JObj SRT left by the animation scheduler. The generated stateless `falcon_sample_fobj_value` architecture cannot represent the state-4/state-5 update cadence.
- Do not apply a frame-minus-one special case or interpolation heuristic. The faithful next implementation is a compact persistent AObj/FObj interpreter translated from HSD, with action entry initializing it and animation ticks mutating live JObj SRT before map collision. Add a direct sequential-vs-stateless regression first.
- The repository has no independent live Dolphin JObj/ECB capture tooling. A future oracle requires external instrumentation around `mpColl_LoadECB_JObj`; Slippi post-frame data does not contain joints or ECB.

### Correction after exact sequential interpreter comparison

- The narrower claim that state-4/state-5 FObj cadence itself changes AttackAirHi pose 6 was disproved. A reference sequential `HSD_FObjInterpretAnim` comparison across all 152 action-71 tracks and frames 0..34 matched the generated stateless track sampler exactly (maximum delta 0). Do not build a persistent FObj cursor solely to fix frame 4926.
- The decomp still exposes a real live-JObj boundary that the stateless evaluator omits: `ftAnim_80070758` removes old AObjs but does not reset JObj SRT to bind pose. A new action only writes channels for which it has FObj tracks; untracked channels retain the previous action's live SRT.
- AttackAirHi has zero tracks on several nodes, including node 6 in the ancestry of ECB source joint 8, while the generated evaluator reconstructs every action from bind SRT. This is now the strongest architectural owner of the intermediate wall correction at frame 4926.
- Slippi enters AttackAirHi at source 4921 with action counter 1. The decomp action request samples pose 0 on entry; the materialized frame-data record labeled action frame 6 correspondingly uses pose 5. A global collision pose `cur_anim_frame - 1` trial is not valid: it regresses the already exact source-3026 wall scrape and cascades badly by source4926. The remaining fix must preserve live cross-action JObj channel state rather than shifting every pose clock.

### Correction after action-change reset and blend-table audit

- Cross-action JObj channel retention is also disproved. In the non-blend action-change path, decomp `ftAnim_8006EBE8` calls `ftAnim_8006FA58(fp, TransN, costume_joint->child)` before attaching the new animation. `ftAnim_8006FA58` copies costume bind SRT into the live fighter parts, so channels omitted by the incoming FigaTree do not retain the prior action's SRT.
- An experimental persistent live-pose implementation immediately regressed the replay at source frames 8 and 13 and was removed. Do not restore it.
- Generic action blending is not the frame-4926 owner. Captain Falcon's `PlCa.dat` motion-state table has blend value zero for AttackAirHi and the neighboring aerial attack entries; only a small unrelated set of entries uses blend 6.
- The next exact architectural question is FigaTree-node attachment. The generated evaluator currently treats each FigaTree node index as a costume skeleton joint index, while decomp `ftAnim_8006F7C8` / `ftAnim_8006FCE4` walk `fp->parts`, skip parts according to FighterBone flags, and consult `ftParts_8007506C` / `ftPartsRemap`. Derive and test that mapping before changing production pose evaluation.

### Conclusion after exact FigaTree attachment audit

- FigaTree-node remapping is not the frame-4926 owner for Captain AttackAirHi. This is a same-kind, non-blended full-tree attachment, so `ftAnim_8006FE08` selects `ftAnim_8006F4C8`, not the cross-kind `ftAnim_8006FCE4` remap path or subtree `ftAnim_8006F7C8` path.
- Falcon's source `parts_num`, extracted costume skeleton joint count, and AttackAirHi FigaTree node count are all exactly 63. `flags_b1` denotes an existing live joint; the extracted 63-joint tree supplies those live parts. `flags_b2` is the duplicate interpolation skeleton path, but AttackAirHi's blend count is zero. No node is skipped in this case, so FigaTree node index and extracted costume-joint traversal index coincide.
- Clean focused verification passes source frames 3026 and 4904 and reproduces the sole frame-4926 failure at the established values (`actual x=-75.00668`, expected `-73.055176`, right ECB x=`7.091729`). The experimental live-pose state is absent.
- Do not implement a part remap for AttackAirHi. Current evidence has eliminated wall collision math, global pose offset, FObj cursor cadence, cross-action retention, action blending, and FigaTree attachment. The unresolved boundary is now specifically the HSD JObj SRT-to-matrix/world-position calculation (including JObj flags and parent-scale/rotation semantics) versus the generated evaluator. A faithful next investigation requires a per-joint matrix comparison or a complete line-by-line translation audit of the HSD JObj matrix builder; do not add a replay-specific correction.

### Final audit before stopping

- The exact HSD JObj matrix path was audited and does not explain frame 4926. All six Falcon ECB source chains use ordinary Euler SRT with `JOBJ_CLASSICAL_SCALE`; none uses quaternion, IK, user matrices, or independent-parent/SRT behavior. Joint 8 independently produces the sampled right edge `7.091729`, and the Rust/extraction calculation matches the decomp path under the active flags.
- Ground-to-air ECB lock timing is also not an implementation discrepancy. Decomp `ftCommon_8007D5D4` sets `ecb_lock = 10` and the locked flag. `Fighter_procMap` decrements before invoking `coll_cb` and clears the flag when the counter reaches zero. Rust installs 10 and performs the same pre-collision decrement/clear ordering.
- Therefore no decomp-supported production change was made for frame 4926. The replay remains exact through source frame 4925 and diverges at 4926 with the established P2 X correction. The next investigation would require a new source-side observable (for example Dolphin instrumentation at `mpColl_LoadECB_JObj`) rather than another inferred heuristic.

## 2026-07-15: frame 4926 source capture and fix

- Captured real post-frame-4926 MRAM in `debug/slippi/source-post-frame-4926-mram.raw` using the instrumented playback build under `.research/project-slippi-Ishiiruka`.
- P2 source Fighter is `0x80DE4F20`; CollData is `0x80DE5610`. Source desired ECB is top `(0, 10.5461216)`, bottom `(0, 2.07768059)`, right `(4.99694443, 7.86185122)`, left `(-4.99694443, 7.86185122)`.
- The six live source JObj matrices match Rust's raw pose geometry. The raw horizontal span is `9.993889`, initially asymmetric at roughly `-2.902168/+7.091720`.
- Root cause was generator-side scale-space conflation. Decomp `ft_80081B38` stores CollData `x128/x12C = 10.0 * fp->x34_scale.y`; source live values are exactly `10.0`. `mpColl_LoadECB_JObj` then canonicalizes the 9.993889 span to symmetric `+/-4.996944`. Rust incorrectly generated `10.0 * FALCON_MODEL_SCALE = 9.7`, skipped canonicalization, and falsely corrected against the left wall.
- Added red generator regression `test_live_jobj_payload_keeps_coll_data_minimums_in_fighter_scale_space`, removed model-render scaling from the two CollData minima, and regenerated `falcon_ecb.rs`.
- Focused runtime regressions at source frames 3026, 4904, and 4926 all pass.
- Replay now advances past 4926. Next first divergence: source frame 4958/core 5081, P1 state mismatch, expected `Fall` (29), actual `AttackAirN` (85), with position identical at the mismatch. At source 4960 both are Landing but x differs by +0.344, downstream of the state mismatch.

## 2026-07-15: frame 4958 DamageAir2 completion fix

- The apparent `AttackAirN` was only the stale visible `MotionState`; authoritative action ID 85 is canonical source-only `DamageAir2` (table index 175). Inputs at 4958 are neutral.
- `DamageAir2` has 24 extracted frames (0..23). Rust's damage animation completion used `anim_frame >= total_frames`, while decomp `ftCo_Damage_Anim` tests `!ftAnim_IsFramesRemaining`; source exits after processing frame 23.
- Added replay regression `slippi_match_start_frame4958_p1_damage_air2_animation_enters_fall` and a damage-specific no-frames-remaining predicate using `total_frames - 1`.
- On the same transition, `ftCo_Fall_Enter` preserves x8c knockback separately but exports self velocity only. Rust already resynchronized x but not y; `enter_fall` now also resynchronizes exported y from `source_self_velocity_y`.
- Focused regression passes with exact state and composed x/y velocity at frame 4958.
- Replay now advances to source frame 4966/core 5089. P1 is incorrectly captured (`226`) and displaced to x=-42.262 while source remains Landing at x=-57.337. Investigate the preceding P2 grab confirm/held-victim geometry as the next owner.

## 2026-07-15: frame 4965 false grab investigation paused

- The first causal mismatch is source frame 4965/core 5088: Rust confirms P2 Catch hitbox 1 against P1 Landing hurtbox 3, then enters CatchPull/CapturePulledLw. The frame-4966 displacement is downstream.
- The focused red regression is `slippi_match_start_through_frame4966_p2_grab_does_not_capture_p1`.
- Plain and matrix-scaled capsule calculations both overlap, while source does not confirm. The Catch script activation at frame 6 matches the extracted source script, and there is no decomp facing-direction rejection. Do not add a facing or replay-specific heuristic.
- Decomp owner is `ftColl_80078A2C` -> `lbColl_80007ECC` / `lbColl_80006E58`, including live capsule state, eligibility/log gates, and the stage-wall rejection in `ft_80084CE4`. Rust's grab path is structurally simplified, but no omitted gate has yet been proven to reject this exact pair.
- An instrumented post-frame-4965 MRAM capture was attempted. The first launch was blocked by command-line path splitting (`Unexpected parameter 'Smash'`). An argument-safe relaunch remained idle for 150 seconds and produced no dump; it was stopped. No Dolphin process from this investigation remains running.
- Conclusion: authoritative live source capsule/flag data is still required before a lossless production change is justified. Resume by repairing the playback-launch/capture path, then compare the source hitbox/hurtbox and grab eligibility fields at post-frame 4965.
### 2026-07-15: capture tooling and GuardSetOff follow-up

- Slippi/Dolphin and a DALL-E login splash opened unexpectedly. No authentication is required; keep replay checks local/headless and treat browser launches as tooling faults.
- The frame 5011 GuardSetOff velocity (expected `0.564`, actual `0.618`) is not safely fixed by applying the current Rust stale-table multiplier to every shield setoff. That experiment fixed 5011 but regressed the earlier frame 4722 setoff (expected `-1.840`, actual `-1.730`) and was reverted.
- Decomp evidence: `ftColl_80076CBC` stores `getEnvDmg(hit0->damage)` in victim `x19A4`; `ftCo_80092F2C` derives both animation rate and pushback from `x19A4`. Next investigation must establish when/how `HitCapsule.damage` itself receives stale scaling and why the current Rust stale attack identity differs between frames 4722 and 5011. Do not special-case either replay frame.
- The newly retained previous fighter root position/Z now participates in the deterministic checksum because it affects sustained collision capsules.

### 2026-07-15: frames 5037-5118 parity frontier

- Frame 5037: SpecialS animation completion now redispatches the newly installed Wait input and WalkSlow physics callbacks in the same tick, matching `Fighter_ChangeMotionState` callback replacement.
- Frame 5040: tap-started Dash permits held digital shield through the inclusive early defensive window and enters EscapeF; the new Escape callback applies frame-1 root motion in the same tick.
- Frame 5081: AttackDash now uses baked TransN root motion on entry and subsequent ticks. Falcon's first delta is `1.346154 * 0.97 * -1 = -1.305769`.
- Frame 5118: Squat, SquatWait, and SquatRv all route through the decomp's shared `ft_80083F88 -> ft_80082708 -> ftCo_Fall_Enter` collision path. AttackDash-to-Squat IASA runs the newly installed Squat traction callback in the same tick before edge loss, producing exact `-0.266` air velocity and position.
- Focused regressions for 5037, 5040, 5081, and 5118 pass. The full 5313-frame check now first diverges at source 5124/core 5247, P2 CliffJumpQuick1 position X: expected `-69.234`, actual `-69.039` (delta `+0.195`). By 5131 source enters CliffJumpQuick2 with `(vx,vy)=(1.0,3.3)` while Rust remains CliffJumpQuick1.
- CLI safety: use the quoted repository-relative replay argument. A bare `.slp` path invokes Windows file association; an unquoted path containing `Super Smash...` yields the `Unexpected parameter Smash` error. Do not use `execs/Play Slippi Replay.cmd` for headless parity work.

### 2026-07-15: CliffJumpQuick frontier

- Implemented the exact airborne CliffJump1 collision chain `ft_800821DC -> mpColl_80048160`: rotate CollData root positions, `mpCollPrev`, live JObj ECB load with flags `0xA`, then airborne inline collision flags `0`. The thin `[-1,+1]` ECB resolves Battlefield's left lip and produces source frame 5124 x `-69.234` exactly.
- Retained regressions for source frames 5124 and 5131 are green. Quick1-to-Quick2 now transitions on source animation completion and applies decomp attributes `ledge_jump_horizontal_velocity=1.0`, `ledge_jump_vertical_velocity=3.3` with same-tick vertical-physics suppression.
- Headless 5313-frame Mole replay check now advances the classified frontier to source frame 5126/core 5249 P1 `position_drift`: expected Fall x `-71.349`, actual `-71.540`; state, Y, and velocities match. A later P2 mismatch occurs at source 5162 where Rust lands during CliffJumpQuick2 while source remains airborne. Investigate 5126 first.

- Quick1 has exactly 12 samples. `ftCo_CliffJump1_Anim` transitions when no frames remain, so source enters Quick2 at source 5131 after Quick1 frame 11; the previous Rust `motion_frame >= frame_count` test was one tick late.
- `ftCo_8009B2F8` samples the Quick2 entry animation, adds `facing * co_attrs.ledge_jump_horizontal_velocity` to self X, and assigns `ledge_jump_vertical_velocity` to self Y. Falcon's extracted `ftCo_DatAttrs` values at offsets `0xA8/0xAC` are `1.0/3.3`; these are now explicit `FighterProfile` fields loaded from source bytes.
- Quick2's first same-tick physics callback only flips its internal gate, so gravity/air drift do not run on transition. Rust now preserves the exact `(1.0, 3.3)` entry velocity and skips vertical physics that tick.
- The first actionable divergence remains source 5124 P2 X during Quick1. Raw ledge-anchored TransN gives `-69.039`; source exports `-69.234`. The decomp collision callback is `ftCo_CliffClimb_Coll -> ft_800821DC -> mpColl_80048160`.
- Reusing Rust's generic `source_mp_coll_air_inline1` for this callback was disproven and reverted: it treated the full generated JObj ECB against Battlefield's lip and overcorrected to approximately `-72.59`. Do not reintroduce that routing. The unresolved architectural boundary is the exact CollData/ECB state consumed by `mpColl_80048160` during `ftCo_MF_CliffAction`, including its globally prepared ECB and prior collision-root cadence.
- Red regressions retained: `slippi_match_start_frame5124_cliff_jump_quick_uses_air_collision_correction` and `slippi_match_start_frame5131_cliff_jump_quick_2_sets_entry_velocity`. The latter's state/velocity implementation is in place, but its exact position remains downstream of the 5124 collision discrepancy.
## 2026-07-15 replay/gameplay boundary and next architectural group

- Sequential match-start and visual replay paths were audited end to end. Slippi post-frame state is read only after the ordinary world step for comparison; no resync or expected-state injection occurs. The shared path is `Slippi inputs -> step_world_with_source_collisions -> step_world_with_source_runtime_data`.
- `--mode seeded` deliberately injects pre-frame state into a fresh world and is diagnostic-only. It must never be used as parity evidence. Match-start remains the authoritative mode.
- Default report tolerances and `MixedPhaseWitness` classification can hide small differences. Final parity requires strict match-start comparison, not merely a default clean-looking report.
- Frame 5126 is a generic animation scheduling gap. Decomp priority order evaluates the active AObj/live JObj before Anim, Phys, and Coll callbacks. Fall also evaluates its costume-specific secondary `x8AC_animSkeleton` in callback order before blending. The existing permanent Fall `sample_frame + 1` consequence and state-filtered post-animation collision sampling are not the final architecture.
- A blanket `cur_anim_frame + frame_speed_mul` collision sample was tested and rejected: existing `cur_anim_frame` semantics are not yet a universal AObj state and the change caused an earlier SquatWait/Fall regression. Do not restore that shortcut.
- Required functional group: explicit AObj state (`curr_frame`, rate, end/rewind frames, first-play/loop/stopped flags), one pre-callback animation evaluation phase, live costume JObj output consumed by collision, and a real secondary Fall skeleton evaluation/blend path. Duration/end-frame metadata must not be inferred from sample-array length.
- Required extraction boundary: generic character pose bundle containing action/FObj data, ECB source joints, model/parts data, and every costume skeleton; one shared Rust HSD/JObj/FigaTree evaluator; generated per-character immutable data only. Generators must consume extracted bundles rather than reopen DATs.
- Required cliff group: bake common motion-state callback topology and translate the complete Catch/Wait/Climb/Attack/Escape/Jump wrapper lifecycle, including `Fighter_procMap`, `ft_800821DC`, `ft_800835B0`, `ft_80084104`, and `mpCollEnd`, rather than adding state-specific replay-era branches.
# 2026-07-15 replay completion: shared AObj/cliff/KO architecture

- Hard invariant: Slippi match-start execution supplies controller inputs only. Expected
  post-frame rows are comparison output and never mutate gameplay state.
- `ftData.x44` values index `fp->parts[]` / FighterBone slots directly. They are not
  semantic `Fighter_Part` IDs and must not pass through `part_to_joint`. Decomp witness:
  `ft_80081B38` uses `bones[temp_r29->unkN].joint` with no remap. Restored Falcon ECB
  source slots `[39, 47, 25, 14, 8, 4]`; frame 326 and frame 5126 witnesses pass.
- Generic extracted AObj descriptors now retain exact FigaTree `end_frame`, zero
  `rewind_frame`, and loop flags. Fall's action endpoint is `8.0` and its fighter loop
  flag wraps pose frame 8 to frame 0, fixing the live JObj ECB boundary at frame 5126.
- `ftCo_CliffJump2_Anim` uses `ftAnim_IsFramesRemaining`, so CliffJumpQuick2 completion
  now follows the live source animation frame/AObj endpoint rather than integer callback
  age. When it enters Fall, Fall physics runs in the same tick, matching the decomp
  scheduler. Frames 5131, 5162, and 5163 pass.
- Final-stock blast-zone handling now enters the directional death motion before match
  flow marks the player slot out. This preserves Melee's frame-5189 `DeadDown` state
  while still ending the match.
- Final strict match-start replay check over all 5,313 compared frames reports `ok: true`,
  zero state mismatches, zero significant position drift, and zero unsupported states.
  The remaining diagnostic `first_classified_divergence` at source 5131 is a Slippi
  mixed-phase composed-velocity witness: expected `self_vel.x` and position both match
  exactly, while Slippi's composed-X field is zero on the CliffJump1->2 transition frame.
  It is not an independent engine-state divergence and the scanner correctly excludes it
  from actionable parity failure.
# 2026-07-15 SDL replay-path completion correction

- User-observed stop at displayed frame 5050 was real. The live log identified core 5049 /
  source 4926, P2 AttackAirHi, X drift `-2020` milli.
- Root cause was bootstrap divergence, not gameplay replay correction: SDL visual playback
  used `default_play_world()` and therefore neutral Falcon costumes, while CLI match-start
  comparison used replay settings (`P1 character_color=1`, `P2 character_color=5`). P2's
  costume-specific skeleton/ECB changed a prior ledge interaction and accumulated exactly
  the observed horizontal displacement.
- SDL visual loading now obtains its ordinary gameplay `World` through the same
  `world_for_slippi_export` bootstrap as CLI match-start comparison. After bootstrap, the
  visual path still supplies only pre-frame controller inputs to
  `step_world_with_source_collisions`; expected post-frame data remains diagnostic-only.
- Added regressions for export-configured costumes and the real source-frame-4926 SDL path.
- The optimized SDL executable then reached source 5131/5132 and exposed a separate live
  diagnostic-gate issue: Slippi's composed-X telemetry is zero during CliffJumpQuick2 even
  though its `air_x`, resulting position, engine self velocity, state, and Y velocity all
  match. General same-state rows whose authoritative component velocities and resulting
  position match are now classified as mixed-phase telemetry rather than engine drift.
  This classification does not alter gameplay state or permit resynchronization.
- Final release verification used the actual SDL executable with dummy video/audio drivers:
  all 5,313 frames ran, `final_frame=5313`, exit code 0, and no divergence log was created.

# 2026-07-15 strict replay stop and TransN architecture correction

- Withdraw the preceding SDL/full-replay completion claim. The live divergence gate treated
  `MixedPhaseWitness` as permission to continue, so a run without a divergence log did not
  prove that every observed replay disagreement was absent. Live playback must stop on every
  classified disagreement; mixed-phase remains a diagnostic label only.
- Fresh dummy-SDL execution now stops at the first observed disagreement: source frame 2293,
  core frame 2416, P2, `MixedPhaseWitness`, with exact state/position but composed Y velocity
  `-1.434` versus Slippi `-2.080`. The log is
  `debug/slippi/runtime-divergence.headless.json`. This is the current strict frontier, and
  parity is not complete.
- `mark_mixed_phase_cascades` now scopes roots to the same player. A P1 telemetry witness may
  not suppress an independent P2 mismatch. Offline first-non-witness comparison consequently
  reports P1 source 2583/core 2706 and P2-only comparison reports source 2586/core 2709.
- Decomp `Fighter.x594_b0` is the global rule: raw action flag `0x80000000` selects the
  TransN-sampling path that records motion and clears the live TransN JObj translation. Rust
  now interprets that bit once in the shared frame-data/runtime architecture. Each character's
  extracted action manifest supplies its own raw flags; no Falcon, EscapeF, SpecialHi, or
  replay-frame branch exists.
- Render hurt capsules and source collision/hit capsules now consume the same flag-gated root
  normalization. Focused regressions cover EscapeF, TurnRun, CliffCatch, and SpecialAirHi,
  including hit collision. This removes the duplicated TransN displacement that made rolls and
  Up-B geometry appear to jump or move twice.
- P2 Catch at source 4959-4988 has zero TransN and exact replay world motion. Its visible reach
  is skeletal pose articulation. However, generic `Fighter_ChangeMotionState` default blend and
  duplicate-skeleton behavior remain incompletely translated; do not call the visual transition
  fully source-faithful yet.
- Sustained hit sweeps still reconstruct the previous endpoint from the preceding baked frame
  and current facing. Decomp persists the actual previous world endpoint (`x58 = x4C`). Full
  parity requires that global ftColl lifecycle architecture, not a move-specific correction.
- Verification: `mole_frame_data` 10/10, `mole_runtime --lib` 39 passed/2 ignored, runtime bin
  13/13, strict-gate focused tests pass, and `cargo check -p mole_runtime --features "sdl wup"`
  passes. The 305-test `runtime_contract` target is not green (221 passed, 84 failed); at least
  some unrelated failures also reproduce in isolation. They must not be hidden or described as
  a passing full suite.

# 2026-07-15 source-frame-2833 global collision-state parity

- The first failure after the TransN correction was source 2833/core 2956, P2 CliffCatch.
  Rust admitted P1's dair and entered DamageFlyN while Slippi kept P2 in CliffCatch.
- Decomp proof: CliffCatch's script starts with `0x68000002`; fighter command index 16 is
  `ftAction_80071A14`, which calls `ftColl_8007B62C(gobj, 2)` and sets the fighter-global
  collision state. Hit admission requires that state to be zero.
- Mole CLI now decodes and exports `fighter.set_collision_state` generically. The adjacent
  index-17 `ftAction_80071A58` command is separately decoded as
  `fighter.set_all_hurt_state`; it is not conflated with global collision state.
- The compact frame-data event format is `MSFC0017` and carries collision-state events through
  extraction, sidecar encoding, runtime metadata, and the core script interpreter. CliffCatch's
  animation callback executes its frame-zero command before subsequent collision checks.
- Source action-table slot 190 has no FigaTree and is intentionally skipped by extraction.
  The erroneous `DownSpotU -> slot 190` canonical runtime binding was removed rather than
  fabricating source animation data.
- Focused source-2833 regression passes. `mole replay check --frames 6000` compares all 5,313
  replay frames with zero classified divergence, zero state mismatch, zero position drift, and
  zero unsupported states. A strict dummy-SDL runtime run reached core frame 6000 without
  creating a divergence log.

# 2026-07-15 post-completion hardening status

- Authoritative milestone record:
  `docs/release_notes/2026-07-15-captain-falcon-battlefield-full-replay-parity.md`.
- Scope is exactly the current Falcon-versus-Falcon Battlefield Slippi fixture.
  Do not generalize this to all fighters, stages, or action families.
- Replay uses the ordinary gameplay engine and recorded inputs. Diagnostic
  expected state must never modify simulation or permit continuation through a
  classified disagreement.
- Rollback snapshots now cover all recently added mutable authoritative player
  fields. Rollback input history is pruned to the retained snapshot horizon.
- Authoritative gameplay remains 60 Hz. A future 120/240 Hz host/network cadence
  may receive packets and start rollback between presentations, but resimulation
  remains whole 60 Hz frames. This higher-rate scheduler is not yet qualified.
- Compact generated runtime source-frame artifacts are exact and total
  14,582,541 bytes. The legacy expanded Falcon ECB Rust table remains known size
  debt and must only be replaced by a lossless source-derived representation.
- Current broad-suite truth: `mole_core` has 568 passing, 46 failing, and 2
  ignored contract tests. Shared AObj callback scheduling and combat/collision
  architecture remain real gaps. The scoped replay milestone can be checkpointed;
  the repository must not be called universally parity-complete or netplay-final.

# 2026-07-16 global callback scheduler parity completion

- The strict replay frontier progressed through source 3284, 3543, 4462, and
  4820 without replay repair. Each correction came from the shared fighter
  scheduler or callback-family ownership.
- `Fighter_8006A1BC` priority 0 decrements hitlag and runs the post-hitlag
  callback before priority 4. The ordinary every-hitlag callback therefore runs
  only while hitlag remains after p0; the final tick performs exit ASDI/DI and
  does not apply another SDI displacement.
- Common `ftCo_Damage_*` states 75-86 publish their current JObj pose in the
  migrated p1 callback. The separate `ftCo_DamageFly_*` callback family 87-91
  remains on legacy playback and p6 samples its post-animation JObj pose. This
  is a source callback-family boundary, not a Falcon move exception.
- At source 4820, `ftCo_AttackAir_Anim` enters Fall during p1. Melee installs
  Fall immediately, but its newly installed `ftCo_Fall_Anim` callback is not
  recursively dispatched during the same p1 slot. Rust had incorrectly run
  Fall's directional animation blend in the later combined p4 branch, changing
  the ECB before p6. Fall animation evaluation now belongs to the global p1
  phase, so the transition tick keeps the neutral entry pose while p4 physics
  and p6 collision reread the new Fall callbacks.
- The mandatory `slippi_match_start_full_fixture_has_no_engine_divergence`
  contract now runs all 5,313 replay frames with no classified engine
  divergence. The 3,200-frame comparison and visual gate contracts now assert
  no fabricated stop rather than preserving obsolete frame-2583 expectations.
- Current broad `mole_core` truth is 580 passed, 41 failed, and 2 ignored in
  `core_contract`, plus 83/83 library and 11/11 collision contracts. The 41
  failures remain classified stale fixtures or out-of-scope shared systems;
  this strict Falcon/Battlefield replay result does not establish universal
  fighter/stage parity.
- Independent review confirmed that p4 and p6 are still structurally inside the
  per-player monolith for unmigrated states. Enabling generic same-tick dispatch
  through that branch also reruns bundled p1 behavior and breaks the strict
  replay. The remaining extraction must separate p4 and p6 primitives before
  globally dispatching every newly installed callback; it is not complete in
  this checkpoint.

# 2026-07-16 rollback and host-cadence hardening

- Slippi reference proof keeps input `frame`, `checksumFrame`, and checksum
  separate. Rust packet v3/datagram v4 now does the same. Friend Connect sends
  only the latest fully confirmed historical checksum; speculative checksums are
  not advertised as deterministic agreement.
- Compatibility is checked before packet admission using protocol version, a
  build-time hash of authoritative Rust sources plus Cargo version, a
  deterministic hash of baked source manifest/capsule bytes, and the shared
  room key. Incompatibility, conflicting checksum claims,
  exact finalized-frame mismatch, or an expired pending checksum is a hard
  session error. No state is repaired from a checksum.
- `RollbackSession` owns a bounded frame ring of pre-frame snapshots, resolved
  inputs, confirmation bits, and post-frame checksums. Correction first proves
  that the complete interval exists, then restores and resimulates with the
  configured production step function. Missing intervals are rejected rather
  than synthesized with neutral input.
- Friend Connect uses Slippi's seven-frame production rollback window. UDP and
  latest controller capture run on adaptive 60/120/180/240 Hz host passes;
  gameplay, replay, collision, and rollback remain whole 60 Hz frames. Local
  peers never negotiate down to a common host/render cadence.
- Runtime source actions use an immutable indexed cache. Snapshot hit-victim
  history is copy-on-write, avoiding a vector clone on unchanged snapshots.
  Release measurements and their limits are recorded in
  `docs/research/rollback-performance-report.md`; do not generalize the observed
  i7-6700K numbers into a low-end-hardware guarantee.
- Deterministic two-peer tests cover bounded delay, reorder, duplicates, loss,
  retransmission at the seven-frame edge, convergence, and stale rejection.
- Final review closed six protocol correctness gaps without replay-specific
  behavior: packet retention now includes future input delay, delay is part of
  session compatibility, duplicate bundles still validate checksums, finalized
  checksums advance only through contiguous confirmed history, legacy v3
  bundles remain decodable, and checksum claims cannot name a future frame.
