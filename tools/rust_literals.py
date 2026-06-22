"""Rust literal rendering helpers for generated source data."""

from __future__ import annotations

import struct
from typing import Any


def render_f32_bits(value: Any) -> str:
    bits = struct.unpack(">I", struct.pack(">f", float(value)))[0]
    return f"f32::from_bits(0x{bits:08x})"
