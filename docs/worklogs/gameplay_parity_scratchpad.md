# Gameplay Parity Scratchpad

Updated: 2026-06-12

Purpose: keep the next full-gameplay parity work in one actionable place. This is
not a source of truth. The Melee decomp is the source, the parity ledgers are an
index, and this file is the working queue for translating decomp-shaped behavior
into compact Rust runtime data without gameplay hotfixes or byte bloat.

Working rule: before implementing any item below, inspect the named decomp
anchor again and write/adjust a focused test first. Runtime must consume baked
Rust/project artifacts, not raw ISO, raw DAT, or decomp files.

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

## Immediate Human-Visible Gaps

- Entry/spawn platform behavior is not yet fully actionable: platform collision
  and broad IASA options need decomp-shaped runtime rules. The source-backed
  platform cue/wireframe now renders during Entry and RebirthWait, and hard-down
  RebirthWait exit installs source `x5D8` intangibility before Fall.
- Match phase is rollback-owned and renders Ready/Go labels, but the full
  source countdown timing still needs the `gm` flow translated.
- Falcon down-air style hits currently produce damage/animation feedback but do
  not yet apply full Melee knockback/DI/tumble/tech outcomes.
- SDI/ASDI/DI, tumble, wall/ceiling/floor tech, cliff attack/jump/climb/escape
  options, KO/stock/respawn, and full four-stock match flow are not yet
  complete.

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

- 2026-06-12: P0/P1 bridge slice in progress. Completed rollback-owned match
  phase metadata, source-backed entry/RebirthWait platform wireframes,
  RebirthWait hard-down exit to Fall with source intangibility, first
  `CliffCatch`/252 ledge grab slice, `CliffCatch -> CliffWait`/253 lifecycle,
  occupied ledge rejection, source ledge cooldown, and source `CliffWait`
  timer/gated away-down release. Next: cliff climb/attack/escape/jump option
  states and deeper ledge collision callbacks from `ftcliffcommon.c` and
  `ftCo_Cliff*.c`.

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
