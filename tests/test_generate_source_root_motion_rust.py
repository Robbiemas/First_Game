import struct

from tools.generate_source_root_motion_rust import (
    SOURCE_ACTIONS,
    SOURCE_ACTION_TABLES,
    SOURCE_STATES,
    fmt_float,
    normalize_vec3_float_literals,
    vec_literal,
)


def f32_bits(value: float) -> str:
    bits = struct.unpack(">I", struct.pack(">f", value))[0]
    return f"f32::from_bits(0x{bits:08x})"


def test_source_root_motion_generator_preserves_f32_bits_in_rust_literals():
    assert fmt_float(2.125) == f32_bits(2.125)
    assert fmt_float(-0.0) == f32_bits(-0.0)
    assert vec_literal({"x": 2.125, "y": -0.0, "z": -1.25}) == (
        f"Vec3::new({f32_bits(2.125)}, {f32_bits(-0.0)}, {f32_bits(-1.25)})"
    )


def test_source_root_motion_generator_covers_falcon_up_special_actions():
    assert SOURCE_STATES["SpecialHi"] == ("SPECIAL_HI", 65)
    assert SOURCE_STATES["SpecialAirHi"] == ("SPECIAL_AIR_HI", 65)


def test_source_root_motion_generator_covers_source_ledge_option_actions():
    for source_action_key in [
        "CliffAttackQuick",
        "CliffAttackSlow",
        "CliffClimbQuick",
        "CliffClimbSlow",
        "CliffEscapeQuick",
        "CliffEscapeSlow",
        "CliffJumpQuick1",
        "CliffJumpQuick2",
        "CliffJumpSlow1",
        "CliffJumpSlow2",
    ]:
        assert source_action_key in SOURCE_ACTIONS


def test_source_root_motion_generator_covers_falcon_side_special_ground_actions():
    assert SOURCE_ACTIONS["SpecialSStart"] == "SPECIAL_S_START"
    assert SOURCE_ACTIONS["SpecialS"] == "SPECIAL_S"


def test_source_root_motion_generator_covers_attack_dash_transn_motion():
    assert SOURCE_ACTION_TABLES["AttackDash"] == "ATTACK_DASH"


def test_source_root_motion_generator_normalizes_existing_vec3_float_literals():
    module = (
        "const TEST: [Vec3; 2] = [\n"
        "    Vec3::new(0.0, -23.810546875, -2.528564453),\n"
        f"    Vec3::new({f32_bits(2.125)}, {f32_bits(0.0)}, {f32_bits(-1.25)}),\n"
        "];\n"
    )

    normalized = normalize_vec3_float_literals(module)

    assert f"Vec3::new({f32_bits(0.0)}, {f32_bits(-23.810546875)}, {f32_bits(-2.528564453)})" in normalized
    assert f"Vec3::new({f32_bits(2.125)}, {f32_bits(0.0)}, {f32_bits(-1.25)})" in normalized
