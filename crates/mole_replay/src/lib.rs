use mole_core::{step_world, Frame, PlayerInput, World};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReplayFrame {
    pub frame: Frame,
    pub inputs: [PlayerInput; 2],
    pub checksum: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayError {
    ChecksumMismatch {
        frame: Frame,
        expected: u64,
        actual: u64,
    },
}

#[derive(Debug, Clone)]
pub struct ReplayLog {
    initial: World,
    frames: Vec<ReplayFrame>,
}

impl ReplayLog {
    pub fn new(initial: World) -> Self {
        Self {
            initial,
            frames: Vec::new(),
        }
    }

    pub fn push(&mut self, frame: ReplayFrame) {
        self.frames.push(frame);
    }

    pub fn frames(&self) -> &[ReplayFrame] {
        &self.frames
    }

    pub fn validate(&self) -> Result<(), ReplayError> {
        let mut world = self.initial.clone();
        for frame in &self.frames {
            step_world(&mut world, frame.frame, &frame.inputs);
            let actual = world.checksum();
            if actual != frame.checksum {
                return Err(ReplayError::ChecksumMismatch {
                    frame: frame.frame,
                    expected: frame.checksum,
                    actual,
                });
            }
        }
        Ok(())
    }
}
