import pygame
import pygame.joystick



def gather_inputs(player, joystick):
    player.main_stick = [joystick.get_axis(0), joystick.get_axis(1)]
    player.c_stick = (joystick.get_axis(5), joystick.get_axis(4))
    player.r_trigger = joystick.get_axis(3)
    player.l_trigger = joystick.get_axis(2)
  #  keys = pygame.key.get_pressed()
   # buttons = joystick.get_buttons()

    # create deadzone
    if abs(player.main_stick[0]) < 0.22:
        player.main_stick[0] = 0
    if abs(player.main_stick[1]) < 0.22:
        player.main_stick[1] = 0


  #  if joystick.event == pygame.JOYBUTTONDOWN:
    if joystick.get_button(0):  # A
        player.akey = True
        print("controller pressed A")
    else:
        player.akey = False
    if joystick.get_button(1):  # B
        player.specialkey = True
    else:
        player.specialkey = False
    if joystick.get_button(2) or joystick.get_button(3):  # X and Y
        player.jumpkey = True
    else:
        player.jumpkey = False
        player.canJump = True
    if joystick.get_button(4):  # Z
        player.grabkey = True
    else:
        player.grabkey = False
    if joystick.get_button(5) or joystick.get_button(6):  # R and L
        player.blockkey = True
    else:
        player.blockkey = False
        player.canBlock = True
    if joystick.get_button(7):  # START
        player.menukey = True
    else:
        player.menukey = False
    if joystick.get_button(8):  # UP
        player.upkey = True
    else:
        player.upkey = False
    if joystick.get_button(9):  # DOWN
        player.downkey = True
    else:
        player.downkey = False
    if joystick.get_button(10):  # LEFT
        player.leftkey = True
    else:
        player.leftkey = False
    if joystick.get_button(11):  # RIGHT
        player.rightkey = True
    else:
        player.rightkey = False

  #  if player.main_stick[0] != 0 and player.xvelocity != 0:
     #   player.apply_traction(player.xvelocity)
