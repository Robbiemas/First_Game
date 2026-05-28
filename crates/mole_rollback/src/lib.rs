use std::collections::BTreeMap;

use mole_core::{step_world, Frame, PlayerInput, World};

#[derive(Debug, Clone)]
struct Snapshot {
    frame: Frame,
    world: World,
}

#[derive(Debug, Clone)]
pub struct SnapshotBuffer {
    entries: Vec<Option<Snapshot>>,
}

impl SnapshotBuffer {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "snapshot capacity must be greater than zero");
        Self {
            entries: vec![None; capacity],
        }
    }

    pub fn save(&mut self, frame: Frame, world: &World) {
        let index = frame.0 as usize % self.entries.len();
        self.entries[index] = Some(Snapshot {
            frame,
            world: world.clone(),
        });
    }

    pub fn load(&self, frame: Frame) -> Option<World> {
        let index = frame.0 as usize % self.entries.len();
        self.entries[index]
            .as_ref()
            .filter(|snapshot| snapshot.frame == frame)
            .map(|snapshot| snapshot.world.clone())
    }
}

#[derive(Debug, Clone)]
pub struct RollbackSession {
    world: World,
    snapshots: SnapshotBuffer,
    inputs: BTreeMap<Frame, [PlayerInput; 2]>,
}

impl RollbackSession {
    pub fn new(initial: World, snapshot_capacity: usize) -> Self {
        Self {
            world: initial,
            snapshots: SnapshotBuffer::new(snapshot_capacity),
            inputs: BTreeMap::new(),
        }
    }

    pub const fn world(&self) -> &World {
        &self.world
    }

    pub fn advance(&mut self, frame: Frame, inputs: [PlayerInput; 2]) {
        self.snapshots.save(frame, &self.world);
        self.inputs.insert(frame, inputs);
        step_world(&mut self.world, frame, &inputs);
    }

    pub fn correct_and_resimulate(
        &mut self,
        corrected_frame: Frame,
        corrected_inputs: [PlayerInput; 2],
        current_frame: Frame,
    ) {
        self.inputs.insert(corrected_frame, corrected_inputs);
        self.world = self
            .snapshots
            .load(corrected_frame)
            .expect("cannot resimulate without a saved snapshot for the corrected frame");

        for frame_number in corrected_frame.0..current_frame.0 {
            let frame = Frame(frame_number);
            let inputs = self
                .inputs
                .get(&frame)
                .copied()
                .unwrap_or([PlayerInput::neutral(), PlayerInput::neutral()]);
            step_world(&mut self.world, frame, &inputs);
        }
    }
}
