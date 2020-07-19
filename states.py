import pygame
from GatherInputs import gather_inputs
from DisplayInputs import text_objects, disp_state
import Camera


def Pause(display, monitor_size, joys, player, scroll, platforms, xpos):

    text = pygame.font.SysFont("calibri", 115)
    TextSurf, TextRect = text_objects("PAUSED", text)
    TextRect.center = ((monitor_size[0] / 2), (monitor_size[1] / 2))
    display.blit(TextSurf, TextRect)
    while True:
        #display.fill([255, 255, 255])
        #Camera.draw_bg(display, scroll)
        gather_inputs(player, joys)
        disp_state(display, player)
        Camera.draw_prev_ecb(display, player, scroll)
        #Camera.draw_char(display, player, xpos, scroll)

        for event in pygame.event.get():  # Gets a list of all of the events that happen
            if event.type == pygame.QUIT:
                return
            if event.type == pygame.KEYDOWN:
                if event.key == pygame.K_ESCAPE:
                    return
                if event.key == joys.get_button(7):
                    return
                #if event.key == joys.get_button(4):
                 #   display.clock.tick(1)
        if player.menukey and player.menu:
            player.menu = False
            player.releasePause = True
            return
        if player.grabkey and player.releasePause:
            player.releasePause = False
            return
        for p in platforms:
            Camera.draw_stage(display, p, scroll)
        pygame.display.flip()



def game_intro(display, monitor_size):
    intro = True
    while intro:
        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                pygame.quit()
                quit()

        display.fill((255,255,255))
        largeText = pygame.font.Font('freesansbold.ttf', 40)
        TextSurf, TextRect = text_objects('Start Menu', largeText)
        TextRect.center = (monitor_size[0] / 2, monitor_size[1] / 2)
        display.blit(TextSurf, TextRect)
        pygame.display.update()

      #  if joys.get_button(7):
      #      glob.intro = False
       #     pygame.display.update()