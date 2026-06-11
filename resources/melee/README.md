# Melee Resource Bootstrap

This folder is a temporary bootstrap path for Melee-derived gameplay values.
It exists so the Rust core can stop relying on guessed provisional constants
while we migrate toward extracted data.

## Raw Files

Put locally extracted files here:

- `resources/melee/raw/PlCo.dat`
- `resources/melee/raw/PlCa.dat`
- `resources/melee/raw/PlCaAJ.dat` when extracting Captain Falcon action
  animation/JObj ECB data
- `resources/melee/raw/PlCaNr.dat` when extracting Captain Falcon neutral
  costume skeleton/JObj data
- `resources/melee/raw/Gr*.dat` when extracting stages through the Rust CLI

Raw DAT/ISO files are ignored by git and should not be committed.

## Stage Extraction

Stage DAT acquisition is Rust-owned. With a local user-owned Melee 1.02 ISO,
run:

```powershell
cargo run -p mole_cli -- stage extract-iso --iso "C:\path\to\Super Smash Bros. Melee.iso" --competitive --write --json
```

That command reads the GameCube ISO file table directly, copies the competitive
baseline stage DATs into ignored local raw inputs, writes compact stage blobs to
`resources/melee/extracted/stages/`, and writes the generated Battlefield engine
blob to `crates/mole_core/src/generated/stages.rs`.

The ISO, decomp, and raw DATs are extraction inputs only. After a stage has been
translated into committed project assets, the game must continue to build and
run from the Rust engine blobs without the ISO/decomp present.

Current competitive baseline IDs:

- `battlefield`
- `final-destination`
- `yoshi-story`
- `fountain-of-dreams`
- `dream-land-64`
- `pokemon-stadium`

For an individual registered stage, use:

```powershell
cargo run -p mole_cli -- stage extract --stage battlefield --write --json
```

For Battlefield, this command updates both
`resources/melee/extracted/stages/battlefield_stage.json` and
`crates/mole_core/src/generated/stages.rs`. The JSON blob is the reviewable
middleware artifact; the generated Rust blob is the engine-owned runtime source.

For a custom DAT already placed in `resources/melee/raw`, use `--dat`:

```powershell
cargo run -p mole_cli -- stage extract --stage custom-dev-stage --stage-name "Custom Dev Stage" --dat resources/melee/raw/CustomStage.dat --write --json
```

## Generated Files

Run:

```powershell
python tools\extract_melee_resources.py
```

If you have a local user-owned Melee ISO/GCM, the tool can extract the needed
raw DAT files first:

```powershell
python tools\extract_melee_resources.py --iso "C:\path\to\Super Smash Bros. Melee.iso"
```

The script writes small JSON snapshots to:

- `resources/melee/extracted/plco_common_data.json`
- `resources/melee/extracted/captain_falcon_profile.json`
- `resources/melee/extracted/captain_falcon_ecb_source.json`
- `resources/melee/extracted/captain_falcon_hurtbox_inits.json`
- `resources/melee/extracted/captain_falcon_action_animation_table.json`
- `resources/melee/extracted/captain_falcon_costume_skeleton.json`
- `resources/melee/extracted/captain_falcon_action_ecb_samples.json`

Those JSON files are the reviewable resource snapshots we can use while this
project is still bootstrapping parity. Long term, the runtime should load from
project-owned extracted data or generated Rust assets rather than keeping raw
Melee files in the repository.

The current runtime export path is:

```powershell
cargo run -p mole_cli -- frame-data export-runtime --all-states --character dolphin_mole --write --json
```

Use the Rust CLI/dev-tool path for engine-feeding exports. Existing Python
scripts are legacy/reference tooling and should be ported to Rust when they are
touched for authoritative frame-data, state-graph, or runtime export work.

That command reads the compact manifest and local raw action archive during the
dev-tool/export step, then writes a Rust-owned source export to
`crates/mole_runtime/src/generated/source_frame_data.rs` plus generated
`source_frame_data/` sidecars. The export includes
`source_frame_capsules.bin`, a baked action/frame capsule plus DownBound
hip-pose sidecar. Runtime
rendering consumes those compile-time included assets directly and must not read
raw DAT files during play. `preload_runtime_source_frame_data` is the runtime
boundary: it decodes the baked capsule sidecar into memory and must not
parse/evaluate the embedded compact FigaTree/JObj export. Gameplay, rendering,
collision, and player startup should only perform cache lookups and coordinate
transforms; any runtime FigaTree/JObj sampling is a performance regression and
should be treated as a bug.

Canonical Melee action-state ids are runtime identity. `MotionState` is only a
temporary compatibility/view enum for states Rust gameplay still names directly.
The compact runtime export therefore includes source-only bindings with
`motion_state: None`; the common Damage and DamageFly states 75-91 map this way
to Captain Falcon source action table ids 165-181, and Passive/PassiveStand
states 199-201 map this way to source action table ids 199-201. These should
not be modeled by adding more compatibility enum variants.

`Entry`, `EntryStart`, and `EntryEnd` are separate Melee action-state identities
(`322`, `323`, and `324`) but share Captain Falcon action table id `238` /
source key `Entry` for runtime pose, ECB, and capsule coverage. Do not treat
the Entry family as missing source data during export or render preload.

Decomp-sourced fighter motion values should stay in their native float shape
through gameplay, collision, replay diagnostics, and render-root state. Melee's
fighter update path keeps `cur_pos`, `prev_pos`, `pos_delta`, `gr_vel`,
`self_vel`, `x74_anim_vel`, and player nudge velocities as floats before
rendering through `HSD_JObjSetTranslate`. The Rust core now has a
`source_position` lane for that `cur_pos` equivalent. Existing `Vec2` milli
positions/velocities are legacy compatibility/readout projections only; do not
add new runtime logic that depends on converting source floats to milli integers
and back. The long-term cleanup is to remove those projections from
runtime-facing gameplay/render/collision APIs once their callers have moved to
source floats.

Damage timing is also source-shaped in the Rust core. `hitlag_frames` is the
decomp `Fighter.dmg.x195c_hitlag_frames` path and freezes the fighter tick while
it counts down. Source-only Damage/DamageFly action ids then advance through the
core damage branch using `damage_hitstun_frames`, the Rust-owned equivalent of
`mv.co.damage.x0 = (int)(kb_applied * p_ftCommonData->x154)`, so old
compatibility `Fall`/`Wait` logic cannot consume inputs or replace the
canonical Damage action during lockout. Runtime preload also carries each
Damage/DamageFly action's compact-export `total_frames` into rollback-owned
`source_action_total_frames`; the core exit gate matches
`ftCo_Damage_Anim`/`ftCo_DamageFly_Anim` by requiring both no animation frames
remaining and the lockout bit cleared. Air physics mirrors
`ftCo_Damage_Phys`/`ftCo_DamageFly_Phys`: locked Damage uses `ft_80084EEC`
gravity plus aerial friction, then unlocked-but-still-animating Damage uses the
ordinary `ft_80084DB0` fall/drift path.
Ordinary Damage ids 75-86 also follow the implemented floor-contact slice of
`ftCo_Damage_Coll`: extracted CommonAttributes `x1E0 = 5.0` and
`x1E4 = 0.5` gate the branch, `0.5 <= |x8c_kb_vel| < 5.0` enters basic
`Landing`, and high knockback enters canonical source-only
`DownBoundU`/`DownBoundD` action ids `183`/`191`. Runtime fills the U/D choice
from baked `FtPart_HipN` matrix components in `source_frame_capsules.bin`,
matching `ftCo_80097570` for normal-fighter flags. DownBound animation end now
enters canonical source-only `DownWaitU`/`DownWaitD` action ids `184`/`192` and
seeds `mv.co.downwait.x0` from extracted CommonAttributes `x424`; timer expiry
then enters canonical source-only `DownStandU`/`DownStandD` action ids
`186`/`194` through baked `DownStand` source data. DownWait's visible IASA slice
also routes fresh source-normalized `HSD_PAD_A | HSD_PAD_B` to baked
`DownAttackU`/`DownAttackD` action ids `187`/`195`, and fresh
source-normalized `HSD_PAD_LR` to `DownStandU`/`DownStandD`. DamageFly and
DamageFlyRoll floor contact now mirrors the represented decomp order:
`ftCo_80090184` / `ftCo_DamageFlyRoll_Coll` attempts PassiveStand then Passive
through `ftCo_800986B0` using rollback-owned `x680`/`x684` digital L/R timers
and extracted CommonAttributes `x1C`, `x250`, and `x254`; if those checks fail,
it falls through to `ftCo_80097D40`, entering baked `DownBoundU`/`DownBoundD`
through the same hip-pose gate. The Hammer-item veto in `ftCo_800C5240`,
wall/ceiling passive callbacks, exact PassiveStand model-velocity physics,
DownWait side getup/roll routing, vertical-stick stand-up thresholds, and downed
damage/passive callbacks remain explicit parity gaps.

`PlCa.dat` contains Captain Falcon attributes, ECB source joint indices, and the
table that points at action animation figatrees. `PlCaAJ.dat` contains those
figatree chunks. The generated action-animation table records the action-state
id, action name, chunk offset, chunk size, figatree root, frame count, node track
counts, and raw `FigaTrack` descriptors for each Captain Falcon action.

`captain_falcon_hurtbox_inits.json` decodes `ftDataCaptain.x30`, the static
hurt capsule init table copied by `ftColl_8007B320` into
`Fighter.hurt_capsules[15]`. Runtime/dev-tool hurt capsule views should derive
from those inits, the Captain Falcon neutral skeleton, and per-action FigaTree
pose the way Melee's JObj path does. Source `{x, y, z}` capsule endpoints stay
preserved in the source-space data; current 2D view/import endpoints flatten Z
only after that projection step. Large per-frame hurtbox sample caches are local
debug artifacts and are ignored by git; the shipping runtime uses the generated
`source_frame_capsules.bin` sidecar plus a decoded in-memory frame cache
instead.

`PlCaNr.dat` contains Captain Falcon's neutral costume joint tree. The generated
costume skeleton snapshot records the HSD joint preorder used by
`ftParts_SetupParts`, which is the same index space consumed by
`ftDataCaptain.x44` and `mpColl_SetECBSource_JObj` for ECB source joints.

The generated action ECB samples evaluate selected Captain Falcon figatrees
against that skeleton and reduce the six source joints through the same
`mpColl_LoadECB_JObj` min/max path. The Rust ECB table is generated from those
samples by:

The legacy generated ECB table maps Rust `MotionState` names only when there is
an explicit Captain Falcon action-table equivalent. Canonical source-only
action ids, such as Damage and DamageFly 75-91, flow through the compact runtime
source export instead of the compatibility `MotionState` table. Derived or
still-abstract Rust states must be split or replaced with their decomp-backed
equivalents before receiving ECB samples; they should not be aliased to visually
similar states.

Ground-to-air ECB locking is separate from the generated desired-ECB samples.
Source paths such as `ftCommon_8007D5D4` preserve the previous floor-contact
bottom probe for ten fighter ticks while animation changes the JObj-sourced ECB.
In Rust's current root-coordinate state this locked probe is represented by
`ecb_bottom_offset_y = 0`; after the lock expires, the collision/render path
returns to generated per-action JObj ECB samples.

Hurt capsule samples are distinct from ECB/body-volume samples. Hurt capsules
are the attack-victim collision targets sourced from `ftDataCaptain.x30`, while
ECB samples remain the environment/body-volume path sourced from
`ftDataCaptain.x44`.
