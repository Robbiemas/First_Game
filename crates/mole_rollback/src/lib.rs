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
}

#[derive(Debug, Clone)]
struct AuthoritativeFrameRecord {
    frame: Frame,
    pre_frame_world: WorldRollbackSnapshot,
    inputs: [PlayerInput; 2],
    input_confirmed: [bool; 2],
    post_frame_checksum: u64,
}

#[derive(Debug, Clone)]
struct AuthoritativeFrameRing {
    entries: Vec<Option<AuthoritativeFrameRecord>>,
}

impl AuthoritativeFrameRing {
    fn new(capacity: usize) -> Self {
        assert!(
            capacity > 0,
            "frame record capacity must be greater than zero"
        );
        Self {
            entries: vec![None; capacity],
        }
    }

    fn get(&self, frame: Frame) -> Option<&AuthoritativeFrameRecord> {
        let index = frame.0 as usize % self.entries.len();
        self.entries[index]
            .as_ref()
            .filter(|record| record.frame == frame)
    }

    fn save(
        &mut self,
        frame: Frame,
        pre_frame_world: WorldRollbackSnapshot,
        inputs: [PlayerInput; 2],
        input_confirmed: [bool; 2],
        post_frame_checksum: u64,
    ) {
        let index = frame.0 as usize % self.entries.len();
        self.entries[index] = Some(AuthoritativeFrameRecord {
            frame,
            pre_frame_world,
            inputs,
            input_confirmed,
            post_frame_checksum,
        });
    }

    fn restore_pre_frame(&self, frame: Frame, world: &mut World) -> bool {
        let Some(record) = self.get(frame) else {
            return false;
        };
        world.restore_rollback_snapshot(&record.pre_frame_world);
        true
    }

    fn retained_count(&self) -> usize {
        self.entries.iter().flatten().count()
    }

    fn capacity(&self) -> usize {
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
    frame_records: AuthoritativeFrameRing,
    inputs: BTreeMap<Frame, [PlayerInput; 2]>,
    step_world_fn: StepWorldFn,
    finalized_through: Option<Frame>,
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
            frame_records: AuthoritativeFrameRing::new(snapshot_capacity),
            inputs: BTreeMap::new(),
            step_world_fn,
            finalized_through: None,
        }
    }

    pub const fn world(&self) -> &World {
        &self.world
    }

    pub fn advance(&mut self, frame: Frame, inputs: [PlayerInput; 2]) {
        self.advance_recorded(frame, inputs, [true; 2]);
    }

    fn advance_recorded(
        &mut self,
        frame: Frame,
        inputs: [PlayerInput; 2],
        input_confirmed: [bool; 2],
    ) {
        let pre_frame_world = self.world.rollback_snapshot();
        self.inputs.insert(frame, inputs);
        self.prune_unrecoverable_inputs(frame);
        (self.step_world_fn)(&mut self.world, frame, &inputs);
        self.frame_records.save(
            frame,
            pre_frame_world,
            inputs,
            input_confirmed,
            self.world.checksum(),
        );
        self.advance_finalized_frontier();
    }

    pub fn retained_input_frame_count(&self) -> usize {
        self.inputs.len()
    }

    pub fn retained_frame_record_count(&self) -> usize {
        self.frame_records.retained_count()
    }

    pub fn retained_frame_range(&self) -> Option<(Frame, Frame)> {
        let mut frames = self
            .frame_records
            .entries
            .iter()
            .flatten()
            .map(|record| record.frame);
        let first = frames.next()?;
        Some(frames.fold((first, first), |(oldest, newest), frame| {
            (oldest.min(frame), newest.max(frame))
        }))
    }

    pub fn historical_checksum(&self, frame: Frame) -> Option<u64> {
        if self
            .finalized_through
            .is_none_or(|finalized| frame > finalized)
        {
            return None;
        }
        self.frame_records
            .get(frame)
            .map(|record| record.post_frame_checksum)
    }

    pub fn latest_finalized_checksum(&self) -> Option<(Frame, u64)> {
        let frame = self.finalized_through?;
        self.frame_records
            .get(frame)
            .map(|record| (frame, record.post_frame_checksum))
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
        self.advance_recorded(frame, resolved, [inputs[0].is_some(), inputs[1].is_some()]);
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
            .frame_records
            .get(frame)
            .map(|record| record.inputs)
            .unwrap_or([PlayerInput::neutral(), PlayerInput::neutral()]);

        if corrected_inputs[player_index] == input {
            if let Some(record) = self.frame_record_mut(frame) {
                record.input_confirmed[player_index] = true;
            }
            self.advance_finalized_frontier();
            return false;
        }

        corrected_inputs[player_index] = input;
        let mut confirmed = self
            .frame_records
            .get(frame)
            .map(|record| record.input_confirmed)
            .unwrap_or([false; 2]);
        confirmed[player_index] = true;
        self.try_correct_and_resimulate_with_confirmation(
            frame,
            corrected_inputs,
            confirmed,
            current_frame,
        )
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
        self.try_correct_and_resimulate_with_confirmation(
            corrected_frame,
            corrected_inputs,
            [true; 2],
            current_frame,
        )
    }

    fn try_correct_and_resimulate_with_confirmation(
        &mut self,
        corrected_frame: Frame,
        corrected_inputs: [PlayerInput; 2],
        corrected_confirmation: [bool; 2],
        current_frame: Frame,
    ) -> bool {
        if current_frame.0 <= corrected_frame.0 {
            return false;
        }

        let mut interval = Vec::with_capacity((current_frame.0 - corrected_frame.0) as usize);
        for frame_number in corrected_frame.0..current_frame.0 {
            let frame = Frame(frame_number);
            let Some(record) = self.frame_records.get(frame) else {
                return false;
            };
            if frame == corrected_frame {
                interval.push((frame, corrected_inputs, corrected_confirmation));
            } else {
                interval.push((frame, record.inputs, record.input_confirmed));
            }
        }

        if !self
            .frame_records
            .restore_pre_frame(corrected_frame, &mut self.world)
        {
            return false;
        }

        for (frame, inputs, input_confirmed) in interval {
            self.inputs.insert(frame, inputs);
            let pre_frame_world = self.world.rollback_snapshot();
            (self.step_world_fn)(&mut self.world, frame, &inputs);
            self.frame_records.save(
                frame,
                pre_frame_world,
                inputs,
                input_confirmed,
                self.world.checksum(),
            );
        }
        self.advance_finalized_frontier();
        true
    }

    fn advance_finalized_frontier(&mut self) {
        let mut next = self
            .finalized_through
            .map(|frame| Frame(frame.0.saturating_add(1)))
            .unwrap_or(Frame(0));
        while self
            .frame_records
            .get(next)
            .is_some_and(|record| record.input_confirmed.iter().all(|confirmed| *confirmed))
        {
            self.finalized_through = Some(next);
            next = Frame(next.0.saturating_add(1));
        }
    }

    fn frame_record_mut(&mut self, frame: Frame) -> Option<&mut AuthoritativeFrameRecord> {
        let index = frame.0 as usize % self.frame_records.entries.len();
        self.frame_records.entries[index]
            .as_mut()
            .filter(|record| record.frame == frame)
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
        let retained_frames = self.frame_records.capacity().saturating_sub(1) as u32;
        let oldest_recoverable = Frame(newest_frame.0.saturating_sub(retained_frames));
        self.inputs = self.inputs.split_off(&oldest_recoverable);
    }
}
