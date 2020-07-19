import pygame
import math

def text_objects(text, font):
    textSurface = font.render(text, True, (0, 0, 0))
    return textSurface, textSurface.get_rect()

def disp_state(win, player):
    font = pygame.font.SysFont("calibri", 28)
    win.blit(text_objects("air:  " + str(player.air), font)[0], (150, 150))
    win.blit(text_objects("standing:  " + str(player.standing), font)[0], (150, 200))
    win.blit(text_objects("walking:   " + str(player.walking), font)[0], (150, 250))
    win.blit(text_objects("running:   " + str(player.running), font)[0], (150, 300))
    win.blit(text_objects("grounded:  " + str(player.grounded), font)[0], (150, 350))
    win.blit(text_objects("resetFlag: " + str(player.resetFlag), font)[0], (150, 400))
    win.blit(text_objects("smashFlag: " + str(player.smashFlag), font)[0], (150, 450))
    win.blit(text_objects("isRight: " + str(player.isRight), font)[0], (150, 500))
    win.blit(text_objects("actionable: " + str(player.actionable), font)[0], (150, 550))
    win.blit(text_objects("landinglag: " + str(player.landingLag), font)[0], (150, 600))


    win.blit(text_objects("airDodge:  " + str(player.airDodge), font)[0], (1620, 150))
    win.blit(text_objects("sliding:  " + str(player.sliding), font)[0], (1620, 200))
    win.blit(text_objects("freeFall:  " + str(player.freeFall), font)[0], (1620, 250))

    win.blit(text_objects("jumpSquat:  " + str(player.jumpSquat), font)[0], (1620, 350))
    win.blit(text_objects("dashing:  " + str(player.dashing), font)[0], (1620, 400))
    win.blit(text_objects("dashCount:  " + str(player.dashCount), font)[0], (1620, 450))
    win.blit(text_objects("endLag:  " + str(player.endLag), font)[0], (1620, 550))
    win.blit(text_objects("state:  " + str(player.state), font)[0], (1620, 600))
    win.blit(text_objects("turnCount:  " + str(player.turnCount), font)[0], (1620, 650))
    win.blit(text_objects("dodgeCount:  " + str(player.dodgeCount), font)[0], (1620, 700))

    win.blit(text_objects("tiltTurn:  " + str(player.tiltTurn), font)[0], (1150, 150))
    win.blit(text_objects("smashTurn:  " + str(player.smashTurn), font)[0], (1150, 200))
    win.blit(text_objects("dashTurn:  " + str(player.runTurn), font)[0], (1150, 250))

    win.blit(text_objects("canDash:  " + str(player.canDash), font)[0], (800, 150))
    win.blit(text_objects("canJump:  " + str(player.canJump), font)[0], (800, 200))
    win.blit(text_objects("canBlock:  " + str(player.canBlock), font)[0], (800, 250))


    win.blit(text_objects("xCount:  " + str(player.xCount), font)[0], (550, 150))
    win.blit(text_objects("xVelocity: " + str(player.xVelocity), font)[0], (550, 200))
    win.blit(text_objects("x stick: " + str(player.main_stick[0]), font)[0], (550, 250))
    win.blit(text_objects("y stick: " + str(player.main_stick[1]), font)[0], (550, 300))
    angle = math.degrees(-math.atan2(player.main_stick[1], player.main_stick[0]))
    win.blit(text_objects("angle: " + str(angle), font)[0], (550, 350))

def disp_fps(win, clock):
    font = pygame.font.SysFont("calibri", 30)
    fps = int(clock.get_fps())
    win.blit(text_objects(str(fps), font)[0], (50, 50))
