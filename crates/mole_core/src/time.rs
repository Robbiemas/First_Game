#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Frame(pub u32);

pub const TICK_RATE_HZ: u32 = 60;
pub const TICK_NANOS: u64 = 1_000_000_000 / TICK_RATE_HZ as u64;

impl Frame {
    pub const fn next(self) -> Self {
        Self(self.0 + 1)
    }
}
