import pygame, sys
import glob
import time
from states import Pause, game_intro, text_objects
from platform import Platform
from stages import Stage
from Characters import Character
from GatherInputs import gather_inputs
from ChooseAction import resolve_action_state
from DisplayInputs import disp_state

pygame.init()
pygame.joystick.init()
clock = pygame.time.Clock()

#global monitor_size
monitor_size = [pygame.display.Info().current_w, pygame.display.Info().current_h]

#global win
win = pygame.display.set_mode(monitor_size, pygame.RESIZABLE)

fullscreen = False


joys = pygame.joystick.Joystick(0)
#joys2 = pygame.joystick.Joystick(1)
joysticks = []

glob.scale_fraction = 1

#Animations
bg = pygame.transform.scale(pygame.image.load('background.png'), monitor_size)

"""
crouching = []
crouchingL = []
walkRight = [pygame.image.load('R (1).png'), pygame.image.load('R (2).png'), pygame.image.load('R (3).png'),
             pygame.image.load('R (4).png'), pygame.image.load('R (5).png'), pygame.image.load('R (6).png'),
             pygame.image.load('R (7).png'), pygame.image.load('R (8).png')]
walkLeft = [pygame.image.load('L (1).png'), pygame.image.load('L (2).png'), pygame.image.load('L (3).png'),
            pygame.image.load('L (4).png'), pygame.image.load('L (5).png'), pygame.image.load('L (6).png'),
            pygame.image.load('L (7).png'), pygame.image.load('L (8).png')]
#char = pygame.image.load('standing.png')
#charL = pygame.image.load('standingL.png')
standing = []
fireBall = [pygame.image.load('Fireball1.png'), pygame.image.load('Fireball2.png'), pygame.image.load('Fireball3.png'),
            pygame.image.load('Fireball4.png'), pygame.image.load('Fireball5.png'), pygame.image.load('Fireball6.png'),
            pygame.image.load('Fireball7.png'), pygame.image.load('Fireball8.png')]
shootFire = [pygame.image.load('fball1.png'), pygame.image.load('fball2.png'), pygame.image.load('fball3.png'), pygame.image.load('fball4.png')]
shootFireL = [pygame.image.load('fball1L.png'), pygame.image.load('fball2L.png'), pygame.image.load('fball3L.png'), pygame.image.load('fball4L.png')]
jumpSquat = pygame.image.load('jumpsquat.png')
jumpSquatL = pygame.image.load('jumpsquatL.png')
air = pygame.image.load('air.png')
airL = pygame.image.load('airL.png')
crouchStart = []
crouchStartL = []
landingLag = []



for filename in glob.glob('standing/*.png'):
    im = pygame.image.load(filename)
    standing.append(im)

for filename in glob.glob('landingLag/*.png'):
    im = pygame.image.load(filename)
    landingLag.append(im)

for filename in glob.glob('crouchStart/*R.png'):
    im = pygame.image.load(filename)
    crouchStart.append(im)

for filename in glob.glob('crouchStart/*L.png'):
    im = pygame.image.load(filename)
    crouchStartL.append(im)

for filename in glob.glob('crouchingpng/*.png'):
    im = pygame.image.load(filename)
    crouching.append(im)
for filename in glob.glob('crouchingpngL/*.png'):
    im = pygame.image.load(filename)
    crouchingL.append(im)

"""


#def collision_check(player, platform):
  #  y = player.y
   # x = player.x
   # if (platform.y + platform.h >= y >= platform.y) and (platform.x <= x <= platform.x + platform.w):
   # else:
   #     return False

for x in range(pygame.joystick.get_count()):
    joysticks.append(pygame.joystick.Joystick(x))
    joysticks[-1].init()
    print("detected joystick" + str(joysticks[-1].get_name()))
   # print("x axis position" + str(js.get_axis(0)))


def message_display(text):
    largeText = pygame.font.Font('freesansbold.ttf',115)
    TextSurf, TextRect = text_objects(text, largeText)
    TextRect.center = (monitor_size[0]/2, monitor_size[1]/2)
    win.blit(TextSurf, TextRect)

    pygame.display.update()

    time.sleep(2)



"""
def redrawGameWindow():
   # pygame.transform(bg, monitor_size, win)
    win.blit(bg, (0, 0))
    mario.draw(win)
    mario.ecb()
    print(str(monitor_size))
    for bullet in bullets:
        bullet.draw(win)
    pygame.display.update()
"""

def gameloop():
    global monitor_size
    global win
    global fullscreen
    run = True
    stageSelection = Stage('first')
    platforms = stageSelection.load_platforms()
    players = []

    player1 = Character(stageSelection.spawn_position(1)[0], stageSelection.spawn_position(1)[1], players)
    player2 = Character(stageSelection.spawn_position(2)[0], stageSelection.spawn_position(2)[1], players)

    player1.choosechar("DolphinMole")
    player2.choosechar("DolphinMole")

    # Main loop
    while run:
        clock.tick(60)

        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                run = False
            if event.type == pygame.VIDEORESIZE:
                if not fullscreen:
                    win = pygame.display.set_mode((event.w, event.h), pygame.RESIZABLE)

                    ##scale characters and stage
                    monitor_size = [pygame.display.Info().current_w, pygame.display.Info().current_h]
                    ##glob.scale_fraction =

            if event.type == pygame.KEYDOWN:
                if event.key == pygame.K_ESCAPE:
                    pygame.quit()
                    sys.exit()
                if event.key == pygame.K_f:
                    fullscreen = not fullscreen
                    if fullscreen:
                        win = pygame.display.set_mode(monitor_size, pygame.FULLSCREEN)
                    else:
                        win = pygame.display.set_mode((win.get_width(), win.get_height()), pygame.RESIZABLE)
                if event.key == joys.get_button(7):
                    Pause(win, monitor_size)

        gather_inputs(player1, joys)
    #    gather_inputs(player2, joys2)



        player1.player_collision(player2)
        player2.player_collision(player1)

        for play in players:
            play.reset_ground()
            for plat in platforms:
                if play.collision_check(plat):
                    play.offset(play.x, plat.y)
                    play.is_grounded()
                    curr = plat

            if play.grounded:
                print("on plat")
                if play.jumpkey:
                    play.reset_ground()
                    play.change_velocity(play.jumpHeight)
                elif play.main_stick[1] > .3 and not curr.solid:
                    play.fast_fall(play.main_stick[1])
                    play.gravity(play.gWeight)
                    play.reset_ground()
                else:
                    play.changeY(curr.plat_y() - 1)

            # in air
            else:
                if not play.jumpkey and play.yVelocity >= 0:
                    play.change_velocity(play.yVelocity)
                play.fast_fall(play.main_stick[1])
                play.gravity(play.gWeight)  # normal gravity

        win.blit(bg, (0, 0))
        xpos = 100
        for play in players:
            resolve_action_state(play)
            play.set_prev_cords()
            play.move_x()
            play.move_y()  # move player
            play.check_death(stageSelection)
            if play.is_dead():
                print("you lose loser")
                play.new_game()
            play.draw_lives(win, xpos, 50)
            play.draw(win)
            play.draw_ecb(win)
            play.draw_prev_ecb(win)
            disp_state(win, play)

            xpos += 650

        # draw platforms
        for p in platforms:
            p.draw_platform(win)
        clock.tick(60)
        pygame.display.update()

gameloop()

"""
        for bullet in bullets:
            if monitor_size[0] > bullet.x > 0:
                bullet.projectile_active = True
                bullet.x += bullet.vel
                bullet.Bounciness(bullet.y)
            else:
                bullets.pop(bullets.index(bullet))
                bullet.projectile_active = False

        if keys[pygame.K_SPACE] or joys.get_button(1):
            if flag:
                startTime = pygame.time.get_ticks()
                if mario.isRight:
                    facing = 1
                else:
                    facing = -1
                if len(bullets) < 9 and flag:
                    bullets.append(projectile(round(mario.x + mario.width //2), round(mario.y + mario.height//2), 6, (0, 0, 0), facing))
                    flag = False

        if not flag and ((pygame.time.get_ticks() - startTime) >= 180) and doubleFire:
            flag = True
            doubleFire = False
        if not flag and ((pygame.time.get_ticks() - startTime) >= 540):
            flag = True
            doubleFire = True

        if not mario.jumpSquat and not mario.attacking and not mario.crouchStart and not landing:
            if (main_stick[0] < -.4 or keys[pygame.K_LEFT] or keys[pygame.K_a]) and mario.x > mario.vel and not mario.action:
                mario.x -= mario.vel
                mario.left = True
                mario.right = False
                mario.standing = False
                mario.walking = True
                mario.crouching = False
                cr = True
            elif (main_stick[0] > .4 or keys[pygame.K_RIGHT] or keys[pygame.K_d]) and mario.x < monitor_size[0] - mario.width and not mario.action:
                mario.x += mario.vel
                mario.left = False
                mario.right = True
                mario.standing = False
                mario.walking = True
                mario.crouching = False
                cr = True
            else:
                if not mario.isJump:
                    mario.standing = True
                mario.walkCount = 0
                mario.walking = False
        if mario.y < 0:
            mario.jumpCount = -3
        if not js and not landing:
            if keys[pygame.K_UP] or keys[pygame.K_w] or joys.get_button(3):
                mario.jumpSquat = True
                mario.crouching = False
                js = True
                jumpTime = pygame.time.get_ticks()
                mario.walkCount = 0
                cr = False
                mario.action = False
                canDown = False
        else:
            if (pygame.time.get_ticks() - jumpTime) >= 80 and js:
                mario.jumpSquat = False
                mario.isJump = True
                if mario.jumpCount >= -10:
                    neg = 1
                    if mario.jumpCount < 0:
                        neg = -1
                    mario.y -= (mario.jumpCount ** 2) * 0.3 * neg
                    mario.jumpCount -= 1
                else:
                    landTime = pygame.time.get_ticks()
                    landing = True
              #      canDown = True
                    js = False
                    mario.standing = False
                    mario.isJump = False
                    mario.jumpCount = 10
                    mario.action = True
            if landing:
                mario.landingLag = True
                if pygame.time.get_ticks() - landTime >= 50:
                    landing = False
                    mario.aniCount = 0
                    mario.landingLag = False
                    canDown = True

        if not mario.jumpSquat and not mario.isJump and not (keys[pygame.K_UP] or keys[pygame.K_w]):
            if (main_stick[1] > .4 or keys[pygame.K_s] or keys[pygame.K_DOWN]) and not mario.isJump and canDown:
                if cr:
                    mario.standing = False
                    mario.crouchStart = True
                    mario.crouching = False
                    mario.walking = False
                    mario.action = True
                    crouchTime = pygame.time.get_ticks()
                    mario.walkCount = 0
                    mario.aniCount = 0
                    cr = False

                if mario.crouchStart:
                    if pygame.time.get_ticks() - crouchTime >= 50:
                        mario.crouchStart = False
                        mario.crouching = True
                        mario.aniCount = 0
            else:
                cr = True
                mario.crouchStart = False
                mario.crouching = False
                mario.action = False
                
"""


