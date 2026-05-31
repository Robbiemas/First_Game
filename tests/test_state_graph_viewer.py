import os
from pathlib import Path
from types import SimpleNamespace

from tools.state_graph_viewer import (
    ALLOWED_STATUSES,
    ECB_COVERAGE_TAB_LABEL,
    INPUT_TRACE_TAB_LABEL,
    PARITY_LEDGER_TAB_LABEL,
    SLIPPI_REPLAY_TAB_LABEL,
    STATUS_STYLES,
    STATE_GRAPHS_TAB_LABEL,
    TOOL_TITLE,
    apply_linked_node_delta,
    apply_saved_layout,
    build_parity_ledger_overview,
    build_value_comparison_rows,
    clamp_zoom,
    compute_edge_lane_offsets,
    edge_lane_offsets,
    expand_scroll_region,
    equivalent_edge_targets,
    equivalent_layout_targets,
    format_ledger_details,
    format_ecb_coverage,
    format_latest_slippi_report,
    format_recent_input_trace,
    load_ecb_coverage,
    is_edge_label_visible,
    is_right_drag_event,
    latest_input_trace_path,
    latest_slippi_report_path,
    load_recent_input_trace_rows,
    load_graphs,
    load_rust_character_values,
    load_rust_global_values,
    load_value_sheets,
    pin_edge_label,
    route_edge,
    save_layout,
    scroll_fraction_for_canvas_coordinate,
    set_global_edge_labels,
    summarize_comparison,
    summarize_value_sheet,
    toggle_edge_label_pin,
    validate_graph,
    zoom_from_wheel_delta,
)


ROOT = Path(__file__).resolve().parents[1]
GRAPH_DIR = ROOT / "docs" / "state_graphs"


def test_dev_tool_title_and_tab_labels_make_ledger_visible():
    assert TOOL_TITLE == "Mole Game Dev Tool"
    assert STATE_GRAPHS_TAB_LABEL == "State Graphs"
    assert PARITY_LEDGER_TAB_LABEL == "Parity Ledger"
    assert ECB_COVERAGE_TAB_LABEL == "ECB Coverage"
    assert INPUT_TRACE_TAB_LABEL == "Input Trace"
    assert SLIPPI_REPLAY_TAB_LABEL == "Slippi Replay"


def test_latest_slippi_report_path_returns_newest_report(tmp_path):
    old_report = tmp_path / "Game_old.report.md"
    new_report = tmp_path / "Game_new.report.md"
    ignored = tmp_path / "Game_new.inputs.json"
    old_report.write_text("old\n", encoding="utf-8")
    new_report.write_text("new\n", encoding="utf-8")
    ignored.write_text("{}\n", encoding="utf-8")
    os.utime(old_report, (1000, 1000))
    os.utime(new_report, (2000, 2000))

    assert latest_slippi_report_path(tmp_path) == new_report


def test_format_latest_slippi_report_shows_latest_report_text(tmp_path):
    report = tmp_path / "Game_test.report.md"
    report.write_text("# Slippi Replay Input Diagnostic\n\nMoonwalk evidence\n", encoding="utf-8")

    text = format_latest_slippi_report(report_dir=tmp_path)

    assert "File: Game_test.report.md" in text
    assert "Slippi Replay Input Diagnostic" in text
    assert "Moonwalk evidence" in text


def test_format_latest_slippi_report_guides_when_missing(tmp_path):
    text = format_latest_slippi_report(report_dir=tmp_path)

    assert "No Slippi replay diagnostic report found yet" in text
    assert "tools\\slippi_replay_to_inputs.cjs" in text


def test_latest_input_trace_path_returns_newest_controller_trace(tmp_path):
    old_trace = tmp_path / "controller-input-trace-100.jsonl"
    new_trace = tmp_path / "controller-input-trace-200.jsonl"
    other_log = tmp_path / "input-debug-300.jsonl"
    old_trace.write_text("old\n", encoding="utf-8")
    new_trace.write_text("new\n", encoding="utf-8")
    other_log.write_text("other\n", encoding="utf-8")

    assert latest_input_trace_path(tmp_path) == new_trace


def test_recent_input_trace_rows_loads_tail_json_lines(tmp_path):
    trace = tmp_path / "controller-input-trace-test.jsonl"
    trace.write_text(
        "\n".join(
            [
                '{"frame":1,"after":{"players":[{"motion_state":"Wait"}]}}',
                "not json",
                '{"frame":2,"after":{"players":[{"motion_state":"Dash"}]}}',
                '{"frame":3,"after":{"players":[{"motion_state":"Turn"}]}}',
            ]
        )
        + "\n",
        encoding="utf-8",
    )

    rows = load_recent_input_trace_rows(trace, limit=2)

    assert [row["frame"] for row in rows] == [2, 3]


def test_format_recent_input_trace_summarizes_physical_to_core_chain(tmp_path):
    trace = tmp_path / "controller-input-trace-test.jsonl"
    trace.write_text(
        (
            '{"frame":7,'
            '"wup":{"players":[{"raw":{"main_x":104,"main_y":0},'
            '"native":{"main_x":104,"main_y":0},'
            '"ucf":{"main_x":127,"main_y":0},'
            '"dashback_amendment":true,'
            '"melee":{"x_tap_timer":0,"dash_direction":1,"held_dash_x_direction":1}}]},'
            '"after":{"players":[{"motion_state":"Dash","state_frame":0,'
            '"velocity_x":2000,"velocity_y":0,'
            '"core_facts":{"dash_direction":1,"held_dash_x_direction":1}}]}}'
        )
        + "\n",
        encoding="utf-8",
    )

    text = format_recent_input_trace(trace, limit=5)

    assert "controller-input-trace-test.jsonl" in text
    assert "F7" in text
    assert "raw=(104,0)" in text
    assert "native=(104,0)" in text
    assert "ucf=(127,0)" in text
    assert "tap=0" in text
    assert "dash=1" in text
    assert "state=Dash" in text
    assert "vx=2000" in text
    assert "ucf_db=True" in text


def test_parity_ledger_overview_summarizes_value_sheets_and_grounded_coverage():
    graphs = load_graphs(GRAPH_DIR)
    sheets = load_value_sheets(ROOT / "docs" / "state_graphs" / "value_sheets")

    text = build_parity_ledger_overview(graphs, sheets)

    assert "Parity Ledger" in text
    assert "global_common_values: 6 categories, 63 fields" in text
    assert "captain_falcon_values: 5 categories, 39 fields" in text
    assert "Grounded ledger coverage: 9/9 nodes" in text
    assert "Dash-related physics edges:" in text
    assert "moonwalk" in text.lower()


def test_ecb_coverage_summary_keeps_unmapped_derived_states_visible():
    coverage = load_ecb_coverage(GRAPH_DIR / "parity_reports" / "falcon_ecb_coverage.json")

    text = format_ecb_coverage(coverage)

    assert "Captain Falcon ECB Coverage" in text
    assert "Mapped exact action-table states: 70" in text
    assert "Missing sampled mappings: 0" in text
    assert "No action-submotion ECB states:" in text
    assert "Entry" in text
    assert coverage["unmapped_derived_motion_states"] == []
    assert "KneeBend" not in coverage["unmapped_derived_motion_states"]
    assert "GuardReflect" not in coverage["unmapped_derived_motion_states"]
    assert "EntryStart" not in coverage["unmapped_derived_motion_states"]
    assert "SpecialS" not in coverage["unmapped_derived_motion_states"]
    assert "SpecialAirS" not in coverage["unmapped_derived_motion_states"]
    assert "Do not alias these states to nearby animation records" in text
    assert "Air" not in coverage["unmapped_derived_motion_states"]


def test_global_value_comparison_rows_include_decomp_and_current_rust_values():
    sheets = load_value_sheets(ROOT / "docs" / "state_graphs" / "value_sheets")
    global_sheet = next(sheet for sheet in sheets if sheet["id"] == "global_common_values")

    rows = build_value_comparison_rows(global_sheet, load_rust_global_values())
    dash_x = next(row for row in rows if row["field"] == "dash_x")
    platform_drop = next(row for row in rows if row["field"] == "platform_drop_delay_ticks")
    fall_drift_threshold = next(
        row for row in rows if row["field"] == "fall_animation_drift_threshold_milli"
    )
    fall_blend = next(row for row in rows if row["field"] == "fall_animation_blend_milli")

    c_stick_x = next(row for row in rows if row["field"] == "c_stick_deadzone_x")

    entry_start = next(row for row in rows if row["field"] == "entry_start_ticks")

    turn_run_x = next(row for row in rows if row["field"] == "turn_run_x")

    assert len(rows) == 63
    assert dash_x["source_field"] == "x3C"
    assert dash_x["decomp_value"] == 102
    assert dash_x["rust_field"] == "dash_x"
    assert dash_x["rust_value"] == 102
    assert dash_x["status"] == "match"
    assert turn_run_x["source_field"] == "x38_someLStickXThreshold"
    assert turn_run_x["decomp_value"] == -48
    assert turn_run_x["rust_field"] == "turn_run_x"
    assert turn_run_x["rust_value"] == -48
    assert turn_run_x["status"] == "match"
    assert platform_drop["decomp_value"] == 2
    assert platform_drop["rust_value"] == 2
    assert platform_drop["status"] == "match"
    assert fall_drift_threshold["source_field"] == "x444"
    assert fall_drift_threshold["decomp_value"] == 100
    assert fall_drift_threshold["rust_value"] == 100
    assert fall_drift_threshold["status"] == "match"
    assert fall_blend["source_field"] == "x448"
    assert fall_blend["decomp_value"] == 500
    assert fall_blend["rust_value"] == 500
    assert fall_blend["status"] == "match"
    assert c_stick_x["decomp_value"] == 36
    assert c_stick_x["rust_value"] == 36
    assert c_stick_x["status"] == "match"
    assert entry_start["source_field"] == "x6BC"
    assert entry_start["decomp_value"] == 30
    assert entry_start["rust_value"] == 30
    assert entry_start["status"] == "match"


def test_character_value_comparison_rows_include_falcon_and_dolphin_mole_values():
    sheets = load_value_sheets(ROOT / "docs" / "state_graphs" / "value_sheets")
    falcon_sheet = next(sheet for sheet in sheets if sheet["id"] == "captain_falcon_values")

    rows = build_value_comparison_rows(
        falcon_sheet,
        load_rust_character_values(),
        character_value=True,
    )
    dash_initial = next(row for row in rows if row["field"] == "dash_initial_velocity")
    max_jumps = next(row for row in rows if row["field"] == "max_jumps")
    air_jump_v = next(row for row in rows if row["field"] == "air_jump_v_multiplier")

    entry_offset = next(row for row in rows if row["field"] == "entry_platform_offset_y")
    landing_air_f = next(row for row in rows if row["field"] == "landingairf_lag")

    assert len(rows) == 39
    assert dash_initial["decomp_value"] == 2000
    assert dash_initial["rust_field"] == "initial_dash_speed_per_tick"
    assert dash_initial["rust_value"] == 2000
    assert dash_initial["status"] == "match"
    assert max_jumps["decomp_value"] == 2
    assert max_jumps["rust_field"] == "max_jumps"
    assert max_jumps["rust_value"] == 2
    assert max_jumps["status"] == "match"
    assert air_jump_v["rust_field"] == "air_jump_v_multiplier_milli"
    assert air_jump_v["rust_value"] == 900
    assert air_jump_v["status"] == "match"
    assert entry_offset["decomp_value"] == 1647
    assert entry_offset["rust_field"] == "entry_platform_offset_y"
    assert entry_offset["rust_value"] == 1647
    assert entry_offset["status"] == "match"
    assert landing_air_f["decomp_value"] == 19
    assert landing_air_f["rust_field"] == "landing_air_f_lag_ticks"
    assert landing_air_f["rust_value"] == 19
    assert landing_air_f["status"] == "match"


def test_state_graph_data_files_define_reference_and_current_graphs():
    graphs = load_graphs(GRAPH_DIR)

    assert [graph["id"] for graph in graphs] == ["melee_reference", "mole_current"]
    assert all(graph["root"] == "Wait" for graph in graphs)
    assert all(validate_graph(graph) == [] for graph in graphs)


def test_mole_graph_statuses_are_color_coded_for_parity_review():
    _reference, mole = load_graphs(GRAPH_DIR)
    node_statuses = {node["status"] for node in mole["nodes"]}
    edge_statuses = {edge["status"] for edge in mole["edges"]}
    graph_statuses = node_statuses | edge_statuses

    assert graph_statuses <= ALLOWED_STATUSES
    assert node_statuses >= {"aligned", "partial", "intentional"}
    assert edge_statuses >= {"aligned", "partial"}
    assert "aligned" in STATUS_STYLES
    assert "mismatch" in STATUS_STYLES
    assert "missing" in STATUS_STYLES
    assert STATUS_STYLES["missing"].fill != STATUS_STYLES["aligned"].fill


def test_current_mole_graph_reflects_rust_core_instead_of_pygame_layer():
    _reference, mole = load_graphs(GRAPH_DIR)
    node_ids = {node["id"] for node in mole["nodes"]}
    source_paths = {source.get("path") for source in mole["sources"]}

    assert mole["title"] == "Mole Current: Rust Core"
    assert "deterministic Rust core" in mole["description"]
    assert {
        "crates/mole_core/src/state.rs",
        "crates/mole_core/src/sim.rs",
        "crates/mole_core/tests/core_contract.rs",
    } <= source_paths
    assert "ChooseAction.py" not in source_paths
    assert "Characters.py" not in source_paths
    assert {
        "Wait",
        "WalkSlow",
        "WalkMiddle",
        "WalkFast",
        "Dash",
        "Run",
        "RunBrake",
        "TurnRun",
        "KneeBend",
        "JumpF",
        "JumpB",
        "Fall",
        "FallAerial",
        "LandingAirN",
        "EscapeAir",
        "FallSpecial",
        "Landing",
    } <= node_ids
    assert "Walk" not in node_ids
    assert "Air" not in node_ids


def test_transition_edges_include_frame_and_input_requirements():
    graphs = load_graphs(GRAPH_DIR)

    for graph in graphs:
        for edge in graph["edges"]:
            assert edge["from"]
            assert edge["to"]
            assert edge["input"]
            assert edge["frames"]


def test_melee_reference_uses_explicit_decomp_common_state_names():
    reference, _mole = load_graphs(GRAPH_DIR)
    node_ids = {node["id"] for node in reference["nodes"]}

    assert {
        "Wait",
        "WalkSlow",
        "WalkMiddle",
        "WalkFast",
        "JumpF",
        "JumpB",
        "Fall",
        "FallF",
        "FallB",
        "GuardSetOff",
        "GuardReflect",
        "LandingAirN",
    } <= node_ids


def test_saved_layout_overrides_node_positions_and_can_be_persisted(tmp_path):
    graphs = load_graphs(GRAPH_DIR)
    layout_path = tmp_path / "state_graph_layout.json"
    graphs[0]["nodes"][0]["pos"] = [9, 8]

    save_layout(graphs, layout_path)
    reloaded = load_graphs(GRAPH_DIR)
    apply_saved_layout(reloaded, layout_path)

    assert reloaded[0]["nodes"][0]["pos"] == [9, 8]


def test_saved_layout_persists_zoom_explicitly(tmp_path):
    graphs = load_graphs(GRAPH_DIR)
    layout_path = tmp_path / "state_graph_layout.json"
    graphs[1]["zoom"] = 1.75

    save_layout(graphs, layout_path)
    reloaded = load_graphs(GRAPH_DIR)
    apply_saved_layout(reloaded, layout_path)

    assert reloaded[1]["zoom"] == 1.75


def test_mole_layout_equivalents_only_map_exact_one_to_one_states():
    assert equivalent_layout_targets("mole_current", "Wait") == [
        ("melee_reference", "Wait")
    ]
    assert equivalent_layout_targets("mole_current", "Dash") == [
        ("melee_reference", "Dash")
    ]
    assert equivalent_layout_targets("mole_current", "WalkSlow") == [
        ("melee_reference", "WalkSlow")
    ]
    assert equivalent_layout_targets("mole_current", "WalkMiddle") == [
        ("melee_reference", "WalkMiddle")
    ]
    assert equivalent_layout_targets("mole_current", "WalkFast") == [
        ("melee_reference", "WalkFast")
    ]
    assert equivalent_layout_targets("mole_current", "RunBrake") == [
        ("melee_reference", "RunBrake")
    ]
    assert equivalent_layout_targets("mole_current", "JumpF") == [
        ("melee_reference", "JumpF")
    ]
    assert equivalent_layout_targets("mole_current", "Fall") == [
        ("melee_reference", "Fall")
    ]
    assert equivalent_layout_targets("mole_current", "FallAerial") == [
        ("melee_reference", "FallAerial")
    ]
    assert equivalent_layout_targets("mole_current", "FallSpecialF") == [
        ("melee_reference", "FallSpecialF")
    ]
    assert equivalent_layout_targets("mole_current", "Squat") == [
        ("melee_reference", "Squat")
    ]
    assert equivalent_layout_targets("mole_current", "Air") == []
    assert equivalent_layout_targets("mole_current", "ShieldTurn") == []


def test_linked_node_delta_moves_only_exact_reference_equivalents():
    graphs = load_graphs(GRAPH_DIR)
    reference = graphs[0]
    dash = next(node for node in reference["nodes"] if node["id"] == "Dash")
    walks = {
        node["id"]: list(node["pos"])
        for node in reference["nodes"]
        if node["id"] in {"WalkSlow", "WalkMiddle", "WalkFast"}
    }
    dash_before = list(dash["pos"])

    moved = apply_linked_node_delta(graphs, "mole_current", "Dash", 2, -1)
    walk_moved = apply_linked_node_delta(graphs, "mole_current", "WalkSlow", 2, -1)
    ignored = apply_linked_node_delta(graphs, "mole_current", "Air", 2, -1)

    walks_after = {
        node["id"]: list(node["pos"])
        for node in reference["nodes"]
        if node["id"] in walks
    }
    assert moved == 1
    assert walk_moved == 1
    assert ignored == 0
    assert dash["pos"] == [dash_before[0] + 2, dash_before[1] - 1]
    assert walks_after["WalkSlow"] == [
        walks["WalkSlow"][0] + 2,
        walks["WalkSlow"][1] - 1,
    ]
    assert walks_after["WalkMiddle"] == walks["WalkMiddle"]
    assert walks_after["WalkFast"] == walks["WalkFast"]


def test_zoom_helpers_change_scale_and_clamp_to_readable_bounds():
    assert zoom_from_wheel_delta(1.0, 120) > 1.0
    assert zoom_from_wheel_delta(1.0, -120) < 1.0
    assert clamp_zoom(0.01) == 0.35
    assert clamp_zoom(10.0) == 2.75


def test_right_drag_event_helper_identifies_right_mouse_button():
    assert is_right_drag_event(SimpleNamespace(num=3)) is True
    assert is_right_drag_event(SimpleNamespace(num=1)) is False


def test_edge_lane_offsets_center_parallel_edges_around_zero():
    assert edge_lane_offsets(1, spacing=10) == [0]
    assert edge_lane_offsets(5, spacing=10) == [-20, -10, 0, 10, 20]


def test_edge_lane_routing_separates_shared_source_and_target_arrows():
    edges = [
        {"from": "Wait", "to": "Dash"},
        {"from": "Wait", "to": "Walk"},
        {"from": "Wait", "to": "Guard"},
        {"from": "Run", "to": "Wait"},
        {"from": "Dash", "to": "Wait"},
    ]

    offsets = compute_edge_lane_offsets(edges, spacing=10)

    assert len(set(offsets[:3])) == 3
    assert offsets[3] != offsets[4]


def test_curved_route_uses_lane_offset_to_avoid_same_line_overlap():
    source = (100, 100)
    target = (300, 300)

    left = route_edge(source, target, lane_offset=-20, curved=True)
    center = route_edge(source, target, lane_offset=0, curved=True)
    right = route_edge(source, target, lane_offset=20, curved=True)

    assert left != center != right
    assert left[2:6] != right[2:6]


def test_scroll_region_expands_with_margin_but_never_shrinks():
    initial = expand_scroll_region(None, (100, 200, 300, 400), margin=50)
    assert initial == (50, 150, 350, 450)

    unchanged = expand_scroll_region(initial, (150, 250, 220, 330), margin=50)
    assert unchanged == initial

    expanded = expand_scroll_region(initial, (-100, 600, 500, 700), margin=50)
    assert expanded == (-150, 150, 550, 750)


def test_scroll_fraction_uses_stable_canvas_coordinates():
    region = (-100, -50, 900, 950)

    assert scroll_fraction_for_canvas_coordinate(region, -100, "x") == 0.0
    assert scroll_fraction_for_canvas_coordinate(region, 400, "x") == 0.5
    assert scroll_fraction_for_canvas_coordinate(region, 950, "y") == 1.0


def test_edge_equivalents_only_match_exact_one_to_one_edges():
    graphs = load_graphs(GRAPH_DIR)
    melee = graphs[0]
    mole = graphs[1]
    dash_to_run = next(
        index
        for index, edge in enumerate(mole["edges"])
        if edge["from"] == "Dash" and edge["to"] == "Run"
    )
    wait_to_walk_slow = next(
        index
        for index, edge in enumerate(mole["edges"])
        if edge["from"] == "Wait" and edge["to"] == "WalkSlow"
    )
    melee_dash_to_run = next(
        index
        for index, edge in enumerate(melee["edges"])
        if edge["from"] == "Dash" and edge["to"] == "Run"
    )
    melee_wait_to_walk_slow = next(
        index
        for index, edge in enumerate(melee["edges"])
        if edge["from"] == "Wait" and edge["to"] == "WalkSlow"
    )

    assert equivalent_edge_targets(graphs, "mole_current", dash_to_run) == [
        ("melee_reference", melee_dash_to_run)
    ]
    assert equivalent_edge_targets(graphs, "mole_current", wait_to_walk_slow) == [
        ("melee_reference", melee_wait_to_walk_slow)
    ]


def test_pinned_edge_labels_survive_show_all_on_but_clear_when_show_all_turns_off():
    graphs = load_graphs(GRAPH_DIR)
    mole = graphs[1]

    pin_edge_label(mole, 2)

    assert is_edge_label_visible(mole, 2, show_all=False) is True
    set_global_edge_labels(graphs, show_all=True)
    assert is_edge_label_visible(mole, 2, show_all=True) is True
    assert is_edge_label_visible(mole, 3, show_all=True) is True
    set_global_edge_labels(graphs, show_all=False)
    assert is_edge_label_visible(mole, 2, show_all=False) is False


def test_edge_label_pin_toggle_can_turn_individual_labels_off():
    graphs = load_graphs(GRAPH_DIR)
    mole = graphs[1]

    assert toggle_edge_label_pin(mole, 2) is True
    assert is_edge_label_visible(mole, 2, show_all=False) is True
    assert toggle_edge_label_pin(mole, 2) is False
    assert is_edge_label_visible(mole, 2, show_all=False) is False


def test_comparison_summary_counts_current_graph_statuses():
    _reference, mole = load_graphs(GRAPH_DIR)

    summary = summarize_comparison(mole)

    assert summary["aligned"] >= 1
    assert summary["partial"] >= 1
    assert summary["missing"] == 0
    assert summary["intentional"] >= 1


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
                "value_refs": ["captain_falcon_values.walk_and_run.traction_per_tick"],
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


def test_value_sheets_load_for_viewer_summary():
    sheets = load_value_sheets(ROOT / "docs" / "state_graphs" / "value_sheets")

    assert [sheet["id"] for sheet in sheets] == [
        "global_common_values",
        "captain_falcon_values",
    ]
    assert summarize_value_sheet(sheets[0])["fields"] >= 10
    assert summarize_value_sheet(sheets[1])["fields"] >= 10


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


def test_reference_dash_documents_source_velocity_chain():
    reference, _mole = load_graphs(GRAPH_DIR)
    dash_node = next(node for node in reference["nodes"] if node["id"] == "Dash")
    physics = "\n".join(dash_node.get("physics", []))

    assert "gr_vel" in physics
    assert "xE4_ground_accel_1" in physics
    assert "xE8_ground_accel_2" in physics
    assert "ftCommon_800804A0" in physics
    assert "ftCommon_8007C98C" in physics
    assert "Fighter_procUpdate" in physics


def test_current_dash_to_turn_documents_x54_velocity_decay():
    _reference, mole = load_graphs(GRAPH_DIR)
    dash_to_turn = next(
        edge for edge in mole["edges"]
        if edge["from"] == "Dash" and edge["to"] == "Turn"
    )
    physics = "\n".join(dash_to_turn.get("physics", []))

    assert "x54" in physics
    assert "Dash IASA" in physics
    assert "Turn_Phys" in physics
    assert "Fighter_procUpdate" in physics


def test_current_wait_and_walk_to_turn_document_same_frame_turn_physics():
    _reference, mole = load_graphs(GRAPH_DIR)
    edges = {
        (edge["from"], edge["to"]): edge
        for edge in mole["edges"]
    }

    for key in [("Wait", "Turn"), ("WalkFast", "Turn")]:
        edge = edges[key]
        physics = "\n".join(edge.get("physics", []))
        assert edge["status"] == "aligned"
        assert "Turn_Phys" in physics
        assert "xE4" in physics


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
