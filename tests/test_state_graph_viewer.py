from pathlib import Path
from types import SimpleNamespace

from tools.state_graph_viewer import (
    ALLOWED_STATUSES,
    STATUS_STYLES,
    apply_linked_node_delta,
    apply_saved_layout,
    clamp_zoom,
    compute_edge_lane_offsets,
    edge_lane_offsets,
    expand_scroll_region,
    equivalent_edge_targets,
    equivalent_layout_targets,
    is_edge_label_visible,
    is_right_drag_event,
    load_graphs,
    pin_edge_label,
    route_edge,
    save_layout,
    scroll_fraction_for_canvas_coordinate,
    set_global_edge_labels,
    summarize_comparison,
    toggle_edge_label_pin,
    validate_graph,
    zoom_from_wheel_delta,
)


ROOT = Path(__file__).resolve().parents[1]
GRAPH_DIR = ROOT / "docs" / "state_graphs"


def test_state_graph_data_files_define_reference_and_current_graphs():
    graphs = load_graphs(GRAPH_DIR)

    assert [graph["id"] for graph in graphs] == ["melee_reference", "mole_current"]
    assert all(graph["root"] == "Wait" for graph in graphs)
    assert all(validate_graph(graph) == [] for graph in graphs)


def test_mole_graph_statuses_are_color_coded_for_parity_review():
    _reference, mole = load_graphs(GRAPH_DIR)
    node_statuses = {node["status"] for node in mole["nodes"]}
    edge_statuses = {edge["status"] for edge in mole["edges"]}

    assert node_statuses | edge_statuses <= ALLOWED_STATUSES
    assert node_statuses >= {"aligned", "partial", "missing", "intentional"}
    assert edge_statuses >= {"aligned", "partial", "mismatch", "intentional"}
    assert "aligned" in STATUS_STYLES
    assert STATUS_STYLES["missing"].fill != STATUS_STYLES["aligned"].fill


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
    assert equivalent_layout_targets("mole_current", "Walk") == []
    assert equivalent_layout_targets("mole_current", "Squat") == []
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
    ignored = apply_linked_node_delta(graphs, "mole_current", "Walk", 2, -1)

    walks_after = {
        node["id"]: list(node["pos"])
        for node in reference["nodes"]
        if node["id"] in walks
    }
    assert moved == 1
    assert ignored == 0
    assert dash["pos"] == [dash_before[0] + 2, dash_before[1] - 1]
    assert walks_after == walks


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
    mole = graphs[1]
    dash_to_run = next(
        index
        for index, edge in enumerate(mole["edges"])
        if edge["from"] == "Dash" and edge["to"] == "Run"
    )
    wait_to_walk = next(
        index
        for index, edge in enumerate(mole["edges"])
        if edge["from"] == "Wait" and edge["to"] == "Walk"
    )

    assert equivalent_edge_targets(graphs, "mole_current", dash_to_run) == [
        ("melee_reference", 13)
    ]
    assert equivalent_edge_targets(graphs, "mole_current", wait_to_walk) == []


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
    assert summary["missing"] >= 1
    assert summary["intentional"] >= 1
