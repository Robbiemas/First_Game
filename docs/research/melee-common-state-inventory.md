# Melee Common Fighter State Inventory

This is the reference inventory for Melee's shared/common fighter motion states.
It intentionally excludes character-specific state tables, such as Falcon's
special move internals, and focuses on the `ftCo_MS_*` states shared through the
common fighter layer.

Primary source: the doldecomp Melee repository's
[`src/melee/ft/ftmotionstates.c`](https://github.com/doldecomp/melee/blob/c75e4117053da384d314fe69f7c72f9a48dd6c59/src/melee/ft/ftmotionstates.c).
The project README describes `ft` as fighter code and `ft/chara/ftCommon` as
shared character code, which is the boundary this document uses:
[`doldecomp/melee`](https://github.com/doldecomp/melee).

## Why This Matters

Melee movement feel is not only physics constants. It is a table of explicit
motion states, and each state owns its own animation, IASA/input callback,
physics callback, collision callback, and camera callback. For Mole, the rule is:
if a mechanic in Melee depends on a named common motion state, we should model
that state directly instead of hiding it behind a patch flag.

## Implementation Tiers

Tier 1 is the movement-control spine we need before judging feel:

- `Wait`, `WalkSlow`, `WalkMiddle`, `WalkFast`
- `Turn`, `TurnRun`, `Dash`, `Run`, `RunDirect`, `RunBrake`
- `KneeBend`, `JumpF`, `JumpB`, `JumpAerialF`, `JumpAerialB`
- `Fall`, `FallF`, `FallB`, `FallAerial`, `FallAerialF`, `FallAerialB`
- `FallSpecial`, `FallSpecialF`, `FallSpecialB`
- `Squat`, `SquatWait`, `SquatRv`
- `Landing`, `LandingFallSpecial`
- `GuardOn`, `Guard`, `GuardOff`, `GuardSetOff`, `GuardReflect`
- `EscapeN`, `EscapeF`, `EscapeB`, `EscapeAir`

Tier 2 is required for combat feel:

- Jab sequence, rapid jab, dash attack, tilts, angled tilts, smashes, angled
  smashes, aerials, and aerial landing states.
- Damage, tumble, knockdown, tech, shield-break, catch, throw, captured, and
  thrown states.

Tier 3 is required for full Melee-like game completeness:

- Item get/throw/swing/shoot states.
- Ledge/cliff states.
- Environment/object states such as barrel, hammer, mushroom, warp star,
  buried, grabbed-by-boss, appeal, teeter, rebound, wall/ceiling bonk, and entry.

## Common State Groups

### System, Death, Rebirth

- `DeadDown`, `DeadLeft`, `DeadRight`, `DeadUp`
- `DeadUpStar`, `DeadUpStarIce`
- `DeadUpFall`, `DeadUpFallHitCamera`, `DeadUpFallHitCameraFlat`,
  `DeadUpFallIce`, `DeadUpFallHitCameraIce`
- `Sleep`, `Rebirth`, `RebirthWait`
- `Entry`, `EntryStart`, `EntryEnd`

### Ground Movement

- `Wait`
- `WalkSlow`, `WalkMiddle`, `WalkFast`
- `Turn`, `TurnRun`
- `Dash`
- `Run`, `RunDirect`, `RunBrake`
- `Ottotto`, `OttottoWait`

### Jump, Air, Fall, Landing

- `KneeBend`
- `JumpF`, `JumpB`
- `JumpAerialF`, `JumpAerialB`
- `Fall`, `FallF`, `FallB`
- `FallAerial`, `FallAerialF`, `FallAerialB`
- `FallSpecial`, `FallSpecialF`, `FallSpecialB`
- `DamageFall`
- `Landing`, `LandingFallSpecial`
- `LandingAirN`, `LandingAirF`, `LandingAirB`, `LandingAirHi`,
  `LandingAirLw`

### Crouch

- `Squat`
- `SquatWait`
- `SquatRv`

### Ground Attacks

- `Attack11`, `Attack12`, `Attack13`
- `Attack100Start`, `Attack100Loop`, `Attack100End`
- `AttackDash`
- `AttackS3Hi`, `AttackS3HiS`, `AttackS3S`, `AttackS3LwS`, `AttackS3Lw`
- `AttackHi3`, `AttackLw3`
- `AttackS4Hi`, `AttackS4HiS`, `AttackS4S`, `AttackS4LwS`, `AttackS4Lw`
- `AttackHi4`, `AttackLw4`

### Aerial Attacks

- `AttackAirN`
- `AttackAirF`
- `AttackAirB`
- `AttackAirHi`
- `AttackAirLw`

### Damage And Knockback

- `DamageHi1`, `DamageHi2`, `DamageHi3`
- `DamageN1`, `DamageN2`, `DamageN3`
- `DamageLw1`, `DamageLw2`, `DamageLw3`
- `DamageAir1`, `DamageAir2`, `DamageAir3`
- `DamageFlyHi`, `DamageFlyN`, `DamageFlyLw`, `DamageFlyTop`,
  `DamageFlyRoll`
- `DamageScrew`, `DamageScrewAir`
- `DamageSong`, `DamageSongWait`, `DamageSongRv`
- `DamageBind`
- `DamageIce`, `DamageIceJump`

### Items

- `LightGet`, `HeavyGet`
- `LightThrowF`, `LightThrowB`, `LightThrowHi`, `LightThrowLw`,
  `LightThrowDash`, `LightThrowDrop`
- `LightThrowAirF`, `LightThrowAirB`, `LightThrowAirHi`, `LightThrowAirLw`
- `HeavyThrowF`, `HeavyThrowB`, `HeavyThrowHi`, `HeavyThrowLw`
- `LightThrowF4`, `LightThrowB4`, `LightThrowHi4`, `LightThrowLw4`
- `LightThrowAirF4`, `LightThrowAirB4`, `LightThrowAirHi4`,
  `LightThrowAirLw4`
- `HeavyThrowF4`, `HeavyThrowB4`, `HeavyThrowHi4`, `HeavyThrowLw4`
- `SwordSwing1`, `SwordSwing3`, `SwordSwing4`, `SwordSwingDash`
- `BatSwing1`, `BatSwing3`, `BatSwing4`, `BatSwingDash`
- `ParasolSwing1`, `ParasolSwing3`, `ParasolSwing4`, `ParasolSwingDash`
- `HarisenSwing1`, `HarisenSwing3`, `HarisenSwing4`, `HarisenSwingDash`
- `StarRodSwing1`, `StarRodSwing3`, `StarRodSwing4`, `StarRodSwingDash`
- `LipstickSwing1`, `LipstickSwing3`, `LipstickSwing4`,
  `LipstickSwingDash`
- `ItemParasolOpen`, `ItemParasolFall`, `ItemParasolFallSpecial`,
  `ItemParasolDamageFall`
- `LGunShoot`, `LGunShootAir`, `LGunShootEmpty`, `LGunShootAirEmpty`
- `FireFlowerShoot`, `FireFlowerShootAir`
- `ItemScrew`, `ItemScrewAir`
- `ItemScopeStart`, `ItemScopeRapid`, `ItemScopeFire`, `ItemScopeEnd`
- `ItemScopeAirStart`, `ItemScopeAirRapid`, `ItemScopeAirFire`,
  `ItemScopeAirEnd`
- `ItemScopeStartEmpty`, `ItemScopeRapidEmpty`, `ItemScopeFireEmpty`,
  `ItemScopeEndEmpty`
- `ItemScopeAirStartEmpty`, `ItemScopeAirRapidEmpty`,
  `ItemScopeAirFireEmpty`, `ItemScopeAirEndEmpty`

### Lift And Carried States

- `LiftWait`, `LiftWalk1`, `LiftWalk2`, `LiftTurn`
- `ShoulderedWait`, `ShoulderedWalkSlow`, `ShoulderedWalkMiddle`,
  `ShoulderedWalkFast`, `ShoulderedTurn`

### Shield, Dodges, Tech, Knockdown

- `GuardOn`, `Guard`, `GuardOff`, `GuardSetOff`, `GuardReflect`
- `EscapeF`, `EscapeB`, `EscapeN`, `EscapeAir`
- `DownBoundU`, `DownWaitU`, `DownDamageU`, `DownStandU`, `DownAttackU`,
  `DownFowardU`, `DownBackU`, `DownSpotU`
- `DownBoundD`, `DownWaitD`, `DownDamageD`, `DownStandD`, `DownAttackD`,
  `DownFowardD`, `DownBackD`, `DownSpotD`
- `Passive`, `PassiveStandF`, `PassiveStandB`, `PassiveWall`,
  `PassiveWallJump`, `PassiveCeil`
- `ShieldBreakFly`, `ShieldBreakFall`, `ShieldBreakDownU`,
  `ShieldBreakDownD`, `ShieldBreakStandU`, `ShieldBreakStandD`, `Furafura`
- `DownReflect`

### Grab, Capture, Throw, Thrown

- `Catch`, `CatchPull`, `CatchDash`, `CatchDashPull`, `CatchWait`,
  `CatchAttack`, `CatchCut`
- `ThrowF`, `ThrowB`, `ThrowHi`, `ThrowLw`
- `ThrownF`, `ThrownB`, `ThrownHi`, `ThrownLw`, `ThrownlwWomen`
- `ThrownFF`, `ThrownFB`, `ThrownFHi`, `ThrownFLw`
- `CapturePulledHi`, `CaptureWaitHi`, `CaptureDamageHi`
- `CapturePulledLw`, `CaptureWaitLw`, `CaptureDamageLw`
- `CaptureCut`, `CaptureJump`, `CaptureNeck`, `CaptureFoot`
- `CaptureCaptain`, `CaptureYoshi`, `YoshiEgg`
- `CaptureKoopa`, `CaptureDamageKoopa`, `CaptureWaitKoopa`,
  `ThrownKoopaF`, `ThrownKoopaB`
- `CaptureKoopaAir`, `CaptureDamageKoopaAir`, `CaptureWaitKoopaAir`,
  `ThrownKoopaAirF`, `ThrownKoopaAirB`
- `CaptureKirby`, `CaptureWaitKirby`, `ThrownKirbyStar`,
  `ThrownCopyStar`, `ThrownKirby`
- `CaptureMewtwo`, `CaptureMewtwoAir`, `ThrownMewtwo`, `ThrownMewtwoAir`
- `CaptureMasterHand`, `CaptureDamageMasterHand`, `CaptureWaitMasterHand`,
  `ThrownMasterHand`
- `CaptureKirbyYoshi`, `KirbyYoshiEgg`
- `CaptureLeadead`, `CaptureLikelike`
- `CaptureCrazyHand`, `CaptureDamageCrazyHand`, `CaptureWaitCrazyHand`,
  `ThrownCrazyHand`

### Ledge, Wall, Ceiling, Rebound

- `Pass`
- `ReboundStop`, `Rebound`
- `FlyReflectWall`, `FlyReflectCeil`
- `StopWall`, `StopCeil`, `MissFoot`
- `CliffCatch`, `CliffWait`
- `CliffClimbSlow`, `CliffClimbQuick`
- `CliffAttackSlow`, `CliffAttackQuick`
- `CliffEscapeSlow`, `CliffEscapeQuick`
- `CliffJumpSlow1`, `CliffJumpSlow2`, `CliffJumpQuick1`, `CliffJumpQuick2`

### Miscellaneous Common States

- `AppealSR`, `AppealSL`
- `BarrelWait`, `Barrel`
- `Bury`, `BuryWait`, `BuryJump`
- `WarpStarJump`, `WarpStarFall`
- `HammerWait`, `HammerWalk`, `HammerTurn`, `HammerKneeBend`,
  `HammerFall`, `HammerJump`, `HammerLanding`
- `KinokoGiantStart`, `KinokoGiantStartAir`, `KinokoGiantEnd`,
  `KinokoGiantEndAir`
- `KinokoSmallStart`, `KinokoSmallStartAir`, `KinokoSmallEnd`,
  `KinokoSmallEndAir`

## Full Common State ID Table

This compact table is copied from the `ftCo_MS_*` comments in
`ftmotionstates.c`.

| IDs | States |
| --- | --- |
| 0-19 | 0 DeadDown, 1 DeadLeft, 2 DeadRight, 3 DeadUp, 4 DeadUpStar, 5 DeadUpStarIce, 6 DeadUpFall, 7 DeadUpFallHitCamera, 8 DeadUpFallHitCameraFlat, 9 DeadUpFallIce, 10 DeadUpFallHitCameraIce, 11 Sleep, 12 Rebirth, 13 RebirthWait, 14 Wait, 15 WalkSlow, 16 WalkMiddle, 17 WalkFast, 18 Turn, 19 TurnRun |
| 20-39 | 20 Dash, 21 Run, 22 RunDirect, 23 RunBrake, 24 KneeBend, 25 JumpF, 26 JumpB, 27 JumpAerialF, 28 JumpAerialB, 29 Fall, 30 FallF, 31 FallB, 32 FallAerial, 33 FallAerialF, 34 FallAerialB, 35 FallSpecial, 36 FallSpecialF, 37 FallSpecialB, 38 DamageFall, 39 Squat |
| 40-59 | 40 SquatWait, 41 SquatRv, 42 Landing, 43 LandingFallSpecial, 44 Attack11, 45 Attack12, 46 Attack13, 47 Attack100Start, 48 Attack100Loop, 49 Attack100End, 50 AttackDash, 51 AttackS3Hi, 52 AttackS3HiS, 53 AttackS3S, 54 AttackS3LwS, 55 AttackS3Lw, 56 AttackHi3, 57 AttackLw3, 58 AttackS4Hi, 59 AttackS4HiS |
| 60-79 | 60 AttackS4S, 61 AttackS4LwS, 62 AttackS4Lw, 63 AttackHi4, 64 AttackLw4, 65 AttackAirN, 66 AttackAirF, 67 AttackAirB, 68 AttackAirHi, 69 AttackAirLw, 70 LandingAirN, 71 LandingAirF, 72 LandingAirB, 73 LandingAirHi, 74 LandingAirLw, 75 DamageHi1, 76 DamageHi2, 77 DamageHi3, 78 DamageN1, 79 DamageN2 |
| 80-99 | 80 DamageN3, 81 DamageLw1, 82 DamageLw2, 83 DamageLw3, 84 DamageAir1, 85 DamageAir2, 86 DamageAir3, 87 DamageFlyHi, 88 DamageFlyN, 89 DamageFlyLw, 90 DamageFlyTop, 91 DamageFlyRoll, 92 LightGet, 93 HeavyGet, 94 LightThrowF, 95 LightThrowB, 96 LightThrowHi, 97 LightThrowLw, 98 LightThrowDash, 99 LightThrowDrop |
| 100-119 | 100 LightThrowAirF, 101 LightThrowAirB, 102 LightThrowAirHi, 103 LightThrowAirLw, 104 HeavyThrowF, 105 HeavyThrowB, 106 HeavyThrowHi, 107 HeavyThrowLw, 108 LightThrowF4, 109 LightThrowB4, 110 LightThrowHi4, 111 LightThrowLw4, 112 LightThrowAirF4, 113 LightThrowAirB4, 114 LightThrowAirHi4, 115 LightThrowAirLw4, 116 HeavyThrowF4, 117 HeavyThrowB4, 118 HeavyThrowHi4, 119 HeavyThrowLw4 |
| 120-139 | 120 SwordSwing1, 121 SwordSwing3, 122 SwordSwing4, 123 SwordSwingDash, 124 BatSwing1, 125 BatSwing3, 126 BatSwing4, 127 BatSwingDash, 128 ParasolSwing1, 129 ParasolSwing3, 130 ParasolSwing4, 131 ParasolSwingDash, 132 HarisenSwing1, 133 HarisenSwing3, 134 HarisenSwing4, 135 HarisenSwingDash, 136 StarRodSwing1, 137 StarRodSwing3, 138 StarRodSwing4, 139 StarRodSwingDash |
| 140-159 | 140 LipstickSwing1, 141 LipstickSwing3, 142 LipstickSwing4, 143 LipstickSwingDash, 144 ItemParasolOpen, 145 ItemParasolFall, 146 ItemParasolFallSpecial, 147 ItemParasolDamageFall, 148 LGunShoot, 149 LGunShootAir, 150 LGunShootEmpty, 151 LGunShootAirEmpty, 152 FireFlowerShoot, 153 FireFlowerShootAir, 154 ItemScrew, 155 ItemScrewAir, 156 DamageScrew, 157 DamageScrewAir, 158 ItemScopeStart, 159 ItemScopeRapid |
| 160-179 | 160 ItemScopeFire, 161 ItemScopeEnd, 162 ItemScopeAirStart, 163 ItemScopeAirRapid, 164 ItemScopeAirFire, 165 ItemScopeAirEnd, 166 ItemScopeStartEmpty, 167 ItemScopeRapidEmpty, 168 ItemScopeFireEmpty, 169 ItemScopeEndEmpty, 170 ItemScopeAirStartEmpty, 171 ItemScopeAirRapidEmpty, 172 ItemScopeAirFireEmpty, 173 ItemScopeAirEndEmpty, 174 LiftWait, 175 LiftWalk1, 176 LiftWalk2, 177 LiftTurn, 178 GuardOn, 179 Guard |
| 180-199 | 180 GuardOff, 181 GuardSetOff, 182 GuardReflect, 183 DownBoundU, 184 DownWaitU, 185 DownDamageU, 186 DownStandU, 187 DownAttackU, 188 DownFowardU, 189 DownBackU, 190 DownSpotU, 191 DownBoundD, 192 DownWaitD, 193 DownDamageD, 194 DownStandD, 195 DownAttackD, 196 DownFowardD, 197 DownBackD, 198 DownSpotD, 199 Passive |
| 200-219 | 200 PassiveStandF, 201 PassiveStandB, 202 PassiveWall, 203 PassiveWallJump, 204 PassiveCeil, 205 ShieldBreakFly, 206 ShieldBreakFall, 207 ShieldBreakDownU, 208 ShieldBreakDownD, 209 ShieldBreakStandU, 210 ShieldBreakStandD, 211 Furafura, 212 Catch, 213 CatchPull, 214 CatchDash, 215 CatchDashPull, 216 CatchWait, 217 CatchAttack, 218 CatchCut, 219 ThrowF |
| 220-239 | 220 ThrowB, 221 ThrowHi, 222 ThrowLw, 223 CapturePulledHi, 224 CaptureWaitHi, 225 CaptureDamageHi, 226 CapturePulledLw, 227 CaptureWaitLw, 228 CaptureDamageLw, 229 CaptureCut, 230 CaptureJump, 231 CaptureNeck, 232 CaptureFoot, 233 EscapeF, 234 EscapeB, 235 EscapeN, 236 EscapeAir, 237 ReboundStop, 238 Rebound, 239 ThrownF |
| 240-259 | 240 ThrownB, 241 ThrownHi, 242 ThrownLw, 243 ThrownlwWomen, 244 Pass, 245 Ottotto, 246 OttottoWait, 247 FlyReflectWall, 248 FlyReflectCeil, 249 StopWall, 250 StopCeil, 251 MissFoot, 252 CliffCatch, 253 CliffWait, 254 CliffClimbSlow, 255 CliffClimbQuick, 256 CliffAttackSlow, 257 CliffAttackQuick, 258 CliffEscapeSlow, 259 CliffEscapeQuick |
| 260-279 | 260 CliffJumpSlow1, 261 CliffJumpSlow2, 262 CliffJumpQuick1, 263 CliffJumpQuick2, 264 AppealSR, 265 AppealSL, 266 ShoulderedWait, 267 ShoulderedWalkSlow, 268 ShoulderedWalkMiddle, 269 ShoulderedWalkFast, 270 ShoulderedTurn, 271 ThrownFF, 272 ThrownFB, 273 ThrownFHi, 274 ThrownFLw, 275 CaptureCaptain, 276 CaptureYoshi, 277 YoshiEgg, 278 CaptureKoopa, 279 CaptureDamageKoopa |
| 280-299 | 280 CaptureWaitKoopa, 281 ThrownKoopaF, 282 ThrownKoopaB, 283 CaptureKoopaAir, 284 CaptureDamageKoopaAir, 285 CaptureWaitKoopaAir, 286 ThrownKoopaAirF, 287 ThrownKoopaAirB, 288 CaptureKirby, 289 CaptureWaitKirby, 290 ThrownKirbyStar, 291 ThrownCopyStar, 292 ThrownKirby, 293 BarrelWait, 294 Bury, 295 BuryWait, 296 BuryJump, 297 DamageSong, 298 DamageSongWait, 299 DamageSongRv |
| 300-319 | 300 DamageBind, 301 CaptureMewtwo, 302 CaptureMewtwoAir, 303 ThrownMewtwo, 304 ThrownMewtwoAir, 305 WarpStarJump, 306 WarpStarFall, 307 HammerWait, 308 HammerWalk, 309 HammerTurn, 310 HammerKneeBend, 311 HammerFall, 312 HammerJump, 313 HammerLanding, 314 KinokoGiantStart, 315 KinokoGiantStartAir, 316 KinokoGiantEnd, 317 KinokoGiantEndAir, 318 KinokoSmallStart, 319 KinokoSmallStartAir |
| 320-339 | 320 KinokoSmallEnd, 321 KinokoSmallEndAir, 322 Entry, 323 EntryStart, 324 EntryEnd, 325 DamageIce, 326 DamageIceJump, 327 CaptureMasterHand, 328 CaptureDamageMasterHand, 329 CaptureWaitMasterHand, 330 ThrownMasterHand, 331 CaptureKirbyYoshi, 332 KirbyYoshiEgg, 333 CaptureLeadead, 334 CaptureLikelike, 335 DownReflect, 336 CaptureCrazyHand, 337 CaptureDamageCrazyHand, 338 CaptureWaitCrazyHand, 339 ThrownCrazyHand |
| 340-340 | 340 Barrel |
