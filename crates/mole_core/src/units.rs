pub const MELEE_UNIT_SCALE: i32 = 1_000;

pub fn melee_units(value: f32) -> i32 {
    melee_units_f32(value)
}

pub fn melee_units_f32(value: f32) -> i32 {
    (value * MELEE_UNIT_SCALE as f32).round() as i32
}

pub fn source_units_to_milli(value: f32) -> i32 {
    melee_units_f32(value)
}

pub fn milli_to_source_units(value: i32) -> f32 {
    value as f32 / MELEE_UNIT_SCALE as f32
}
