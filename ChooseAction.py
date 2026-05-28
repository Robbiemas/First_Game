import pygame
import math


def _fresh_digital_trigger_press(play):
    if hasattr(play, "melee_air_dodge_pressed"):
        return bool(play.melee_air_dodge_pressed)

    return bool(
        getattr(play, "l_trigger_digital_pressed", False)
        or getattr(play, "r_trigger_digital_pressed", False)
        or (
            getattr(play, "l_trigger_digital", False)
            and getattr(play, "l_shield_pressed", False)
        )
        or (
            getattr(play, "r_trigger_digital", False)
            and getattr(play, "r_shield_pressed", False)
        )
    )


def _fall_from_landing_state(play):
    play.set_state("air")
    play.actionable = True
    play.canDash = True
    play.landLagCount = 0
    play.dodgeCount = 0


def resolve_action_state(play):
    stick = play.main_stick[0]    # SAVE SPACE WHEN CALLING X AXIS
    started_grounded = bool(play.grounded)
    if hasattr(play, "update_x_tap_timer"):
        play.update_x_tap_timer(stick)
    if hasattr(play, "update_y_tap_timer"):
        play.update_y_tap_timer(play.main_stick[1])

    if play.main_stick[1] == 0:  # resets the count for dropping through platforms if stick Y axis = 0
        play.dropCount = 0
    else:
        play.dropCount += 1

    if stick == 0:
        play.xCount = 0
        #play.dashCount = 0
        if play.running:
            play.endLag = True
        if play.walking and play.dashTurn:
            play.dashTurn = False
        if play.standing and play.dashTurn:
            play.turnAround()
            play.dashTurn = False
        if play.turning and play.dashTurn:
            play.dashTurn = False
    else:
        play.xCount += 1
        #play.dashCount += 1



    if play.grounded:                       # if grounded
        play.jumpCount = play.jumps
        walk_exited_to_wait = False
        entered_run_turn_this_frame = False

        if play.airDodge or getattr(play, "fallSpecial", False):                                # LANDING from air dodge / fall-special
            play.jumpCount = play.jumps
            play.set_state("landingFallSpecial")
            play.dodgeCount = 0
            play.yVelocity = 0

        if play.air or play.freeFall:                                # LANDING from the AIR
            play.jumpCount = play.jumps                 # reset jumps
            play.set_state("landingLag")
            play.dodgeCount = 0

        if play.landingFallSpecial:
            play.landLagCount += 1
            play.canJump = False
            play.actionable = False
            play.canDash = False
            play.apply_traction(play.xVelocity)
            play.yVelocity = 0
            if play.landLagCount >= getattr(play, "landingFallSpecialFrames", 10):
                play.actionable = True
                play.landLagCount = 0
                play.canDash = True
                play.set_state("standing")
            return

        if play.landingLag:                         # in LANDING LAG
            play.landLagCount += 1                      # count landLagCount
            play.canJump = False
            play.actionable = False                     # no longer ACTIONABLE
            play.canDash = False
            #play.apply_traction(play.xVelocity)
            if play.landLagCount >= getattr(play, "landingLagFrames", 10):                      # wait for character landing lag
                play.actionable = True                          # make ACTIONABLE
                play.landLagCount = 0                           # reset langLagCount
                play.canDash = True
                play.set_state("standing")
                #play.canJump = True

        if play.blockkey and not (play.landingLag or play.landingFallSpecial or play.jumpSquat or play.guardOff):

            # fix later
            #if play.dashing:
            #    play.turnAround()
            play.actionable = False
            if not play.blocking:
                play.set_state("blocking")
            play.canBlock = False

        if play.blocking:
            play.block()
        elif play.guardOff:
            play.guard_off()
        else:
            play.shieldHP += play.shieldRegenRate

        if play.walking and (
            (play.isRight and stick < 0) or (not play.isRight and stick > 0)
        ):
            play.set_state("standing")
            play.xVelocity = 0
            play.actionable = True
            play.dashCount = 0
            walk_exited_to_wait = True
        elif play.standing or play.walking:  # in a neutral state
            #play.canJump = True             # allow jumping and running, later shielding and attacking
            if play.standing and play.isRight and stick < 0:
                play.set_state("turning")
            if play.standing and not play.isRight and stick > 0:
                play.set_state("turning")
            play.tiltTurn = False
            play.smashTurn = False
            play.dashCount = 0
            if play.standing:
                play.apply_traction(play.xVelocity)

        if play.actionable and not walk_exited_to_wait:     # if actionable
            if play.has_fresh_x_tap(stick) and (play.standing or play.walking):  # providing two frames for a dash input
                play.set_state("dashing")  # the stick crossed the horizontal smash/dash threshold
                if stick > 0:
                    play.isRight = True
                    play.xVelocity += play.initialDash * 6
                else:
                    play.isRight = False
                    play.xVelocity += -play.initialDash * 6
                play.actionable = False
                play.canDash = False
            if stick != 0 and not play.jumpSquat:
                if play.dashing or play.running or play.turning or play.runTurn:  # if stick is no longer at 0 and not running, begin walk
                    play.walking = False
                elif not play.jumpSquat:
                    play.set_state("walking")



        if play.turning:
            play.turn(stick)

        transitioned_from_dash = False
        if play.dashing:
            play.dash(stick)
            transitioned_from_dash = not play.dashing

        if play.runTurn and not transitioned_from_dash:
            play.endLag = False
            play.turnCount += 1
            if stick == 0:
                play.apply_traction(play.xVelocity)
                if play.xVelocity == 0:
                    play.set_state("standing")
                    play.turnCount = 0
            else:
                opposite_input = (play.isRight and stick < 0) or (
                    not play.isRight and stick > 0
                )
                same_direction_input = (play.isRight and stick > 0) or (
                    not play.isRight and stick < 0
                )
                if opposite_input:
                    accel, target_velocity = play.ground_accel_and_target(
                        stick,
                        play.max_run_velocity(),
                    )
                    play.approach_ground_velocity(
                        accel,
                        target_velocity,
                        play.ground_friction(),
                        play.max_run_velocity(),
                    )
                    crossed_zero = (play.isRight and play.xVelocity <= 0) or (
                        not play.isRight and play.xVelocity >= 0
                    )
                    if crossed_zero:
                        play.turnAround()
                        play.turnCount = 0
                        if (play.isRight and stick >= 0.64) or (
                            not play.isRight and stick <= -0.64
                        ):
                            play.set_state("running")
                elif same_direction_input and (
                    (play.isRight and play.xVelocity > 0)
                    or (not play.isRight and play.xVelocity < 0)
                ):
                    play.set_state("running")
                    play.turnCount = 0
                else:
                    play.apply_traction(play.xVelocity)
            #play.apply_traction(play.xVelocity)

        if play.running and not transitioned_from_dash:
            max_run_velocity = play.max_run_velocity() if hasattr(play, "max_run_velocity") else play.runSpeed * 6
            if abs(play.xVelocity) > max_run_velocity:
                play.apply_traction(play.xVelocity)
            play.canDash = False
            entered_run_turn = False
            if play.isRight and stick <= -0.64:
                play.set_state("runTurn")
                play.turnCount = 0
                entered_run_turn = True
                entered_run_turn_this_frame = True
            elif not play.isRight and stick >= 0.64:
                play.set_state("runTurn")
                play.turnCount = 0
                entered_run_turn = True
                entered_run_turn_this_frame = True
            elif play.xCount == 0:
                play.endLag = True
            if entered_run_turn:
                pass
            elif not play.endLag:
                play.run(stick)  # ACTIVATE RUN
            else:
                if play.lagCount > 20:
                    play.set_state("standing")
                    play.lagCount = 0
                    play.endLag = False
                    play.canDash = True
                    play.actionable = True

                play.lagCount += 1

        if play.walking and not transitioned_from_dash:
            play.walk(stick)  # ACTIVATE WALK
            if stick == 0:
                play.set_state("standing")

        if play.jumpSquat:  # if in JUMPSQUAT
            play.jsCount += 1  # add to jsCount
            if not play.jumpkey:
                play.jump_released_during_squat = True
                play.canJump = False
            #play.canBlock = False

        if play.jumpkey and play.canJump and not play.jumpSquat:  # if JUMP BUTTON pressed and canJump
            play.jsCount = 0  # resets jsCount

            play.set_state("jumpSquat")
            play.jump_released_during_squat = False
            play.dodgeCount = 0
            play.canJump = False  # set canJump to FALSE
            play.canDash = True

        if (play.jsCount == play.js - 1) and play.jumpSquat:  # if done with JUMPSQUAT
            play.reset_ground()  # set no longer grounded
            play.jumpCount -= 1  # removes jump count
            play.jumpWait += 1
            play.actionable = True
        elif (play.jsCount > play.js - 1) and play.jumpSquat:
            play.reset_ground()
            if getattr(play, "jump_released_during_squat", False):
                play.change_velocity(play.shortHop)
            else:
                play.change_velocity(play.jumpHeight)
            play.set_state("air")
            play.jump_released_during_squat = False
            play.dashCount = 0
            play.canDash = True
            play.canJump = False
        if (
            play.standing
            or play.endLag
            or play.turning
            or play.blocking
            or play.guardOff
            or play.landingLag
            or (play.runTurn and entered_run_turn_this_frame)
        ):
            play.apply_traction(play.xVelocity)

    if not play.grounded and not started_grounded:  # if airborne

        play.shielding = False
        play.jumpWait += 1

        if play.landingLag or play.landingFallSpecial:
            _fall_from_landing_state(play)
            return

        if play.airDodge:
            play.air_dodge()
            return

        if play.freeFall or getattr(play, "fallSpecial", False):
            play.gravity(play.gWeight)
            play.drift(stick)

        if play.jumpSquat:
            if getattr(play, "jump_released_during_squat", False):
                play.change_velocity(play.shortHop)  # set new velocity to jumpHeight          ~~~~JUMPS!!!~~~~
            else:
                play.change_velocity(play.jumpHeight)  # set new velocity to jumpHeight       ~~~~JUMPS!!!~~~~
            play.set_state("air")
            play.jump_released_during_squat = False
            play.dashCount = 0
            play.canDash = True
            play.canJump = False

        if play.running or play.walking or play.standing or play.dashing or play.landingLag or play.runTurn or play.blocking or play.guardOff:
            play.set_state("air")
            play.actionable = True
            play.dashCount = 0
            play.canDash = True
            #play.canJump = True

        if play.actionable:

            play.fast_fall(play.main_stick[1])

            if play.jumpkey and play.jumpCount > 0 and play.canJump: # and play.jumpWait > 8:    # if jump button pressed
                play.change_velocity(play.airJumpHeight)   # set new velocity to jumpHeight             ~~~~JUMPS!!!~~~~
                play.gCount = 1
                play.jumpCount -= 1                     # removes jump count
                play.canJump = False
                play.jumpWait = 0
                play.xVelocity = 0
                play.xVelocity = play.airSpeed * 0.85 * stick      # allow reversing momentum with jumps
                if stick > 0:
                    play.isRight = True
                if stick < 0:
                    play.isRight = False
            if _fresh_digital_trigger_press(play):
                play.canBlock = False
                play.set_state("airDodge")

            if play.airDodge:
                play.air_dodge()
                return

            play.drift(stick)
            play.gravity(play.gWeight)

        if play.sliding:
            play.set_state("air")
          #  play.air = True
            play.actionable = True
         #   play.sliding = False
         #   play.airDodge = False

            # actionable if coming out of hitstun
            # actionable if sliding off of platform in shield


        play.air_friction()
