import pygame, sys
from pygame.locals import *
import time
from states import Pause, text_objects
from stages import Stage
from Characters import Character
from GatherInputs import gather_inputs
from ChooseAction import resolve_action_state
from DisplayInputs import disp_fps
from DisplayInputs import disp_state

pygame.init()

framePassed = 0
initialized = False

if framePassed < 1:
    framePassed += 1
else:
    import Camera
    initialized = True



flags = FULLSCREEN | HWSURFACE | DOUBLEBUF | HWACCEL


#pygame.init()
pygame.joystick.init()
clock = pygame.time.Clock()

true_scroll = [0, 0]
zoom = 1

#global monitor_size
monitor_size = [pygame.display.Info().current_w, pygame.display.Info().current_h]

#global win
win = pygame.display.set_mode(monitor_size, flags)

fullscreen = True


joys = pygame.joystick.Joystick(0)
joys2 = pygame.joystick.Joystick(1)
joysticks = []



for x in range(pygame.joystick.get_count()):
    joysticks.append(pygame.joystick.Joystick(x))
    joysticks[-1].init()


def message_display(text):
    largeText = pygame.font.Font('freesansbold.ttf',115)
    TextSurf, TextRect = text_objects(text, largeText)
    TextRect.center = (monitor_size[0]/2, monitor_size[1]/2)
    win.blit(TextSurf, TextRect)

    pygame.display.update()

    time.sleep(2)



def gameloop():
    global monitor_size
    global win
    global fullscreen
    global flags
    global framePassed
    global initialized

    run = True
    stageSelection = Stage('first')
    platforms = stageSelection.load_platforms()
    players = []
    hasImportedCamera = False

    player1 = Character(stageSelection.spawn_position(1)[0], stageSelection.spawn_position(1)[1], players)
    player1.spawn = stageSelection.spawn_position(1)
    player2 = Character(stageSelection.spawn_position(2)[0], stageSelection.spawn_position(2)[1], players)
    player2.spawn = stageSelection.spawn_position(2)

    player1.choosechar("DolphinMole")
    player2.choosechar("DolphinMole")

    # Main loop
    while run:
        #clock.tick(60)
        clock.tick_busy_loop(60)
        win.fill([255, 255, 255])
        if framePassed < 1:
            framePassed += 1
        else:
            initialized = True
            if not hasImportedCamera:
                import Camera
                Camera.grab_images()

                hasImportedCamera = True

        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                run = False
            if event.type == pygame.VIDEORESIZE:
                if not fullscreen:
                    win = pygame.display.set_mode((event.w, event.h), pygame.RESIZABLE)
                    monitor_size = [pygame.display.Info().current_w, pygame.display.Info().current_h]

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
                if curr.solid or play.main_stick[1] < 0.55 or play.dropCount >= 5 or not play.actionable:
                    play.changeY(curr.plat_y() - 1)

        xpos = 100
        if initialized:
            scroll = Camera.camera_adjust(player1, player2, monitor_size, true_scroll)
            Camera.draw_bg(win, scroll)
            for play in players:
                gather_inputs(player1, joys)
                gather_inputs(player2, joys2)
                resolve_action_state(play)
                play.set_prev_cords()
                play.move_x()
                play.move_y()  # move player
                play.check_death(stageSelection)
                if play.is_dead():
                    print("you lose loser")
                    play.new_game()
                Camera.draw_prev_ecb(win, play, scroll)
                Camera.draw_char(win, play, xpos, scroll)
                if (play.menukey and play.menu) or (joys2.get_button(4) and not play.releasePause):
                    play.menu = False
                    Pause(win, monitor_size, joys2, play, scroll, platforms, xpos)
              #  Camera.draw_ecb(win, play, scroll)



                xpos += 650

            # draw platforms
           # map(lambda n: Camera.draw_stage(win, n, scroll), platforms)
            for p in platforms:
                Camera.draw_stage(win, p, scroll)
        #disp_state(win, player2)
        disp_fps(win, clock)
        pygame.display.flip()


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


