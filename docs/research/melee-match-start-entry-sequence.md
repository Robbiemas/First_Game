# Melee Match Start And Entry Sequence Reference

This note records the decomp-backed start-of-match path that matters for Rust
parity. It is scoped to the normal match intro leading into players becoming
actionable, with extra notes where the same machinery overlaps respawn.

## Source Files

- `src/melee/gm/gm_18A5.c`: tournament/match countdown and timer display flow,
  especially `fn_8019AF50`.
- `src/melee/if/iftime.c`: countdown and match timer display object behavior.
- `src/melee/ft/ft_0C31.c`: common Entry, EntryStart, and EntryEnd fighter
  states.
- `src/melee/ft/ft_0D4D.c`: Rebirth and RebirthWait, useful because it shares
  the spawn-platform/accessory pattern with Entry.
- `src/melee/pl/player.c`: per-player `unk4C` entry stagger timer access.
- `src/melee/ft/types.h`: common-data fields used by Entry and Rebirth.

## Global Match Countdown

The visible match countdown is not owned by the fighter action states. The gm
layer advances match/tournament display state in `fn_8019AF50` in
`gm_18A5.c:7041`.

Key behavior:

- `fn_8019AF50` receives `TmData* tm` through `arg0`, uses
  `lbl_804799D8.x0` as the global counter, and updates SIS text/timer display
  each frame.
- When the bracket/match data says countdown should be shown
  (`lbl_80473AB8[bracketIdx].x18 != 0`), the counter increments until `0xFA`
  frames. Before `0x64`, it displays the base timer format. From `0x64`
  onward, it progressively copies pairs from `(u8*) counter + 0x4E` into the
  display buffer in 15-frame chunks.
- After the counter reaches `0xFA`, the code switches to the post-countdown
  display path, incrementing by 2 per frame but clamping back to `0xFA` unless
  the global mode flag allows it.
- If countdown display is disabled, the counter is forced to `0xFA`.
- `ifTime_GetCountdownSeconds` derives the visible countdown number from
  `gm_8016AEEC()` seconds and `gm_8016AF0C()` centiseconds: when centiseconds
  are zero it returns `5 - seconds`, otherwise `4 - seconds`, clamped at zero.
- `ifTime_UpdateCountdown` only swaps the countdown model when that derived
  second value changes, then animates the countdown object every frame.
- `ifTime_HideTimers` and `ifTime_ShowTimers` hide/show both match and
  countdown display objects; `ifTime_FreeCountdown` destroys the countdown
  object.

Parity implication: Rust should treat match intro/countdown as global
rollback-owned match flow state, not as a fighter motion-state timer. The
fighter Entry timers determine when each fighter leaves the platform sequence;
the gm/if timer path determines the displayed countdown and any start-game
phase gating.

## Per-Player Entry Stagger

The first common fighter state is `Entry`:

- `ftCo_800C61B0` initializes `fp->mv.co.entry.timer` from
  `Player_GetUnk4C(fp->player_id)`.
- `Player_GetUnk4C` simply returns `player_slots[slot].unk4C`.
- Existing Rust starts P1/P2 with staggered entry timers of 5 and 10 ticks,
  which matches the observed source-shaped stagger expectation but should remain
  tied back to the source player slot field rather than hard-coded forever.

Entry setup also:

- stores the fighter's original model scale into `mv.co.entry.x8`;
- writes a temporary model scale with Y set to common-data `x6C4`;
- stores the base Y position in `mv.co.common.x4.x`;
- changes motion state to `ftCo_MS_Entry`;
- marks the fighter invisible and sets several no-interaction/no-visibility
  flags;
- snapshots collision shape into `mv.co.entry.x2C` with `ft_80084CB0`.

`ftCo_Entry_Anim` waits until the stagger timer is zero, then calls
`ftCo_800C6408` to enter `EntryStart`; it decrements the timer after the zero
check. Entry has empty IASA, Phys, and Coll callbacks.

Parity implication: the hidden `Entry` state should have no normal movement,
input, or collision ownership. The only behavior that matters for gameplay
timing is the per-player stagger timer and its exact decrement/check order.

## EntryStart

`ftCo_800C6408` transitions from hidden Entry into `EntryStart`:

- sets `mv.co.entry.timer = p_ftCommonData->x6BC`;
- changes to `ftCo_MS_EntryStart`;
- if this is the primary fighter rather than a transformed follower, creates
  the trophy/entry platform accessory with `ftCommon_SetAccessory`;
- scales that platform from fighter scale times `co_attrs.trophy_scale`;
- computes platform height values:
  - `mv.co.entry.x24 = platform scale`;
  - `mv.co.entry.x20 = 1.497345 * scale`;
  - `mv.co.entry.x28` is the current platform offset height;
- places the platform at `cur_pos.x - facing_dir * ftCommon_800804EC(fp)`,
  current Y/Z, with a Y rotation of `pi/2 * facing_dir`;
- sets `accessory1_cb = fn_800C69F4`;
- spawns the entry effect `0x43E`, plays SFX `0x8B`, and starts voice/audio
  event `0x75`.

Extracted common values currently identify:

- `x6BC` / `entry_start_ticks`: 30 ticks.
- `x6C4` / `entry_initial_scale_y`: about `0.01`.

`ftCo_EntryStart_Anim` decrements the timer first, then calls
`ftCo_800C6B6C` when it reaches zero.

`ftCo_EntryStart_Phys` is platform/player interpolation:

- progress = `(x6BC - timer) / x6BC`;
- fighter model scale Y interpolates from `x6C4` to the original scale Y;
- platform accessory scale Y grows as `platform_scale * progress`;
- platform offset `x28` grows as `x20 * progress`;
- fighter `cur_pos.y = base_y + x28`.

For transformed/follower fighters, Y is copied from the primary fighter.

`ftCo_EntryStart_Coll` owns platform collision through the saved entry collision
shape:

- sets `mv.co.entry.x2C.bottom = -x28` for the primary fighter;
- transformed/follower fighters reuse the primary fighter's `x28`;
- if the fighter is airborne, calls `ft_80083E64(gobj, &x2C, fn_800C63BC)`;
- if grounded, calls `ft_800846B0(gobj, &x2C, fn_800C63E0)`;
- those callbacks route to `ftCommon_8007D7FC` and `ftCommon_8007D5D4`,
  respectively, to flip ground/air collision state.

Parity implication: EntryStart platform behavior is gameplay state, not just a
render cue. It directly sets `cur_pos.y`, updates an ECB bottom offset based on
the platform height, and runs source map-collision helpers each frame.

## EntryEnd

`ftCo_800C6B6C` transitions from `EntryStart` into `EntryEnd`:

- sets `mv.co.entry.timer = p_ftCommonData->x6C0`;
- restores the fighter model scale to original `mv.co.entry.x8`;
- places the fighter at `base_y + x20`, the full platform height;
- changes to `ftCo_MS_EntryEnd` with flags `0x3000`;
- keeps the platform accessory callback active, now as `fn_800C6F34`.

Extracted common values currently identify:

- `x6C0` / `entry_end_ticks`: 30 ticks.
- `x6C8` / `entry_collision_landing_lag_ticks`: 120 ticks.

`ftCo_EntryEnd_Anim` decrements the timer first. When it reaches zero:

- if `Player_GetFlagsBit4(fp->player_id)` is set, it calls
  `ftColl_8007B760(gobj, p_ftCommonData->x6C8)`;
- then it calls `ftCommon_8007D92C(gobj)`.

`ftCommon_8007D92C` is a small air/ground handoff in `ftcommon.c:606`: if
`ground_or_air == GA_Air`, it calls `ftCo_Fall_Enter`; otherwise it calls
`ft_8008A2BC`.

`ftCo_EntryEnd_Phys` shrinks the platform and lowers the fighter:

- progress = `timer / x6BC` rather than `timer / x6C0`;
- platform scale Y = platform scale * progress;
- platform offset `x28 = x20 * progress`;
- fighter `cur_pos.y = base_y + x28`.

`ftCo_EntryEnd_Coll` mirrors EntryStart collision ownership:

- bottom offset is `-x28`;
- airborne uses `ft_80083E64(..., fn_800C63BC)`;
- grounded uses `ft_800846B0(..., fn_800C63E0)`.

Parity implication: the actionability handoff is conditional on the current
source collision state. `EntryEnd` exits to `Fall` only if the source still
considers the fighter airborne; if the entry platform collision path has made
the fighter grounded, it exits through `ft_8008A2BC` instead.

## Respawn Overlap

Respawn uses related platform/accessory behavior but a different fighter state
family:

- `ftCo_Rebirth_Anim` decrements `mv.co.common.x0` from common-data `x5D0`
  (`rebirth_ticks`, currently 60) and calls `ftCo_800D5600` at zero.
- `ftCo_Rebirth_Phys` steers velocity toward either the stage/player spawn
  target or a transformed primary fighter.
- `ftCo_800D5600` updates collision with `mpColl_80043680`, zeroes vertical
  self velocity, sets `mv.co.common.x0 = p_ftCommonData->x5D4`
  (`rebirth_wait_ticks`, currently 240), enters `RebirthWait`, and installs a
  platform/accessory callback.
- `ftCo_RebirthWait_Anim` decrements that timer. At zero it calls
  `ftColl_8007B7A4(gobj, p_ftCommonData->x5D8)` and then `ftCo_Fall_Enter`.
  Extracted `x5D8` is `rebirth_hurt_intangible_ticks`, currently 120.
- `ftCo_RebirthWait_IASA` can also break to Fall early after several airborne
  action/input checks or companion-state checks.

Parity implication: start-of-match Entry and stock respawn should not be
collapsed into one Rust shortcut. They share platform rendering/collision
concepts, but they use different timers, different intangibility calls, and
different exit routines.

## Rust Parity Checklist

Use this list when deciding whether the Rust engine needs changes:

- Global countdown:
  - represented as match-flow state, separate from fighter Entry timers;
  - includes the `0xFA` counter boundary and `0x64`/15-frame staged display
    behavior if UI/Slippi-visible timing depends on it;
  - countdown seconds derive from match timer seconds/centiseconds like
    `ifTime_GetCountdownSeconds`.
- Initial Entry:
  - per-player stagger comes from the player slot's `unk4C` equivalent;
  - Entry zero-check/decrement order matches `ftCo_Entry_Anim`;
  - fighter is non-actionable, invisible, and collision-inert during hidden
    Entry.
- EntryStart:
  - uses common-data `x6BC = 30`;
  - model scale Y interpolates from `x6C4` to original scale;
  - platform height grows from 0 to `1.497345 * trophy_scale`;
  - fighter `cur_pos.y` is written from base Y plus platform offset;
  - collision uses the entry ECB bottom offset and source air/ground collision
    helpers each frame.
- EntryEnd:
  - uses common-data `x6C0 = 30`;
  - platform/fighter Y shrink with `timer / x6BC` as in decomp;
  - collision uses the same source helpers as EntryStart;
  - exit grants `x6C8` entry intangibility when `Player_GetFlagsBit4` is set;
  - exit goes through `ftCommon_8007D92C`: air becomes Fall, ground enters the
    `ft_8008A2BC` grounded handoff.
- Respawn:
  - Rebirth/RebirthWait use `x5D0`, `x5D4`, and `x5D8`;
  - RebirthWait can exit by timer or IASA/input logic;
  - respawn platform collision/accessory behavior is parallel to Entry but not
    identical.

## Current Suspicion For Replay Parity

The current replay boundary has an early P2 Y-position drift before the first
state mismatch. Given the decomp above, likely suspects are:

- Rust `EntryEnd` may be collapsing to `Fall` rather than preserving the
  source air/ground branch in `ftCommon_8007D92C`.
- Rust may not be preserving the decomp's EntryStart/EntryEnd collision helper
  ownership while the platform height changes.
- Rust currently exposes match phase as `Ready` while any player remains in
  Entry/EntryStart/EntryEnd; the gm countdown path should be checked separately
  for the exact frame where player actions should begin relative to visible
  countdown.
