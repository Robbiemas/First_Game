# Decomp Animation Frame Contract

The decomp does not expose a Rust-style `motion_frame` field as animation truth.
Fighter animation state is carried by `fp->cur_anim_frame` and `fp->frame_speed_mul`,
seeded through `Fighter_ChangeMotionState(gobj, msid, flags, anim_start, anim_speed, blend, NULL)`.

Rust translation rules:

- `PlayerState::cur_anim_frame()` maps to decomp `fp->cur_anim_frame`.
- `PlayerState::frame_speed_mul()` maps to decomp `fp->frame_speed_mul`.
- `motion_frame` is local bookkeeping for integer state age and command-script gates.
- Source pose, source ECB, source hurt capsules, source hit capsules, root-motion sampling, and
  `ftAnim_IsFramesRemaining`-style completion must use the decomp animation clock.
- `motion_frame` may only drive behavior where the decomp route has been checked and the value is
  intentionally an integer script/state-age gate.

When a source-backed state appears visually static, first verify that `cur_anim_frame` is advancing
before adding state-specific animation logic.
