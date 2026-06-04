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

Raw DAT/ISO files are ignored by git and should not be committed.

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
debug artifacts and are ignored by git.

`PlCaNr.dat` contains Captain Falcon's neutral costume joint tree. The generated
costume skeleton snapshot records the HSD joint preorder used by
`ftParts_SetupParts`, which is the same index space consumed by
`ftDataCaptain.x44` and `mpColl_SetECBSource_JObj` for ECB source joints.

The generated action ECB samples evaluate selected Captain Falcon figatrees
against that skeleton and reduce the six source joints through the same
`mpColl_LoadECB_JObj` min/max path. The Rust ECB table is generated from those
samples by:

```powershell
python tools\generate_falcon_ecb_rust.py
```

The generated table currently maps 61 Rust `MotionState` names only when there
is an explicit Captain Falcon action-table equivalent. The generator also writes
`docs/state_graphs/parity_reports/falcon_ecb_coverage.json`, which keeps
derived or still-abstract Rust states visible. Those states must be split or
replaced with their decomp-backed equivalents before receiving ECB samples;
they should not be aliased to visually similar states.

Hurt capsule samples are distinct from ECB/body-volume samples. Hurt capsules
are the attack-victim collision targets sourced from `ftDataCaptain.x30`, while
ECB samples remain the environment/body-volume path sourced from
`ftDataCaptain.x44`.
