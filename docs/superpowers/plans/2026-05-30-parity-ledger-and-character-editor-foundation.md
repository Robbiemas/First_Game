# Parity Ledger And Character Editor Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the existing dual Melee/Mole graph viewer into a source-backed parity ledger with global and Captain Falcon value sheets, shaped so it can later become a character editor.

**Architecture:** Keep Rust authoritative and engine-agnostic. Python/Tkinter tooling reads plain JSON ledgers and generated value sheets, but gameplay values still live in Rust data seams and extracted Melee resources. The first checkpoint enriches grounded locomotion documentation and tooling only; it does not tune movement or introduce new gameplay mechanics.

**Tech Stack:** Rust workspace, Python 3 standard library, `pytest`, existing `tools/state_graph_viewer.py`, existing `resources/melee/extracted/*.json`, JSON graph/value-sheet files.

---

### Task 1: Generate Engine-Agnostic Value Sheets

**Files:**
- Create: `D:\Mole Game\First_Game\tools\generate_value_sheets.py`
- Create: `D:\Mole Game\First_Game\tests\test_value_sheets.py`
- Create directory/output: `D:\Mole Game\First_Game\docs\state_graphs\value_sheets\`
- Generate: `D:\Mole Game\First_Game\docs\state_graphs\value_sheets\global_common_values.json`
- Generate: `D:\Mole Game\First_Game\docs\state_graphs\value_sheets\captain_falcon_values.json`

- [ ] **Step 1: Write failing tests for generated value sheets**

Add `tests/test_value_sheets.py`:

```python
from pathlib import Path

from tools.generate_value_sheets import (
    build_character_sheet,
    build_global_sheet,
    generate_value_sheets,
)


ROOT = Path(__file__).resolve().parents[1]
COMMON = ROOT / "resources" / "melee" / "extracted" / "plco_common_data.json"
FALCON = ROOT / "resources" / "melee" / "extracted" / "captain_falcon_profile.json"


def test_global_value_sheet_groups_common_movement_fields():
    sheet = build_global_sheet(COMMON)

    assert sheet["id"] == "global_common_values"
    assert sheet["scope"] == "global"
    assert sheet["engine_boundary"] == "rust_core_authority"
    fields = {field["rust_name"]: field for category in sheet["categories"] for field in category["fields"]}

    assert fields["dash_x"]["source_name"] == "x3C"
    assert fields["dash_x"]["offset_hex"] == "0x3c"
    assert fields["dash_x"]["converted_value"] == 102
    assert fields["dash_tap_window"]["converted_value"] == 2
    assert fields["run_accel_taper"]["source_name"] == "x5C"
    assert fields["run_accel_taper"]["kind"] == "source_f32"
    assert fields["run_ground_friction_multiplier"]["source_name"] == "x60_someFrictionMul"
    assert fields["run_ground_friction_multiplier"]["kind"] == "source_f32"


def test_character_value_sheet_groups_falcon_locomotion_fields():
    sheet = build_character_sheet(FALCON, character_id="captain_falcon")

    assert sheet["id"] == "captain_falcon_values"
    assert sheet["scope"] == "character"
    assert sheet["character_id"] == "captain_falcon"
    fields = {field["rust_name"]: field for category in sheet["categories"] for field in category["fields"]}

    assert fields["dash_initial_velocity"]["converted_value"] == 2.0
    assert fields["dash_run_acceleration_a"]["converted_value"] == 0.15000000596046448
    assert fields["dash_run_terminal_velocity"]["converted_value"] == 2.299999952316284
    assert fields["ground_friction"]["converted_value"] == 0.07999999821186066
    assert fields["grav"]["converted_value"] == 0.12999999523162842


def test_generate_value_sheets_writes_stable_json_files(tmp_path):
    output_dir = tmp_path / "value_sheets"

    generated = generate_value_sheets(COMMON, FALCON, output_dir)

    assert generated == [
        output_dir / "global_common_values.json",
        output_dir / "captain_falcon_values.json",
    ]
    assert generated[0].read_text(encoding="utf-8").endswith("\n")
    assert generated[1].read_text(encoding="utf-8").endswith("\n")
```

- [ ] **Step 2: Run the tests and verify they fail**

Run:

```powershell
pytest tests/test_value_sheets.py -q
```

Expected: import failure for `tools.generate_value_sheets`.

- [ ] **Step 3: Implement the value-sheet generator**

Create `tools/generate_value_sheets.py`:

```python
from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_COMMON = ROOT / "resources" / "melee" / "extracted" / "plco_common_data.json"
DEFAULT_FALCON = ROOT / "resources" / "melee" / "extracted" / "captain_falcon_profile.json"
DEFAULT_OUTPUT = ROOT / "docs" / "state_graphs" / "value_sheets"

GLOBAL_CATEGORIES = {
    "input": [
        "main_stick_deadzone_x",
        "main_stick_deadzone_y",
        "tap_x_threshold",
        "tap_y_threshold",
        "trigger_deadzone",
        "trigger_timer_threshold",
        "z_shield_analog",
    ],
    "grounded_locomotion": [
        "walk_x",
        "walk_middle_velocity_ratio_milli",
        "walk_fast_velocity_ratio_milli",
        "walk_accel_taper_milli",
        "turn_x",
        "dash_x",
        "dash_tap_window",
        "dash_early_action_window",
        "dash_defensive_action_window",
        "dash_late_action_window",
        "dash_velocity_decay_milli",
        "run_x",
        "run_accel_taper_milli",
        "run_ground_friction_multiplier_milli",
        "high_speed_ground_friction_multiplier_milli",
        "run_turn_run_no_interrupt_frames",
    ],
    "jump_and_air": [
        "tap_jump_y",
        "tap_jump_window",
        "air_jump_backward_x",
        "tap_jump_release_y",
        "fast_fall_y",
        "fast_fall_window",
        "aerial_neutral_x",
        "aerial_neutral_y",
        "aerial_vertical_angle_tan_milli",
    ],
    "defense_and_platforms": [
        "escape_x",
        "escape_x_tap_window",
        "escape_y",
        "escape_y_tap_window",
        "fallspecial_platform_landing_y",
        "platform_pass_y",
        "platform_pass_y_tap_window",
        "pass_initial_y_velocity",
        "platform_drop_delay_ticks",
        "guard_on_catch_dash_window",
    ],
    "escape_air": [
        "escapeair_deadzone_x",
        "escapeair_deadzone_y",
        "escapeair_iasa_timer_ticks",
        "escapeair_force",
        "escapeair_decay",
        "escapeair_landing_lag_ticks",
    ],
}

CHARACTER_CATEGORIES = {
    "walk_and_run": [
        "walk_initial_velocity",
        "walk_accel",
        "walk_max_vel",
        "slow_walk_max_velocity",
        "mid_walk_threshold",
        "fast_walk_threshold",
        "ground_friction",
        "dash_run_terminal_velocity",
        "run_animation_scaling",
        "max_run_brake_frames",
        "ground_max_horizontal_velocity",
    ],
    "dash_and_turn": [
        "dash_initial_velocity",
        "dash_run_acceleration_a",
        "dash_run_acceleration_b",
        "frames_to_change_direction_on_standing_turn",
    ],
    "jump_and_air": [
        "jump_startup_time",
        "jump_h_initial_velocity",
        "jump_v_initial_velocity",
        "ground_to_air_jump_momentum_multiplier",
        "jump_h_max_velocity",
        "hop_v_initial_velocity",
        "air_jump_v_multiplier",
        "air_jump_h_multiplier",
        "max_jumps",
        "air_drift_stick_mul",
        "aerial_drift_base",
        "air_drift_max",
        "aerial_friction",
        "air_max_horizontal_velocity",
    ],
    "gravity_and_landing": [
        "grav",
        "terminal_vel",
        "fast_fall_velocity",
        "normal_landing_lag",
    ],
}


def build_global_sheet(path: Path = DEFAULT_COMMON) -> dict[str, Any]:
    payload = _read_json(path)
    return _build_sheet(
        sheet_id="global_common_values",
        title="Global Common Values",
        scope="global",
        source=payload["source"],
        fields=payload["fields"],
        categories=GLOBAL_CATEGORIES,
        extra={},
    )


def build_character_sheet(
    path: Path = DEFAULT_FALCON,
    *,
    character_id: str = "captain_falcon",
) -> dict[str, Any]:
    payload = _read_json(path)
    return _build_sheet(
        sheet_id=f"{character_id}_values",
        title="Captain Falcon Character Values",
        scope="character",
        source=payload["source"],
        fields=payload["fields"],
        categories=CHARACTER_CATEGORIES,
        extra={"character_id": character_id},
    )


def generate_value_sheets(
    common_path: Path = DEFAULT_COMMON,
    falcon_path: Path = DEFAULT_FALCON,
    output_dir: Path = DEFAULT_OUTPUT,
) -> list[Path]:
    output_dir.mkdir(parents=True, exist_ok=True)
    sheets = [
        (output_dir / "global_common_values.json", build_global_sheet(common_path)),
        (output_dir / "captain_falcon_values.json", build_character_sheet(falcon_path)),
    ]
    for path, sheet in sheets:
        path.write_text(json.dumps(sheet, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return [path for path, _sheet in sheets]


def _build_sheet(
    *,
    sheet_id: str,
    title: str,
    scope: str,
    source: dict[str, Any],
    fields: dict[str, Any],
    categories: dict[str, list[str]],
    extra: dict[str, Any],
) -> dict[str, Any]:
    return {
        "id": sheet_id,
        "title": title,
        "scope": scope,
        "engine_boundary": "rust_core_authority",
        "source": source,
        **extra,
        "categories": [
            {
                "id": category_id,
                "label": category_id.replace("_", " ").title(),
                "fields": [_field_row(rust_name, fields[rust_name]) for rust_name in rust_names],
            }
            for category_id, rust_names in categories.items()
        ],
    }


def _field_row(rust_name: str, field: dict[str, Any]) -> dict[str, Any]:
    return {
        "rust_name": rust_name,
        "source_name": field["source_name"],
        "offset": field["offset"],
        "offset_hex": f"0x{field['offset']:x}",
        "kind": field["kind"],
        "raw": field.get("raw"),
        "converted_value": _converted_value(field),
        "provenance": "extracted_melee_dat",
    }


def _converted_value(field: dict[str, Any]) -> int | float | None:
    for key in ("stick_byte", "trigger_byte", "ticks", "milli"):
        if key in field:
            return field[key]
    return field.get("raw")


def _read_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Generate parity value sheets.")
    parser.add_argument("--common", type=Path, default=DEFAULT_COMMON)
    parser.add_argument("--falcon", type=Path, default=DEFAULT_FALCON)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    for path in generate_value_sheets(args.common, args.falcon, args.output):
        print(path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **Step 4: Run the focused value-sheet tests**

Run:

```powershell
pytest tests/test_value_sheets.py -q
```

Expected: all tests pass.

- [ ] **Step 5: Generate the committed value sheets**

Run:

```powershell
.venv\Scripts\python.exe tools\generate_value_sheets.py
```

Expected: prints the two files under `docs/state_graphs/value_sheets`.

### Task 2: Validate Ledger Fields In Graph Data

**Files:**
- Modify: `D:\Mole Game\First_Game\tools\state_graph_viewer.py`
- Modify: `D:\Mole Game\First_Game\tests\test_state_graph_viewer.py`

- [ ] **Step 1: Add failing tests for optional ledger metadata**

Append to `tests/test_state_graph_viewer.py`:

```python
def test_graph_validation_accepts_ledger_reference_fields():
    graph = {
        "id": "test_graph",
        "title": "Test Graph",
        "root": "Wait",
        "nodes": [
            {
                "id": "Wait",
                "label": "Wait",
                "pos": [0, 0],
                "status": "aligned",
                "notes": "neutral",
                "source_refs": [{"label": "source", "path": "src/melee/ft/chara/ftCommon/ftCo_Wait.c"}],
                "rust_refs": [{"label": "rust", "path": "crates/mole_core/src/sim.rs"}],
                "value_refs": ["global_common_values.grounded_locomotion.walk_x"],
                "known_gaps": ["none"],
            }
        ],
        "edges": [
            {
                "from": "Wait",
                "to": "Wait",
                "input": "neutral",
                "frames": "current frame",
                "status": "aligned",
                "source_refs": [{"label": "source", "path": "src/melee/ft/chara/ftCommon/ftCo_Wait.c"}],
                "rust_refs": [{"label": "test", "path": "crates/mole_core/tests/core_contract.rs"}],
                "value_refs": ["captain_falcon_values.walk_and_run.ground_friction"],
                "physics": ["ground friction"],
                "known_gaps": [],
            }
        ],
    }

    assert validate_graph(graph) == []


def test_graph_validation_rejects_malformed_ledger_reference_fields():
    graph = {
        "id": "bad_graph",
        "title": "Bad Graph",
        "root": "Wait",
        "nodes": [
            {
                "id": "Wait",
                "label": "Wait",
                "pos": [0, 0],
                "status": "aligned",
                "source_refs": ["not a reference object"],
            }
        ],
        "edges": [
            {
                "from": "Wait",
                "to": "Wait",
                "input": "neutral",
                "frames": "current frame",
                "status": "aligned",
            }
        ],
    }

    assert "node 'Wait' source_refs[0] must be an object" in validate_graph(graph)
```

- [ ] **Step 2: Run the graph tests and verify failure**

Run:

```powershell
pytest tests/test_state_graph_viewer.py -q
```

Expected: malformed ledger validation assertion fails because the validator does not inspect ledger fields yet.

- [ ] **Step 3: Add ledger-field validation helpers**

In `tools/state_graph_viewer.py`, add near `ALLOWED_STATUSES`:

```python
LEDGER_LIST_FIELDS = {
    "source_refs",
    "rust_refs",
    "value_refs",
    "known_gaps",
    "physics",
}
REFERENCE_FIELDS = {"source_refs", "rust_refs"}
```

Add helper functions near `_is_position`:

```python
def validate_ledger_fields(item: dict[str, Any], *, item_label: str) -> list[str]:
    errors: list[str] = []
    for field in LEDGER_LIST_FIELDS:
        if field not in item:
            continue
        value = item[field]
        if not isinstance(value, list):
            errors.append(f"{item_label} {field} must be a list")
            continue
        for index, entry in enumerate(value):
            if field in REFERENCE_FIELDS:
                if not isinstance(entry, dict):
                    errors.append(f"{item_label} {field}[{index}] must be an object")
                    continue
                if not entry.get("label"):
                    errors.append(f"{item_label} {field}[{index}] missing label")
                if not (entry.get("path") or entry.get("url") or entry.get("function")):
                    errors.append(f"{item_label} {field}[{index}] missing path, url, or function")
            elif not isinstance(entry, str):
                errors.append(f"{item_label} {field}[{index}] must be a string")
    return errors
```

Call it inside `validate_graph` for each node and edge:

```python
errors.extend(validate_ledger_fields(node, item_label=f"node {node_id!r}"))
errors.extend(validate_ledger_fields(edge, item_label=f"edge {index}"))
```

- [ ] **Step 4: Run graph tests**

Run:

```powershell
pytest tests/test_state_graph_viewer.py -q
```

Expected: all graph tests pass.

### Task 3: Load And Summarize Value Sheets In The Viewer

**Files:**
- Modify: `D:\Mole Game\First_Game\tools\state_graph_viewer.py`
- Modify: `D:\Mole Game\First_Game\tests\test_state_graph_viewer.py`

- [ ] **Step 1: Add failing tests for value-sheet loading and check output**

Update the imports in `tests/test_state_graph_viewer.py` to include:

```python
    load_value_sheets,
    summarize_value_sheet,
```

Add tests:

```python
def test_value_sheets_load_for_viewer_summary():
    sheets = load_value_sheets(ROOT / "docs" / "state_graphs" / "value_sheets")

    assert [sheet["id"] for sheet in sheets] == [
        "global_common_values",
        "captain_falcon_values",
    ]
    assert summarize_value_sheet(sheets[0])["fields"] >= 10
    assert summarize_value_sheet(sheets[1])["fields"] >= 10
```

- [ ] **Step 2: Run the targeted test and verify failure**

Run:

```powershell
pytest tests/test_state_graph_viewer.py::test_value_sheets_load_for_viewer_summary -q
```

Expected: import failure for `load_value_sheets`.

- [ ] **Step 3: Implement value-sheet loading**

In `tools/state_graph_viewer.py`, add:

```python
DEFAULT_VALUE_SHEET_DIR = DEFAULT_GRAPH_DIR / "value_sheets"
VALUE_SHEET_FILES = ("global_common_values.json", "captain_falcon_values.json")
```

Add functions:

```python
def load_value_sheets(value_sheet_dir: Path = DEFAULT_VALUE_SHEET_DIR) -> list[dict[str, Any]]:
    sheets = []
    for filename in VALUE_SHEET_FILES:
        path = value_sheet_dir / filename
        with path.open("r", encoding="utf-8") as handle:
            sheets.append(json.load(handle))
    return sheets


def summarize_value_sheet(sheet: dict[str, Any]) -> dict[str, int]:
    categories = sheet.get("categories", [])
    field_count = sum(len(category.get("fields", [])) for category in categories)
    return {"categories": len(categories), "fields": field_count}
```

Update `parse_args`:

```python
parser.add_argument("--value-sheets", type=Path, default=DEFAULT_VALUE_SHEET_DIR)
```

Update `main` so `--check` also prints sheet summaries:

```python
if args.check:
    apply_saved_layout(graphs, args.layout)
    for graph in graphs:
        summary = summarize_comparison(graph)
        print(f"{graph['id']}: {len(graph['nodes'])} nodes, {len(graph['edges'])} edges")
        if any(summary.values()):
            print("  " + ", ".join(f"{key}={value}" for key, value in summary.items()))
    for sheet in load_value_sheets(args.value_sheets):
        summary = summarize_value_sheet(sheet)
        print(f"{sheet['id']}: {summary['categories']} categories, {summary['fields']} fields")
    return 0
```

- [ ] **Step 4: Run viewer tests and check command**

Run:

```powershell
pytest tests/test_state_graph_viewer.py -q
.venv\Scripts\python.exe tools\state_graph_viewer.py --check
```

Expected: tests pass; check output includes both graphs and both value sheets.

### Task 4: Enrich Grounded Locomotion Graph Ledger

**Files:**
- Modify: `D:\Mole Game\First_Game\docs\state_graphs\mole_current_graph.json`
- Modify: `D:\Mole Game\First_Game\docs\state_graphs\melee_reference_graph.json`
- Modify: `D:\Mole Game\First_Game\tests\test_state_graph_viewer.py`

- [ ] **Step 1: Add failing tests for grounded ledger coverage**

Add to `tests/test_state_graph_viewer.py`:

```python
def test_grounded_locomotion_nodes_have_source_rust_test_and_value_refs():
    _reference, mole = load_graphs(GRAPH_DIR)
    nodes = {node["id"]: node for node in mole["nodes"]}

    for node_id in ["Wait", "WalkSlow", "WalkMiddle", "WalkFast", "Turn", "Dash", "Run", "RunBrake", "TurnRun"]:
        node = nodes[node_id]
        assert node["source_refs"], node_id
        assert node["rust_refs"], node_id
        assert node["value_refs"], node_id


def test_dash_edges_have_physics_and_moonwalk_gap_tracking():
    _reference, mole = load_graphs(GRAPH_DIR)
    dash_edges = [
        edge for edge in mole["edges"]
        if edge["from"] == "Dash" or edge["to"] == "Dash"
    ]

    assert dash_edges
    assert all(edge.get("physics") for edge in dash_edges)
    dash_node = next(node for node in mole["nodes"] if node["id"] == "Dash")
    assert any("moonwalk" in gap.lower() for gap in dash_node.get("known_gaps", []))
```

- [ ] **Step 2: Run the targeted tests and verify failure**

Run:

```powershell
pytest tests/test_state_graph_viewer.py::test_grounded_locomotion_nodes_have_source_rust_test_and_value_refs tests/test_state_graph_viewer.py::test_dash_edges_have_physics_and_moonwalk_gap_tracking -q
```

Expected: tests fail because graph records do not yet contain the new ledger fields.

- [ ] **Step 3: Enrich the grounded locomotion nodes**

For each grounded node in `docs/state_graphs/mole_current_graph.json`, add ledger fields. Use this shape for `Dash`, then mirror the same structure for the other grounded states:

```json
"source_refs": [
  {
    "label": "Melee Dash source",
    "path": ".research/doldecomp-melee/src/melee/ft/chara/ftCommon/ftCo_Dash.c",
    "function": "ftCo_Dash_Enter / ftCo_Dash_IASA / ftCo_Dash_Phys"
  },
  {
    "label": "Melee dash/run accel helper",
    "path": ".research/doldecomp-melee/src/melee/ft/inlines.h",
    "function": "getAccelAndTarget"
  }
],
"rust_refs": [
  {
    "label": "Rust Dash simulation",
    "path": "crates/mole_core/src/sim.rs",
    "function": "enter_dash / apply_dash_velocity / exit_dash"
  },
  {
    "label": "Dash and moonwalk contract tests",
    "path": "crates/mole_core/tests/core_contract.rs"
  }
],
"value_refs": [
  "global_common_values.grounded_locomotion.dash_x",
  "global_common_values.grounded_locomotion.dash_tap_window",
  "global_common_values.grounded_locomotion.dash_early_action_window",
  "global_common_values.grounded_locomotion.dash_defensive_action_window",
  "global_common_values.grounded_locomotion.dash_late_action_window",
  "global_common_values.grounded_locomotion.dash_velocity_decay_milli",
  "captain_falcon_values.dash_and_turn.dash_initial_velocity",
  "captain_falcon_values.dash_and_turn.dash_run_acceleration_a",
  "captain_falcon_values.dash_and_turn.dash_run_acceleration_b",
  "captain_falcon_values.walk_and_run.dash_run_terminal_velocity"
],
"known_gaps": [
  "Human moonwalk feel is still short/not chainable in playtest; ledger should isolate whether remaining gap is input timing, stick path capture, Dash live accel, or follow-up state timing."
]
```

Minimum grounded node coverage:

- `Wait`: `ftCo_Wait`, Rust wait branch, traction/value refs.
- `WalkSlow/Middle/Fast`: `ftCo_Walk`/`ftwalkcommon`, Rust `enter_walk`/`apply_walk_velocity`, walk value refs.
- `Turn`: `ftCo_Turn`, Rust turn helpers, turn/dash value refs.
- `Dash`: source above.
- `Run`: `ftCo_Run`, Rust run branch, run threshold/taper/friction/Falcon run value refs.
- `RunBrake`: `ftCo_RunBrake`, Rust run brake branch, traction/max run brake refs.
- `TurnRun`: `ftCo_TurnRun`, Rust turn-run branch, run-turn no-interrupt and dash/run accel refs.

- [ ] **Step 4: Enrich key Dash edges**

Add `source_refs`, `rust_refs`, `value_refs`, `physics`, and `known_gaps` to these Mole graph edges:

- `Wait -> Dash`
- `WalkFast -> Dash`
- `Dash -> Turn`
- `Dash -> Run`
- `Dash -> Wait`
- `Run -> TurnRun`
- `Run -> RunBrake`
- `TurnRun -> Run`

For `Dash -> Turn`, include:

```json
"physics": [
  "fresh opposite dash/smash-turn input",
  "tap-start early Dash gate",
  "live Dash acceleration when opposite input ages or stays below dashback path"
]
```

- [ ] **Step 5: Add concise reference metadata on the Melee graph**

Add `source_refs` to the corresponding reference nodes and edges in
`docs/state_graphs/melee_reference_graph.json`. The reference graph does not
need Rust refs or value refs for every record in this checkpoint.

- [ ] **Step 6: Run graph tests**

Run:

```powershell
pytest tests/test_state_graph_viewer.py -q
```

Expected: all graph tests pass.

### Task 5: Display Ledger Details In The Viewer

**Files:**
- Modify: `D:\Mole Game\First_Game\tools\state_graph_viewer.py`
- Modify: `D:\Mole Game\First_Game\tests\test_state_graph_viewer.py`

- [ ] **Step 1: Add failing tests for detail formatting**

Update imports in `tests/test_state_graph_viewer.py`:

```python
    format_ledger_details,
```

Add:

```python
def test_format_ledger_details_includes_sources_values_physics_and_gaps():
    item = {
        "source_refs": [{"label": "Dash source", "path": "ftCo_Dash.c", "function": "ftCo_Dash_Phys"}],
        "rust_refs": [{"label": "Rust sim", "path": "crates/mole_core/src/sim.rs", "function": "apply_dash_velocity"}],
        "value_refs": ["global_common_values.grounded_locomotion.dash_x"],
        "physics": ["xE8 staged dash entry delta"],
        "known_gaps": ["moonwalk feel still under review"],
    }

    text = format_ledger_details(item)

    assert "Source:" in text
    assert "Dash source: ftCo_Dash.c (ftCo_Dash_Phys)" in text
    assert "Rust:" in text
    assert "Values:" in text
    assert "Physics:" in text
    assert "Known gaps:" in text
```

- [ ] **Step 2: Run the targeted test and verify failure**

Run:

```powershell
pytest tests/test_state_graph_viewer.py::test_format_ledger_details_includes_sources_values_physics_and_gaps -q
```

Expected: import failure for `format_ledger_details`.

- [ ] **Step 3: Implement ledger detail formatting**

Add to `tools/state_graph_viewer.py`:

```python
def format_ledger_details(item: dict[str, Any]) -> str:
    sections: list[str] = []
    _append_ref_section(sections, "Source", item.get("source_refs", []))
    _append_ref_section(sections, "Rust", item.get("rust_refs", []))
    _append_string_section(sections, "Values", item.get("value_refs", []))
    _append_string_section(sections, "Physics", item.get("physics", []))
    _append_string_section(sections, "Known gaps", item.get("known_gaps", []))
    return "\n\n".join(sections)


def _append_ref_section(sections: list[str], label: str, refs: list[dict[str, Any]]) -> None:
    if not refs:
        return
    lines = [f"{label}:"]
    for ref in refs:
        target = ref.get("path") or ref.get("url") or ""
        function = ref.get("function")
        suffix = f" ({function})" if function else ""
        lines.append(f"- {ref['label']}: {target}{suffix}")
    sections.append("\n".join(lines))


def _append_string_section(sections: list[str], label: str, values: list[str]) -> None:
    if not values:
        return
    sections.append("\n".join([f"{label}:"] + [f"- {value}" for value in values]))
```

Update `_show_node_details` and `_show_edge_details` to append ledger details:

```python
ledger = format_ledger_details(node)
body = f"{node['label']} [{style.label}]\n\n{node.get('notes', 'No notes yet.')}"
if ledger:
    body = f"{body}\n\n{ledger}"
self._set_details(body)
```

Use the same pattern for edges after the existing input/frames/notes text.

- [ ] **Step 4: Run viewer tests**

Run:

```powershell
pytest tests/test_state_graph_viewer.py -q
```

Expected: all viewer tests pass.

### Task 6: Keep Runtime Launch Integration And Engine Boundary Clean

**Files:**
- Modify: `D:\Mole Game\First_Game\tests\test_launch_inputs.py`
- Modify: `D:\Mole Game\First_Game\docs\architecture\visual-asset-and-state-graph-migration.md`

- [ ] **Step 1: Add/adjust launcher documentation test**

Extend `test_sdl3_runtime_launcher_opens_state_graph_viewer` in
`tests/test_launch_inputs.py`:

```python
def test_sdl3_runtime_launcher_opens_state_graph_viewer():
    launcher = ROOT / "execs" / "Run SDL3 Runtime.cmd"

    text = launcher.read_text(encoding="utf-8")

    assert "Open State Graphs.cmd" in text
    assert 'start "Mole State Graphs"' in text
    assert "tools\\state_graph_viewer.py" not in text
```

The last assertion keeps the game launcher coupled to the dedicated graph
launcher, not to viewer internals.

- [ ] **Step 2: Update architecture note**

In `docs/architecture/visual-asset-and-state-graph-migration.md`, add a short
section:

```markdown
## Engine-Agnostic Editor Direction

The state graph viewer and value sheets are development tooling around the
Rust-authoritative core. They should stay plain-data driven so the same
simulation and profile data can later be inspected or edited from a custom Rust
tool, Godot, Unity, or another frontend without changing gameplay authority.
```

- [ ] **Step 3: Run Python tests covering launch and graph tooling**

Run:

```powershell
pytest tests/test_value_sheets.py tests/test_state_graph_viewer.py tests/test_launch_inputs.py -q
```

Expected: all selected Python tests pass.

### Task 7: Verification And Handoff

**Files:**
- Modify: `D:\Mole Game\First_Game\docs\superpowers\plans\2026-05-30-parity-ledger-and-character-editor-foundation.md`

- [ ] **Step 1: Validate generated files and graph check**

Run:

```powershell
.venv\Scripts\python.exe tools\generate_value_sheets.py
.venv\Scripts\python.exe tools\state_graph_viewer.py --check
```

Expected: the graph check prints two graph summaries and two value-sheet summaries.

- [ ] **Step 2: Run full Python test slice**

Run:

```powershell
pytest tests/test_value_sheets.py tests/test_state_graph_viewer.py tests/test_live_test_session.py tests/test_launch_inputs.py -q
```

Expected: all selected Python tests pass.

- [ ] **Step 3: Run Rust verification gate to prove gameplay authority did not move**

Run:

```powershell
cargo fmt --check
cargo test --workspace --all-features --jobs 1
cargo clippy --workspace --all-targets --all-features --jobs 1 -- -D warnings
git diff --check
```

Expected: all commands pass. Any failure must be investigated before claiming the tooling checkpoint is complete.

- [ ] **Step 4: Runtime smoke**

Run:

```powershell
$env:PATH = "D:\Mole Game\First_Game\.local\SDL3\lib\x64;$env:PATH"
cargo run -p mole_runtime --features "sdl wup" -- --sdl --frames 3
```

Expected: exits successfully and reports `input_backend=wup`.

- [ ] **Step 5: Record the checkpoint result**

Update this plan's current status with:

- generated sheets created
- graph ledger enriched for grounded locomotion
- viewer check output verified
- tests/format/clippy results
- any remaining known gaps for moonwalk parity

Do not mark moonwalk fixed from this tooling pass. The output should make the
next source-backed moonwalk investigation easier.

## Current Status - 2026-05-30 Checkpoint

- Generated value sheets created:
  - `docs/state_graphs/value_sheets/global_common_values.json`
  - `docs/state_graphs/value_sheets/captain_falcon_values.json`
- Grounded locomotion graph ledger enriched for `Wait`, `WalkSlow`,
  `WalkMiddle`, `WalkFast`, `Turn`, `Dash`, `Run`, `RunBrake`, and `TurnRun`,
  plus dash/run transition edges. Reference graph records now carry concise
  Melee source metadata.
- Viewer foundation verified: `tools/state_graph_viewer.py --check` reports
  `melee_reference: 43 nodes, 68 edges`, `mole_current: 37 nodes, 56 edges`,
  `global_common_values: 5 categories, 48 fields`, and
  `captain_falcon_values: 4 categories, 33 fields`.
- Python verification passed:
  `pytest tests/test_value_sheets.py tests/test_state_graph_viewer.py tests/test_live_test_session.py tests/test_launch_inputs.py -q`
  reported `86 passed`.
- Rust/tooling verification passed:
  `cargo fmt --check`, `cargo test --workspace --all-features --jobs 1`,
  `cargo clippy --workspace --all-targets --all-features --jobs 1 -- -D warnings`,
  and `git diff --check` all exited successfully. `git diff --check` emitted
  line-ending normalization warnings only.
- Runtime smoke passed:
  `cargo run -p mole_runtime --features "sdl wup" -- --sdl --frames 3`
  exited successfully with `final_frame=3` and `input_backend=wup`.
- Remaining moonwalk parity gap: human playtest feel is still short/not
  chainable. This pass did not tune moonwalk or gameplay; the ledger now
  isolates the next investigation around input timing, stick path capture,
  live Dash acceleration, and follow-up state timing.
