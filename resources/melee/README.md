# Melee Resource Bootstrap

This folder is a temporary bootstrap path for Melee-derived gameplay values.
It exists so the Rust core can stop relying on guessed provisional constants
while we migrate toward extracted data.

## Raw Files

Put locally extracted files here:

- `resources/melee/raw/PlCo.dat`
- `resources/melee/raw/PlCa.dat`

Raw DAT/ISO files are ignored by git and should not be committed.

## Generated Files

Run:

```powershell
python tools\extract_melee_resources.py
```

The script writes small JSON snapshots to:

- `resources/melee/extracted/plco_common_data.json`
- `resources/melee/extracted/captain_falcon_profile.json`

Those JSON files are the reviewable resource snapshots we can use while this
project is still bootstrapping parity. Long term, the runtime should load from
project-owned extracted data or generated Rust assets rather than keeping raw
Melee files in the repository.
