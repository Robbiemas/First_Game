import pygame
from states import text_objects

def disp_state(win, player):
    font = pygame.font.SysFont("calibri", 45)
    win.blit(text_objects("standing:  " + str(player.standing), font)[0], (300, 200))
    win.blit(text_objects("walking:   " + str(player.walking), font)[0], (300, 300))
    win.blit(text_objects("running:   " + str(player.running), font)[0], (300, 400))
    win.blit(text_objects("grounded:  " + str(player.grounded), font)[0], (300, 500))
    win.blit(text_objects("crouching: " + str(player.crouching), font)[0], (300, 600))
