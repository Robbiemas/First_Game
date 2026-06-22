import json
import subprocess
import sys
from pathlib import Path

from tools.export_parity_diff_report import (
    build_parity_diff_report,
    write_parity_diff_reports,
)


ROOT = Path(__file__).resolve().parents[1]
VALUE_SHEETS = ROOT / "docs" / "state_graphs" / "value_sheets"


def test_parity_diff_report_summarizes_global_and_character_matches_without_derived_rows():
    report = build_parity_diff_report(VALUE_SHEETS)

    assert report["id"] == "value_parity_diff_report"
    global_values = report["sections"]["global_values"]
    character_values = report["sections"]["test_character_values"]
    global_combat_values = report["sections"]["global_combat_values"]
    falcon_combat_values = report["sections"]["captain_falcon_combat_values"]

    assert global_values["summary"] == {
        "total": 95,
        "match": 95,
        "diff": 0,
        "derived": 0,
        "missing": 0,
        "actionable": 0,
    }
    assert character_values["summary"] == {
        "total": 41,
        "match": 41,
        "diff": 0,
        "derived": 0,
        "missing": 0,
        "actionable": 0,
    }
    assert global_combat_values["summary"] == {
        "total": 56,
        "match": 56,
        "diff": 0,
        "derived": 0,
        "missing": 0,
        "actionable": 0,
    }
    assert falcon_combat_values["summary"] == {
        "total": 1,
        "match": 1,
        "diff": 0,
        "derived": 0,
        "missing": 0,
        "actionable": 0,
    }

    assert global_values["actionable_rows"] == []
    assert character_values["derived_rows"] == []
    assert global_combat_values["actionable_rows"] == []
    assert falcon_combat_values["actionable_rows"] == []
    shield_start_health = next(
        row for row in global_combat_values["rows"] if row["field"] == "shield_start_health"
    )
    assert shield_start_health["decomp_value"] == 60.0
    assert shield_start_health["rust_value"] == 60.0
    assert shield_start_health["status"] == "match"
    max_jumps = next(row for row in character_values["rows"] if row["field"] == "max_jumps")
    assert max_jumps["decomp_value"] == 2
    assert max_jumps["rust_value"] == 2
    assert max_jumps["status"] == "match"


def test_write_parity_diff_reports_writes_stable_json_and_markdown(tmp_path):
    generated = write_parity_diff_reports(VALUE_SHEETS, tmp_path)

    assert generated == [
        tmp_path / "value_diffs.json",
        tmp_path / "value_diffs.md",
    ]
    payload = json.loads(generated[0].read_text(encoding="utf-8"))
    markdown = generated[1].read_text(encoding="utf-8")

    assert generated[0].read_text(encoding="utf-8").endswith("\n")
    assert generated[1].read_text(encoding="utf-8").endswith("\n")
    assert payload["sections"]["global_values"]["summary"]["actionable"] == 0
    assert payload["sections"]["global_combat_values"]["summary"]["actionable"] == 0
    assert payload["sections"]["captain_falcon_combat_values"]["summary"]["actionable"] == 0
    assert "## Global Values" in markdown
    assert "No non-matching rows." in markdown
    assert "## Test Character Values" in markdown
    assert "## Global Combat Values" in markdown
    assert "## Captain Falcon Combat Values" in markdown
    assert "Derived Rows" not in markdown


def test_parity_diff_report_cli_runs_from_project_root(tmp_path):
    result = subprocess.run(
        [
            sys.executable,
            "tools/export_parity_diff_report.py",
            "--value-sheets",
            str(VALUE_SHEETS),
            "--output",
            str(tmp_path),
        ],
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=False,
    )

    assert result.returncode == 0, result.stderr
    assert (tmp_path / "value_diffs.json").exists()
    assert (tmp_path / "value_diffs.md").exists()
