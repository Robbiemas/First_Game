import pygame
import math
from StoreImages import DolphinMoleAnimations, run_image_store


#DMAni = DolphinMoleAnimations
            ######## USE pygame.image.load(imagepath).convert_alpha()
            ######## when implimenting images
            ######## just use .conver() if no alpha in image

"""
standing = []

for filename in glob.glob('DolphinMole/standing/*.png'):
    im = pygame.image.load(filename).convert_alpha()
    standing.append(im)
"""


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


def scale(surface, xscale, yscale):
    return pygame.transform.scale(surface, [xscale, yscale])


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

def draw_char(win, play, xpos, scroll):
    play.draw_lives(win, xpos, 50)

    x = (play.x - (play.width // 2)) - scroll[0]
    y = (play.y - play.height) - scroll[1]

    #  if play.walkCount + 1 >= "walk animation frame #":
    #      play.walkCount = 0

    # setting animations
    if play.crouching:
        if play.isRight:
            if play.aniCount + 1 >= "crouch animation frames":
                play.aniCount = 0
            win.blit("crouch folder with pngs"[play.aniCount], (x, y))
            play.aniCount += 1
        else:
            if play.aniCount + 1 >= "crouch animation frames":
                play.aniCount = 0
            win.blit(pygame.transform.flip("crouch folder with pngs"[play.aniCount], True, False), (x, y))
            play.aniCount += 1
    if play.standing:
        play.height = DMAni[0][(play.aniCount // 50)-1].get_height()
        play.width = DMAni[0][(play.aniCount // 50)-1].get_width()
        if play.isRight:
            if play.aniCount + 1 > 100:
                play.aniCount = 0
            if play.character == 'DolphinMole':
                win.blit(DMAni[0][play.aniCount//50], (get_x(play, scroll), get_y(play, scroll)))        # [play.aniCount//8]
            play.aniCount += 1
        else:
            if play.aniCount + 1 > 100:
                play.aniCount = 0
            if play.character == 'DolphinMole':
                win.blit(pygame.transform.flip(DMAni[0][play.aniCount//50], True, False), (get_x(play, scroll), get_y(play, scroll)))
            play.aniCount += 1

      #  play.height = DMAni[0][play.aniCount//50].get_height()
      #  play.width = DMAni[0][play.aniCount//50].get_width()


    if play.air:
        play.height = DMAni[2][play.aniCount].get_height()
        play.width = DMAni[2][play.aniCount].get_width()

        if play.isRight:
            if play.aniCount + 1 >= 2:
                play.aniCount = 0
            if play.character == 'DolphinMole':
                win.blit(DMAni[2][play.aniCount], (get_x(play, scroll), get_y(play, scroll)))
            play.aniCount += 1
        else:
            if play.aniCount + 1 >= 2:
                play.aniCount = 0
            if play.character == 'DolphinMole':
                win.blit(pygame.transform.flip(DMAni[2][play.aniCount], True, False),
                         (get_x(play, scroll), get_y(play, scroll)))
            play.aniCount += 1

    if play.jumpSquat:
        pygame.draw.rect(win, (255, 0, 0), (x, y + 30, play.width, play.height))
        # actual code
        """
        if play.isRight:
            if play.aniCount + 1 >= "jumpquat animation frames":
                play.aniCount = 0
            win.blit("jumpSquat folder"[play.aniCount], (x, y))
            play.aniCount += 1
        else:
            if play.aniCount + 1 >= "jumpsquat animation frames":
                play.aniCount = 0
            win.blit("jumpSquat folder"[play.aniCount], (x, y))
            play.aniCount += 1
        """
    if play.landingLag:
        pygame.draw.rect(win, (255, 0, 0), (x, y + 12, play.width, play.height))
        """
        # actual code
        if play.isRight:
            if play.aniCount + 1 >= "landing animation frames":
                play.aniCount = 0
            win.blit("landingLag folder with pngs"[play.aniCount], (x, y))
            play.aniCount += 1
        else:
            if play.aniCount + 1 >= "landing animation frames":
                play.aniCount = 0
            win.blit(pygame.transform.flip("landingLag folder with pngs"[play.aniCount], True, False), (x, y))
            play.aniCount += 1
        """
    if play.crouchStart:
        if play.isRight:
            if play.aniCount + 1 >= "crouch start animation frames":
                play.aniCount = 0
            win.blit("crouch start folder with pngs"[play.aniCount], (x, y))
            play.aniCount += 1
        else:
            if play.aniCount + 1 >= "crouch start animation frames":
                play.aniCount = 0
            win.blit(pygame.transform.flip("crouch start folder with pngs"[play.aniCount], True, False), (x, y))
            play.aniCount += 1
    if play.dashing:
        play.height = DMAni[3][play.aniCount].get_height()
        play.width = DMAni[3][play.aniCount].get_width()

        if play.isRight:
            if play.aniCount + 1 >= 2:
                play.aniCount = 0
            if play.character == 'DolphinMole':
                win.blit(DMAni[3][play.aniCount], (get_x(play, scroll), get_y(play, scroll)))
            play.aniCount += 1
        else:
            if play.aniCount + 1 >= 2:
                play.aniCount = 0
            if play.character == 'DolphinMole':
                win.blit(pygame.transform.flip(DMAni[3][play.aniCount], True, False),
                         (get_x(play, scroll), get_y(play, scroll)))
            play.aniCount += 1

    if play.walking:
        pygame.draw.rect(win, (255, 0, 0), (x, y, play.width, play.height))
        # actual code
        """
        # if holding past .30 on x stick, else do slow walk
        if play.isRight:
            if play.aniCount + 1 >= "walk animation frames":
                play.aniCount = 0
            win.blit("walk folder"[play.aniCount], (x, y))
            play.aniCount += 1
        else:
            if play.aniCount + 1 >= "walk animation frames":
                play.aniCount = 0
            win.blit(pygame.transform.flip("walk folder"[play.aniCount], True, False), (x, y))
            play.aniCount += 1
        """

    if play.running:
        play.height = DMAni[1][play.aniCount].get_height()
        play.width = DMAni[1][play.aniCount].get_width()

        if play.isRight:
            if play.aniCount + 1 >= 2:
                play.aniCount = 0
            if play.character == 'DolphinMole':
                win.blit(DMAni[1][play.aniCount], (get_x(play, scroll), get_y(play, scroll)))
            play.aniCount += 1
        else:
            if play.aniCount + 1 >= 2:
                play.aniCount = 0
            if play.character == 'DolphinMole':
                win.blit(pygame.transform.flip(DMAni[1][play.aniCount], True, False), (get_x(play, scroll), get_y(play, scroll)))
            play.aniCount += 1



    if play.blocking:
        if play.isRight:
            if play.aniCount + 1 >= "block animation frames":
                play.aniCount = 0
            win.blit("block folder"[play.aniCount], (x, y))
            play.aniCount += 1
        else:
            if play.aniCount + 1 >= "block animation frames":
                play.aniCount = 0
            win.blit(pygame.transform.flip("block folder"[play.aniCount], True, False), (x, y))
            play.aniCount += 1
    if play.roll:
        if play.isRight:
            if play.aniCount + 1 >= "roll animation frames":
                play.aniCount = 0
            win.blit("roll folder"[play.aniCount], (x, y))
            play.aniCount += 1
        else:
            if play.aniCount + 1 >= "roll animation frames":
                play.aniCount = 0
            win.blit(pygame.transform.flip("roll folder"[play.aniCount], True, False), (x, y))
            play.aniCount += 1
    if play.dodge:
        if play.isRight:
            if play.aniCount + 1 >= "dodge animation frames":
                play.aniCount = 0
            win.blit("dodge folder"[play.aniCount], (x, y))
            play.aniCount += 1
        else:
            if play.aniCount + 1 >= "dodge animation frames":
                play.aniCount = 0
            win.blit(pygame.transform.flip("dodge folder"[play.aniCount], True, False), (x, y))
            play.aniCount += 1

    if play.uair:
        if play.isRight:
            if play.aniCount + 1 >= "uair animation frames":
                play.aniCount = 0
            win.blit("uair folder"[play.aniCount], (x, y))
            play.aniCount += 1
        else:
            if play.aniCount + 1 >= "uair animation frames":
                play.aniCount = 0
            win.blit(pygame.transform.flip("uair folder"[play.aniCount], True, False), (x, y))
            play.aniCount += 1
    if play.fair:
        if play.isRight:
            if play.aniCount + 1 >= "fair animation frames":
                play.aniCount = 0
            win.blit("air folder"[play.aniCount], (x, y))
            play.aniCount += 1
        else:
            if play.aniCount + 1 >= "fair animation frames":
                play.aniCount = 0
            win.blit(pygame.transform.flip("fair folder"[play.aniCount], True, False), (x, y))
            play.aniCount += 1
    if play.dair:
        if play.isRight:
            if play.aniCount + 1 >= "dair animation frames":
                play.aniCount = 0
            win.blit("dair folder"[play.aniCount], (x, y))
            play.aniCount += 1
        else:
            if play.aniCount + 1 >= "dair animation frames":
                play.aniCount = 0
            win.blit(pygame.transform.flip("dair folder"[play.aniCount], True, False), (x, y))
            play.aniCount += 1
    if play.bair:
        if play.isRight:
            if play.aniCount + 1 >= "bair animation frames":
                play.aniCount = 0
            win.blit("bair folder"[play.aniCount], (x, y))
            play.aniCount += 1
        else:
            if play.aniCount + 1 >= "bair animation frames":
                play.aniCount = 0
            win.blit(pygame.transform.flip("bair folder"[play.aniCount], True, False), (x, y))
            play.aniCount += 1
    if play.nair:
        if play.isRight:
            if play.aniCount + 1 >= "nair animation frames":
                play.aniCount = 0
            win.blit("nair folder"[play.aniCount], (x, y))
            play.aniCount += 1
        else:
            if play.aniCount + 1 >= "nair animation frames":
                play.aniCount = 0
            win.blit(pygame.transform.flip("nair folder"[play.aniCount], True, False), (x, y))
            play.aniCount += 1

    if play.ftilt:
        if play.isRight:
            if play.aniCount + 1 >= "ftilt animation frames":
                play.aniCount = 0
            win.blit("ftilt folder"[play.aniCount], (x, y))
            play.aniCount += 1
        else:
            if play.aniCount + 1 >= "ftilt animation frames":
                play.aniCount = 0
            win.blit(pygame.transform.flip("ftilt folder"[play.aniCount], True, False), (x, y))
            play.aniCount += 1
    if play.utilt:
        if play.isRight:
            if play.aniCount + 1 >= "utilt animation frames":
                play.aniCount = 0
            win.blit("utilt folder"[play.aniCount], (x, y))
            play.aniCount += 1
        else:
            if play.aniCount + 1 >= "utilt animation frames":
                play.aniCount = 0
            win.blit(pygame.transform.flip("utilt folder"[play.aniCount], True, False), (x, y))
            play.aniCount += 1
    if play.dtilt:
        if play.isRight:
            if play.aniCount + 1 >= "dtilt animation frames":
                play.aniCount = 0
            win.blit("dtilt folder"[play.aniCount], (x, y))
            play.aniCount += 1
        else:
            if play.aniCount + 1 >= "dtilt animation frames":
                play.aniCount = 0
            win.blit(pygame.transform.flip("dtilt folder"[play.aniCount], True, False), (x, y))
            play.aniCount += 1


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

