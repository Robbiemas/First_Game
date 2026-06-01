from pathlib import Path
import json

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
    assert fields["turn_run_x"]["source_name"] == "x38_someLStickXThreshold"
    assert fields["turn_run_x"]["offset_hex"] == "0x38"
    assert fields["turn_run_x"]["converted_value"] == -48
    assert fields["dash_tap_window"]["converted_value"] == 2
    assert fields["run_accel_taper_milli"]["source_name"] == "x5C"
    assert fields["run_ground_friction_multiplier_milli"]["source_name"] == "x60_someFrictionMul"
    assert fields["run_brake_animation_pause_velocity_milli"]["source_name"] == "x42C"
    assert fields["fall_animation_drift_threshold_milli"]["source_name"] == "x444"
    assert fields["fall_animation_drift_threshold_milli"]["converted_value"] == 100
    assert fields["fall_animation_blend_milli"]["source_name"] == "x448"
    assert fields["fall_animation_blend_milli"]["converted_value"] == 500


def test_character_value_sheet_groups_falcon_locomotion_fields():
    sheet = build_character_sheet(FALCON, character_id="captain_falcon")

    assert sheet["id"] == "captain_falcon_values"
    assert sheet["scope"] == "character"
    assert sheet["character_id"] == "captain_falcon"
    fields = {field["rust_name"]: field for category in sheet["categories"] for field in category["fields"]}

    assert fields["dash_initial_velocity"]["converted_value"] == 2000
    assert fields["dash_run_acceleration_a"]["kind"] == "source_f32"
    assert fields["dash_run_acceleration_a"]["comparison_value_kind"] == "source_f32"
    assert fields["dash_run_acceleration_a"]["converted_value"] == 0.15000000596046448
    assert fields["dash_run_terminal_velocity"]["kind"] == "source_f32"
    assert fields["dash_run_terminal_velocity"]["comparison_value_kind"] == "source_f32"
    assert fields["dash_run_terminal_velocity"]["converted_value"] == 2.299999952316284
    assert fields["traction_per_tick"]["converted_value"] == 80
    assert fields["grav"]["converted_value"] == 130
    assert fields["landingairn_lag"]["converted_value"] == 15
    assert fields["landingairf_lag"]["converted_value"] == 19
    assert fields["landingairb_lag"]["converted_value"] == 18
    assert fields["landingairhi_lag"]["converted_value"] == 15
    assert fields["landingairlw_lag"]["converted_value"] == 24
    assert fields["entry_platform_offset_y"]["converted_value"] == 1647


def test_value_sheets_expose_every_extracted_decomp_field():
    common_fields = set(json.loads(COMMON.read_text(encoding="utf-8"))["fields"])
    falcon_fields = set(json.loads(FALCON.read_text(encoding="utf-8"))["fields"])

    global_sheet = build_global_sheet(COMMON)
    falcon_sheet = build_character_sheet(FALCON, character_id="captain_falcon")
    global_sheet_fields = {
        field["rust_name"]
        for category in global_sheet["categories"]
        for field in category["fields"]
    }
    falcon_sheet_fields = {
        field["rust_name"]
        for category in falcon_sheet["categories"]
        for field in category["fields"]
    }

    assert global_sheet_fields == common_fields
    assert falcon_sheet_fields == falcon_fields


def test_generate_value_sheets_writes_stable_json_files(tmp_path):
    output_dir = tmp_path / "value_sheets"

    generated = generate_value_sheets(COMMON, FALCON, output_dir)

    assert generated == [
        output_dir / "global_common_values.json",
        output_dir / "captain_falcon_values.json",
    ]
    assert generated[0].read_text(encoding="utf-8").endswith("\n")
    assert generated[1].read_text(encoding="utf-8").endswith("\n")
