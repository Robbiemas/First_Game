# Floating Character Pose Decoupling Note

## Context

While fixing the source hurtbox pose selection for the Falcon parity replay, a bug made grounded/common states render source hurtboxes from a stale falling action identity. The result looked visually interesting: the rendered character could keep grounded gameplay state while its visible pose drifted toward an airborne/floating silhouette.

## Keep Out Of Core Parity

For Melee parity, source hurtboxes must follow the active decomp pose. A common state such as `Wait` should render `Wait1` hurt capsules even if a stale common action id, such as `Fall`, is still present on a diagnostic/render frame. Offensive hitbox scripts still use action identity because source-only actions such as damage, throws, captures, and special subactions are real decomp action scripts.

## Future Character Use

If a future floating character intentionally wants this effect, implement it as an explicit character visual/controller layer:

- Keep collision and hurtboxes sourced from the active decomp pose.
- Add a separate optional visual pose channel for sprite/model presentation.
- Do not let the optional visual channel change source collision capsules, action script hitboxes, or replay parity logs.

