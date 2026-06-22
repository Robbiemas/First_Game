from tools.falcon_ecb_mapping import (
    CHARACTER_SPECIAL_ACTION_BINDINGS,
    FALCON_SPECIAL_ACTION_BINDINGS,
)


def test_falcon_special_bindings_cover_decomp_action_table():
    assert FALCON_SPECIAL_ACTION_BINDINGS == (
        ("SpecialN", 347, 301, "SpecialN", 100),
        ("SpecialAirN", 348, 302, "SpecialAirN", 100),
        ("SpecialSStart", 349, 303, "SpecialSStart", 80),
        ("SpecialS", 350, 304, "SpecialS", 25),
        ("SpecialAirSStart", 351, 305, "SpecialAirSStart", 80),
        ("SpecialAirS", 352, 306, "SpecialAirS", 45),
        ("SpecialHi", 353, 307, "SpecialHi", 65),
        ("SpecialAirHi", 354, 308, "SpecialAirHi", 65),
        (None, 355, 309, "SpecialHiCatch", 16),
        (None, 356, 310, "SpecialHiThrow", 60),
        ("SpecialLw", 357, 311, "SpecialLw", 40),
        (None, 358, 312, "SpecialLwEnd", 30),
        ("SpecialAirLw", 359, 313, "SpecialAirLw", 30),
        (None, 360, 314, "SpecialAirLwEnd", 45),
        (None, 361, 316, "SpecialAirLwEndAir", 29),
        (None, 362, 315, "SpecialLwEndAir", 30),
        (None, 363, 317, "SpecialHiThrow", 60),
    )


def test_character_special_bindings_are_keyed_for_future_characters():
    assert CHARACTER_SPECIAL_ACTION_BINDINGS == {
        "captain": FALCON_SPECIAL_ACTION_BINDINGS,
    }
