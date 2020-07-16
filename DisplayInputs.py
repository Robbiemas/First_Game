import pygame
from states import text_objects

def disp_state(win, player):
    font = pygame.font.SysFont("calibri", 30)
    win.blit(text_objects("air:  " + str(player.air), font)[0], (300, 150))
    win.blit(text_objects("standing:  " + str(player.standing), font)[0], (300, 200))
    win.blit(text_objects("walking:   " + str(player.walking), font)[0], (300, 250))
    win.blit(text_objects("running:   " + str(player.running), font)[0], (300, 300))
    win.blit(text_objects("grounded:  " + str(player.grounded), font)[0], (300, 350))
    win.blit(text_objects("crouching: " + str(player.crouching), font)[0], (300, 400))
    win.blit(text_objects("jumpCount: " + str(player.jumpCount), font)[0], (300, 450))
    win.blit(text_objects("isRight: " + str(player.isRight), font)[0], (300, 500))
    win.blit(text_objects("actionable: " + str(player.actionable), font)[0], (300, 550))
    win.blit(text_objects("landinglag: " + str(player.landingLag), font)[0], (300, 600))

    win.blit(text_objects("airDodge:  " + str(player.airDodge), font)[0], (1520, 150))
    win.blit(text_objects("sliding:  " + str(player.sliding), font)[0], (1520, 200))
    win.blit(text_objects("freeFall:  " + str(player.freeFall), font)[0], (1520, 250))
    win.blit(text_objects("xCount:  " + str(player.xCount), font)[0], (1520, 300))
    win.blit(text_objects("jumpSquat:  " + str(player.jumpSquat), font)[0], (1520, 350))
    win.blit(text_objects("dashing:  " + str(player.dashing), font)[0], (1520, 400))
    win.blit(text_objects("dashCount:  " + str(player.dashCount), font)[0], (1520, 450))
    win.blit(text_objects("canRun:  " + str(player.canRun), font)[0], (1520, 500))

def disp_fps(win, clock):
    font = pygame.font.SysFont("calibri", 30)
    fps = int(clock.get_fps())
    win.blit(text_objects(str(fps), font)[0], (50, 50))
