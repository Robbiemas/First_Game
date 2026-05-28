use mole_core::MotionState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyAnimationKey {
    Standing,
    Running,
    Dashing,
    Walking,
    Air,
    LandingLag,
    AirDodge,
    JumpSquat,
    FreeFall,
    Turning,
    RunTurn,
    Blocking,
    Shield,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegacyAnimationSpec {
    pub key: LegacyAnimationKey,
    pub directory: &'static str,
    pub frames: &'static [&'static str],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegacySpriteCue {
    pub animation: LegacyAnimationKey,
    pub directory: &'static str,
    pub frame: &'static str,
    pub flip_x: bool,
}

pub const LEGACY_DOLPHIN_MOLE_ANIMATIONS: &[LegacyAnimationSpec] = &[
    LegacyAnimationSpec {
        key: LegacyAnimationKey::Standing,
        directory: "DolphinMole/standing",
        frames: &["Standing1.png", "Standing2.png"],
    },
    LegacyAnimationSpec {
        key: LegacyAnimationKey::Running,
        directory: "DolphinMole/running",
        frames: &["running1 - Copy.png", "running1.png"],
    },
    LegacyAnimationSpec {
        key: LegacyAnimationKey::Dashing,
        directory: "DolphinMole/dashing",
        frames: &["running1 - Copy.png", "running1.png"],
    },
    LegacyAnimationSpec {
        key: LegacyAnimationKey::Walking,
        directory: "DolphinMole/walking",
        frames: &["Standing1.png", "Standing2.png"],
    },
    LegacyAnimationSpec {
        key: LegacyAnimationKey::Air,
        directory: "DolphinMole/air",
        frames: &["Air1 - Copy.png", "Air1.png"],
    },
    LegacyAnimationSpec {
        key: LegacyAnimationKey::LandingLag,
        directory: "DolphinMole/landingLag",
        frames: &["running1 - Copy.png", "running1.png"],
    },
    LegacyAnimationSpec {
        key: LegacyAnimationKey::AirDodge,
        directory: "DolphinMole/airDodge",
        frames: &["running1 - Copy.png"],
    },
    LegacyAnimationSpec {
        key: LegacyAnimationKey::JumpSquat,
        directory: "DolphinMole/jumpSquat",
        frames: &["Standing1.png", "Standing2.png"],
    },
    LegacyAnimationSpec {
        key: LegacyAnimationKey::FreeFall,
        directory: "DolphinMole/freeFall",
        frames: &["running1 - Copy.png"],
    },
    LegacyAnimationSpec {
        key: LegacyAnimationKey::Turning,
        directory: "DolphinMole/turning",
        frames: &["Standing1.png"],
    },
    LegacyAnimationSpec {
        key: LegacyAnimationKey::RunTurn,
        directory: "DolphinMole/runTurn",
        frames: &["Standing1.png"],
    },
    LegacyAnimationSpec {
        key: LegacyAnimationKey::Blocking,
        directory: "DolphinMole/blocking",
        frames: &["Standing1.png"],
    },
    LegacyAnimationSpec {
        key: LegacyAnimationKey::Shield,
        directory: "DolphinMole/shield",
        frames: &["shield.png"],
    },
];

impl LegacySpriteCue {
    pub fn for_player(motion_state: MotionState, state_frame: u8, facing: i8) -> Self {
        let animation = legacy_animation_for_motion_state(motion_state);
        let spec = legacy_animation_spec(animation);
        let frame_index = state_frame as usize % spec.frames.len();

        Self {
            animation,
            directory: spec.directory,
            frame: spec.frames[frame_index],
            flip_x: facing < 0,
        }
    }
}

pub fn legacy_animation_for_motion_state(motion_state: MotionState) -> LegacyAnimationKey {
    match motion_state {
        MotionState::Wait
        | MotionState::Attack1
        | MotionState::AttackS3
        | MotionState::AttackHi3
        | MotionState::AttackLw3
        | MotionState::AttackS4
        | MotionState::AttackHi4
        | MotionState::AttackLw4
        | MotionState::SpecialN
        | MotionState::SpecialS
        | MotionState::SpecialHi
        | MotionState::SpecialLw
        | MotionState::Catch
        | MotionState::CatchDash
        | MotionState::Squat => LegacyAnimationKey::Standing,
        MotionState::WalkSlow | MotionState::WalkMiddle | MotionState::WalkFast => {
            LegacyAnimationKey::Walking
        }
        MotionState::Dash => LegacyAnimationKey::Dashing,
        MotionState::Run | MotionState::RunBrake | MotionState::AttackDash => {
            LegacyAnimationKey::Running
        }
        MotionState::Turn => LegacyAnimationKey::Turning,
        MotionState::TurnRun => LegacyAnimationKey::RunTurn,
        MotionState::KneeBend => LegacyAnimationKey::JumpSquat,
        MotionState::JumpF
        | MotionState::JumpB
        | MotionState::JumpAerialF
        | MotionState::JumpAerialB
        | MotionState::Air
        | MotionState::AttackAirN
        | MotionState::AttackAirF
        | MotionState::AttackAirB
        | MotionState::AttackAirHi
        | MotionState::AttackAirLw
        | MotionState::SpecialAirN
        | MotionState::SpecialAirS
        | MotionState::SpecialAirHi
        | MotionState::SpecialAirLw => LegacyAnimationKey::Air,
        MotionState::GuardOn | MotionState::Guard | MotionState::GuardOff => {
            LegacyAnimationKey::Blocking
        }
        MotionState::EscapeN | MotionState::EscapeF | MotionState::EscapeB => {
            LegacyAnimationKey::Dashing
        }
        MotionState::EscapeAir => LegacyAnimationKey::AirDodge,
        MotionState::FallSpecial => LegacyAnimationKey::FreeFall,
        MotionState::Landing | MotionState::LandingFallSpecial => LegacyAnimationKey::LandingLag,
    }
}

pub fn legacy_animation_spec(key: LegacyAnimationKey) -> &'static LegacyAnimationSpec {
    LEGACY_DOLPHIN_MOLE_ANIMATIONS
        .iter()
        .find(|spec| spec.key == key)
        .expect("legacy animation key is covered by manifest")
}
