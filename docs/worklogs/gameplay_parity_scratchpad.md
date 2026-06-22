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
- Render currently has a lightweight entry-platform cue, but not a true
  collision/runtime actor.

Implementation notes:
- Represent entry platform as a rollback-owned transient platform actor or a
  fighter-attached collision surface, whichever matches decomp after inspection.
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
