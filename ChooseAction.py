from Characters import Character


def resolve_action_state(play):
    stick = play.main_stick[0]
    walk = play.walking
    run = play.running

    # If not airborne select option
    if play.grounded:
        play.jumpCount = play.jumps  #reset jumps
        print(str(play.jumpCount))
        if stick == 0:  # if stick is centered
            play.xCount = 0
            play.running = False  # set run and walk to false
            play.walking = False
            print(str(play.walking))
            if play.xVelocity == 0:  # if not sliding set standing to true
                play.standing = True
            if not (run or walk):
                print("stick TRACTION")
                play.apply_traction(play.xVelocity)  # if grounded and sliding apply traction

        if stick != 0:  # if using stick
            play.xCount += 1
            play.standing = False  # stop standing
            if abs(stick) > 0.6 and not walk and play.xCount > 1:  # if 2 frames have passed since moving stick and
                                                                # stick past 0.6 x, initiate run
                play.running = True                             # set running true
                play.run(stick)                                 # start running
            if not play.running and play.xCount > 1:            # if stick is no longer at 0 and not running begin walk
                play.walking = True                             # set walking true
                play.walk(stick)                                # start walking
        if play.jumpkey:    # if jump button pressed
            play.reset_ground()                     # set no longer grounded
            play.change_velocity(play.jumpHeight*1.5)   # set new velocity to jumpHeight             ~~~~JUMPS!!!~~~~
            play.jumpCount -= 1                     # removes jump count
    else:  # if airborne
        play.drift(stick)
        play.air_friction()

        if play.jumpkey and play.jumpCount == 1:    # if jump button pressed
       #     play.reset_ground()                     # set no longer grounded
            play.change_velocity(play.jumpHeight/2)   # set new velocity to jumpHeight             ~~~~JUMPS!!!~~~~
            play.gCount = 1
            play.jumpCount -= 1                     # removes jump count
        else:
            play.fast_fall(play.main_stick[1])
            play.gravity(play.gWeight)
            play.reset_ground()

