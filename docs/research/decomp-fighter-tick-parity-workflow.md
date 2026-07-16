# Decomp Fighter Tick Parity Workflow

This workflow is for replay divergences where Rust and Slippi disagree around
fighter state, position, velocity, collision, or action timing. The decomp is
the primary reference. Slippi is a secondary witness and may expose mixed-phase
post-frame fields.

## Command Loop

1. Find the first divergence:

```powershell
cargo run -p mole_cli -- replay check --inputs debug/slippi/Game_20260530T214929.inputs.json --frames 700 --json
```

2. Explain the local phase window:

```powershell
cargo run -p mole_cli -- replay explain --inputs debug/slippi/Game_20260530T214929.inputs.json --player 2 --frame 213 --window 2 --frames 700 --format markdown
```

3. Pull the decomp function bodies named by the explanation:

```powershell
cargo run -p mole_cli -- decomp show "src/melee/ft/chara/ftCommon/ftCo_EscapeAir.c" --line 96 --context 80 --json
cargo run -p mole_cli -- decomp show "src/melee/ft/chara/ftCommon/ftCo_Landing.c" --line 73 --context 130 --json
cargo run -p mole_cli -- decomp show "src/melee/ft/ftcommon.c" --line 143 --context 80 --json
```

## Rules

- Do not patch Rust to match a Slippi row until the decomp tick phase and
  source fields at that logical point are proven.
- Assign each divergence to a decomp phase before changing code.
- Map each Rust field to one decomp field, or explicitly mark it as a
  public/composed/export field.
- Preserve f32 data as f32 through extraction, baked artifacts, runtime state,
  and physics. Milli-integer conversion is for reporting and public display.
- Treat Slippi replay rows as sampled witness data. A row can contain action,
  position, and velocity fields captured from different internal phases.

## Fighter Tick Phase Skeleton

For the EscapeAir to LandingFallSpecial platform-contact class:

1. `ftCo_EscapeAir_Phys`
   Applies airdodge decay while `cmd_skip_decay` is false.

2. `ftCo_EscapeAir_Coll -> ft_80082C74 -> mpColl_800471F8`
   Performs the airborne collision sweep from previous/current ECB state.

3. `ftCo_80099D70 -> ftCo_LandingFallSpecial_Enter`
   Runs the landing callback immediately when the decomp collision helper
   reports floor contact.

4. `ftCommon_8007D7FC -> ftCommon_8007D6A4`
   Enters ground ownership and copies the correct pre-transition horizontal
   velocity into `gr_vel`.

5. `ftCo_Landing_Phys -> ft_80084F3C`
   Stages ground friction in `xE4_ground_accel_1`, including high-speed and
   floor-friction multipliers.

6. `fighter.c` outer commit
   Commits `gr_vel += xE4_ground_accel_1 + xE8_ground_accel_2`, adds
   `x74_anim_vel` into `self_vel`, clears staged values, then advances
   position from the committed fighter velocity path.

## Frame 336 Note

For `debug/slippi/Game_20260530T214929.inputs.json`, player 2, source frame
213 / core frame 336 is downstream of source frame 212 / core frame 335.

Slippi reports one more EscapeAir-style decay on the first visible
LandingFallSpecial row:

```text
-1.824294 * 0.9 = -1.641865
-0.899300 * 0.9 = -0.809370
```

The decomp-backed Rust path has already entered LandingFallSpecial on the
previous collision callback and applies the first landing ground-traction tick:

```text
-1.824294 + 0.160 = -1.664294
```

Do not delay Rust landing to match that Slippi row unless later decomp evidence
proves the collision callback or fighter tick phase is wrong.
