import pygame
import math
from StoreImages import DolphinMoleAnimations, run_image_store

d = {
    'standing': 0,
    'running': 1,
    'dashing': 2,
    'walking': 3,
    'air': 4,
    'landingLag': 5,
    'airDodge': 6,
    'jumpSquat': 7,
    'freeFall': 8,
    'turning': 9,
    'runTurn': 10,
    'blocking': 11,
    'shield': 12
}

def grab_images():
    global DMAni
    run_image_store()
    DMAni = DolphinMoleAnimations


def find_zoom(player1, player2):
    xDist = player2.x - player1.x
    yDist = player2.y - player2.y
    dist = int(math.hypot(xDist, yDist))
    if dist <= 200:
        return 3
    if dist >= 1600:
        return 0.375
    else:
        return 1 / (dist / 600)


def camera_adjust(player1, player2, monitor_size, true_scroll):
    xDist = player2.x - player1.x
    yDist = player2.y - player2.y
    dist = int(math.hypot(xDist, yDist))

    xCenter = (abs(player2.x - player1.x) / 2) + min(player1.x, player2.x)
    yCenter = (abs((player2.y - player2.height) - (player1.y - player1.height)) / 2) + min(player1.y, player2.y)

    true_scroll[0] += (xCenter - true_scroll[0] - (monitor_size[0] / 2)) / 80
    true_scroll[1] += (yCenter - true_scroll[1] - (monitor_size[1] / 2)) / 80
    scroll = true_scroll.copy()
    return [int(scroll[0]), int(scroll[1])]


def draw_bg(win, scroll):
    bg = pygame.image.load('background.png')
    win.blit(bg, (0 - scroll[0], 0 - scroll[1]))


def get_x(play, scroll):
    x = (play.x - (play.width // 2)) - scroll[0]
    return x


def get_y(play, scroll):
    y = (play.y - play.height) - scroll[1]
    return y

def draw_shield(win, play, scroll):
    if play.shieldHP > 60:
        play.shieldHP = 60
    if play.shieldHP < 0:
        play.shieldHP = 0
    size = int(play.shieldHP)
    #circle = pygame.draw.circle(win, (255, 0, 0), (play.widgth/2 - scroll[0], play.height/2 - scroll[1]), size)
    if size > 0:
        #pygame.draw.circle(win, (255, 0, 0), (int(play.x - scroll[0]), int(play.y - (play.height/2) - scroll[1])), size)
        if play.isRight:
            win.blit(DMAni[12][0], (get_x(play, scroll) + 30, get_y(play, scroll)))
        else:
            win.blit(pygame.transform.flip(DMAni[12][0], True, False), (get_x(play, scroll) - 10 , get_y(play, scroll)))


def get_mask(play):
    state = play.state
    if play.character == 'DolphinMole':
        if play.aniCount + 1 > len(DMAni[d[state]]):
            play.aniCount = 0

        play.height = DMAni[d[state]][play.aniCount].get_height()
        play.width = DMAni[d[state]][play.aniCount].get_width()

        if play.isRight:
            play.mask = pygame.mask.from_surface(DMAni[d[state]][play.aniCount])
            print(play.mask)
        else:
            play.mask = pygame.mask.from_surface(pygame.transform.flip(DMAni[d[state]][play.aniCount], True, False))


def draw_char(win, play, xpos, scroll):
    play.draw_lives(win, xpos, 50)
    state = play.state
    x = (play.x - (play.width // 2)) - scroll[0]
    y = (play.y - play.height) - scroll[1]

    if play.character == 'DolphinMole':
        if play.aniCount + 1 > len(DMAni[d[state]]):
            play.aniCount = 0

        #play.height = DMAni[d[state]][play.aniCount].get_height()
        #play.width = DMAni[d[state]][play.aniCount].get_width()

        if play.isRight:
            win.blit(DMAni[d[state]][play.aniCount], (get_x(play, scroll), get_y(play, scroll)))
            play.aniCount += 1
        else:
            win.blit(pygame.transform.flip(DMAni[d[state]][play.aniCount], True, False),
                     (get_x(play, scroll), get_y(play, scroll)))
            play.aniCount += 1

    if play.shielding:
        draw_shield(win, play, scroll)

def draw_ecb(win, self, scroll):
    ecb_bot = (self.x - scroll[0], self.y - scroll[1])
    ecb_top = (self.x - scroll[0], self.y - scroll[1] - self.height)
    ecb_left = (self.x - scroll[0] - self.width / 2, self.y - scroll[1] - self.height / 2)
    ecb_right = (self.x - scroll[0] + self.width / 2 - 1, self.y - scroll[1] - self.height / 2)
    pygame.draw.polygon(win, (255, 165, 0), (ecb_top, ecb_right, ecb_bot, ecb_left))


def draw_prev_ecb(win, self, scroll):
    ecb_bot = (self.prevX - scroll[0], self.prevY - scroll[1])
    ecb_top = (self.prevX - scroll[0], self.prevY - scroll[1] - self.height)
    ecb_left = (self.prevX - scroll[0] - self.width / 2, self.prevY - scroll[1] - self.height / 2)
    ecb_right = (self.prevX - scroll[0] + self.width / 2 - 1, self.prevY - scroll[1] - self.height / 2)
    pygame.draw.polygon(win, (235, 155, 0), (ecb_top, ecb_right, ecb_bot, ecb_left))


def draw_stage(win, plat, scroll):
    pygame.draw.rect(win, (0, 225, 0), (plat.x - scroll[0], plat.y - scroll[1], plat.w, plat.h))
