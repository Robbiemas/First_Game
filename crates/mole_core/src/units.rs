pub const MELEE_UNIT_SCALE: i32 = 1_000;

pub fn melee_units(value: f32) -> i32 {
    melee_units_f32(value)
}

pub fn melee_units_f32(value: f32) -> i32 {
    (value * MELEE_UNIT_SCALE as f32).round() as i32
}
