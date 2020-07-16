import pygame
import math

def resolve_action_state(play):
    stick = play.main_stick[0]    # SAVE SPACE WHEN CALLING X AXIS
    walk = play.walking
    run = play.running

    if play.main_stick[1] == 0:  # resets the count for dropping through platforms if stick Y axis = 0
        play.dropCount = 0
    else:
        play.dropCount += 1

    if stick == 0:
        play.running = False
        play.walking = False
        play.xCount = 0
        play.canRun = True
      #  play.aniCount = 0
        play.dashCount = 0
        if play.dashing:
            play.dashing = False
            play.aniCount = 0


    if play.grounded:                       # if grounded
        play.freeFall = False                   # set cancel freeFall

        if play.xVelocity == 0:                 # STOPS SLIDING when not moving
            play.sliding = False

        if play.airDodge:                       # LANDING with an AIR DODGE
            play.jumpCount = play.jumps             # reset jumps
            play.airDodge = False                   # sets airDodge to FALSE
            play.air = True                         # sets air to TRUE to initialize landingLag
            play.sliding = True                     # set's sliding to True buy default

        if play.air:                                # LANDING from the AIR
            play.jumpCount = play.jumps                 # reset jumps
            play.timer = pygame.time.get_ticks()        # sets TIMER for landingLag
            play.air = False                            # sets air to FALSE
            play.landingLag = True                      # sets landingLag to TRUE

        if play.landingLag:                         # in LANDING LAG
            play.landLagCount += 1                      # count landLagCount
            play.standing = False                       # sets standing to FALSE
            play.actionable = False                     # no longer ACTIONABLE
            play.apply_traction(play.xVelocity)         # apply TRACTION to come to stop
            play.sliding = True                         # set SLIDING TRUE
            if play.landLagCount == 6:                      # wait 10 frames
                play.landingLag = False                         # set landingLag to FALSE
                play.actionable = True                          # make ACTIONABLE
                play.landLagCount = 0                           # reset langLagCount
                play.canRun = False

        if play.actionable:                     # when ACTIONABLE and GROUNDED

            if play.jumpSquat:                      # if in JUMPSQUAT
                play.jsCount += 1                       # add to jsCount

            if stick == 0 and not play.jumpSquat:   # if stick is centered and NOT in jumpSquat
                play.xCount = 0                         # reset counter for 2 frame dash, xCount
                play.running = False                    # set run and walk to FALSE
                play.walking = False
                if not play.sliding:                     # if NOT SLIDING
                    play.standing = True                    # set STANDING to TRUE
                if not (run or walk):                    # prevent TRACTION for RUN and WALK
                    play.apply_traction(play.xVelocity)  # if grounded, actionable, and not holding stick apply traction

            if stick != 0 and not play.jumpSquat:  # if using stick and not jumpSquat
                play.xCount += 1                        # add count to DASH COUNT
                play.standing = False                   # not longer standing

                if abs(stick) > 0.6 and play.xCount <= 2 and play.canRun:   # providing two frames for a dash input
                                                                            # the stick is past 0.6 on x, initiate dash
                    play.walking = False                            # sets walking to FALSE
                    play.dashing = True                             # sets dash to TRUE
                    if play.isRight:
                        play.xVelocity = play.initialDash
                    else:
                        play.xVelocity = -play.initialDash
                    play.aniCount = 0

                if play.dashCount > play.dashFrames:                # if dashFrames elapsed
                    print("hmmm")
                    play.running = True                     # set running TRUE
                    play.dashing = False                    # set dashing False
                    play.canRun = False                     # can no longer Dash until stopped running
                    play.run(stick)                         # ACTIVATE RUN
                if play.dashing:
                    play.dash(stick)            # ACTIVATE DASH
                    play.dashCount += 1
                 #   if

                if play.dashing or play.running:        # if stick is no longer at 0 and not running, begin walk
                    play.walking = False                         # set walking true
                elif not play.jumpSquat:
                    play.walking = True
                if play.walking:
                    play.walk(stick)                                # ACTIVATE WALK

            if play.jumpkey and play.canJump:               # if JUMP BUTTON pressed and canJump
                play.jsCount = 0                                # resets jsCount
                play.standing = False                           # turn off standing walking and running
                play.walking = False
                play.running = False
                play.jumpSquat = True                           # set JUMPSQUAT to TRUE
                play.canJump = False                            # set canJump to FALSE

            if play.jsCount >= play.js and play.jumpSquat:     # if done with JUMPSQUAT
                if play.canJump:
                 #   play.jumpSquat = False
                    play.reset_ground()             # set no longer grounded
                    play.change_velocity(play.shortHop)  # set new velocity to jumpHeight          ~~~~JUMPS!!!~~~~
                    play.jumpCount -= 1  # removes jump count
                    play.jumpWait += 1
                else:
                 #   play.jumpSquat = False
                    play.reset_ground()  # set no longer grounded
                    play.change_velocity(play.jumpHeight)  # set new velocity to jumpHeight       ~~~~JUMPS!!!~~~~
                    play.jumpCount -= 1  # removes jump count
                    play.jumpWait += 1



    else:  # if airborne
        play.jumpWait += 1

        if play.freeFall:
            play.gravity(play.gWeight)
            play.drift(stick)


        if play.jumpSquat:              # leaving jumpsquat
            play.jumpSquat = False
            play.actionable = True
            play.air = True
            play.aniCount = 0
        if play.running or play.walking or play.standing:
            play.walking = False
            play.standing = False
            play.running = False
            play.actionable = True
            play.air = True
            play.aniCount = 0

        if play.airDodge:
            play.dodgeCount += 1
            if play.xVelocity < 0:
                play.xVelocity = -((math.sqrt(abs(play.xVelocity)) - 0.2) ** 2)
            else:
                play.xVelocity = (math.sqrt(play.xVelocity) - 0.2) ** 2
            if play.yVelocity < 0:
                play.yVelocity = -((math.sqrt(abs(play.yVelocity)) - 0.2) ** 2)
            else:
                play.yVelocity = (math.sqrt(play.yVelocity) - 0.2) ** 2

            if play.dodgeCount >= 40:
                play.air = True
                play.airDodge = False
                play.freeFall = True
                play.dodgeCount = 0
                play.xVelocity = 0

        if play.sliding:
            play.air = True
            play.actionable = True
            play.sliding = False
            play.airDodge = False

            # actionable if coming out of hitstun
            # actionable if sliding off of platform in shield

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
                play.air_dodge()

            play.drift(stick)
            play.gravity(play.gWeight)


        play.air_friction()
