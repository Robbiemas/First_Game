def resolve_action_state(player):

    stick = player.main_stick[0]
    walk = player.walking
    run = player.running


    # If not airborne select option
    if player.grounded:
        if stick == 0:  # if stick is centered
            run = False  # set run and walk to false
            walk = False
            if player.xVelocity == 0:   # if not sliding set standing to true
                player.standing = True
        if not (run or walk):
            player.apply_traction(player.xVelocity)  # if grounded and sliding apply traction

        if stick != 0:  # if using stick
       #     if player.standing:  # if not sliding and stick was previously centered (not walking or running but maybe sliding)
            player.standing = False  # stop standing
            if abs(stick) > 0.6 and not walk:  # when going from 0 to 0.8 in one frame and not walking
                print(str(stick))
                player.running = True               # set running true
                player.run(stick)        # start running
            if not player.running:                  # if stick is no longer at 0 and not running begin walk
                player.walking = True              # set walking true
                player.walk(stick)       # start walking
