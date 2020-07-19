import pygame


class Platform:
    def __init__(self, x, y, w, h, platforms, solid):
        self.x = x
        self.y = y
        self.w = w
        self.h = h
        self.solid = solid
        platforms.add(self)

    def plat_y(self):
        return self.y

   # def get_h(self):
   #     return self.h

    def draw_platform(self, win):
        pygame.draw.rect(win, (0, 225, 0), (self.x, self.y, self.w, self.h))
