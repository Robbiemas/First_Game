from __future__ import annotations

import argparse
import json
import sys
from collections import Counter
from pathlib import Path
from typing import Any

if __package__ is None or __package__ == "":
    sys.path.append(str(Path(__file__).resolve().parents[1]))

from tools.state_graph_viewer import (
    build_value_comparison_rows,
    load_rust_character_values,
    load_rust_global_values,
    load_value_sheets,
)


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_VALUE_SHEETS = ROOT / "docs" / "state_graphs" / "value_sheets"
DEFAULT_OUTPUT = ROOT / "docs" / "state_graphs" / "parity_reports"
ACTIONABLE_STATUSES = {"diff", "missing"}
REPORT_FILES = ("value_diffs.json", "value_diffs.md")


def build_parity_diff_report(value_sheet_dir: Path = DEFAULT_VALUE_SHEETS) -> dict[str, Any]:
    sheets = {sheet["id"]: sheet for sheet in load_value_sheets(value_sheet_dir)}
    global_rows = build_value_comparison_rows(
        sheets["global_common_values"],
        load_rust_global_values(),
    )
    character_rows = build_value_comparison_rows(
        sheets["captain_falcon_values"],
        load_rust_character_values(),
        character_value=True,
    )
    rust_combat_values = {
        **load_rust_global_values(),
        **load_rust_character_values(),
    }
    global_combat_rows = build_value_comparison_rows(
        sheets["global_combat_values"],
        rust_combat_values,
    )
    falcon_combat_rows = build_value_comparison_rows(
        sheets["captain_falcon_combat_values"],
        rust_combat_values,
    )
    return {
        "id": "value_parity_diff_report",
        "engine_boundary": "rust_core_authority",
        "sections": {
            "global_values": _section(global_rows),
            "test_character_values": _section(character_rows),
            "global_combat_values": _section(global_combat_rows),
            "captain_falcon_combat_values": _section(falcon_combat_rows),
        },
    }


def write_parity_diff_reports(
    value_sheet_dir: Path = DEFAULT_VALUE_SHEETS,
    output_dir: Path = DEFAULT_OUTPUT,
) -> list[Path]:
    output_dir.mkdir(parents=True, exist_ok=True)
    report = build_parity_diff_report(value_sheet_dir)
    json_path = output_dir / REPORT_FILES[0]
    markdown_path = output_dir / REPORT_FILES[1]
    json_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    markdown_path.write_text(format_parity_diff_markdown(report) + "\n", encoding="utf-8")
    return [json_path, markdown_path]


def format_parity_diff_markdown(report: dict[str, Any]) -> str:
    lines = [
        "# Value Parity Diff Report",
        "",
        "Engine boundary: Rust core remains authoritative. Treat `diff` rows as investigation targets, not automatic tuning instructions.",
    ]
    for section_id, title in (
        ("global_values", "Global Values"),
        ("test_character_values", "Test Character Values"),
        ("global_combat_values", "Global Combat Values"),
        ("captain_falcon_combat_values", "Captain Falcon Combat Values"),
    ):
        section = report["sections"][section_id]
        summary = section["summary"]
        lines.extend(
            [
                "",
                f"## {title}",
                "",
                (
                    f"Total: {summary['total']} | Match: {summary['match']} | "
                    f"Diff: {summary['diff']} | Derived: {summary['derived']} | "
                    f"Missing: {summary['missing']} | Actionable: {summary['actionable']}"
                ),
                "",
            ]
        )
        if section["actionable_rows"]:
            lines.extend(_table("Actionable Rows", section["actionable_rows"]))
        if section["derived_rows"]:
            lines.extend(_table("Derived Rows", section["derived_rows"]))
        if not section["actionable_rows"] and not section["derived_rows"]:
            lines.append("No non-matching rows.")
    return "\n".join(lines)


def _section(rows: list[dict[str, Any]]) -> dict[str, Any]:
    summary = _summary(rows)
    return {
        "summary": summary,
        "actionable_rows": [row for row in rows if row["status"] in ACTIONABLE_STATUSES],
        "derived_rows": [row for row in rows if row["status"] == "derived"],
        "rows": rows,
    }


def _summary(rows: list[dict[str, Any]]) -> dict[str, int]:
    counts = Counter(row["status"] for row in rows)
    return {
        "total": len(rows),
        "match": counts["match"],
        "diff": counts["diff"],
        "derived": counts["derived"],
        "missing": counts["missing"],
        "actionable": sum(counts[status] for status in ACTIONABLE_STATUSES),
    }


def _table(title: str, rows: list[dict[str, Any]]) -> list[str]:
    lines = [
        f"### {title}",
        "",
        "| Field | Category | Decomp Field | Decomp Value | Rust Field | Rust Value | Status |",
        "| --- | --- | --- | --- | --- | --- | --- |",
    ]
    for row in rows:
        lines.append(
            "| "
            + " | ".join(
                _markdown_cell(row[key])
                for key in (
                    "field",
                    "category",
                    "source_field",
                    "decomp_value",
                    "rust_field",
                    "rust_value",
                    "status",
                )
            )
            + " |"
        )
    lines.append("")
    return lines


def _markdown_cell(value: Any) -> str:
    text = "" if value is None else str(value)
    return text.replace("|", "\\|").replace("\n", " ")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Export value parity diff reports.")
    parser.add_argument("--value-sheets", type=Path, default=DEFAULT_VALUE_SHEETS)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    for path in write_parity_diff_reports(args.value_sheets, args.output):
        print(path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
