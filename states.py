import pygame


def text_objects(text, font):
    textSurface = font.render(text, True, (0, 0, 0))
    return textSurface, textSurface.get_rect()


def Pause(display, monitor_size):
    paused = True
    text = pygame.font.SysFont("calibri", 115)
    TextSurf, TextRect = text_objects("PAUSED", text)
    TextRect.center = ((monitor_size[0] / 2), (monitor_size[1] / 2))
    display.blit(TextSurf, TextRect)
    while paused:
        pygame.display.update()
        for event in pygame.event.get():  # Gets a list of all of the events that happen
            if event.type == pygame.QUIT:
                return
            if event.type == pygame.KEYDOWN:
                if event.key == pygame.K_ESCAPE:
                    return
                if event.key == pygame.K_p:
                    return


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