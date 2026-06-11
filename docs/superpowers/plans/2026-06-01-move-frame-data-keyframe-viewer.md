# Move Frame Data Keyframe Viewer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a read-only `move_frame_data` pipeline that extracts Captain Falcon-derived attack data into Dolphin Mole-scoped artifacts, exposes it through Mole CLI, and displays it in a new dev-tool keyframe viewer tab.

**Architecture:** Add a canonical JSON artifact under `resources/melee/frame_data/<target_character>/<state>.json`. Rust CLI commands load or synthesize that artifact for agents, and the Rust dev tool consumes the same artifact for the current keyframe editor shell. Preserve all source coordinates as 3D `{x, y, z}` and apply 2D flattening only through explicit projection metadata at render/import boundaries. Prefer Rust for reusable extraction/schema/import logic and UI work; use Python only as historical reference or temporary legacy extraction support while Rust replacements are added.

**Tech Stack:** Rust `mole_cli` with `serde_json`, Python `tools/state_graph_viewer.py` with Tk/ttk, pytest contract tests, existing extracted Melee resources and local decomp lookup.

---

## File Structure

- Create `resources/melee/frame_data/dolphin_mole/AttackAirN.json`
  - Canonical first artifact for Dolphin Mole using Captain Falcon neutral air as the source character/state.
- Modify `crates/mole_cli/src/lib.rs`
  - Add `frame-data` command parsing, command structs, help catalog entries, and command routing.
- Create `crates/mole_cli/src/frame_data.rs`
  - Load frame-data artifacts, synthesize the first Dolphin Mole/Captain Falcon `AttackAirN` report, and build JSON reports.
- Modify `crates/mole_cli/src/formatting.rs`
  - Add markdown formatting for `frame-data extract` and `frame-data show`.
- Modify `crates/mole_cli/README.md`
  - Add agent-facing examples.
- Modify `crates/mole_cli/tests/cli_contract.rs`
  - Add CLI contract tests for help, metadata, 3D preservation, gaps, and markdown citations.
- Modify `tools/state_graph_viewer.py`
  - Add constants, frame-data artifact loaders/formatters, `Move Keyframes` tab, character/state selectors, projected overlay rendering, and empty-state handling.
- Modify `tests/test_state_graph_viewer.py`
  - Add loader/formatter tests for Dolphin Mole populated data and the empty second test character.

---

### Task 1: Add Canonical Dolphin Mole AttackAirN Artifact And Python Loader Tests

**Files:**
- Create: `resources/melee/frame_data/dolphin_mole/AttackAirN.json`
- Modify: `tools/state_graph_viewer.py`
- Modify: `tests/test_state_graph_viewer.py`

- [ ] **Step 1: Write failing tests for character-scoped move data loading**

Add these imports to `tests/test_state_graph_viewer.py`:

```python
from tools.state_graph_viewer import (
    MOVE_KEYFRAMES_TAB_LABEL,
    DEFAULT_MOVE_FRAME_DATA_DIR,
    build_move_keyframe_empty_state,
    format_move_keyframe_details,
    list_move_frame_data_characters,
    list_move_frame_data_states,
    load_move_frame_data,
    project_move_point,
)
```

If the file already imports from `tools.state_graph_viewer`, merge these names into the existing import list.

Add these tests near the other dev-tool tab tests:

```python
def test_move_keyframes_tab_label_and_default_characters_are_visible():
    assert MOVE_KEYFRAMES_TAB_LABEL == "Move Keyframes"
    characters = list_move_frame_data_characters(DEFAULT_MOVE_FRAME_DATA_DIR)

    assert characters[0]["id"] == "dolphin_mole"
    assert characters[0]["label"] == "Dolphin Mole"
    assert characters[0]["populated"] is True
    assert characters[1]["id"] == "test_character_2"
    assert characters[1]["label"] == "Test Character 2"
    assert characters[1]["populated"] is False


def test_move_frame_data_loads_dolphin_mole_attack_air_n_source_metadata():
    data = load_move_frame_data(DEFAULT_MOVE_FRAME_DATA_DIR, "dolphin_mole", "AttackAirN")

    assert data["schema_version"] == 1
    assert data["target_character"] == "dolphin_mole"
    assert data["target_character_label"] == "Dolphin Mole"
    assert data["source_character"] == "captain"
    assert data["state"] == "AttackAirN"
    assert data["label"] == "Neutral Air"
    assert data["projection"]["source_space"] == "melee_xyz"
    assert data["projection"]["z_policy"] == "preserve_and_project"
    assert data["summary"]["total_frames"] == 35
    assert data["sources"][0]["kind"] == "decomp"
    assert data["keyframes"][0]["hurtboxes"][0]["a"]["z"] == 0.0


def test_move_frame_data_states_are_character_scoped_and_empty_character_is_honest():
    dolphin_states = list_move_frame_data_states(DEFAULT_MOVE_FRAME_DATA_DIR, "dolphin_mole")
    empty_states = list_move_frame_data_states(DEFAULT_MOVE_FRAME_DATA_DIR, "test_character_2")

    assert dolphin_states == [{"state": "AttackAirN", "label": "Neutral Air", "populated": True}]
    assert empty_states == []
    assert "No move frame data populated for Test Character 2" in build_move_keyframe_empty_state(
        "Test Character 2"
    )


def test_move_keyframe_details_preserve_z_and_projection_flattens_only_for_view():
    data = load_move_frame_data(DEFAULT_MOVE_FRAME_DATA_DIR, "dolphin_mole", "AttackAirN")
    frame = data["keyframes"][1]
    details = format_move_keyframe_details(data, frame)
    point = project_move_point({"x": 3.5, "y": 8.0, "z": -1.25}, data["projection"])

    assert "Frame 6" in details
    assert "hitbox[0]" in details
    assert "z=-1.25" in details
    assert point == {"x": 3.5, "y": 8.0, "z": -1.25, "view_x": 3.5, "view_y": 8.0}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```powershell
pytest tests/test_state_graph_viewer.py -k "move_frame_data or move_keyframes" -q
```

Expected: FAIL with import errors for the new constants/functions.

- [ ] **Step 3: Create the first artifact**

Create `resources/melee/frame_data/dolphin_mole/AttackAirN.json` with this exact content:

```json
{
  "schema_version": 1,
  "target_character": "dolphin_mole",
  "target_character_label": "Dolphin Mole",
  "source_character": "captain",
  "source_character_label": "Captain Falcon",
  "state": "AttackAirN",
  "label": "Neutral Air",
  "projection": {
    "source_space": "melee_xyz",
    "default_view": "xy",
    "z_policy": "preserve_and_project",
    "view_scale": 10.0
  },
  "sources": [
    {
      "kind": "decomp",
      "path": "src/melee/ft/chara/ftCommon/ftCo_AttackAir.c",
      "line": 1,
      "purpose": "common aerial attack state semantics and callback flow"
    },
    {
      "kind": "extracted_action",
      "path": "resources/melee/extracted/captain_falcon_action_animation_table.json",
      "action_state_id": 68,
      "purpose": "Captain Falcon AttackAirN action figatree metadata"
    },
    {
      "kind": "generated_ecb",
      "path": "crates/mole_core/src/generated/falcon_ecb.rs",
      "purpose": "sampled Falcon ECB reference frames for projected hurt volume preview"
    }
  ],
  "summary": {
    "total_frames": 35,
    "iasa_frame": "unknown",
    "landing_lag_frames": "unknown",
    "active_hitbox_windows": [
      {"start": 6, "end": 9, "source": "fixture_for_viewer_pipeline"}
    ],
    "active_hurtbox_windows": [
      {"start": 1, "end": 35, "source": "generated_ecb_preview"}
    ]
  },
  "keyframes": [
    {
      "frame": 1,
      "interpolates_from_previous": false,
      "pose": [],
      "hitboxes": [],
      "hurtboxes": [
        {
          "id": "torso",
          "kind": "capsule",
          "bone": "TransN",
          "a": {"x": -0.4, "y": 6.0, "z": 0.0},
          "b": {"x": 0.2, "y": 13.0, "z": 0.0},
          "radius": 3.8,
          "source": "generated_ecb_preview",
          "confidence": "preview"
        }
      ]
    },
    {
      "frame": 6,
      "interpolates_from_previous": true,
      "pose": [],
      "hitboxes": [
        {
          "id": 0,
          "kind": "sphere",
          "bone": "HandR",
          "center": {"x": 3.5, "y": 8.0, "z": -1.25},
          "radius": 3.4,
          "damage": "unknown",
          "angle": "unknown",
          "kbg": "unknown",
          "bkb": "unknown",
          "source": "fixture_for_viewer_pipeline",
          "confidence": "needs_decomp_table_extraction"
        }
      ],
      "hurtboxes": [
        {
          "id": "torso",
          "kind": "capsule",
          "bone": "TransN",
          "a": {"x": 0.1, "y": 6.7, "z": 0.0},
          "b": {"x": 0.5, "y": 13.8, "z": 0.0},
          "radius": 3.9,
          "source": "generated_ecb_preview",
          "confidence": "preview"
        }
      ]
    }
  ],
  "gaps": [
    {
      "field": "hitboxes.damage_angle_knockback",
      "reason": "first slice preserves schema and viewer path before authoritative hitbox table extraction"
    },
    {
      "field": "iasa_frame",
      "reason": "not yet proven from decomp or subaction command variables"
    }
  ],
  "overrides": []
}
```

- [ ] **Step 4: Add loader constants and pure helpers**

In `tools/state_graph_viewer.py`, add constants near the other tab/default path constants:

```python
MOVE_KEYFRAMES_TAB_LABEL = "Move Keyframes"
DEFAULT_MOVE_FRAME_DATA_DIR = PROJECT_ROOT / "resources" / "melee" / "frame_data"
MOVE_FRAME_DATA_CHARACTERS = [
    {"id": "dolphin_mole", "label": "Dolphin Mole"},
    {"id": "test_character_2", "label": "Test Character 2"},
]
```

Add these helper functions after `load_ecb_coverage`:

```python
def list_move_frame_data_characters(
    frame_data_dir: Path = DEFAULT_MOVE_FRAME_DATA_DIR,
) -> list[dict[str, Any]]:
    characters = []
    for character in MOVE_FRAME_DATA_CHARACTERS:
        character_dir = frame_data_dir / character["id"]
        characters.append(
            {
                "id": character["id"],
                "label": character["label"],
                "populated": character_dir.is_dir()
                and any(character_dir.glob("*.json")),
            }
        )
    return characters


def list_move_frame_data_states(
    frame_data_dir: Path,
    character_id: str,
) -> list[dict[str, Any]]:
    character_dir = frame_data_dir / character_id
    if not character_dir.is_dir():
        return []
    states: list[dict[str, Any]] = []
    for path in sorted(character_dir.glob("*.json")):
        try:
            data = _read_json_file(path)
        except (OSError, json.JSONDecodeError):
            continue
        states.append(
            {
                "state": str(data.get("state") or path.stem),
                "label": str(data.get("label") or path.stem),
                "populated": True,
            }
        )
    return states


def load_move_frame_data(
    frame_data_dir: Path,
    character_id: str,
    state: str,
) -> dict[str, Any]:
    path = frame_data_dir / character_id / f"{state}.json"
    return _read_json_file(path)


def build_move_keyframe_empty_state(character_label: str) -> str:
    return (
        f"No move frame data populated for {character_label}.\n\n"
        "Use `mole frame-data extract --character dolphin_mole --source-character captain --state AttackAirN` "
        "to inspect extracted source data, or switch back to Dolphin Mole."
    )


def project_move_point(point: dict[str, Any], projection: dict[str, Any]) -> dict[str, float]:
    x = float(point.get("x", 0.0))
    y = float(point.get("y", 0.0))
    z = float(point.get("z", 0.0))
    default_view = projection.get("default_view", "xy")
    if default_view == "xz":
        view_x, view_y = x, z
    elif default_view == "yz":
        view_x, view_y = y, z
    else:
        view_x, view_y = x, y
    return {"x": x, "y": y, "z": z, "view_x": view_x, "view_y": view_y}


def format_move_keyframe_details(data: dict[str, Any], frame: dict[str, Any]) -> str:
    lines = [
        f"{data.get('target_character_label', data.get('target_character', 'Unknown'))} - {data.get('label', data.get('state', 'Unknown'))}",
        f"Frame {frame.get('frame', '?')}",
        f"Projection: {data.get('projection', {}).get('default_view', 'xy')} ({data.get('projection', {}).get('z_policy', 'preserve_and_project')})",
        "",
        "Hitboxes:",
    ]
    hitboxes = frame.get("hitboxes", [])
    if hitboxes:
        for hitbox in hitboxes:
            center = hitbox.get("center", {})
            lines.append(
                "hitbox[{id}] bone={bone} center=(x={x}, y={y}, z={z}) radius={radius} damage={damage} angle={angle}".format(
                    id=hitbox.get("id", "?"),
                    bone=hitbox.get("bone", "unknown"),
                    x=center.get("x", "unknown"),
                    y=center.get("y", "unknown"),
                    z=center.get("z", "unknown"),
                    radius=hitbox.get("radius", "unknown"),
                    damage=hitbox.get("damage", "unknown"),
                    angle=hitbox.get("angle", "unknown"),
                )
            )
    else:
        lines.append("none")
    lines.extend(["", "Hurtboxes:"])
    hurtboxes = frame.get("hurtboxes", [])
    if hurtboxes:
        for hurtbox in hurtboxes:
            a = hurtbox.get("a", {})
            b = hurtbox.get("b", {})
            lines.append(
                "hurtbox[{id}] bone={bone} a=(x={ax}, y={ay}, z={az}) b=(x={bx}, y={by}, z={bz}) radius={radius}".format(
                    id=hurtbox.get("id", "?"),
                    bone=hurtbox.get("bone", "unknown"),
                    ax=a.get("x", "unknown"),
                    ay=a.get("y", "unknown"),
                    az=a.get("z", "unknown"),
                    bx=b.get("x", "unknown"),
                    by=b.get("y", "unknown"),
                    bz=b.get("z", "unknown"),
                    radius=hurtbox.get("radius", "unknown"),
                )
            )
    else:
        lines.append("none")
    lines.extend(["", "Sources:"])
    for source in data.get("sources", []):
        line = source.get("line")
        suffix = f":{line}" if line else ""
        lines.append(f"- {source.get('kind', 'source')}: {source.get('path', 'unknown')}{suffix}")
    return "\n".join(lines)
```

Add this shared JSON helper near other private helpers:

```python
def _read_json_file(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as handle:
        data = json.load(handle)
    if not isinstance(data, dict):
        raise ValueError(f"{path} did not contain a JSON object")
    return data
```

- [ ] **Step 5: Run tests to verify they pass**

Run:

```powershell
pytest tests/test_state_graph_viewer.py -k "move_frame_data or move_keyframes" -q
```

Expected: PASS.

- [ ] **Step 6: Commit this task**

```powershell
git add resources/melee/frame_data/dolphin_mole/AttackAirN.json tools/state_graph_viewer.py tests/test_state_graph_viewer.py
git commit -m "feat: add move frame data artifact loader"
```

---

### Task 2: Add Read-Only Frame-Data CLI Commands

**Files:**
- Create: `crates/mole_cli/src/frame_data.rs`
- Modify: `crates/mole_cli/src/lib.rs`
- Modify: `crates/mole_cli/tests/cli_contract.rs`

- [ ] **Step 1: Write failing CLI contract tests**

Add these tests to `crates/mole_cli/tests/cli_contract.rs` near the decomp tests:

```rust
#[test]
fn frame_data_extract_returns_dolphin_mole_scoped_attack_air_n_artifact() {
    let root = temp_project_root("frame_data_extract");
    write_json(
        &root.join("resources/melee/frame_data/dolphin_mole/AttackAirN.json"),
        &json!({
            "schema_version": 1,
            "target_character": "dolphin_mole",
            "target_character_label": "Dolphin Mole",
            "source_character": "captain",
            "state": "AttackAirN",
            "label": "Neutral Air",
            "projection": {
                "source_space": "melee_xyz",
                "default_view": "xy",
                "z_policy": "preserve_and_project"
            },
            "sources": [{"kind": "decomp", "path": "src/melee/ft/chara/ftCommon/ftCo_AttackAir.c", "line": 1}],
            "summary": {"total_frames": 35, "iasa_frame": "unknown", "active_hitbox_windows": []},
            "keyframes": [{
                "frame": 6,
                "interpolates_from_previous": true,
                "hitboxes": [{"id": 0, "center": {"x": 3.5, "y": 8.0, "z": -1.25}}],
                "hurtboxes": []
            }],
            "gaps": [{"field": "iasa_frame", "reason": "not yet proven"}],
            "overrides": []
        }),
    );

    let output = run_cli(&[
        "--root".to_string(),
        root.display().to_string(),
        "frame-data".to_string(),
        "extract".to_string(),
        "--character".to_string(),
        "dolphin_mole".to_string(),
        "--source-character".to_string(),
        "captain".to_string(),
        "--state".to_string(),
        "AttackAirN".to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(parsed["command"], "frame-data extract");
    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["artifact"]["target_character"], "dolphin_mole");
    assert_eq!(parsed["artifact"]["target_character_label"], "Dolphin Mole");
    assert_eq!(parsed["artifact"]["source_character"], "captain");
    assert_eq!(parsed["artifact"]["projection"]["z_policy"], "preserve_and_project");
    assert_eq!(parsed["artifact"]["keyframes"][0]["hitboxes"][0]["center"]["z"], -1.25);
    assert_eq!(parsed["artifact"]["gaps"][0]["field"], "iasa_frame");
}


#[test]
fn frame_data_show_reads_existing_artifact_without_source_character_flag() {
    let root = temp_project_root("frame_data_show");
    write_json(
        &root.join("resources/melee/frame_data/dolphin_mole/AttackAirN.json"),
        &json!({
            "schema_version": 1,
            "target_character": "dolphin_mole",
            "target_character_label": "Dolphin Mole",
            "source_character": "captain",
            "state": "AttackAirN",
            "label": "Neutral Air",
            "projection": {"source_space": "melee_xyz", "default_view": "xy", "z_policy": "preserve_and_project"},
            "sources": [{"kind": "decomp", "path": "src/melee/ft/chara/ftCommon/ftCo_AttackAir.c", "line": 1}],
            "summary": {"total_frames": 35, "iasa_frame": "unknown", "active_hitbox_windows": []},
            "keyframes": [],
            "gaps": [],
            "overrides": []
        }),
    );

    let output = run_cli(&[
        "--root".to_string(),
        root.display().to_string(),
        "frame-data".to_string(),
        "show".to_string(),
        "--character".to_string(),
        "dolphin_mole".to_string(),
        "--state".to_string(),
        "AttackAirN".to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(parsed["command"], "frame-data show");
    assert_eq!(parsed["ok"], true);
    assert_eq!(parsed["artifact"]["label"], "Neutral Air");
}


#[test]
fn frame_data_extract_reports_missing_character_state_without_guessing() {
    let root = temp_project_root("frame_data_missing");
    let output = run_cli(&[
        "--root".to_string(),
        root.display().to_string(),
        "frame-data".to_string(),
        "extract".to_string(),
        "--character".to_string(),
        "test_character_2".to_string(),
        "--source-character".to_string(),
        "captain".to_string(),
        "--state".to_string(),
        "AttackAirN".to_string(),
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

    assert_eq!(parsed["command"], "frame-data extract");
    assert_eq!(parsed["ok"], false);
    assert!(parsed["errors"][0]
        .as_str()
        .unwrap()
        .contains("move frame data artifact not found"));
    assert!(parsed["artifact"].is_null());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```powershell
cargo test -p mole_cli frame_data_ -- --nocapture
```

Expected: FAIL with `unknown mole command: frame-data`.

- [ ] **Step 3: Add CLI command types and parser**

In `crates/mole_cli/src/lib.rs`, add `mod frame_data;` with the other modules.

Add a `FrameData` command variant:

```rust
FrameData(FrameDataCommand),
```

Add these structs/enums near the other command option structs:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FrameDataCommand {
    Extract(FrameDataOptions),
    Show(FrameDataOptions),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FrameDataOptions {
    pub character: String,
    pub source_character: Option<String>,
    pub state: String,
}
```

Route `"frame-data"` in `parse_command`:

```rust
"frame-data" => parse_frame_data_command(&positional[1..]).map(CliCommand::FrameData),
```

Add parser functions:

```rust
fn parse_frame_data_command(args: &[String]) -> Result<FrameDataCommand, String> {
    let subcommand = args.first().map(String::as_str).unwrap_or("show");
    let rest = subcommand_args(args);
    match subcommand {
        "extract" => parse_frame_data_options(rest, true).map(FrameDataCommand::Extract),
        "show" => parse_frame_data_options(rest, false).map(FrameDataCommand::Show),
        other => Err(format!("unknown mole frame-data command: {other}")),
    }
}

fn parse_frame_data_options(
    args: &[String],
    allow_source_character: bool,
) -> Result<FrameDataOptions, String> {
    let mut character = None;
    let mut source_character = None;
    let mut state = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--character" => character = Some(take_flag_value(args, &mut index, "--character")?),
            "--source-character" if allow_source_character => {
                source_character = Some(take_flag_value(args, &mut index, "--source-character")?)
            }
            "--state" => state = Some(take_flag_value(args, &mut index, "--state")?),
            other => return Err(format!("unexpected argument for frame-data: {other}")),
        }
        index += 1;
    }
    Ok(FrameDataOptions {
        character: character.ok_or_else(|| "frame-data requires --character <id>".to_string())?,
        source_character,
        state: state.ok_or_else(|| "frame-data requires --state <MotionState>".to_string())?,
    })
}
```

Route reports in `command_report`:

```rust
CliCommand::FrameData(command) => frame_data::frame_data_report(&options.root, command),
```

- [ ] **Step 4: Add the frame-data report module**

Create `crates/mole_cli/src/frame_data.rs`:

```rust
use serde_json::{json, Value};
use std::{fs, path::Path};

use crate::{base_report, FrameDataCommand, FrameDataOptions};

pub(crate) fn frame_data_report(root: &Path, command: &FrameDataCommand) -> Value {
    match command {
        FrameDataCommand::Extract(options) => artifact_report(root, "frame-data extract", options),
        FrameDataCommand::Show(options) => artifact_report(root, "frame-data show", options),
    }
}

fn artifact_report(root: &Path, command_name: &str, options: &FrameDataOptions) -> Value {
    let mut report = base_report(command_name, root);
    report["character"] = json!(options.character);
    report["source_character"] = json!(options.source_character);
    report["state"] = json!(options.state);

    let path = root
        .join("resources")
        .join("melee")
        .join("frame_data")
        .join(&options.character)
        .join(format!("{}.json", options.state));
    report["artifact_path"] = json!(path.display().to_string());

    match fs::read_to_string(&path) {
        Ok(text) => match serde_json::from_str::<Value>(&text) {
            Ok(artifact) if artifact.is_object() => {
                report["ok"] = json!(true);
                report["artifact"] = artifact;
                report["errors"] = json!([]);
            }
            Ok(_) => {
                report["ok"] = json!(false);
                report["artifact"] = Value::Null;
                report["errors"] = json!([format!(
                    "move frame data artifact is not a JSON object: {}",
                    path.display()
                )]);
            }
            Err(error) => {
                report["ok"] = json!(false);
                report["artifact"] = Value::Null;
                report["errors"] = json!([format!(
                    "failed to parse move frame data artifact {}: {error}",
                    path.display()
                )]);
            }
        },
        Err(_) => {
            report["ok"] = json!(false);
            report["artifact"] = Value::Null;
            report["errors"] = json!([format!(
                "move frame data artifact not found: {}",
                path.display()
            )]);
        }
    }

    report
}
```

- [ ] **Step 5: Run CLI tests**

Run:

```powershell
cargo test -p mole_cli frame_data_ -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Commit this task**

```powershell
git add crates/mole_cli/src/lib.rs crates/mole_cli/src/frame_data.rs crates/mole_cli/tests/cli_contract.rs
git commit -m "feat: add frame data CLI reports"
```

---

### Task 3: Add CLI Help, Markdown Formatting, And README Examples

**Files:**
- Modify: `crates/mole_cli/src/lib.rs`
- Modify: `crates/mole_cli/src/formatting.rs`
- Modify: `crates/mole_cli/README.md`
- Modify: `crates/mole_cli/tests/cli_contract.rs`

- [ ] **Step 1: Extend failing help and markdown tests**

In `help_command_exposes_full_agent_command_catalog`, add:

```rust
"frame-data extract",
"frame-data show",
```

Add assertions:

```rust
let frame_data_extract = commands
    .iter()
    .find(|command| command["name"] == "frame-data extract")
    .unwrap();
assert_eq!(frame_data_extract["mutates_workspace"], false);
assert!(frame_data_extract["purpose"]
    .as_str()
    .unwrap()
    .contains("move frame data"));
assert!(parsed["examples"]
    .as_array()
    .unwrap()
    .iter()
    .any(|example| example.as_str().unwrap().contains("frame-data extract")));
```

Add a markdown test:

```rust
#[test]
fn frame_data_show_markdown_summarizes_sources_keyframes_and_gaps() {
    let root = temp_project_root("frame_data_markdown");
    write_json(
        &root.join("resources/melee/frame_data/dolphin_mole/AttackAirN.json"),
        &json!({
            "schema_version": 1,
            "target_character": "dolphin_mole",
            "target_character_label": "Dolphin Mole",
            "source_character": "captain",
            "source_character_label": "Captain Falcon",
            "state": "AttackAirN",
            "label": "Neutral Air",
            "projection": {"source_space": "melee_xyz", "default_view": "xy", "z_policy": "preserve_and_project"},
            "sources": [{"kind": "decomp", "path": "src/melee/ft/chara/ftCommon/ftCo_AttackAir.c", "line": 1}],
            "summary": {"total_frames": 35, "iasa_frame": "unknown", "active_hitbox_windows": [{"start": 6, "end": 9}]},
            "keyframes": [{"frame": 6, "hitboxes": [{"id": 0, "center": {"x": 3.5, "y": 8.0, "z": -1.25}}], "hurtboxes": []}],
            "gaps": [{"field": "iasa_frame", "reason": "not yet proven"}],
            "overrides": []
        }),
    );

    let output = run_cli(&[
        "--root".to_string(),
        root.display().to_string(),
        "frame-data".to_string(),
        "show".to_string(),
        "--character".to_string(),
        "dolphin_mole".to_string(),
        "--state".to_string(),
        "AttackAirN".to_string(),
        "--format".to_string(),
        "markdown".to_string(),
    ])
    .unwrap();

    assert!(output.starts_with("# Mole Frame Data"));
    assert!(output.contains("Dolphin Mole"));
    assert!(output.contains("Captain Falcon"));
    assert!(output.contains("Neutral Air"));
    assert!(output.contains("preserve_and_project"));
    assert!(output.contains("Frame 6"));
    assert!(output.contains("src/melee/ft/chara/ftCommon/ftCo_AttackAir.c:1"));
    assert!(output.contains("iasa_frame"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```powershell
cargo test -p mole_cli frame_data_show_markdown_summarizes_sources_keyframes_and_gaps help_command_exposes_full_agent_command_catalog -- --nocapture
```

Expected: FAIL because markdown/help entries are missing.

- [ ] **Step 3: Add markdown formatter**

In `crates/mole_cli/src/formatting.rs`, route commands in `format_markdown_report`:

```rust
Some("frame-data extract") | Some("frame-data show") => format_frame_data_markdown(report),
```

Add:

```rust
fn format_frame_data_markdown(report: &Value) -> String {
    let mut lines = vec!["# Mole Frame Data".to_string(), String::new()];
    let ok = report.get("ok").and_then(Value::as_bool).unwrap_or(false);
    lines.push(format!("- OK: `{ok}`"));
    if let Some(path) = report.get("artifact_path").and_then(Value::as_str) {
        lines.push(format!("- Artifact: `{path}`"));
    }
    if let Some(artifact) = report.get("artifact").filter(|value| !value.is_null()) {
        lines.push(format!(
            "- Character: `{}`",
            artifact
                .get("target_character_label")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
        ));
        lines.push(format!(
            "- Source character: `{}`",
            artifact
                .get("source_character_label")
                .and_then(Value::as_str)
                .unwrap_or_else(|| artifact.get("source_character").and_then(Value::as_str).unwrap_or("unknown"))
        ));
        lines.push(format!(
            "- State: `{}` ({})",
            artifact.get("state").and_then(Value::as_str).unwrap_or("unknown"),
            artifact.get("label").and_then(Value::as_str).unwrap_or("unknown")
        ));
        if let Some(projection) = artifact.get("projection") {
            lines.push(format!(
                "- Projection: `{}` / `{}`",
                projection.get("default_view").and_then(Value::as_str).unwrap_or("xy"),
                projection
                    .get("z_policy")
                    .and_then(Value::as_str)
                    .unwrap_or("preserve_and_project")
            ));
        }
        if let Some(summary) = artifact.get("summary") {
            lines.push(String::new());
            lines.push("## Summary".to_string());
            if let Some(total) = summary.get("total_frames") {
                lines.push(format!("- Total frames: `{}`", display_json_scalar(total)));
            }
            if let Some(iasa) = summary.get("iasa_frame") {
                lines.push(format!("- IASA: `{}`", display_json_scalar(iasa)));
            }
            if let Some(windows) = summary.get("active_hitbox_windows").and_then(Value::as_array) {
                for window in windows {
                    lines.push(format!(
                        "- Active hitboxes: `{}`-`{}`",
                        window.get("start").and_then(Value::as_i64).unwrap_or(0),
                        window.get("end").and_then(Value::as_i64).unwrap_or(0)
                    ));
                }
            }
        }
        lines.push(String::new());
        lines.push("## Keyframes".to_string());
        if let Some(keyframes) = artifact.get("keyframes").and_then(Value::as_array) {
            for frame in keyframes.iter().take(12) {
                lines.push(format!(
                    "- Frame {}: {} hitbox(es), {} hurtbox(es)",
                    frame.get("frame").and_then(Value::as_i64).unwrap_or(0),
                    frame.get("hitboxes").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
                    frame.get("hurtboxes").and_then(Value::as_array).map(Vec::len).unwrap_or(0)
                ));
            }
        }
        lines.push(String::new());
        lines.push("## Sources".to_string());
        if let Some(sources) = artifact.get("sources").and_then(Value::as_array) {
            for source in sources {
                let path = source.get("path").and_then(Value::as_str).unwrap_or("unknown");
                let suffix = source
                    .get("line")
                    .and_then(Value::as_i64)
                    .map(|line| format!(":{line}"))
                    .unwrap_or_default();
                lines.push(format!(
                    "- `{}` `{}`{} - {}",
                    source.get("kind").and_then(Value::as_str).unwrap_or("source"),
                    path,
                    suffix,
                    source.get("purpose").and_then(Value::as_str).unwrap_or("")
                ));
            }
        }
        if let Some(gaps) = artifact.get("gaps").and_then(Value::as_array) {
            if !gaps.is_empty() {
                lines.push(String::new());
                lines.push("## Gaps".to_string());
                for gap in gaps {
                    lines.push(format!(
                        "- `{}` - {}",
                        gap.get("field").and_then(Value::as_str).unwrap_or("unknown"),
                        gap.get("reason").and_then(Value::as_str).unwrap_or("")
                    ));
                }
            }
        }
    }
    if let Some(errors) = report.get("errors").and_then(Value::as_array) {
        if !errors.is_empty() {
            lines.push(String::new());
            lines.push("## Errors".to_string());
            for error in errors.iter().filter_map(Value::as_str) {
                lines.push(format!("- {error}"));
            }
        }
    }
    lines.push(String::new());
    lines.join("\n")
}

fn display_json_scalar(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| value.to_string())
}
```

If `display_json_scalar` conflicts with an existing helper name, name it `display_frame_data_scalar` and update both call sites.

- [ ] **Step 4: Add help catalog entries and README examples**

In `help_report` examples in `crates/mole_cli/src/lib.rs`, add:

```rust
"cargo run -p mole_cli -- frame-data extract --character dolphin_mole --source-character captain --state AttackAirN --json",
"cargo run -p mole_cli -- frame-data show --character dolphin_mole --state AttackAirN --format markdown",
```

In `command_help_catalog`, add entries:

```rust
{
    "name": "frame-data extract",
    "usage": "mole frame-data extract --character ID --source-character ID --state MotionState [--json|--format markdown]",
    "purpose": "Load source-derived move frame data for a target character, preserving raw 3D keyframes and extraction gaps.",
    "mutates_workspace": false,
    "writes": [],
    "output_modes": ["json", "markdown"],
    "required_flags": ["--character", "--source-character", "--state"],
    "optional_flags": ["--root", "--json", "--format"],
    "aliases": [],
    "agent_notes": "Use for replay-parity attack work such as Captain Falcon Nair assigned to Dolphin Mole."
},
{
    "name": "frame-data show",
    "usage": "mole frame-data show --character ID --state MotionState [--json|--format markdown]",
    "purpose": "Show an existing move frame data artifact with keyframes, projection metadata, source citations, and gaps.",
    "mutates_workspace": false,
    "writes": [],
    "output_modes": ["json", "markdown"],
    "required_flags": ["--character", "--state"],
    "optional_flags": ["--root", "--json", "--format"],
    "aliases": [],
    "agent_notes": "Use after extraction to cite the canonical artifact that also feeds the Move Keyframes dev-tool tab."
},
```

In `crates/mole_cli/README.md`, add both command examples to the command list and a short note:

```markdown
Use `frame-data extract` and `frame-data show` when an agent needs attack data such as Captain Falcon Nair. The first artifact is character-scoped to `dolphin_mole`, preserves source `{x, y, z}` coordinates, and records projection metadata for the current 2D view.
```

- [ ] **Step 5: Run tests**

Run:

```powershell
cargo test -p mole_cli frame_data_show_markdown_summarizes_sources_keyframes_and_gaps help_command_exposes_full_agent_command_catalog -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Commit this task**

```powershell
git add crates/mole_cli/src/lib.rs crates/mole_cli/src/formatting.rs crates/mole_cli/README.md crates/mole_cli/tests/cli_contract.rs
git commit -m "docs: expose frame data CLI help"
```

---

### Task 4: Add Move Keyframes Dev-Tool Tab

**Files:**
- Modify: `tools/state_graph_viewer.py`
- Modify: `tests/test_state_graph_viewer.py`

- [ ] **Step 1: Write failing tab-label and format tests**

Extend `test_dev_tool_title_and_tab_labels_make_ledger_visible`:

```python
assert MOVE_KEYFRAMES_TAB_LABEL == "Move Keyframes"
```

Add a test for selected-frame summary:

```python
def test_move_keyframe_summary_lists_timeline_frames_and_gaps():
    data = load_move_frame_data(DEFAULT_MOVE_FRAME_DATA_DIR, "dolphin_mole", "AttackAirN")
    text = format_move_frame_data_summary(data)

    assert "Dolphin Mole" in text
    assert "Neutral Air" in text
    assert "Total frames: 35" in text
    assert "Frame 1" in text
    assert "Frame 6" in text
    assert "hitboxes.damage_angle_knockback" in text
    assert "preserve_and_project" in text
```

Add `format_move_frame_data_summary` to the imports.

- [ ] **Step 2: Run tests to verify they fail**

Run:

```powershell
pytest tests/test_state_graph_viewer.py -k "move_keyframe_summary or dev_tool_title" -q
```

Expected: FAIL because `format_move_frame_data_summary` and the tab launch integration are missing.

- [ ] **Step 3: Add summary formatter**

In `tools/state_graph_viewer.py`, add:

```python
def format_move_frame_data_summary(data: dict[str, Any]) -> str:
    projection = data.get("projection", {})
    summary = data.get("summary", {})
    lines = [
        "Move Keyframes",
        "",
        f"Character: {data.get('target_character_label', data.get('target_character', 'Unknown'))}",
        f"Source Character: {data.get('source_character_label', data.get('source_character', 'Unknown'))}",
        f"State: {data.get('state', 'Unknown')} ({data.get('label', 'Unknown')})",
        f"Projection: {projection.get('default_view', 'xy')} / {projection.get('z_policy', 'preserve_and_project')}",
        f"Total frames: {summary.get('total_frames', 'unknown')}",
        "",
        "Keyframes:",
    ]
    for frame in data.get("keyframes", []):
        lines.append(
            "Frame {frame}: {hitboxes} hitbox(es), {hurtboxes} hurtbox(es)".format(
                frame=frame.get("frame", "?"),
                hitboxes=len(frame.get("hitboxes", [])),
                hurtboxes=len(frame.get("hurtboxes", [])),
            )
        )
    gaps = data.get("gaps", [])
    if gaps:
        lines.extend(["", "Gaps:"])
        for gap in gaps:
            lines.append(f"- {gap.get('field', 'unknown')}: {gap.get('reason', '')}")
    return "\n".join(lines)
```

- [ ] **Step 4: Add the tab to `launch_viewer`**

In `launch_viewer`, create and add the tab after Slippi Replay:

```python
move_keyframes_tab = tk.Frame(notebook, bg="#ffffff")
notebook.add(move_keyframes_tab, text=MOVE_KEYFRAMES_TAB_LABEL)
```

Call:

```python
draw_move_keyframes_tab(move_keyframes_tab)
```

- [ ] **Step 5: Implement `draw_move_keyframes_tab`**

Add:

```python
def draw_move_keyframes_tab(
    parent: Any,
    frame_data_dir: Path = DEFAULT_MOVE_FRAME_DATA_DIR,
) -> None:
    import tkinter as tk
    from tkinter import ttk

    frame = tk.Frame(parent, bg="#ffffff", padx=12, pady=12)
    frame.pack(fill=tk.BOTH, expand=True)

    header_row = tk.Frame(frame, bg="#ffffff")
    header_row.pack(fill=tk.X, pady=(0, 8))
    tk.Label(
        header_row,
        text=MOVE_KEYFRAMES_TAB_LABEL,
        anchor="w",
        bg="#ffffff",
        fg="#0f172a",
        font=("Segoe UI", 13, "bold"),
    ).pack(side=tk.LEFT)

    characters = list_move_frame_data_characters(frame_data_dir)
    character_by_label = {character["label"]: character for character in characters}
    selected_character = tk.StringVar(value=characters[0]["label"])
    selected_state = tk.StringVar(value="")

    controls = tk.Frame(frame, bg="#ffffff")
    controls.pack(fill=tk.X, pady=(0, 8))
    tk.Label(controls, text="Character", bg="#ffffff").pack(side=tk.LEFT)
    character_combo = ttk.Combobox(
        controls,
        textvariable=selected_character,
        values=[character["label"] for character in characters],
        state="readonly",
        width=24,
    )
    character_combo.pack(side=tk.LEFT, padx=(6, 16))
    tk.Label(controls, text="State", bg="#ffffff").pack(side=tk.LEFT)
    state_combo = ttk.Combobox(controls, textvariable=selected_state, state="readonly", width=24)
    state_combo.pack(side=tk.LEFT, padx=(6, 16))

    body = tk.PanedWindow(frame, orient=tk.HORIZONTAL, sashrelief=tk.RAISED)
    body.pack(fill=tk.BOTH, expand=True)
    canvas = tk.Canvas(body, bg="#111827", width=720, height=460, highlightthickness=0)
    details = tk.Text(
        body,
        wrap=tk.WORD,
        bg="#f8fafc",
        fg="#0f172a",
        relief=tk.FLAT,
        font=("Consolas", 9),
        padx=10,
        pady=8,
    )
    body.add(canvas, stretch="always")
    body.add(details, stretch="always")

    state_records: list[dict[str, Any]] = []
    loaded_data: dict[str, Any] | None = None

    def refresh_states() -> None:
        nonlocal state_records
        character = character_by_label[selected_character.get()]
        state_records = list_move_frame_data_states(frame_data_dir, character["id"])
        state_combo.configure(values=[state["label"] for state in state_records])
        selected_state.set(state_records[0]["label"] if state_records else "")
        redraw()

    def current_state_record() -> dict[str, Any] | None:
        for record in state_records:
            if record["label"] == selected_state.get():
                return record
        return None

    def redraw() -> None:
        nonlocal loaded_data
        canvas.delete("all")
        character = character_by_label[selected_character.get()]
        record = current_state_record()
        if record is None:
            loaded_data = None
            _set_text(details, build_move_keyframe_empty_state(character["label"]))
            canvas.create_text(
                360,
                210,
                text=build_move_keyframe_empty_state(character["label"]),
                fill="#cbd5e1",
                font=("Segoe UI", 12),
                justify=tk.CENTER,
            )
            return
        loaded_data = load_move_frame_data(frame_data_dir, character["id"], record["state"])
        _set_text(details, format_move_frame_data_summary(loaded_data))
        draw_move_keyframe_canvas(canvas, loaded_data, selected_frame_index=0)

    def on_state_changed(_event: Any | None = None) -> None:
        redraw()

    character_combo.bind("<<ComboboxSelected>>", lambda _event: refresh_states())
    state_combo.bind("<<ComboboxSelected>>", on_state_changed)
    refresh_states()
```

- [ ] **Step 6: Implement projected overlay canvas drawing**

Add:

```python
def draw_move_keyframe_canvas(canvas: Any, data: dict[str, Any], selected_frame_index: int = 0) -> None:
    import tkinter as tk

    keyframes = data.get("keyframes", [])
    if not keyframes:
        canvas.create_text(360, 220, text="No keyframes", fill="#cbd5e1")
        return
    selected_frame_index = max(0, min(selected_frame_index, len(keyframes) - 1))
    frame = keyframes[selected_frame_index]
    previous = keyframes[selected_frame_index - 1] if selected_frame_index > 0 else None
    projection = data.get("projection", {})
    scale = float(projection.get("view_scale", 10.0))
    origin_x, origin_y = 360.0, 260.0

    canvas.create_text(
        16,
        16,
        anchor=tk.NW,
        text=f"{data.get('target_character_label', 'Unknown')} - {data.get('label', 'Unknown')} - Frame {frame.get('frame', '?')}",
        fill="#e2e8f0",
        font=("Segoe UI", 12, "bold"),
    )

    if previous is not None:
        _draw_move_frame_volumes(canvas, previous, projection, scale, origin_x, origin_y, ghost=True)
    _draw_move_frame_volumes(canvas, frame, projection, scale, origin_x, origin_y, ghost=False)

    total_frames = int(data.get("summary", {}).get("total_frames", max(1, len(keyframes))))
    cell_width = max(8, min(18, 680 // max(1, total_frames)))
    timeline_x = 20
    timeline_y = 420
    keyed_frames = {int(item.get("frame", 0)) for item in keyframes}
    for frame_number in range(1, total_frames + 1):
        x0 = timeline_x + (frame_number - 1) * cell_width
        x1 = x0 + cell_width - 2
        fill = "#0f172a"
        outline = "#334155"
        if frame_number in keyed_frames:
            fill = "#1d4ed8"
            outline = "#60a5fa"
        if frame_number == int(frame.get("frame", 0)):
            fill = "#047857"
            outline = "#34d399"
        canvas.create_rectangle(x0, timeline_y, x1, timeline_y + 18, fill=fill, outline=outline)


def _draw_move_frame_volumes(
    canvas: Any,
    frame: dict[str, Any],
    projection: dict[str, Any],
    scale: float,
    origin_x: float,
    origin_y: float,
    ghost: bool,
) -> None:
    hit_outline = "#6ee7b7" if ghost else "#10b981"
    hurt_outline = "#93c5fd" if ghost else "#3b82f6"
    dash = (4, 3) if ghost else None
    for hurtbox in frame.get("hurtboxes", []):
        a = project_move_point(hurtbox.get("a", {}), projection)
        b = project_move_point(hurtbox.get("b", {}), projection)
        radius = float(hurtbox.get("radius", 1.0)) * scale
        ax = origin_x + a["view_x"] * scale
        ay = origin_y - a["view_y"] * scale
        bx = origin_x + b["view_x"] * scale
        by = origin_y - b["view_y"] * scale
        canvas.create_line(ax, ay, bx, by, fill=hurt_outline, width=2, dash=dash)
        for cx, cy in [(ax, ay), (bx, by)]:
            canvas.create_oval(cx - radius, cy - radius, cx + radius, cy + radius, outline=hurt_outline, width=2, dash=dash)
    for hitbox in frame.get("hitboxes", []):
        center = project_move_point(hitbox.get("center", {}), projection)
        radius = float(hitbox.get("radius", 1.0)) * scale
        cx = origin_x + center["view_x"] * scale
        cy = origin_y - center["view_y"] * scale
        canvas.create_oval(cx - radius, cy - radius, cx + radius, cy + radius, outline=hit_outline, width=2, dash=dash)
```

- [ ] **Step 7: Run dev-tool tests**

Run:

```powershell
pytest tests/test_state_graph_viewer.py -k "move_frame_data or move_keyframes or move_keyframe_summary or dev_tool_title" -q
```

Expected: PASS.

- [ ] **Step 8: Commit this task**

```powershell
git add tools/state_graph_viewer.py tests/test_state_graph_viewer.py
git commit -m "feat: add move keyframes dev tool tab"
```

---

### Task 5: Final Verification

**Files:**
- All files touched in Tasks 1-4.

- [ ] **Step 1: Format Rust code**

Run:

```powershell
cargo fmt -p mole_cli --check
```

Expected: PASS. If it fails, run `cargo fmt -p mole_cli`, inspect the diff, and rerun the check.

- [ ] **Step 2: Run full CLI tests**

Run:

```powershell
cargo test -p mole_cli
```

Expected: PASS.

- [ ] **Step 3: Run state graph viewer tests**

Run:

```powershell
pytest tests/test_state_graph_viewer.py -q
```

Expected: PASS.

- [ ] **Step 4: Smoke-test live CLI commands**

Run:

```powershell
cargo run -p mole_cli -- frame-data extract --character dolphin_mole --source-character captain --state AttackAirN --json
cargo run -p mole_cli -- frame-data show --character dolphin_mole --state AttackAirN --format markdown
```

Expected:

- JSON output has `"ok": true`.
- JSON output has `"target_character": "dolphin_mole"`.
- JSON output preserves a hitbox `center.z`.
- Markdown output starts with `# Mole Frame Data`.
- Markdown output lists `Dolphin Mole`, `Captain Falcon`, `Neutral Air`, sources, keyframes, and gaps.

- [ ] **Step 5: Smoke-test dev tool imports without launching Tk**

Run:

```powershell
python - <<'PY'
from tools.state_graph_viewer import DEFAULT_MOVE_FRAME_DATA_DIR, load_move_frame_data, format_move_frame_data_summary
data = load_move_frame_data(DEFAULT_MOVE_FRAME_DATA_DIR, "dolphin_mole", "AttackAirN")
print(format_move_frame_data_summary(data).splitlines()[0])
print(data["keyframes"][1]["hitboxes"][0]["center"]["z"])
PY
```

Expected output includes:

```text
Move Keyframes
-1.25
```

- [ ] **Step 6: Review changed files**

Run:

```powershell
git status --short
git diff -- crates/mole_cli/src/lib.rs crates/mole_cli/src/frame_data.rs crates/mole_cli/src/formatting.rs crates/mole_cli/tests/cli_contract.rs crates/mole_cli/README.md tools/state_graph_viewer.py tests/test_state_graph_viewer.py resources/melee/frame_data/dolphin_mole/AttackAirN.json
```

Expected: only the planned files changed, plus any already-dirty unrelated files from concurrent work that should not be reverted.

- [ ] **Step 7: Commit final verification notes if needed**

If Task 5 required follow-up edits, commit them:

```powershell
git add crates/mole_cli/src/lib.rs crates/mole_cli/src/frame_data.rs crates/mole_cli/src/formatting.rs crates/mole_cli/tests/cli_contract.rs crates/mole_cli/README.md tools/state_graph_viewer.py tests/test_state_graph_viewer.py resources/melee/frame_data/dolphin_mole/AttackAirN.json
git commit -m "test: verify move frame data tooling"
```

If no follow-up edits were needed, do not create an empty commit.

---

## Self-Review

- Spec coverage: The plan covers the canonical artifact, Dolphin Mole target scoping, visible empty second test character, CLI extract/show, dev-tool tab, 3D preservation with 2D projection metadata, source citations, gaps, and first-slice read-only behavior.
- Scope: The plan intentionally does not implement manual editing, engine import/export, hot reload, or authoritative hitbox table extraction beyond the first artifact path.
- Type consistency: The artifact uses `target_character`, `source_character`, `state`, `projection`, `summary`, `keyframes`, `sources`, `gaps`, and `overrides` consistently across CLI, Python loader, and tests.
