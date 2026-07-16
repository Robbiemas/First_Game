use std::collections::BTreeMap;

use mole_core::{step_world, Frame, PlayerInput, World, WorldRollbackSnapshot};

pub type StepWorldFn = fn(&mut World, Frame, &[PlayerInput; 2]);

#[derive(Debug, Clone)]
struct Snapshot {
    frame: Frame,
    world: WorldRollbackSnapshot,
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
            world: world.rollback_snapshot(),
        });
    }

    pub fn restore(&self, frame: Frame, world: &mut World) -> bool {
        let index = frame.0 as usize % self.entries.len();
        let Some(snapshot) = self.entries[index]
            .as_ref()
            .filter(|snapshot| snapshot.frame == frame)
        else {
            return false;
        };
        world.restore_rollback_snapshot(&snapshot.world);
        true
    }

    const fn capacity(&self) -> usize {
        self.entries.len()
    }
}

#[derive(Debug, Clone)]
pub struct InputDelayBuffer {
    delay_frames: u32,
    inputs: BTreeMap<Frame, PlayerInput>,
}

impl InputDelayBuffer {
    pub const fn new(delay_frames: u32) -> Self {
        Self {
            delay_frames,
            inputs: BTreeMap::new(),
        }
    }

    pub const fn delay_frames(&self) -> u32 {
        self.delay_frames
    }

    pub fn push_and_get_committed(&mut self, frame: Frame, input: PlayerInput) -> PlayerInput {
        self.inputs.insert(frame, input);
        let Some(committed_frame) = frame.0.checked_sub(self.delay_frames).map(Frame) else {
            return PlayerInput::neutral();
        };
        let committed = self
            .inputs
            .remove(&committed_frame)
            .unwrap_or(PlayerInput::neutral());
        self.drop_inputs_before(committed_frame);
        committed
    }

    fn drop_inputs_before(&mut self, frame: Frame) {
        self.inputs = self.inputs.split_off(&frame);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlippiDelayedInput {
    pub scheduled_frame: Frame,
    pub scheduled_input: PlayerInput,
    pub current_frame_input: PlayerInput,
}

#[derive(Debug, Clone)]
pub struct SlippiInputDelayBuffer {
    delay_frames: u32,
    inputs: BTreeMap<Frame, PlayerInput>,
}

impl SlippiInputDelayBuffer {
    pub const fn new(delay_frames: u32) -> Self {
        Self {
            delay_frames,
            inputs: BTreeMap::new(),
        }
    }

    pub const fn delay_frames(&self) -> u32 {
        self.delay_frames
    }

    pub fn push_physical_input(&mut self, frame: Frame, input: PlayerInput) -> SlippiDelayedInput {
        let scheduled_frame = Frame(frame.0.saturating_add(self.delay_frames));
        self.inputs.insert(scheduled_frame, input);
        let current_frame_input = self.input_for_frame(frame);
        SlippiDelayedInput {
            scheduled_frame,
            scheduled_input: input,
            current_frame_input,
        }
    }

    fn input_for_frame(&mut self, frame: Frame) -> PlayerInput {
        let input = self.inputs.remove(&frame).unwrap_or(PlayerInput::neutral());
        self.drop_inputs_before(frame);
        input
    }

    fn drop_inputs_before(&mut self, frame: Frame) {
        self.inputs = self.inputs.split_off(&frame);
    }
}

#[derive(Debug, Clone)]
pub struct RollbackSession {
    world: World,
    snapshots: SnapshotBuffer,
    inputs: BTreeMap<Frame, [PlayerInput; 2]>,
    step_world_fn: StepWorldFn,
}

impl RollbackSession {
    pub fn new(initial: World, snapshot_capacity: usize) -> Self {
        Self::new_with_step(initial, snapshot_capacity, step_world)
    }

    pub fn new_with_step(
        initial: World,
        snapshot_capacity: usize,
        step_world_fn: StepWorldFn,
    ) -> Self {
        Self {
            world: initial,
            snapshots: SnapshotBuffer::new(snapshot_capacity),
            inputs: BTreeMap::new(),
            step_world_fn,
        }
    }

    pub const fn world(&self) -> &World {
        &self.world
    }

    pub fn advance(&mut self, frame: Frame, inputs: [PlayerInput; 2]) {
        self.snapshots.save(frame, &self.world);
        self.inputs.insert(frame, inputs);
        self.prune_unrecoverable_inputs(frame);
        (self.step_world_fn)(&mut self.world, frame, &inputs);
    }

    pub fn retained_input_frame_count(&self) -> usize {
        self.inputs.len()
    }

    pub fn advance_with_prediction(
        &mut self,
        frame: Frame,
        inputs: [Option<PlayerInput>; 2],
    ) -> [PlayerInput; 2] {
        let resolved = [
            inputs[0].unwrap_or_else(|| self.predict_input(frame, 0)),
            inputs[1].unwrap_or_else(|| self.predict_input(frame, 1)),
        ];
        self.advance(frame, resolved);
        resolved
    }

    pub fn confirm_input(
        &mut self,
        frame: Frame,
        player_index: usize,
        input: PlayerInput,
        current_frame: Frame,
    ) -> bool {
        assert!(player_index < 2, "player index out of range");
        let mut corrected_inputs = self
            .inputs
            .get(&frame)
            .copied()
            .unwrap_or([PlayerInput::neutral(), PlayerInput::neutral()]);

        if corrected_inputs[player_index] == input {
            return false;
        }

        corrected_inputs[player_index] = input;
        self.try_correct_and_resimulate(frame, corrected_inputs, current_frame)
    }

    pub fn correct_and_resimulate(
        &mut self,
        corrected_frame: Frame,
        corrected_inputs: [PlayerInput; 2],
        current_frame: Frame,
    ) {
        assert!(
            self.try_correct_and_resimulate(corrected_frame, corrected_inputs, current_frame),
            "cannot resimulate without a saved snapshot for the corrected frame"
        );
    }

    pub fn try_correct_and_resimulate(
        &mut self,
        corrected_frame: Frame,
        corrected_inputs: [PlayerInput; 2],
        current_frame: Frame,
    ) -> bool {
        if !self.snapshots.restore(corrected_frame, &mut self.world) {
            return false;
        }
        self.inputs.insert(corrected_frame, corrected_inputs);

        for frame_number in corrected_frame.0..current_frame.0 {
            let frame = Frame(frame_number);
            let inputs = self
                .inputs
                .get(&frame)
                .copied()
                .unwrap_or([PlayerInput::neutral(), PlayerInput::neutral()]);
            self.snapshots.save(frame, &self.world);
            (self.step_world_fn)(&mut self.world, frame, &inputs);
        }
        true
    }

    fn predict_input(&self, frame: Frame, player_index: usize) -> PlayerInput {
        if frame.0 == 0 {
            return PlayerInput::neutral();
        }

        self.inputs
            .get(&Frame(frame.0 - 1))
            .map(|inputs| inputs[player_index])
            .unwrap_or(PlayerInput::neutral())
    }

    fn prune_unrecoverable_inputs(&mut self, newest_frame: Frame) {
        let retained_frames = self.snapshots.capacity().saturating_sub(1) as u32;
        let oldest_recoverable = Frame(newest_frame.0.saturating_sub(retained_frames));
        self.inputs = self.inputs.split_off(&oldest_recoverable);
    }
}
