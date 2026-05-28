from __future__ import annotations

import argparse
import copy
import json
import sys
from collections import Counter
from dataclasses import dataclass
from pathlib import Path
from typing import Any


PROJECT_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_GRAPH_DIR = PROJECT_ROOT / "docs" / "state_graphs"
DEFAULT_LAYOUT_PATH = PROJECT_ROOT / "config" / "state_graph_layout.json"
GRAPH_FILES = ("melee_reference_graph.json", "mole_current_graph.json")
MIN_ZOOM = 0.35
MAX_ZOOM = 2.75
ZOOM_STEP = 1.12
SCROLL_REGION_MARGIN = 2400
ONE_TO_ONE_LAYOUT_EQUIVALENTS = {
    "Wait",
    "WalkSlow",
    "WalkMiddle",
    "WalkFast",
    "Turn",
    "Dash",
    "Squat",
    "GuardOn",
    "KneeBend",
    "Run",
    "RunBrake",
    "TurnRun",
    "Guard",
    "GuardOff",
    "EscapeN",
    "EscapeF",
    "EscapeB",
    "JumpF",
    "JumpB",
    "JumpAerialF",
    "JumpAerialB",
    "EscapeAir",
    "FallSpecial",
    "LandingFallSpecial",
    "Landing",
}


@dataclass(frozen=True)
class StatusStyle:
    fill: str
    outline: str
    text: str
    label: str


STATUS_STYLES = {
    "reference": StatusStyle("#eef2f7", "#64748b", "#0f172a", "Reference"),
    "aligned": StatusStyle("#dcfce7", "#16a34a", "#052e16", "Aligned"),
    "partial": StatusStyle("#fef9c3", "#ca8a04", "#422006", "Partial"),
    "mismatch": StatusStyle("#fee2e2", "#dc2626", "#450a0a", "Mismatch"),
    "missing": StatusStyle("#fecaca", "#991b1b", "#450a0a", "Missing"),
    "intentional": StatusStyle("#dbeafe", "#2563eb", "#172554", "Intentional"),
}
ALLOWED_STATUSES = set(STATUS_STYLES)


def load_graphs(graph_dir: Path = DEFAULT_GRAPH_DIR) -> list[dict[str, Any]]:
    graphs = []
    for filename in GRAPH_FILES:
        path = graph_dir / filename
        with path.open("r", encoding="utf-8") as handle:
            graph = json.load(handle)
        errors = validate_graph(graph)
        if errors:
            joined = "\n".join(f"  - {error}" for error in errors)
            raise ValueError(f"{path} is not a valid state graph:\n{joined}")
        graphs.append(graph)
    return graphs


def apply_saved_layout(
    graphs: list[dict[str, Any]],
    layout_path: Path = DEFAULT_LAYOUT_PATH,
) -> None:
    if not layout_path.exists():
        return
    with layout_path.open("r", encoding="utf-8") as handle:
        payload = json.load(handle)
    saved_graphs = payload.get("graphs", {})
    for graph in graphs:
        saved_graph = saved_graphs.get(graph["id"], {})
        if isinstance(saved_graph, dict) and isinstance(saved_graph.get("zoom"), (int, float)):
            graph["zoom"] = clamp_zoom(saved_graph["zoom"])
        saved_nodes = saved_graph.get("nodes", saved_graph)
        for node in graph["nodes"]:
            saved_pos = saved_nodes.get(node["id"])
            if _is_position(saved_pos):
                node["pos"] = saved_pos


def clamp_zoom(value: float) -> float:
    return max(MIN_ZOOM, min(MAX_ZOOM, round(float(value), 4)))


def zoom_from_wheel_delta(current_zoom: float, wheel_delta: int) -> float:
    if wheel_delta > 0:
        return clamp_zoom(current_zoom * ZOOM_STEP)
    if wheel_delta < 0:
        return clamp_zoom(current_zoom / ZOOM_STEP)
    return clamp_zoom(current_zoom)


def equivalent_layout_targets(source_graph_id: str, source_node_id: str) -> list[tuple[str, str]]:
    if source_graph_id != "mole_current":
        return []
    if source_node_id not in ONE_TO_ONE_LAYOUT_EQUIVALENTS:
        return []
    return [("melee_reference", source_node_id)]


def equivalent_edge_targets(
    graphs: list[dict[str, Any]],
    source_graph_id: str,
    source_edge_index: int,
) -> list[tuple[str, int]]:
    graph_by_id = {graph["id"]: graph for graph in graphs}
    source_graph = graph_by_id.get(source_graph_id)
    reference = graph_by_id.get("melee_reference")
    if source_graph_id != "mole_current" or source_graph is None or reference is None:
        return []
    try:
        source_edge = source_graph["edges"][source_edge_index]
    except IndexError:
        return []
    if (
        source_edge["from"] not in ONE_TO_ONE_LAYOUT_EQUIVALENTS
        or source_edge["to"] not in ONE_TO_ONE_LAYOUT_EQUIVALENTS
    ):
        return []
    matches = []
    for index, edge in enumerate(reference["edges"]):
        if edge["from"] == source_edge["from"] and edge["to"] == source_edge["to"]:
            matches.append(("melee_reference", index))
    return matches


def pin_edge_label(graph: dict[str, Any], edge_index: int) -> None:
    graph.setdefault("_pinned_edge_labels", set()).add(edge_index)


def unpin_edge_label(graph: dict[str, Any], edge_index: int) -> None:
    graph.setdefault("_pinned_edge_labels", set()).discard(edge_index)


def toggle_edge_label_pin(graph: dict[str, Any], edge_index: int) -> bool:
    pinned = graph.setdefault("_pinned_edge_labels", set())
    if edge_index in pinned:
        pinned.remove(edge_index)
        return False
    pinned.add(edge_index)
    return True


def clear_pinned_edge_labels(graphs: list[dict[str, Any]]) -> None:
    for graph in graphs:
        graph.setdefault("_pinned_edge_labels", set()).clear()


def set_global_edge_labels(graphs: list[dict[str, Any]], show_all: bool) -> None:
    if not show_all:
        clear_pinned_edge_labels(graphs)


def is_edge_label_visible(
    graph: dict[str, Any],
    edge_index: int,
    *,
    show_all: bool,
) -> bool:
    return show_all or edge_index in graph.setdefault("_pinned_edge_labels", set())


def apply_linked_node_delta(
    graphs: list[dict[str, Any]],
    source_graph_id: str,
    source_node_id: str,
    delta_x: float,
    delta_y: float,
) -> int:
    graph_by_id = {graph["id"]: graph for graph in graphs}
    moved = 0
    for target_graph_id, target_node_id in equivalent_layout_targets(
        source_graph_id,
        source_node_id,
    ):
        target_graph = graph_by_id.get(target_graph_id)
        if target_graph is None:
            continue
        target_node = _find_node(target_graph, target_node_id)
        if target_node is None:
            continue
        target_node["pos"] = [
            round(target_node["pos"][0] + delta_x, 3),
            round(target_node["pos"][1] + delta_y, 3),
        ]
        moved += 1
    return moved


def save_layout(
    graphs: list[dict[str, Any]],
    layout_path: Path = DEFAULT_LAYOUT_PATH,
) -> None:
    payload = {
        "version": 1,
        "graphs": {
            graph["id"]: {
                "zoom": round(float(graph.get("zoom", 1.0)), 3),
                "nodes": {
                    node["id"]: [round(node["pos"][0], 3), round(node["pos"][1], 3)]
                    for node in graph["nodes"]
                },
            }
            for graph in graphs
        },
    }
    layout_path.parent.mkdir(parents=True, exist_ok=True)
    with layout_path.open("w", encoding="utf-8") as handle:
        json.dump(payload, handle, indent=2)
        handle.write("\n")


def validate_graph(graph: dict[str, Any]) -> list[str]:
    errors = []
    required = ("id", "title", "root", "nodes", "edges")
    for key in required:
        if key not in graph:
            errors.append(f"missing top-level key {key!r}")

    nodes = graph.get("nodes", [])
    edges = graph.get("edges", [])
    if not isinstance(nodes, list):
        errors.append("'nodes' must be a list")
        nodes = []
    if not isinstance(edges, list):
        errors.append("'edges' must be a list")
        edges = []

    node_ids = set()
    for index, node in enumerate(nodes):
        node_id = node.get("id")
        if not node_id:
            errors.append(f"node {index} missing id")
            continue
        if node_id in node_ids:
            errors.append(f"duplicate node id {node_id!r}")
        node_ids.add(node_id)
        status = node.get("status")
        if status not in ALLOWED_STATUSES:
            errors.append(f"node {node_id!r} has unknown status {status!r}")
        if not _is_position(node.get("pos")):
            errors.append(f"node {node_id!r} must have numeric [x, y] pos")

    root = graph.get("root")
    if root and root not in node_ids:
        errors.append(f"root {root!r} is not present in nodes")

    for index, edge in enumerate(edges):
        source = edge.get("from")
        target = edge.get("to")
        if source not in node_ids:
            errors.append(f"edge {index} source {source!r} is not a node")
        if target not in node_ids:
            errors.append(f"edge {index} target {target!r} is not a node")
        for key in ("input", "frames"):
            if not edge.get(key):
                errors.append(f"edge {index} missing {key!r}")
        status = edge.get("status")
        if status not in ALLOWED_STATUSES:
            errors.append(f"edge {index} has unknown status {status!r}")

    return errors


def summarize_comparison(graph: dict[str, Any]) -> dict[str, int]:
    counter = Counter()
    for collection_name in ("nodes", "edges"):
        for item in graph.get(collection_name, []):
            status = item.get("status")
            if status in ALLOWED_STATUSES and status != "reference":
                counter[status] += 1
    return {status: counter[status] for status in sorted(ALLOWED_STATUSES - {"reference"})}


def _find_node(graph: dict[str, Any], node_id: str) -> dict[str, Any] | None:
    for node in graph["nodes"]:
        if node["id"] == node_id:
            return node
    return None


def launch_viewer(
    graphs: list[dict[str, Any]],
    layout_path: Path = DEFAULT_LAYOUT_PATH,
) -> None:
    import tkinter as tk

    graphs = copy.deepcopy(graphs)
    apply_saved_layout(graphs, layout_path)

    root = tk.Tk()
    root.title("Mole State Transition Graphs")
    root.geometry("1540x940")
    root.minsize(1100, 760)

    show_labels = tk.BooleanVar(value=False)
    curved_edges = tk.BooleanVar(value=True)
    status_text = tk.StringVar(value="Layout changes are not saved until you click Save Layout.")

    toolbar = tk.Frame(root, padx=10, pady=7)
    toolbar.pack(fill=tk.X)
    draw_legend(toolbar)
    tk.Label(toolbar, textvariable=status_text, fg="#475569").pack(side=tk.LEFT, padx=(8, 18))
    tk.Button(
        toolbar,
        text="Save Layout",
        command=lambda: save_current_layout(graphs, layout_path, status_text),
    ).pack(side=tk.RIGHT, padx=(12, 0))
    tk.Checkbutton(
        toolbar,
        text="Curved edge lanes",
        variable=curved_edges,
        command=lambda: [pane.redraw() for pane in panes],
    ).pack(side=tk.RIGHT, padx=(12, 0))
    tk.Checkbutton(
        toolbar,
        text="Show edge labels",
        variable=show_labels,
        command=lambda: on_show_labels_changed(),
    ).pack(side=tk.RIGHT)

    split = tk.PanedWindow(root, orient=tk.HORIZONTAL, sashrelief=tk.RAISED)
    split.pack(fill=tk.BOTH, expand=True)

    panes: list[StateGraphPane] = []

    def on_node_moved(source_pane: StateGraphPane, node_id: str, dx: float, dy: float) -> None:
        moved = apply_linked_node_delta(
            graphs,
            source_pane.graph["id"],
            node_id,
            dx,
            dy,
        )
        if moved:
            status_text.set(f"Moved {moved} equivalent reference node(s). Click Save Layout to keep it.")
            for pane in panes:
                if pane is not source_pane:
                    pane.redraw()
        elif source_pane.graph["id"] == "mole_current" and node_id:
            status_text.set(
                f"No one-to-one Melee reference node for {node_id}; only the Mole node moved. Click Save Layout to keep it."
            )
        else:
            status_text.set("Layout changed. Click Save Layout to keep it.")

    def on_edge_label_pinned(source_pane: StateGraphPane, edge_index: int) -> None:
        pinned = toggle_edge_label_pin(source_pane.graph, edge_index)
        changed_equivalents = 0
        graph_by_id = {graph["id"]: graph for graph in graphs}
        for target_graph_id, target_edge_index in equivalent_edge_targets(
            graphs,
            source_pane.graph["id"],
            edge_index,
        ):
            target_graph = graph_by_id.get(target_graph_id)
            if target_graph is None:
                continue
            if pinned:
                pin_edge_label(target_graph, target_edge_index)
            else:
                unpin_edge_label(target_graph, target_edge_index)
            changed_equivalents += 1
        action = "Pinned" if pinned else "Unpinned"
        if changed_equivalents:
            status_text.set(f"{action} edge label and {changed_equivalents} exact Melee equivalent(s).")
        else:
            status_text.set(f"{action} edge label. No exact Melee equivalent changed.")
        for pane in panes:
            pane.redraw()

    def on_show_labels_changed() -> None:
        show_all = bool(show_labels.get())
        set_global_edge_labels(graphs, show_all)
        if show_all:
            status_text.set("Showing all edge labels; pinned labels are unchanged.")
        else:
            status_text.set("Hid all edge labels and cleared pinned labels.")
        for pane in panes:
            pane.redraw()

    panes = [
        StateGraphPane(
            split,
            graphs[0],
            show_labels,
            curved_edges,
            on_node_moved,
            on_edge_label_pinned,
            "left",
        ),
        StateGraphPane(
            split,
            graphs[1],
            show_labels,
            curved_edges,
            on_node_moved,
            on_edge_label_pinned,
            "right",
        ),
    ]
    for pane in panes:
        split.add(pane.frame, stretch="always")
        pane.redraw()

    root.protocol("WM_DELETE_WINDOW", root.destroy)
    root.mainloop()


def save_current_layout(graphs: list[dict[str, Any]], layout_path: Path, status_text: Any) -> None:
    save_layout(graphs, layout_path)
    status_text.set(f"Saved layout to {layout_path}.")


class StateGraphPane:
    NODE_WIDTH = 190
    NODE_HEIGHT = 62
    X_GAP = 240
    Y_GAP = 130
    X_ORIGIN = 40
    Y_ORIGIN = 112

    def __init__(
        self,
        parent: Any,
        graph: dict[str, Any],
        show_labels: Any,
        curved_edges: Any,
        on_node_moved: Any,
        on_edge_label_pinned: Any,
        side: str,
    ) -> None:
        import tkinter as tk

        self.graph = graph
        self.show_labels = show_labels
        self.curved_edges = curved_edges
        self.on_node_moved = on_node_moved
        self.on_edge_label_pinned = on_edge_label_pinned
        self.side = side
        self.zoom = clamp_zoom(graph.get("zoom", 1.0))
        self.graph["zoom"] = self.zoom
        self.drag_node_id: str | None = None
        self.drag_offset = (0, 0)
        self.node_boxes: dict[str, tuple[float, float, float, float]] = {}
        self.edge_lane_offsets: list[float] = []
        self.scroll_region: tuple[float, float, float, float] | None = None
        self.x_min = min(0, *(node["pos"][0] for node in graph["nodes"]))
        self.y_min = min(0, *(node["pos"][1] for node in graph["nodes"]))

        self.frame = tk.Frame(parent, bg="#ffffff")
        title = tk.Label(
            self.frame,
            text=graph["title"],
            anchor="w",
            bg="#ffffff",
            fg="#0f172a",
            font=("Segoe UI", 13, "bold"),
            padx=10,
            pady=4,
        )
        title.pack(fill=tk.X)
        description = tk.Label(
            self.frame,
            text=graph.get("description", ""),
            anchor="w",
            justify=tk.LEFT,
            wraplength=680,
            bg="#ffffff",
            fg="#475569",
            font=("Segoe UI", 9),
            padx=10,
        )
        description.pack(fill=tk.X)

        canvas_frame = tk.Frame(self.frame)
        canvas_frame.pack(fill=tk.BOTH, expand=True)
        self.canvas = tk.Canvas(canvas_frame, bg="#ffffff", highlightthickness=0)
        x_scroll = tk.Scrollbar(canvas_frame, orient=tk.HORIZONTAL, command=self.canvas.xview)
        y_scroll = tk.Scrollbar(canvas_frame, orient=tk.VERTICAL, command=self.canvas.yview)
        self.canvas.configure(xscrollcommand=x_scroll.set, yscrollcommand=y_scroll.set)
        y_scroll.pack(side=tk.RIGHT, fill=tk.Y)
        x_scroll.pack(side=tk.BOTTOM, fill=tk.X)
        self.canvas.pack(side=tk.LEFT, fill=tk.BOTH, expand=True)

        self.details = tk.Text(
            self.frame,
            height=5,
            wrap=tk.WORD,
            bg="#f8fafc",
            fg="#0f172a",
            relief=tk.FLAT,
            font=("Segoe UI", 9),
        )
        self.details.pack(fill=tk.X)
        self.details.insert(
            "1.0",
            "Mouse wheel zooms around the cursor. Right-click drag pans the graph. Click a node or edge badge for details. Left-click drag nodes to customize the layout; click Save Layout to keep it.",
        )
        self.details.configure(state=tk.DISABLED)

        self.canvas.bind("<ButtonPress-1>", self._on_press)
        self.canvas.bind("<Double-Button-1>", self._on_double_press)
        self.canvas.bind("<B1-Motion>", self._on_drag)
        self.canvas.bind("<ButtonRelease-1>", self._on_release)
        self.canvas.bind("<ButtonPress-3>", self._on_pan_press)
        self.canvas.bind("<B3-Motion>", self._on_pan_drag)
        self.canvas.bind("<ButtonRelease-3>", self._on_pan_release)
        self.canvas.bind("<MouseWheel>", self._on_mousewheel_zoom)
        self.canvas.bind("<Button-4>", self._on_mousewheel_zoom)
        self.canvas.bind("<Button-5>", self._on_mousewheel_zoom)

    def redraw(self) -> None:
        view_left = self.canvas.canvasx(0)
        view_top = self.canvas.canvasy(0)
        self.canvas.delete("all")
        self.node_boxes = {node["id"]: self._node_box(node) for node in self.graph["nodes"]}
        self.edge_lane_offsets = compute_edge_lane_offsets(
            self.graph["edges"],
            spacing=20 * self.zoom,
        )
        for index, edge in enumerate(self.graph["edges"], start=1):
            self._draw_edge(index, edge)
        for node in self.graph["nodes"]:
            self._draw_node(node)
        self._update_scroll_region()
        self._scroll_canvas_top_left_to(view_left, view_top)

    def _node_box(self, node: dict[str, Any]) -> tuple[float, float, float, float]:
        x, y = node["pos"]
        left = self.X_ORIGIN + (x - self.x_min) * self._x_gap()
        top = self.Y_ORIGIN + (y - self.y_min) * self._y_gap()
        return (left, top, left + self._node_width(), top + self._node_height())

    def _draw_node(self, node: dict[str, Any]) -> None:
        left, top, right, bottom = self.node_boxes[node["id"]]
        style = STATUS_STYLES[node["status"]]
        tags = ("node", f"node:{node['id']}")
        self.canvas.create_rectangle(
            left,
            top,
            right,
            bottom,
            fill=style.fill,
            outline=style.outline,
            width=max(1, round(2 * self.zoom)),
            tags=tags,
        )
        self.canvas.create_text(
            left + 10 * self.zoom,
            top + 9 * self.zoom,
            text=node["label"],
            anchor="nw",
            width=(right - left) - 20 * self.zoom,
            fill=style.text,
            font=("Segoe UI", self._font_size(9), "bold"),
            tags=tags,
        )
        self.canvas.create_text(
            left + 10 * self.zoom,
            bottom - 20 * self.zoom,
            text=style.label,
            anchor="nw",
            fill=style.outline,
            font=("Segoe UI", self._font_size(8)),
            tags=tags,
        )

    def _draw_edge(self, index: int, edge: dict[str, Any]) -> None:
        source = _edge_anchor(self.node_boxes[edge["from"]], self.node_boxes[edge["to"]])
        target = _target_anchor(self.node_boxes[edge["from"]], self.node_boxes[edge["to"]])
        style = STATUS_STYLES[edge["status"]]
        tags = ("edge", f"edge:{index - 1}")
        lane_offset = self.edge_lane_offsets[index - 1] if self.edge_lane_offsets else 0
        use_curves = bool(self.curved_edges.get())
        points = route_edge(source, target, lane_offset=lane_offset, curved=use_curves)
        self.canvas.create_line(
            *points,
            fill=style.outline,
            width=max(1, round(2 * self.zoom)),
            arrow="last",
            smooth=use_curves,
            splinesteps=24,
            tags=tags,
        )
        badge_x, badge_y = edge_badge_position(points)
        if is_edge_label_visible(self.graph, index - 1, show_all=bool(self.show_labels.get())):
            self._draw_edge_label(edge, badge_x, badge_y, tags)
        else:
            badge_radius = max(7, 11 * self.zoom)
            self.canvas.create_oval(
                badge_x - badge_radius,
                badge_y - badge_radius,
                badge_x + badge_radius,
                badge_y + badge_radius,
                fill=style.fill,
                outline=style.outline,
                width=max(1, round(2 * self.zoom)),
                tags=tags,
            )
            self.canvas.create_text(
                badge_x,
                badge_y,
                text=str(index),
                fill=style.text,
                font=("Segoe UI", self._font_size(8), "bold"),
                tags=tags,
            )

    def _draw_edge_label(
        self,
        edge: dict[str, Any],
        x: float,
        y: float,
        tags: tuple[str, str],
    ) -> None:
        text_id = self.canvas.create_text(
            x,
            y,
            text=f"{edge['input']}\n{edge['frames']}",
            anchor="center",
            width=170 * self.zoom,
            fill="#111827",
            font=("Segoe UI", self._font_size(7)),
            tags=tags,
        )
        bbox = self.canvas.bbox(text_id)
        if bbox is None:
            return
        pad = 3
        rect = self.canvas.create_rectangle(
            bbox[0] - pad,
            bbox[1] - pad,
            bbox[2] + pad,
            bbox[3] + pad,
            fill="#ffffff",
            outline="#e2e8f0",
            tags=tags,
        )
        self.canvas.tag_lower(rect, text_id)

    def _on_press(self, event: Any) -> None:
        tags = self.canvas.gettags("current")
        node_id = _tag_value(tags, "node:")
        edge_index = _tag_value(tags, "edge:")
        canvas_x = self.canvas.canvasx(event.x)
        canvas_y = self.canvas.canvasy(event.y)
        if node_id:
            box = self.node_boxes[node_id]
            self.drag_node_id = node_id
            self.drag_offset = (canvas_x - box[0], canvas_y - box[1])
            self._show_node_details(node_id)
        elif edge_index is not None:
            self.drag_node_id = None
            self._show_edge_details(int(edge_index))
        else:
            self.drag_node_id = None

    def _on_drag(self, event: Any) -> None:
        if not self.drag_node_id:
            return
        node = self._node_by_id(self.drag_node_id)
        old_x, old_y = node["pos"]
        canvas_x = self.canvas.canvasx(event.x)
        canvas_y = self.canvas.canvasy(event.y)
        left = max(0, canvas_x - self.drag_offset[0])
        top = max(70, canvas_y - self.drag_offset[1])
        new_pos = [
            round(self.x_min + (left - self.X_ORIGIN) / self._x_gap(), 3),
            round(self.y_min + (top - self.Y_ORIGIN) / self._y_gap(), 3),
        ]
        node["pos"] = new_pos
        delta_x = round(new_pos[0] - old_x, 3)
        delta_y = round(new_pos[1] - old_y, 3)
        if delta_x or delta_y:
            self.on_node_moved(self, self.drag_node_id, delta_x, delta_y)
        self.redraw()

    def _on_double_press(self, event: Any) -> str:
        tags = self.canvas.gettags("current")
        edge_index = _tag_value(tags, "edge:")
        if edge_index is None:
            return "break"
        self.drag_node_id = None
        self.on_edge_label_pinned(self, int(edge_index))
        self._show_edge_details(int(edge_index))
        return "break"

    def _on_release(self, _event: Any) -> None:
        self.drag_node_id = None

    def _on_mousewheel_zoom(self, event: Any) -> str:
        delta = _event_wheel_delta(event)
        if delta == 0:
            return "break"
        before_canvas_x = self.canvas.canvasx(event.x)
        before_canvas_y = self.canvas.canvasy(event.y)
        graph_x, graph_y = self._canvas_to_graph_point(before_canvas_x, before_canvas_y)
        next_zoom = zoom_from_wheel_delta(self.zoom, delta)
        if next_zoom == self.zoom:
            return "break"

        self.zoom = next_zoom
        self.graph["zoom"] = next_zoom
        self.redraw()

        after_canvas_x, after_canvas_y = self._graph_to_canvas_point(graph_x, graph_y)
        self._scroll_canvas_to_keep_cursor_point(after_canvas_x, after_canvas_y, event.x, event.y)
        self._set_details(
            f"{self.graph['title']}\n\n"
            f"Zoom: {self.zoom:.2f}x. Mouse wheel zooms around the cursor; click Save Layout to keep it."
        )
        self.on_node_moved(self, "", 0, 0)
        return "break"

    def _on_pan_press(self, event: Any) -> str:
        if not is_right_drag_event(event):
            return "break"
        self.drag_node_id = None
        self.canvas.scan_mark(event.x, event.y)
        self.canvas.configure(cursor="fleur")
        return "break"

    def _on_pan_drag(self, event: Any) -> str:
        self.canvas.scan_dragto(event.x, event.y, gain=1)
        return "break"

    def _on_pan_release(self, _event: Any) -> str:
        self.canvas.configure(cursor="")
        return "break"

    def _show_node_details(self, node_id: str) -> None:
        node = self._node_by_id(node_id)
        style = STATUS_STYLES[node["status"]]
        self._set_details(
            f"{node['label']} [{style.label}]\n\n"
            f"{node.get('notes', 'No notes yet.')}"
        )

    def _show_edge_details(self, edge_index: int) -> None:
        edge = self.graph["edges"][edge_index]
        style = STATUS_STYLES[edge["status"]]
        notes = edge.get("notes", "No notes yet.")
        self._set_details(
            f"{edge['from']} -> {edge['to']} [{style.label}]\n\n"
            f"Input: {edge['input']}\n"
            f"Frames: {edge['frames']}\n\n"
            f"{notes}"
        )

    def _set_details(self, text: str) -> None:
        self.details.configure(state="normal")
        self.details.delete("1.0", "end")
        self.details.insert("1.0", text)
        self.details.configure(state="disabled")

    def _node_by_id(self, node_id: str) -> dict[str, Any]:
        for node in self.graph["nodes"]:
            if node["id"] == node_id:
                return node
        raise KeyError(node_id)

    def _node_width(self) -> float:
        return self.NODE_WIDTH * self.zoom

    def _node_height(self) -> float:
        return self.NODE_HEIGHT * self.zoom

    def _x_gap(self) -> float:
        return self.X_GAP * self.zoom

    def _y_gap(self) -> float:
        return self.Y_GAP * self.zoom

    def _font_size(self, base_size: int) -> int:
        return max(5, min(16, round(base_size * self.zoom)))

    def _canvas_to_graph_point(self, canvas_x: float, canvas_y: float) -> tuple[float, float]:
        return (
            self.x_min + (canvas_x - self.X_ORIGIN) / self._x_gap(),
            self.y_min + (canvas_y - self.Y_ORIGIN) / self._y_gap(),
        )

    def _graph_to_canvas_point(self, graph_x: float, graph_y: float) -> tuple[float, float]:
        return (
            self.X_ORIGIN + (graph_x - self.x_min) * self._x_gap(),
            self.Y_ORIGIN + (graph_y - self.y_min) * self._y_gap(),
        )

    def _scroll_canvas_to_keep_cursor_point(
        self,
        canvas_x: float,
        canvas_y: float,
        cursor_x: float,
        cursor_y: float,
    ) -> None:
        if self.scroll_region is None:
            return
        left, top, right, bottom = self.scroll_region
        width = max(1, right - left)
        height = max(1, bottom - top)
        target_x = (canvas_x - cursor_x - left) / width
        target_y = (canvas_y - cursor_y - top) / height
        self.canvas.xview_moveto(max(0.0, min(1.0, target_x)))
        self.canvas.yview_moveto(max(0.0, min(1.0, target_y)))

    def _update_scroll_region(self) -> None:
        bbox = self.canvas.bbox("all")
        if bbox is None:
            return
        self.scroll_region = expand_scroll_region(
            self.scroll_region,
            bbox,
            margin=SCROLL_REGION_MARGIN * self.zoom,
        )
        self.canvas.configure(scrollregion=self.scroll_region)

    def _scroll_canvas_top_left_to(self, canvas_x: float, canvas_y: float) -> None:
        if self.scroll_region is None:
            return
        self.canvas.xview_moveto(scroll_fraction_for_canvas_coordinate(self.scroll_region, canvas_x, "x"))
        self.canvas.yview_moveto(scroll_fraction_for_canvas_coordinate(self.scroll_region, canvas_y, "y"))


def edge_lane_offsets(count: int, spacing: float = 22) -> list[float]:
    if count <= 0:
        return []
    center = (count - 1) / 2
    return [round((index - center) * spacing, 3) for index in range(count)]


def compute_edge_lane_offsets(edges: list[dict[str, Any]], spacing: float = 22) -> list[float]:
    offsets = [0.0 for _edge in edges]
    _add_lane_group_offsets(offsets, edges, "from", spacing, weight=1.0)
    _add_lane_group_offsets(offsets, edges, "to", spacing, weight=0.55)
    return [round(offset, 3) for offset in offsets]


def route_edge(
    source: tuple[float, float],
    target: tuple[float, float],
    *,
    lane_offset: float = 0,
    curved: bool = False,
) -> list[float]:
    if not curved:
        if abs(source[0] - target[0]) < 8:
            return [source[0], source[1], target[0], target[1]]
        mid_y = (source[1] + target[1]) / 2
        return [source[0], source[1], source[0], mid_y, target[0], mid_y, target[0], target[1]]

    normal_x, normal_y = _normal(source, target)
    dx = target[0] - source[0]
    dy = target[1] - source[1]
    control_one = (
        source[0] + dx * 0.35 + normal_x * lane_offset,
        source[1] + dy * 0.35 + normal_y * lane_offset,
    )
    control_two = (
        source[0] + dx * 0.65 + normal_x * lane_offset,
        source[1] + dy * 0.65 + normal_y * lane_offset,
    )
    return [
        source[0],
        source[1],
        control_one[0],
        control_one[1],
        control_two[0],
        control_two[1],
        target[0],
        target[1],
    ]


def edge_badge_position(points: list[float]) -> tuple[float, float]:
    if len(points) >= 8:
        return (points[2] + points[4]) / 2, (points[3] + points[5]) / 2
    return (points[0] + points[-2]) / 2, (points[1] + points[-1]) / 2


def _add_lane_group_offsets(
    offsets: list[float],
    edges: list[dict[str, Any]],
    key: str,
    spacing: float,
    *,
    weight: float,
) -> None:
    groups: dict[str, list[int]] = {}
    for index, edge in enumerate(edges):
        groups.setdefault(str(edge[key]), []).append(index)
    for indices in groups.values():
        if len(indices) == 1:
            continue
        for index, lane_offset in zip(indices, edge_lane_offsets(len(indices), spacing)):
            offsets[index] += lane_offset * weight


def _normal(
    source: tuple[float, float],
    target: tuple[float, float],
) -> tuple[float, float]:
    dx = target[0] - source[0]
    dy = target[1] - source[1]
    length = (dx * dx + dy * dy) ** 0.5
    if length == 0:
        return 0, 1
    return -dy / length, dx / length


def expand_scroll_region(
    current: tuple[float, float, float, float] | None,
    content_bbox: tuple[float, float, float, float],
    *,
    margin: float,
) -> tuple[float, float, float, float]:
    expanded = (
        content_bbox[0] - margin,
        content_bbox[1] - margin,
        content_bbox[2] + margin,
        content_bbox[3] + margin,
    )
    if current is None:
        return expanded
    return (
        min(current[0], expanded[0]),
        min(current[1], expanded[1]),
        max(current[2], expanded[2]),
        max(current[3], expanded[3]),
    )


def scroll_fraction_for_canvas_coordinate(
    scroll_region: tuple[float, float, float, float],
    canvas_coordinate: float,
    axis: str,
) -> float:
    if axis == "x":
        start, end = scroll_region[0], scroll_region[2]
    elif axis == "y":
        start, end = scroll_region[1], scroll_region[3]
    else:
        raise ValueError(f"unknown axis {axis!r}")
    span = max(1.0, end - start)
    return max(0.0, min(1.0, (canvas_coordinate - start) / span))


def _legacy_route_edge(source: tuple[float, float], target: tuple[float, float]) -> list[float]:
    if abs(source[0] - target[0]) < 8:
        return [source[0], source[1], target[0], target[1]]
    mid_y = (source[1] + target[1]) / 2
    return [source[0], source[1], source[0], mid_y, target[0], mid_y, target[0], target[1]]


def draw_legend(parent: Any) -> None:
    import tkinter as tk

    for status in ("aligned", "partial", "mismatch", "missing", "intentional", "reference"):
        style = STATUS_STYLES[status]
        swatch = tk.Label(parent, width=2, bg=style.fill, relief=tk.SOLID, borderwidth=1)
        swatch.pack(side=tk.LEFT, padx=(0, 4))
        label = tk.Label(parent, text=style.label, fg="#0f172a")
        label.pack(side=tk.LEFT, padx=(0, 18))


def _edge_anchor(
    source_box: tuple[float, float, float, float],
    target_box: tuple[float, float, float, float],
) -> tuple[float, float]:
    s_left, s_top, s_right, s_bottom = source_box
    _t_left, t_top, _t_right, t_bottom = target_box
    s_center_x = (s_left + s_right) / 2
    s_center_y = (s_top + s_bottom) / 2
    t_center_y = (t_top + t_bottom) / 2
    if t_center_y >= s_center_y:
        return s_center_x, s_bottom
    return s_center_x, s_top


def _target_anchor(
    source_box: tuple[float, float, float, float],
    target_box: tuple[float, float, float, float],
) -> tuple[float, float]:
    _s_left, s_top, _s_right, s_bottom = source_box
    t_left, t_top, t_right, t_bottom = target_box
    s_center_y = (s_top + s_bottom) / 2
    t_center_x = (t_left + t_right) / 2
    t_center_y = (t_top + t_bottom) / 2
    if t_center_y >= s_center_y:
        return t_center_x, t_top
    return t_center_x, t_bottom


def _tag_value(tags: tuple[str, ...], prefix: str) -> str | None:
    for tag in tags:
        if tag.startswith(prefix):
            return tag.removeprefix(prefix)
    return None


def _event_wheel_delta(event: Any) -> int:
    if getattr(event, "num", None) == 4:
        return 120
    if getattr(event, "num", None) == 5:
        return -120
    return int(getattr(event, "delta", 0))


def is_right_drag_event(event: Any) -> bool:
    return getattr(event, "num", None) == 3


def _is_position(value: Any) -> bool:
    return (
        isinstance(value, list)
        and len(value) == 2
        and all(isinstance(item, (int, float)) for item in value)
    )


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Open the Mole/Melee state graph viewer.")
    parser.add_argument("--graph-dir", type=Path, default=DEFAULT_GRAPH_DIR)
    parser.add_argument("--layout", type=Path, default=DEFAULT_LAYOUT_PATH)
    parser.add_argument("--check", action="store_true", help="Validate graph data without opening a window.")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    graphs = load_graphs(args.graph_dir)
    if args.check:
        apply_saved_layout(graphs, args.layout)
        for graph in graphs:
            summary = summarize_comparison(graph)
            print(f"{graph['id']}: {len(graph['nodes'])} nodes, {len(graph['edges'])} edges")
            if any(summary.values()):
                print("  " + ", ".join(f"{key}={value}" for key, value in summary.items()))
        return 0
    launch_viewer(graphs, args.layout)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
