// Generated compact Melee source frame-data export.
// Runtime loads the CLI-baked action/frame capsule and DownBound hip-pose sidecar.
// Gameplay, rendering, collision, and player startup must not sample source FigaTrees.

// Canonical Melee action-state bindings may have no Rust MotionState alias.
use mole_core::{MeleeActionStateId, MotionState};
pub(crate) const SOURCE_ARTIFACT_KIND: &str = "runtime_source_frame_data";
pub(crate) const SOURCE_FRAME_CAPSULES_BYTES: &[u8] = include_bytes!("source_frame_data/source_frame_capsules.bin");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeActionBinding {
    pub(crate) action_state_id: MeleeActionStateId,
    pub(crate) source_action_key: &'static str,
    pub(crate) motion_state: Option<MotionState>,
}

pub(crate) const ACTION_BINDINGS: &[RuntimeActionBinding] = &[
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(14), source_action_key: "Wait1", motion_state: Some(MotionState::Wait) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(15), source_action_key: "WalkSlow", motion_state: Some(MotionState::WalkSlow) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(16), source_action_key: "WalkMiddle", motion_state: Some(MotionState::WalkMiddle) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(17), source_action_key: "WalkFast", motion_state: Some(MotionState::WalkFast) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(18), source_action_key: "Turn", motion_state: Some(MotionState::Turn) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(19), source_action_key: "TurnRun", motion_state: Some(MotionState::TurnRun) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(20), source_action_key: "Dash", motion_state: Some(MotionState::Dash) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(21), source_action_key: "Run", motion_state: Some(MotionState::Run) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(23), source_action_key: "RunBrake", motion_state: Some(MotionState::RunBrake) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(42), source_action_key: "Landing", motion_state: Some(MotionState::Landing) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(25), source_action_key: "JumpF", motion_state: Some(MotionState::JumpF) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(26), source_action_key: "JumpB", motion_state: Some(MotionState::JumpB) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(27), source_action_key: "JumpAerialF", motion_state: Some(MotionState::JumpAerialF) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(28), source_action_key: "JumpAerialB", motion_state: Some(MotionState::JumpAerialB) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(29), source_action_key: "Fall", motion_state: Some(MotionState::Fall) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(30), source_action_key: "FallF", motion_state: Some(MotionState::FallF) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(31), source_action_key: "FallB", motion_state: Some(MotionState::FallB) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(32), source_action_key: "FallAerial", motion_state: Some(MotionState::FallAerial) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(33), source_action_key: "FallAerialF", motion_state: Some(MotionState::FallAerialF) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(34), source_action_key: "FallAerialB", motion_state: Some(MotionState::FallAerialB) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(35), source_action_key: "FallSpecial", motion_state: Some(MotionState::FallSpecial) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(36), source_action_key: "FallSpecialF", motion_state: Some(MotionState::FallSpecialF) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(37), source_action_key: "FallSpecialB", motion_state: Some(MotionState::FallSpecialB) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(39), source_action_key: "Squat", motion_state: Some(MotionState::Squat) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(40), source_action_key: "SquatWait", motion_state: Some(MotionState::SquatWait) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(41), source_action_key: "SquatRv", motion_state: Some(MotionState::SquatRv) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(178), source_action_key: "GuardOn", motion_state: Some(MotionState::GuardOn) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(179), source_action_key: "Guard", motion_state: Some(MotionState::Guard) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(180), source_action_key: "GuardOff", motion_state: Some(MotionState::GuardOff) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(235), source_action_key: "EscapeN", motion_state: Some(MotionState::EscapeN) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(233), source_action_key: "EscapeF", motion_state: Some(MotionState::EscapeF) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(234), source_action_key: "EscapeB", motion_state: Some(MotionState::EscapeB) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(236), source_action_key: "EscapeAir", motion_state: Some(MotionState::EscapeAir) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(44), source_action_key: "Attack11", motion_state: Some(MotionState::Attack1) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(50), source_action_key: "AttackDash", motion_state: Some(MotionState::AttackDash) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(53), source_action_key: "AttackS3S", motion_state: Some(MotionState::AttackS3) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(56), source_action_key: "AttackHi3", motion_state: Some(MotionState::AttackHi3) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(57), source_action_key: "AttackLw3", motion_state: Some(MotionState::AttackLw3) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(60), source_action_key: "AttackS4S", motion_state: Some(MotionState::AttackS4) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(63), source_action_key: "AttackHi4", motion_state: Some(MotionState::AttackHi4) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(64), source_action_key: "AttackLw4", motion_state: Some(MotionState::AttackLw4) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(65), source_action_key: "AttackAirN", motion_state: Some(MotionState::AttackAirN) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(66), source_action_key: "AttackAirF", motion_state: Some(MotionState::AttackAirF) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(67), source_action_key: "AttackAirB", motion_state: Some(MotionState::AttackAirB) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(68), source_action_key: "AttackAirHi", motion_state: Some(MotionState::AttackAirHi) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(69), source_action_key: "AttackAirLw", motion_state: Some(MotionState::AttackAirLw) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(70), source_action_key: "LandingAirN", motion_state: Some(MotionState::LandingAirN) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(71), source_action_key: "LandingAirF", motion_state: Some(MotionState::LandingAirF) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(72), source_action_key: "LandingAirB", motion_state: Some(MotionState::LandingAirB) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(73), source_action_key: "LandingAirHi", motion_state: Some(MotionState::LandingAirHi) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(74), source_action_key: "LandingAirLw", motion_state: Some(MotionState::LandingAirLw) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(244), source_action_key: "Pass", motion_state: Some(MotionState::Pass) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(322), source_action_key: "Entry", motion_state: Some(MotionState::Entry) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(212), source_action_key: "Catch", motion_state: Some(MotionState::Catch) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(214), source_action_key: "CatchDash", motion_state: Some(MotionState::CatchDash) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(347), source_action_key: "SpecialN", motion_state: Some(MotionState::SpecialN) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(348), source_action_key: "SpecialAirN", motion_state: Some(MotionState::SpecialAirN) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(349), source_action_key: "SpecialSStart", motion_state: Some(MotionState::SpecialSStart) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(350), source_action_key: "SpecialS", motion_state: Some(MotionState::SpecialS) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(351), source_action_key: "SpecialAirSStart", motion_state: Some(MotionState::SpecialAirSStart) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(352), source_action_key: "SpecialAirS", motion_state: Some(MotionState::SpecialAirS) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(353), source_action_key: "SpecialHi", motion_state: Some(MotionState::SpecialHi) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(354), source_action_key: "SpecialAirHi", motion_state: Some(MotionState::SpecialAirHi) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(357), source_action_key: "SpecialLw", motion_state: Some(MotionState::SpecialLw) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(359), source_action_key: "SpecialAirLw", motion_state: Some(MotionState::SpecialAirLw) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(22), source_action_key: "Run", motion_state: Some(MotionState::RunDirect) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(24), source_action_key: "Landing", motion_state: Some(MotionState::KneeBend) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(181), source_action_key: "GuardDamage", motion_state: Some(MotionState::GuardSetOff) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(182), source_action_key: "GuardOn", motion_state: Some(MotionState::GuardReflect) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(43), source_action_key: "Landing", motion_state: Some(MotionState::LandingFallSpecial) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(323), source_action_key: "Entry", motion_state: Some(MotionState::EntryStart) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(324), source_action_key: "Entry", motion_state: Some(MotionState::EntryEnd) },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(45), source_action_key: "Attack12", motion_state: None },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(46), source_action_key: "Attack13", motion_state: None },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(47), source_action_key: "Attack100Start", motion_state: None },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(48), source_action_key: "Attack100Loop", motion_state: None },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(49), source_action_key: "Attack100End", motion_state: None },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(75), source_action_key: "DamageHi1", motion_state: None },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(76), source_action_key: "DamageHi2", motion_state: None },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(77), source_action_key: "DamageHi3", motion_state: None },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(78), source_action_key: "DamageN1", motion_state: None },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(79), source_action_key: "DamageN2", motion_state: None },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(80), source_action_key: "DamageN3", motion_state: None },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(81), source_action_key: "DamageLw1", motion_state: None },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(82), source_action_key: "DamageLw2", motion_state: None },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(83), source_action_key: "DamageLw3", motion_state: None },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(84), source_action_key: "DamageAir1", motion_state: None },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(85), source_action_key: "DamageAir2", motion_state: None },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(86), source_action_key: "DamageAir3", motion_state: None },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(87), source_action_key: "DamageFlyHi", motion_state: None },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(88), source_action_key: "DamageFlyN", motion_state: None },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(89), source_action_key: "DamageFlyLw", motion_state: None },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(90), source_action_key: "DamageFlyTop", motion_state: None },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(91), source_action_key: "DamageFlyRoll", motion_state: None },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(183), source_action_key: "DownBoundU", motion_state: None },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(184), source_action_key: "DownWaitU", motion_state: None },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(186), source_action_key: "DownStandU", motion_state: None },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(187), source_action_key: "DownAttackU", motion_state: None },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(191), source_action_key: "DownBoundD", motion_state: None },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(192), source_action_key: "DownWaitD", motion_state: None },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(194), source_action_key: "DownStandD", motion_state: None },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(195), source_action_key: "DownAttackD", motion_state: None },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(199), source_action_key: "Passive", motion_state: None },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(200), source_action_key: "PassiveStandF", motion_state: None },
    RuntimeActionBinding { action_state_id: MeleeActionStateId::new(201), source_action_key: "PassiveStandB", motion_state: None },
];

pub(crate) fn action_binding_for_motion_state(state: MotionState) -> Option<&'static RuntimeActionBinding> {
    ACTION_BINDINGS
        .iter()
        .find(|binding| binding.motion_state == Some(state))
}

pub(crate) fn source_action_key_for_action_state_id(
    action_state_id: MeleeActionStateId,
) -> Option<&'static str> {
    ACTION_BINDINGS
        .iter()
        .find(|binding| binding.action_state_id == action_state_id)
        .map(|binding| binding.source_action_key)
}

pub(crate) fn source_action_key_for_state(state: MotionState) -> Option<&'static str> {
    action_binding_for_motion_state(state).map(|binding| binding.source_action_key)
}
