import pygame
from GatherInputs import gather_inputs
from DisplayInputs import text_objects, disp_state
import Camera


START_GAME = "start_game"
QUIT_GAME = "quit"


class MainMenu:
    def __init__(self, display, monitor_size, clock=None, fps=60):
        self.display = display
        self.monitor_size = monitor_size
        self.clock = clock or pygame.time.Clock()
        self.fps = fps
        self.start_button_rect = self._make_start_button_rect()

    def _make_start_button_rect(self):
        width, height = self.monitor_size
        button_width = max(220, min(360, int(width * 0.32)))
        button_height = 72
        return pygame.Rect(0, 0, button_width, button_height).move(
            (width - button_width) // 2,
            int(height * 0.58),
        )

    def resize(self, width, height):
        self.monitor_size[:] = [width, height]
        self.start_button_rect = self._make_start_button_rect()

    def handle_event(self, event):
        if event.type == pygame.QUIT:
            return QUIT_GAME
        if event.type == pygame.VIDEORESIZE:
            self.resize(event.w, event.h)
            return None
        if event.type == pygame.KEYDOWN:
            if event.key == pygame.K_ESCAPE:
                return QUIT_GAME
            if event.key in (pygame.K_RETURN, pygame.K_SPACE):
                return START_GAME
        if event.type == pygame.MOUSEBUTTONDOWN:
            if event.button == 1 and self.start_button_rect.collidepoint(event.pos):
                return START_GAME
        return None

    def draw(self):
        width, height = self.monitor_size
        self.display.fill((18, 24, 28))

        horizon_y = int(height * 0.68)
        pygame.draw.rect(self.display, (54, 94, 72), (0, horizon_y, width, height - horizon_y))
        pygame.draw.rect(self.display, (80, 136, 94), (0, horizon_y, width, 8))

        title_font = pygame.font.SysFont("calibri", 82, bold=True)
        title_surface = title_font.render("Mole Game", True, (235, 239, 232))
        title_rect = title_surface.get_rect(center=(width / 2, height * 0.36))
        self.display.blit(title_surface, title_rect)

        pygame.draw.rect(self.display, (231, 190, 86), self.start_button_rect, border_radius=8)
        pygame.draw.rect(self.display, (88, 64, 31), self.start_button_rect, width=3, border_radius=8)
        button_font = pygame.font.SysFont("calibri", 38, bold=True)
        button_surface = button_font.render("Start Game", True, (26, 27, 24))
        button_rect = button_surface.get_rect(center=self.start_button_rect.center)
        self.display.blit(button_surface, button_rect)

    def run(self):
        while True:
            self.clock.tick_busy_loop(self.fps)
            for event in pygame.event.get():
                action = self.handle_event(event)
                if action is not None:
                    return action
            self.draw()
            pygame.display.flip()


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
                if event.key == pygame.K_p:
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
