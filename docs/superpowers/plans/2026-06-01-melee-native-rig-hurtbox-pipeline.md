# Melee Native Rig Hurtbox Pipeline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring Melee's native fighter rig, JObj pose, static hurt capsule definitions, action-script hurt-state changes, and ECB/body-volume samples into the frame-data pipeline before continuing broader hitbox/import work.

**Architecture:** Keep decomp-derived concepts separate: rig skeleton and part indices come from fighter DAT/JObj data, static hurt capsules come from `ftData.x30`, per-frame hurt capsule state comes from action script procedures, and ECB/body volume comes from the existing `ftData.x44` source-joint path. The frame-data artifact stores full source 3D data plus current flattened 2D projections; the dev tool displays both without pretending ECB and hurt capsules are the same thing.

**Tech Stack:** Python DAT extractor (`tools/extract_melee_resources.py`) for raw Melee DAT decoding already present in this repo, Rust CLI (`crates/mole_cli/src/frame_data.rs`) for canonical frame-data reports, JSON artifacts under `resources/melee/extracted` and `resources/melee/frame_data`, Tk dev tool (`tools/state_graph_viewer.py`), and pytest/cargo contract tests.

---

## Source Anchors

Use these source anchors before changing code:

- `D:\Mole Game\.research\doldecomp-melee\src\melee\ft\types.h`
  - `ftData.x30` points to hurt capsule init records.
  - `ftData.x44` points to ECB source joint data.
  - `Fighter.hurt_capsules_len` and `Fighter.hurt_capsules[15]` are runtime storage.
  - `Fighter.parts[]` and `Fighter.x8AC_animSkeleton` are JObj/part storage.
- `D:\Mole Game\.research\doldecomp-melee\src\melee\ft\chara\ftCommon\types.h`
  - `struct ftHurtboxInit { Fighter_Part bone_idx; HurtHeight height; u32 is_grabbable; Vec3 a_offset; Vec3 b_offset; float scale; }`.
- `D:\Mole Game\.research\doldecomp-melee\src\melee\lb\forward.h`
  - `HurtCapsuleState` values: `HurtCapsule_Enabled`, `HurtCapsule_Disabled`, `Intangible`.
  - `HurtHeight` values: `HurtHeight_Low`, `HurtHeight_Mid`, `HurtHeight_High`.
- `D:\Mole Game\.research\doldecomp-melee\src\melee\lb\types.h`
  - `struct HurtCapsule` and `struct FighterHurtCapsule`.
  - `struct set_hurt_state { opcode:6, bone_idx:8, state:18 }`.
- `D:\Mole Game\.research\doldecomp-melee\src\melee\ft\ftcoll.c`
  - `ftColl_8007B320` initializes `Fighter.hurt_capsules` from `fp->ft_data->x30`.
  - `ftColl_8007B0C0` sets all hurt capsules to one `HurtCapsuleState`.
  - `ftColl_8007B128` sets one hurt capsule by `bone_idx`.
- `D:\Mole Game\.research\doldecomp-melee\src\melee\ft\ftaction.c`
  - `ftAction_803C06E8` command table index 18 maps to `ftAction_80071A9C`.
  - `ftAction_80071A9C` calls `ftColl_8007B128`.
- Existing extraction code:
  - `tools/extract_melee_resources.py` already reads `PlCa.dat`, `PlCaAJ.dat`, and `PlCaNr.dat`.
  - It already samples FigaTree/JObj poses and reduces ECB source joints through `compute_ecb_from_jobj_pose`.

## Files

- Modify: `tools/extract_melee_resources.py`
  - Add Captain Falcon static hurt capsule extraction from `ftData.x30`.
  - Add requested-action hurt capsule pose sampling from FigaTree + skeleton + static hurt capsule offsets.
  - Keep existing ECB extraction intact and labeled separately.
- Modify: `tests/test_extract_melee_resources.py`
  - Prove static hurt capsule records are extracted from `PlCa.dat`.
  - Prove sampled hurt capsules preserve source 3D endpoints and flattened view endpoints.
- Create: `resources/melee/extracted/captain_falcon_hurtbox_inits.json`
  - Static `ftData.x30` hurt capsule init snapshot.
- Create or modify: `resources/melee/extracted/captain_falcon_action_hurtbox_samples.json`
  - Per-action, per-frame hurt capsule samples for selected Captain Falcon actions, initially including `AttackAirN`.
- Modify: `crates/mole_cli/src/frame_data.rs`
  - Load static hurt capsule snapshots and per-action sampled hurtboxes.
  - Decode action-script hurt-state procedures.
  - Merge static definitions, per-frame pose samples, and state overlays into `move_frame_data.keyframes[].hurtboxes`.
- Modify: `crates/mole_cli/tests/cli_contract.rs`
  - Prove `frame-data extract` emits source-extracted hurtboxes for `AttackAirN`.
  - Prove no `generated_ecb_preview` hurtbox survives when source-extracted hurtboxes exist.
  - Prove `source_a/source_b` retain Z while `a/b` are flattened for current 2D use.
- Modify: `resources/melee/frame_data/dolphin_mole/AttackAirN.json`
  - Regenerate from the CLI after extraction support exists.
- Modify: `tools/state_graph_viewer.py`
  - Display hurt capsule details with the same right-side value discipline as hitboxes.
  - Draw hurt capsule current, ghost, and travel lines with tags distinct from hitbox travel.
  - Draw ECB/body volume as a separate overlay class when present.
- Modify: `tests/test_state_graph_viewer.py`
  - Prove hurtbox details expose all values.
  - Prove hurtbox travel and ECB overlays are drawn with separate canvas tags.
- Modify: `docs/superpowers/specs/2026-06-01-move-frame-data-design.md`
  - Amend the schema to distinguish `hurtboxes` from `body_volumes`/`ecb`.
- Modify: `resources/melee/README.md`
  - Document new generated hurtbox snapshots and the exact raw-file dependencies.

---

### Task 1: Add Static Hurt Capsule Extraction From `ftData.x30`

**Files:**
- Modify: `tools/extract_melee_resources.py`
- Modify: `tests/test_extract_melee_resources.py`
- Create: `resources/melee/extracted/captain_falcon_hurtbox_inits.json`

- [ ] **Step 1: Write the failing extraction unit test**

Add this test near the Captain Falcon profile/ECB extraction tests in `tests/test_extract_melee_resources.py`:

```python
def test_extract_captain_hurtbox_inits_from_plca_reads_ftdata_x30():
    raw_path = ROOT / "resources" / "melee" / "raw" / "PlCa.dat"
    if not raw_path.exists():
        pytest.skip("local PlCa.dat is required for hurt capsule extraction")

    from tools.extract_melee_resources import extract_captain_hurtbox_inits_from_plca

    snapshot = extract_captain_hurtbox_inits_from_plca(raw_path.read_bytes(), raw_path)

    assert snapshot["source"]["symbol"] == "ftDataCaptain"
    assert snapshot["source"]["format"] == "HSD DAT, big-endian ftData.x30 hurt capsule init table"
    assert snapshot["count"] > 0
    assert snapshot["count"] <= 15
    first = snapshot["hurtboxes"][0]
    assert set(first) == {
        "id",
        "bone_idx",
        "height",
        "is_grabbable",
        "a_offset_raw",
        "a_offset_milli",
        "b_offset_raw",
        "b_offset_milli",
        "scale_raw",
        "scale_milli",
    }
    assert set(first["a_offset_raw"]) == {"x", "y", "z"}
    assert set(first["b_offset_raw"]) == {"x", "y", "z"}
```

- [ ] **Step 2: Run the focused test and verify it fails**

Run:

```powershell
python -m pytest tests/test_extract_melee_resources.py::test_extract_captain_hurtbox_inits_from_plca_reads_ftdata_x30 -q
```

Expected: fail because `extract_captain_hurtbox_inits_from_plca` does not exist.

- [ ] **Step 3: Add the extraction function**

Add this helper in `tools/extract_melee_resources.py` after `extract_captain_ecb_source_from_plca`:

```python
FT_HURTBOX_INIT_SIZE = 0x28


def extract_captain_hurtbox_inits_from_plca(dat: bytes, source_path: Path) -> dict[str, object]:
    symbol, ftdata_offset = find_root(dat, lambda name: name == "ftDataCaptain", "ftDataCaptain")
    header_offset = 0x20 + ftdata_offset
    hurtbox_table_offset = read_u32(dat, header_offset + 0x30)
    data_block_size, _relocation_count, _root_count, _external_count = dat_header_counts(dat)
    if hurtbox_table_offset == 0 or hurtbox_table_offset + 8 > data_block_size:
        raise DatExtractError("ftDataCaptain.x30 does not cover ftData_x30 hurtbox init metadata")

    table = 0x20 + hurtbox_table_offset
    count = read_i32(dat, table)
    inits_offset = read_u32(dat, table + 0x04)
    if count < 0 or count > 15:
        raise DatExtractError(f"Captain hurt capsule count {count} is outside Fighter.hurt_capsules[15]")
    if inits_offset == 0 or inits_offset + count * FT_HURTBOX_INIT_SIZE > data_block_size:
        raise DatExtractError("Captain hurt capsule init records are outside PlCa.dat data block")

    hurtboxes: list[dict[str, object]] = []
    for index in range(count):
        record = 0x20 + inits_offset + index * FT_HURTBOX_INIT_SIZE
        a_offset_raw = vec3_raw(dat, inits_offset + index * FT_HURTBOX_INIT_SIZE + 0x0C)
        b_offset_raw = vec3_raw(dat, inits_offset + index * FT_HURTBOX_INIT_SIZE + 0x18)
        scale_raw = read_f32(dat, record + 0x24)
        hurtboxes.append(
            {
                "id": index,
                "bone_idx": read_u32(dat, record),
                "height": read_u32(dat, record + 0x04),
                "is_grabbable": read_u32(dat, record + 0x08) != 0,
                "a_offset_raw": a_offset_raw,
                "a_offset_milli": vec3_milli(a_offset_raw),
                "b_offset_raw": b_offset_raw,
                "b_offset_milli": vec3_milli(b_offset_raw),
                "scale_raw": scale_raw,
                "scale_milli": rust_round(scale_raw * 1000.0),
            }
        )

    return {
        "source": {
            "file": source_path_for_json(source_path),
            "symbol": symbol,
            "ft_data_offset": ftdata_offset,
            "ftdata_hurtbox_table_offset": hurtbox_table_offset,
            "hurtbox_inits_offset": inits_offset,
            "format": "HSD DAT, big-endian ftData.x30 hurt capsule init table",
        },
        "count": count,
        "hurtboxes": hurtboxes,
    }
```

- [ ] **Step 4: Wire the generated snapshot**

In `extract_resources`, after writing `captain_falcon_ecb_source.json`, add:

```python
out_path = out_dir / "captain_falcon_hurtbox_inits.json"
write_json(out_path, extract_captain_hurtbox_inits_from_plca(plca_bytes, plca))
written.append(out_path)
```

- [ ] **Step 5: Run the focused test and regenerate resources**

Run:

```powershell
python -m pytest tests/test_extract_melee_resources.py::test_extract_captain_hurtbox_inits_from_plca_reads_ftdata_x30 -q
python tools\extract_melee_resources.py
```

Expected: test passes and `resources/melee/extracted/captain_falcon_hurtbox_inits.json` is written.

---

### Task 2: Sample Hurt Capsules From FigaTree/JObj Pose

**Files:**
- Modify: `tools/extract_melee_resources.py`
- Modify: `tests/test_extract_melee_resources.py`
- Create: `resources/melee/extracted/captain_falcon_action_hurtbox_samples.json`

- [ ] **Step 1: Write the failing sampling test**

Add:

```python
def test_captain_action_hurtbox_samples_preserve_source_z_and_flatten_view_z():
    raw_dir = ROOT / "resources" / "melee" / "raw"
    plca = raw_dir / "PlCa.dat"
    plcaaj = raw_dir / "PlCaAJ.dat"
    plcanr = raw_dir / "PlCaNr.dat"
    if not (plca.exists() and plcaaj.exists() and plcanr.exists()):
        pytest.skip("local PlCa.dat, PlCaAJ.dat, and PlCaNr.dat are required")

    from tools.extract_melee_resources import (
        extract_captain_action_animation_table,
        extract_captain_action_hurtbox_samples,
        extract_captain_costume_skeleton_from_plcanr,
        extract_captain_hurtbox_inits_from_plca,
    )

    action_table = extract_captain_action_animation_table(
        plca.read_bytes(), plca, plcaaj.read_bytes(), plcaaj
    )
    skeleton = extract_captain_costume_skeleton_from_plcanr(plcanr.read_bytes(), plcanr)
    hurtbox_inits = extract_captain_hurtbox_inits_from_plca(plca.read_bytes(), plca)

    samples = extract_captain_action_hurtbox_samples(
        plcaaj.read_bytes(),
        action_table,
        skeleton,
        hurtbox_inits,
        action_state_ids=(68,),
    )

    action = samples["actions"][0]
    assert action["action_state_id"] == 68
    assert action["name"].endswith("AttackAirN_figatree")
    assert len(action["frames"]) == 35
    first_hurtbox = action["frames"][0]["hurtboxes"][0]
    assert first_hurtbox["source_a"]["z"] == first_hurtbox["source_a"]["z"]
    assert first_hurtbox["a"]["z"] == 0.0
    assert first_hurtbox["source_b"]["z"] == first_hurtbox["source_b"]["z"]
    assert first_hurtbox["b"]["z"] == 0.0
    assert first_hurtbox["source"] == "ftData.x30 + PlCaAJ FigaTree + PlCaNr JObj skeleton"
```

- [ ] **Step 2: Run the focused test and verify it fails**

Run:

```powershell
python -m pytest tests/test_extract_melee_resources.py::test_captain_action_hurtbox_samples_preserve_source_z_and_flatten_view_z -q
```

Expected: fail because `extract_captain_action_hurtbox_samples` does not exist.

- [ ] **Step 3: Add sampling helpers**

Add these helpers after `compute_ecb_from_jobj_pose`:

```python
def _vec3_add(left: dict[str, float], right: dict[str, object]) -> dict[str, float]:
    return {
        "x": float(left["x"]) + float(right["x"]),
        "y": float(left["y"]) + float(right["y"]),
        "z": float(left["z"]) + float(right["z"]),
    }


def _flatten_z(point: dict[str, float]) -> dict[str, float]:
    return {"x": point["x"], "y": point["y"], "z": 0.0}


def sample_hurtboxes_from_pose(
    pose: dict[str, object],
    hurtbox_inits: dict[str, object],
) -> list[dict[str, object]]:
    joints = pose.get("joints")
    hurtboxes = hurtbox_inits.get("hurtboxes")
    if not isinstance(joints, list) or not isinstance(hurtboxes, list):
        raise DatExtractError("hurtbox sampling requires pose joints and static hurtbox init records")

    sampled: list[dict[str, object]] = []
    for init in hurtboxes:
        if not isinstance(init, dict):
            raise DatExtractError("hurtbox init record is malformed")
        bone_idx = int(init["bone_idx"])
        joint = joints[bone_idx]
        if not isinstance(joint, dict) or not isinstance(joint.get("world_position_raw"), dict):
            raise DatExtractError(f"pose joint {bone_idx} does not contain a world position")
        base = joint["world_position_raw"]
        source_a = _vec3_add(base, init["a_offset_raw"])
        source_b = _vec3_add(base, init["b_offset_raw"])
        sampled.append(
            {
                "id": init["id"],
                "kind": "capsule",
                "bone": bone_idx,
                "height": init["height"],
                "is_grabbable": init["is_grabbable"],
                "a": _flatten_z(source_a),
                "b": _flatten_z(source_b),
                "source_a": source_a,
                "source_b": source_b,
                "a_offset": init["a_offset_raw"],
                "b_offset": init["b_offset_raw"],
                "radius": init["scale_raw"],
                "state": "HurtCapsule_Enabled",
                "source": "ftData.x30 + PlCaAJ FigaTree + PlCaNr JObj skeleton",
                "confidence": "source_extracted",
            }
        )
    return sampled
```

- [ ] **Step 4: Add per-action sampling**

Add:

```python
def extract_captain_action_hurtbox_samples(
    plcaaj: bytes,
    action_table: dict[str, object],
    skeleton: dict[str, object],
    hurtbox_inits: dict[str, object],
    *,
    action_state_ids: tuple[int, ...] = (68,),
) -> dict[str, object]:
    actions = action_table.get("actions")
    if not isinstance(actions, list):
        raise DatExtractError("Captain action animation table is malformed")

    sampled_actions: list[dict[str, object]] = []
    for action_state_id in action_state_ids:
        action = next(
            (
                item
                for item in actions
                if isinstance(item, dict) and int(item["action_state_id"]) == action_state_id
            ),
            None,
        )
        if action is None:
            raise DatExtractError(f"Captain action state {action_state_id} is missing")
        figatree = action.get("figatree")
        if not isinstance(figatree, dict):
            raise DatExtractError(f"Captain action state {action_state_id} has no FigaTree")
        offset = int(action["figatree_archive_offset"])
        size = int(action["figatree_archive_size"])
        if offset < 0 or size <= 0 or offset + size > len(plcaaj):
            raise DatExtractError(
                f"Captain action state {action_state_id} FigaTree archive range is outside PlCaAJ.dat"
            )

        chunk = plcaaj[offset : offset + size]
        frame_count = int(figatree["frames_ticks"])
        frames: list[dict[str, object]] = []
        for frame in range(frame_count):
            pose = sample_figatree_skeleton_pose(chunk, skeleton, frame=float(frame))
            frames.append(
                {
                    "frame": frame + 1,
                    "hurtboxes": sample_hurtboxes_from_pose(pose, hurtbox_inits),
                }
            )
        sampled_actions.append(
            {
                "action_state_id": action_state_id,
                "name": action.get("name", ""),
                "figatree_root": action.get("figatree_root"),
                "frames": frames,
            }
        )

    return {
        "source": {
            "format": "Captain Falcon hurt capsule samples from ftData.x30, PlCaAJ FigaTree, and PlCaNr HSD_Joint skeleton",
        },
        "actions": sampled_actions,
    }
```

- [ ] **Step 5: Wire the generated snapshot**

In `extract_resources`, after action ECB samples are written, also write:

```python
out_path = out_dir / "captain_falcon_action_hurtbox_samples.json"
write_json(
    out_path,
    extract_captain_action_hurtbox_samples(
        plcaaj_bytes,
        action_table_snapshot,
        skeleton_snapshot,
        hurtbox_inits_snapshot,
        action_state_ids=ECB_SAMPLE_ACTION_STATE_IDS,
    ),
)
written.append(out_path)
```

Use the same action id tuple as ECB sampling for the first pass so non-attack states already covered by the existing ECB report receive hurt capsule samples too.

- [ ] **Step 6: Run tests and regenerate resources**

Run:

```powershell
python -m pytest tests/test_extract_melee_resources.py::test_captain_action_hurtbox_samples_preserve_source_z_and_flatten_view_z -q
python tools\extract_melee_resources.py
```

Expected: test passes and `captain_falcon_action_hurtbox_samples.json` is generated.

---

### Task 3: Decode Action-Script Hurt Capsule State Commands

**Files:**
- Modify: `crates/mole_cli/src/frame_data.rs`
- Modify: `crates/mole_cli/tests/cli_contract.rs`

- [ ] **Step 1: Write the failing CLI test**

Add this test in `crates/mole_cli/tests/cli_contract.rs` near `frame_data_extract_decodes_source_action_script_hitbox_procedures`:

```rust
#[test]
fn frame_data_extract_decodes_source_hurt_state_procedures() {
    let root = temp_project_root("frame_data_source_hurt_state");
    write_frame_data_fixture(&root);
    write_captain_action_table_fixture(&root);
    write_captain_raw_script_fixture_with_hurt_state(&root);

    let output = run_cli_json(
        &root,
        [
            "frame-data",
            "extract",
            "--character",
            "dolphin_mole",
            "--source-character",
            "captain",
            "--state",
            "AttackAirN",
            "--json",
        ],
    );
    let artifact = &output["artifact"];
    let procedures = artifact["decoded_action_script"]["procedures"].as_array().unwrap();

    assert!(procedures.iter().any(|procedure| {
        procedure["procedure"] == "fighter.set_hurt_state"
            && procedure["handler"] == "ftAction_80071A9C"
            && procedure["bone_idx"] == 14
            && procedure["state"] == "Intangible"
    }));
}
```

- [ ] **Step 2: Add the fixture writer**

Add this helper near the existing raw script fixture writer:

```rust
fn write_captain_raw_script_fixture_with_hurt_state(root: &Path) {
    let raw_dir = root.join("resources/melee/raw");
    fs::create_dir_all(&raw_dir).unwrap();
    let mut raw = vec![0u8; 0x20 + 19908 + 64];
    let words = [
        0x0400_0007u32, // wait 7
        0x700e_0002u32, // opcode 28, bone_idx 14, state 2
        0x0000_0000u32,
    ];
    for (index, word) in words.iter().enumerate() {
        raw[0x20 + 19908 + index * 4..0x20 + 19908 + index * 4 + 4]
            .copy_from_slice(&word.to_be_bytes());
    }
    fs::write(raw_dir.join("PlCa.dat"), raw).unwrap();
}
```

- [ ] **Step 3: Run the focused test and verify it fails**

Run:

```powershell
cargo test -p mole_cli frame_data_extract_decodes_source_hurt_state_procedures
```

Expected: fail because `fighter.set_hurt_state` is not decoded.

- [ ] **Step 4: Add decoded hurt-state support**

In `crates/mole_cli/src/frame_data.rs`:

```rust
#[derive(Debug, Clone)]
struct DecodedHurtState {
    frame: u64,
    word_offset: usize,
    raw_word: u32,
    bone_idx: u64,
    state: u64,
}
```

Add this enum variant:

```rust
SetHurtState(DecodedHurtState),
```

In `decode_action_script`, add this `fighter_index` match arm:

```rust
18 => {
    procedures.push(DecodedProcedure::SetHurtState(decode_set_hurt_state(
        current_frame,
        word_offset,
        word,
    )));
}
```

Add:

```rust
fn decode_set_hurt_state(frame: u64, word_offset: usize, raw_word: u32) -> DecodedHurtState {
    DecodedHurtState {
        frame,
        word_offset,
        raw_word,
        bone_idx: bitfield(raw_word, 6, 8) as u64,
        state: bitfield(raw_word, 14, 18) as u64,
    }
}

fn hurt_capsule_state_name(state: u64) -> &'static str {
    match state {
        0 => "HurtCapsule_Enabled",
        1 => "HurtCapsule_Disabled",
        2 => "Intangible",
        _ => "unknown",
    }
}
```

In `decoded_action_script_json`, add:

```rust
DecodedProcedure::SetHurtState(hurt_state) => json!({
    "procedure": "fighter.set_hurt_state",
    "handler": "ftAction_80071A9C",
    "frame": hurt_state.frame,
    "word_offset": hurt_state.word_offset,
    "raw_words": raw_words_json(&[hurt_state.raw_word]),
    "bone_idx": hurt_state.bone_idx,
    "state": hurt_capsule_state_name(hurt_state.state),
    "state_raw": hurt_state.state,
}),
```

- [ ] **Step 5: Run the focused test**

Run:

```powershell
cargo test -p mole_cli frame_data_extract_decodes_source_hurt_state_procedures
```

Expected: pass.

---

### Task 4: Merge Source Hurtbox Samples Into Frame Data

**Files:**
- Modify: `crates/mole_cli/src/frame_data.rs`
- Modify: `crates/mole_cli/tests/cli_contract.rs`
- Modify: `resources/melee/frame_data/dolphin_mole/AttackAirN.json`

- [ ] **Step 1: Write the failing CLI contract test**

Add:

```rust
#[test]
fn frame_data_extract_uses_source_hurtbox_samples_instead_of_preview() {
    let output = run_cli_json(
        project_root(),
        [
            "frame-data",
            "extract",
            "--character",
            "dolphin_mole",
            "--source-character",
            "captain",
            "--state",
            "AttackAirN",
            "--json",
        ],
    );
    let artifact = &output["artifact"];
    let frame_7 = artifact["keyframes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|frame| frame["frame"] == 7)
        .unwrap();
    let hurtboxes = frame_7["hurtboxes"].as_array().unwrap();

    assert!(!hurtboxes.is_empty());
    assert!(hurtboxes.iter().all(|hurtbox| hurtbox["source"] != "generated_ecb_preview"));
    assert_eq!(hurtboxes[0]["confidence"], "source_extracted");
    assert!(hurtboxes[0]["source_a"].is_object());
    assert!(hurtboxes[0]["source_b"].is_object());
    assert_eq!(hurtboxes[0]["a"]["z"], 0.0);
    assert_eq!(hurtboxes[0]["b"]["z"], 0.0);
}
```

- [ ] **Step 2: Run it and verify it fails**

Run:

```powershell
cargo test -p mole_cli frame_data_extract_uses_source_hurtbox_samples_instead_of_preview
```

Expected: fail because the CLI has not loaded `captain_falcon_action_hurtbox_samples.json`.

- [ ] **Step 3: Load the sample snapshot**

Add helpers in `crates/mole_cli/src/frame_data.rs`:

```rust
fn action_hurtbox_samples_path(root: &Path, source_character: &str) -> PathBuf {
    let filename = match source_character {
        "captain" | "captain_falcon" => "captain_falcon_action_hurtbox_samples.json".to_string(),
        other => format!("{other}_action_hurtbox_samples.json"),
    };
    root.join("resources").join("melee").join("extracted").join(filename)
}

fn source_hurtboxes_for_action(
    root: &Path,
    source_character: &str,
    action_state_id: u64,
) -> Option<Vec<Value>> {
    let samples = read_json_object(&action_hurtbox_samples_path(root, source_character)).ok()?;
    let actions = samples.get("actions")?.as_array()?;
    let action = actions.iter().find(|action| {
        action.get("action_state_id").and_then(Value::as_u64) == Some(action_state_id)
    })?;
    Some(action.get("frames")?.as_array()?.clone())
}
```

- [ ] **Step 4: Apply hurtboxes to keyframes**

After action lookup in `decoded_action_script_artifact`, pass source character/action id into a new merge function:

```rust
if let Some(hurtbox_frames) = source_hurtboxes_for_action(root, source_character, action_state_id) {
    apply_source_hurtboxes_to_keyframes(&mut artifact, hurtbox_frames);
    remove_gap(&mut artifact, "hurtboxes");
}
```

Add:

```rust
fn apply_source_hurtboxes_to_keyframes(artifact: &mut Value, hurtbox_frames: Vec<Value>) {
    let mut keyframes = artifact
        .get("keyframes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    for source_frame in hurtbox_frames {
        let Some(frame_number) = source_frame.get("frame").and_then(Value::as_u64) else {
            continue;
        };
        let hurtboxes = source_frame
            .get("hurtboxes")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if let Some(existing) = keyframes
            .iter_mut()
            .find(|item| item.get("frame").and_then(Value::as_u64) == Some(frame_number))
        {
            existing["hurtboxes"] = Value::Array(hurtboxes);
        } else {
            keyframes.push(json!({
                "frame": frame_number,
                "interpolates_from_previous": true,
                "pose": [],
                "hitboxes": [],
                "hurtboxes": hurtboxes,
            }));
        }
    }

    keyframes.sort_by_key(|item| item.get("frame").and_then(Value::as_u64).unwrap_or(0));
    artifact["keyframes"] = Value::Array(keyframes);
}
```

- [ ] **Step 5: Re-run test and regenerate `AttackAirN.json`**

Run:

```powershell
cargo test -p mole_cli frame_data_extract_uses_source_hurtbox_samples_instead_of_preview
cargo run -q -p mole_cli -- frame-data extract --character dolphin_mole --source-character captain --state AttackAirN --json
```

Write the emitted artifact back to `resources/melee/frame_data/dolphin_mole/AttackAirN.json` with PowerShell so the canonical file matches CLI output:

```powershell
$report = cargo run -q -p mole_cli -- frame-data extract --character dolphin_mole --source-character captain --state AttackAirN --json | ConvertFrom-Json
$artifactPath = "resources\melee\frame_data\dolphin_mole\AttackAirN.json"
$report.artifact | ConvertTo-Json -Depth 100 | Set-Content -Path $artifactPath -Encoding utf8
```

After writing, run:

```powershell
cargo run -q -p mole_cli -- frame-data show --character dolphin_mole --state AttackAirN --format markdown
```

Expected: the markdown summary still resolves `AttackAirN`, and the artifact keeps `decoded_action_script`.

---

### Task 5: Keep ECB/Body Volume Separate From Hurt Capsules

**Files:**
- Modify: `crates/mole_cli/src/frame_data.rs`
- Modify: `resources/melee/frame_data/dolphin_mole/AttackAirN.json`
- Modify: `docs/superpowers/specs/2026-06-01-move-frame-data-design.md`

- [ ] **Step 1: Write the failing schema expectation**

Add to the CLI contract test from Task 4:

```rust
assert!(artifact["keyframes"][0]["hurtboxes"].is_array());
assert!(artifact["keyframes"][0]["body_volumes"].is_array());
assert!(artifact["keyframes"][0]["body_volumes"][0]["source"]
    .as_str()
    .unwrap()
    .contains("ftData.x44"));
```

- [ ] **Step 2: Run the focused test and verify it fails**

Run:

```powershell
cargo test -p mole_cli frame_data_extract_uses_source_hurtbox_samples_instead_of_preview
```

Expected: fail because `body_volumes` are not populated.

- [ ] **Step 3: Load `captain_falcon_action_ecb_samples.json` separately**

Add helpers mirroring the hurtbox sample loader:

```rust
fn action_ecb_samples_path(root: &Path, source_character: &str) -> PathBuf {
    let filename = match source_character {
        "captain" | "captain_falcon" => "captain_falcon_action_ecb_samples.json".to_string(),
        other => format!("{other}_action_ecb_samples.json"),
    };
    root.join("resources").join("melee").join("extracted").join(filename)
}
```

Convert each frame's ECB sample into:

```json
{
  "id": "ecb",
  "kind": "diamond",
  "source": "ftData.x44 + PlCaAJ FigaTree + PlCaNr JObj skeleton",
  "top": {"x": 0.0, "y": 0.0, "z": 0.0},
  "bottom": {"x": 0.0, "y": 0.0, "z": 0.0},
  "left": {"x": 0.0, "y": 0.0, "z": 0.0},
  "right": {"x": 0.0, "y": 0.0, "z": 0.0},
  "source_points": []
}
```

Use the source sample's `*_raw` values for source coordinates, and flatten emitted `top/bottom/left/right.z` to `0.0`.

- [ ] **Step 4: Amend the spec wording**

In `docs/superpowers/specs/2026-06-01-move-frame-data-design.md`, change the data model bullet:

```text
- hurtbox or ECB/capsule definitions and active windows,
```

to:

```text
- hurt capsule definitions and active windows,
- ECB/body-volume definitions and active windows, kept separate from hurt capsules,
```

- [ ] **Step 5: Run focused test and regenerate artifact**

Run:

```powershell
cargo test -p mole_cli frame_data_extract_uses_source_hurtbox_samples_instead_of_preview
cargo run -q -p mole_cli -- frame-data extract --character dolphin_mole --source-character captain --state AttackAirN --json
```

Expected: `hurtboxes` and `body_volumes` are both present and separately sourced.

---

### Task 6: Upgrade Dev Tool Hurtbox And Body-Volume Display

**Files:**
- Modify: `tools/state_graph_viewer.py`
- Modify: `tests/test_state_graph_viewer.py`

- [ ] **Step 1: Write failing detail-panel test**

Add:

```python
def test_move_keyframe_details_expose_all_source_hurtbox_values():
    data = load_move_frame_data(DEFAULT_MOVE_FRAME_DATA_DIR, "dolphin_mole", "AttackAirN")
    frame = next(item for item in data["keyframes"] if item["frame"] == 7)

    details = format_move_keyframe_details(data, frame)

    assert "hurtbox[0]" in details
    assert "source_a=(" in details
    assert "source_b=(" in details
    assert "height=" in details
    assert "is_grabbable=" in details
    assert "state=HurtCapsule_Enabled" in details
    assert "confidence=source_extracted" in details
    assert "Body volumes:" in details
    assert "body_volume[ecb]" in details
```

- [ ] **Step 2: Write failing canvas test**

Add:

```python
def test_move_keyframe_canvas_draws_hurtbox_travel_and_body_volume_separately():
    data = load_move_frame_data(DEFAULT_MOVE_FRAME_DATA_DIR, "dolphin_mole", "AttackAirN")
    canvas = RecordingCanvas()

    draw_move_keyframe_canvas(canvas, data, selected_frame_number=7)

    hurtbox_travel = [
        call
        for call in canvas.calls
        if call[0] == "create_line" and "move_hurtbox_travel" in call[2].get("tags", ())
    ]
    body_lines = [
        call
        for call in canvas.calls
        if call[0] == "create_line" and "move_body_volume" in call[2].get("tags", ())
    ]
    assert hurtbox_travel
    assert body_lines
```

- [ ] **Step 3: Run tests and verify they fail**

Run:

```powershell
python -m pytest tests/test_state_graph_viewer.py -k "source_hurtbox_values or hurtbox_travel_and_body_volume" -q
```

Expected: fail because details/canvas do not expose the new fields yet.

- [ ] **Step 4: Expand details formatting**

In `format_move_keyframe_details`, for each hurtbox, include:

```python
for field in [
    "radius",
    "height",
    "is_grabbable",
    "state",
    "source_handler",
    "source_word_offset",
    "confidence",
    "source",
]:
    if field in hurtbox:
        lines.append(f"  {field}={hurtbox[field]}")
```

Also add a `Body volumes:` section that lists `body_volume[{id}] kind={kind}` and source fields for each `frame.get("body_volumes", [])`.

- [ ] **Step 5: Expand canvas drawing**

Add `_draw_move_hurtbox_travel` matching hurtboxes by `id`, drawing dashed lines between previous/current `a` and previous/current `b` with tag `("move_hurtbox_travel",)`.

Add `_draw_move_body_volumes` that draws ECB diamond lines from `top/right/bottom/left/top` with tag `("move_body_volume",)`.

Call both from `draw_move_keyframe_canvas` before drawing current hitboxes, so body volumes sit behind hit/hurt capsules.

- [ ] **Step 6: Run focused and full viewer tests**

Run:

```powershell
python -m pytest tests/test_state_graph_viewer.py -k "source_hurtbox_values or hurtbox_travel_and_body_volume" -q
python -m pytest tests/test_state_graph_viewer.py -q
```

Expected: focused tests pass and full viewer tests pass.

---

### Task 7: Document The New Source Model

**Files:**
- Modify: `resources/melee/README.md`
- Modify: `docs/superpowers/specs/2026-06-01-move-frame-data-design.md`

- [ ] **Step 1: Update resource README**

Add this text under the generated files section in `resources/melee/README.md`:

```markdown
- `resources/melee/extracted/captain_falcon_hurtbox_inits.json`
- `resources/melee/extracted/captain_falcon_action_hurtbox_samples.json`
```

Add:

```markdown
`captain_falcon_hurtbox_inits.json` decodes `ftDataCaptain.x30`, the static
hurt capsule init table copied by `ftColl_8007B320` into
`Fighter.hurt_capsules[15]`. `captain_falcon_action_hurtbox_samples.json`
samples those capsule offsets against the Captain Falcon neutral skeleton and
per-action FigaTree pose. These are distinct from ECB/body-volume samples:
hurt capsules are the attack-victim collision targets, while ECB samples remain
the environment/body-volume path produced from `ftDataCaptain.x44`.
```

- [ ] **Step 2: Update frame-data spec**

Add:

```markdown
`hurtboxes` means Melee `FighterHurtCapsule` / `HurtCapsule` data sourced from
`ftData.x30`, per-frame JObj pose, and action-script hurt-state commands.
`body_volumes` means ECB/body-volume samples sourced from `ftData.x44` and
`mpColl_LoadECB_JObj`-equivalent reduction. The two are related through the rig
but are not interchangeable and must not share provenance labels.
```

- [ ] **Step 3: Run documentation-adjacent checks**

Run:

```powershell
git diff --check -- resources/melee/README.md docs/superpowers/specs/2026-06-01-move-frame-data-design.md
```

Expected: exit 0.

---

### Task 8: Final Verification Gate

**Files:**
- No new files.

- [ ] **Step 1: Run focused Python extractor tests**

Run:

```powershell
python -m pytest tests/test_extract_melee_resources.py -q
```

Expected: all tests pass. If raw DAT files are missing in another environment, raw-DAT-dependent tests should skip with explicit skip messages.

- [ ] **Step 2: Run focused Rust CLI tests**

Run:

```powershell
cargo test -p mole_cli frame_data
```

Expected: frame-data tests pass.

- [ ] **Step 3: Run dev tool tests**

Run:

```powershell
python -m pytest tests/test_state_graph_viewer.py -q
```

Expected: all viewer tests pass.

- [ ] **Step 4: Run formatting and whitespace checks**

Run:

```powershell
cargo fmt -p mole_cli --check
git diff --check
```

Expected: `cargo fmt` exits 0. `git diff --check` exits 0; CRLF warnings may appear in this dirty checkout and should be reported as pre-existing line-ending warnings when no whitespace errors are emitted.

- [ ] **Step 5: Sanity-check generated frame-data output**

Run:

```powershell
cargo run -q -p mole_cli -- frame-data show --character dolphin_mole --state AttackAirN --format markdown
```

Expected: markdown summary lists hitboxes, source-extracted hurtboxes, and separately sourced body volumes for `AttackAirN`.

---

## Execution Notes

- Do not replace the hitbox decoder with Python output. The Rust CLI remains the canonical frame-data surface.
- Do not label ECB/body-volume samples as hurtboxes.
- Do not drop source Z values. The source fields keep `{x, y, z}`; current 2D fields flatten `z` to `0.0`.
- Do not make this Captain Falcon-only in schema. The first concrete data set is Captain Falcon/Dolphin Mole because that is the available local source, but file naming and loader functions should map by `source_character`.
- Do not implement editable overrides in this slice. The right-side dev-tool value display should make future editing obvious, but mutation belongs to the next import/editor plan.

## Self-Review

- Spec coverage: The plan covers native JObj rig sampling, static hurt capsule extraction, action-script hurt-state overlays, ECB/body-volume separation, frame-data artifact integration, dev-tool display, and verification.
- Placeholder scan: No task uses an unspecified implementation placeholder. Each task has concrete commands and expected outcomes.
- Scope check: This is intentionally foundation-only. It does not rewrite the Rust runtime rig system and does not implement editable override persistence.
