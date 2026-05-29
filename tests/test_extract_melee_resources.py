import struct
from pathlib import Path

from tools.extract_melee_resources import (
    DatExtractError,
    PROJECT_ROOT,
    extract_captain_profile_from_plca,
    extract_common_data_from_plco,
)


def be32(value):
    return value.to_bytes(4, "big")


def put_f32(data, offset, value):
    data[offset : offset + 4] = struct.pack(">f", value)


def put_i32(data, offset, value):
    data[offset : offset + 4] = struct.pack(">i", value)


def make_dat(root_name, data_block, root_data_offset):
    header = bytearray(0x20)
    header[0x00:0x04] = be32(0x20 + len(data_block))
    header[0x04:0x08] = be32(len(data_block))
    header[0x08:0x0C] = be32(0)
    header[0x0C:0x10] = be32(1)
    header[0x10:0x14] = be32(0)
    root_table = bytearray()
    root_table += be32(root_data_offset)
    root_table += be32(0)
    string_table = root_name.encode("ascii") + b"\0"
    return bytes(header + data_block + root_table + string_table)


def test_extract_common_data_from_plco_uses_ftload_common_attribute_pointer():
    data_block = bytearray(0x700)
    ftload_offset = 0x40
    common_offset = 0x100
    data_block[ftload_offset : ftload_offset + 4] = be32(common_offset)

    put_f32(data_block, common_offset + 0x08, 0.37)
    put_f32(data_block, common_offset + 0x0C, 0.38)
    put_f32(data_block, common_offset + 0x10, 0.12)
    put_f32(data_block, common_offset + 0x14, 0.31)
    put_f32(data_block, common_offset + 0x18, 0.55)
    put_f32(data_block, common_offset + 0x20, 0.7853982)
    put_f32(data_block, common_offset + 0x24, 0.21)
    put_f32(data_block, common_offset + 0x28, 0.31)
    put_f32(data_block, common_offset + 0x2C, 0.52)
    put_f32(data_block, common_offset + 0x30, 0.74)
    put_f32(data_block, common_offset + 0x34, 0.24)
    put_f32(data_block, common_offset + 0x3C, 0.82)
    put_i32(data_block, common_offset + 0x40, 5)
    put_f32(data_block, common_offset + 0x44, 6.0)
    put_f32(data_block, common_offset + 0x48, 7.0)
    put_f32(data_block, common_offset + 0x4C, 16.0)
    put_f32(data_block, common_offset + 0x58, 0.66)
    put_f32(data_block, common_offset + 0x68, 4.0)
    put_f32(data_block, common_offset + 0x70, 0.81)
    put_i32(data_block, common_offset + 0x74, 4)
    put_f32(data_block, common_offset + 0x78, 0.22)
    put_f32(data_block, common_offset + 0x7C, 0.41)
    put_f32(data_block, common_offset + 0x88, 0.83)
    put_i32(data_block, common_offset + 0x8C, 2)
    put_f32(data_block, common_offset + 0x90, 0.35)
    put_f32(data_block, common_offset + 0x94, 0.28)
    put_f32(data_block, common_offset + 0x98, 0.25)
    put_f32(data_block, common_offset + 0xAC, 0.26)
    put_f32(data_block, common_offset + 0xDC, 0.43)
    put_f32(data_block, common_offset + 0xE0, 0.44)
    put_f32(data_block, common_offset + 0x25C, -0.62)
    put_f32(data_block, common_offset + 0x314, 0.84)
    put_i32(data_block, common_offset + 0x318, 3)
    put_f32(data_block, common_offset + 0x31C, 0.85)
    put_i32(data_block, common_offset + 0x320, 4)
    put_f32(data_block, common_offset + 0x32C, 0.20)
    put_f32(data_block, common_offset + 0x330, 0.25)
    put_i32(data_block, common_offset + 0x334, 15)
    put_f32(data_block, common_offset + 0x338, 3.1)
    put_f32(data_block, common_offset + 0x33C, 0.915)
    put_f32(data_block, common_offset + 0x344, 10.0)
    put_f32(data_block, common_offset + 0x430, 2.0)
    put_f32(data_block, common_offset + 0x464, 0.63)
    put_f32(data_block, common_offset + 0x468, 5.0)
    put_f32(data_block, common_offset + 0x46C, -1.25)
    put_f32(data_block, common_offset + 0x470, 6.0)

    dat = make_dat("ftLoadCommonData", data_block, ftload_offset)
    extracted = extract_common_data_from_plco(
        dat, source_path=PROJECT_ROOT / "resources" / "melee" / "raw" / "PlCo.dat"
    )

    assert extracted["source"]["symbol"] == "ftLoadCommonData"
    assert extracted["source"]["file"] == "resources/melee/raw/PlCo.dat"
    assert extracted["source"]["common_attributes_offset"] == common_offset
    assert extracted["fields"]["escapeair_force"]["milli"] == 3100
    assert extracted["fields"]["escapeair_decay_milli"]["milli"] == 915
    assert extracted["fields"]["escapeair_landing_lag_ticks"]["ticks"] == 10


def test_extract_captain_profile_from_plca_uses_ftdata_attribute_range():
    data_block = bytearray(0x400)
    ftdata_offset = 0x60
    attrs_offset = 0x100
    attrs_end = 0x280
    data_block[ftdata_offset : ftdata_offset + 4] = be32(attrs_offset)
    data_block[ftdata_offset + 4 : ftdata_offset + 8] = be32(attrs_end)

    put_f32(data_block, attrs_offset + 0x18, 0.08)
    put_f32(data_block, attrs_offset + 0x1C, 2.0)
    put_f32(data_block, attrs_offset + 0x30, 8.0)
    put_f32(data_block, attrs_offset + 0x34, 2.35)
    put_f32(data_block, attrs_offset + 0x3C, 0.4)
    put_f32(data_block, attrs_offset + 0x40, 3.1)
    put_f32(data_block, attrs_offset + 0x5C, 0.13)
    put_f32(data_block, attrs_offset + 0x60, 2.9)
    put_f32(data_block, attrs_offset + 0x64, 0.04)
    put_f32(data_block, attrs_offset + 0x68, 0.02)
    put_f32(data_block, attrs_offset + 0x6C, 1.12)
    put_f32(data_block, attrs_offset + 0x70, 0.01)

    dat = make_dat("ftDataCaptain", data_block, ftdata_offset)
    extracted = extract_captain_profile_from_plca(dat, source_path=Path("PlCa.dat"))

    assert extracted["source"]["symbol"] == "ftDataCaptain"
    assert extracted["source"]["ftco_dat_attrs_offset"] == attrs_offset
    assert extracted["source"]["ftco_dat_attrs_len"] == attrs_end - attrs_offset
    assert extracted["fields"]["traction_per_tick"]["milli"] == 80
    assert extracted["fields"]["dash_initial_velocity"]["milli"] == 2000
    assert extracted["fields"]["max_run_brake_frames"]["ticks"] == 8
    assert extracted["fields"]["ground_max_horizontal_velocity"]["milli"] == 2350
    assert extracted["fields"]["air_drift_max"]["milli"] == 1120


def test_extract_captain_profile_rejects_non_captain_roots():
    data_block = bytearray(0x400)
    dat = make_dat("ftDataFox", data_block, 0x60)

    try:
        extract_captain_profile_from_plca(dat, source_path=Path("PlCa.dat"))
    except DatExtractError as error:
        assert "ftDataCaptain" in str(error)
    else:
        raise AssertionError("expected non-Captain DAT root to be rejected")
