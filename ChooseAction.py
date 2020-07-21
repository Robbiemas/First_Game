import pygame
import math

def resolve_action_state(play):
    stick = play.main_stick[0]    # SAVE SPACE WHEN CALLING X AXIS

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

        if play.air or play.airDodge or play.freeFall:                                # LANDING from the AIR
            play.jumpCount = play.jumps                 # reset jumps
            play.set_state("landingLag")
            play.dodgeCount = 0

        if play.landingLag:                         # in LANDING LAG
            play.landLagCount += 1                      # count landLagCount
            play.canJump = False
            play.actionable = False                     # no longer ACTIONABLE
            play.canDash = False
            #play.apply_traction(play.xVelocity)
            if play.landLagCount == 10:                      # wait 10 frames
                play.actionable = True                          # make ACTIONABLE
                play.landLagCount = 0                           # reset langLagCount
                play.canDash = True
                play.set_state("standing")
                #play.canJump = True

        if play.blockkey and not (play.landingLag or play.jumpSquat):

            # fix later
            #if play.dashing:
            #    play.turnAround()
            play.actionable = False
            play.set_state("blocking")
            play.canBlock = False

        if play.blocking:
            play.block()
        else:
            play.shieldHP += play.shieldRegenRate

        if play.standing or play.walking:  # in a neutral state
            #play.canJump = True             # allow jumping and running, later shielding and attacking
            if play.isRight and stick < 0:
                play.set_state("turning")
            if not play.isRight and stick > 0:
                play.set_state("turning")
            play.tiltTurn = False
            play.smashTurn = False
            play.dashCount = 0
            if play.standing:
                play.apply_traction(play.xVelocity)

        if play.actionable:     # if actionable
            if abs(stick) >= 0.6 and play.xCount <= 2 and (play.standing or play.walking):  # providing two frames for a dash input
                play.set_state("dashing")  # the stick is past 0.6 on x, initiate dash
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
            print("yeee")

        if play.dashing:
            play.dash(stick)

        if play.runTurn:
            play.endLag = False
            play.canJump = False
            if play.turnCount > 30 and abs(stick) >= 0.80:
                play.canJump = True
                if play.isRight:
                    if stick > 0.64:
                        play.set_state("running")
                        print("showHERE")
                    else:
                        play.set_state("walking")
                else:
                    if stick < -0.64:
                        play.set_state("running")
                    else:
                        play.set_state("walking")
                play.turnCount = -1
            if play.turnCount > 30 and 0 < abs(stick) < 0.80:
                play.canJump = True
                play.set_state("walking")
                play.turnCount = -1
            play.turnCount += 1
            if stick == 0:
                play.apply_traction(play.xVelocity)
                if play.turnCount > 30:
                    play.set_state("standing")
                    play.canJump = True
                    play.xVelocity = 0
                    play.turnCount = 0
            if play.isRight:
                if stick < 0:
                    play.apply_traction(play.xVelocity)
                elif play.turnCount > 20:
                    play.run(stick)
            else:
                if stick > 0:
                    play.apply_traction(play.xVelocity)
                elif play.turnCount > 20:
                    play.run(stick)
            #play.apply_traction(play.xVelocity)

        if play.running:
            if abs(play.xVelocity) > play.runSpeed * 6:
                play.apply_traction(play.xVelocity)
            play.canDash = False
            if play.isRight and stick <= -0.64:
                play.set_state("runTurn")
                play.isRight = False
                play.turnCount = 0
            elif not play.isRight and stick >= 0.64:
                play.set_state("runTurn")
                play.isRight = True
                play.turnCount = 0
            elif play.xCount == 0:
                play.endLag = True
            if not play.endLag:
                play.run(stick)  # ACTIVATE RUN
            else:
                if play.lagCount > 20:
                    play.set_state("standing")
                    play.lagCount = 0
                    play.endLag = False
                    play.canDash = True
                    play.actionable = True

                play.lagCount += 1

        if play.walking:
            play.walk(stick)  # ACTIVATE WALK
            if stick == 0:
                play.set_state("standing")

        if play.jumpSquat:  # if in JUMPSQUAT
            play.jsCount += 1  # add to jsCount
            #play.canBlock = False

        if play.jumpkey and play.canJump:  # if JUMP BUTTON pressed and canJump
            play.jsCount = 0  # resets jsCount

            play.set_state("jumpSquat")
            play.dodgeCount = 0
            play.canJump = False  # set canJump to FALSE
            play.canDash = True

        if (play.jsCount == play.js-1) and play.jumpSquat:  # if done with JUMPSQUAT
            play.reset_ground()  # set no longer grounded
            play.jumpCount -= 1  # removes jump count
            play.jumpWait += 1
            play.actionable = True
        if play.standing or play.endLag or play.turning or play.blocking or play.landingLag or play.runTurn:
            play.apply_traction(play.xVelocity)

    if not play.grounded:  # if airborne

        play.shielding = False
        play.jumpWait += 1

        if play.freeFall:
            play.gravity(play.gWeight)
            play.drift(stick)

        if play.jumpSquat:

            if play.blockkey and play.canBlock:
                play.canBlock = False
                play.set_state("airDodge")
            else:
                if play.canJump:
                    play.change_velocity(play.shortHop)  # set new velocity to jumpHeight          ~~~~JUMPS!!!~~~~
                else:
                    play.change_velocity(play.jumpHeight)  # set new velocity to jumpHeight       ~~~~JUMPS!!!~~~~
                play.set_state("air")
                play.dashCount = 0
                play.canDash = True
                play.canJump = False
                print("weeeeeee")

        if play.running or play.walking or play.standing or play.dashing or play.landingLag or play.runTurn or play.blocking:
            play.set_state("air")
            play.actionable = True
            play.dashCount = 0
            play.canDash = True
            #play.canJump = True

        if play.actionable:

            if play.dropCount > 6:
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
            if play.blockkey and play.canBlock:
                play.canBlock = False
                play.set_state("airDodge")

            play.drift(stick)
            play.gravity(play.gWeight)

        if play.airDodge:
            play.air_dodge()

        if play.sliding:
            play.set_state("air")
          #  play.air = True
            play.actionable = True
         #   play.sliding = False
         #   play.airDodge = False

            # actionable if coming out of hitstun
            # actionable if sliding off of platform in shield


        play.air_friction()
