import pygame
import glob
import pygame_input
import pygame_assets
pygame.init()

screenWidth = 580
screenHeight = 476
floor = screenHeight / 2


win = pygame.display.set_mode((screenWidth, screenHeight))

pygame.display.set_caption("First Game")

crouching = []
crouchingL = []
walkRight = [pygame.image.load('R (1).png'), pygame.image.load('R (2).png'), pygame.image.load('R (3).png'),
             pygame.image.load('R (4).png'), pygame.image.load('R (5).png'), pygame.image.load('R (6).png'),
             pygame.image.load('R (7).png'), pygame.image.load('R (8).png')]
walkLeft = [pygame.image.load('L (1).png'), pygame.image.load('L (2).png'), pygame.image.load('L (3).png'),
            pygame.image.load('L (4).png'), pygame.image.load('L (5).png'), pygame.image.load('L (6).png'),
            pygame.image.load('L (7).png'), pygame.image.load('L (8).png')]
char = pygame.image.load('standing.png')
charL = pygame.image.load('standingL.png')
bg = pygame.image.load('FinalDestination.png')
fireBall = [pygame.image.load('Fireball1.png'), pygame.image.load('Fireball2.png'), pygame.image.load('Fireball3.png'),
            pygame.image.load('Fireball4.png'), pygame.image.load('Fireball5.png'), pygame.image.load('Fireball6.png'),
            pygame.image.load('Fireball7.png'), pygame.image.load('Fireball8.png')]
shootFire = [pygame.image.load('fball1.png'), pygame.image.load('fball2.png'), pygame.image.load('fball3.png'), pygame.image.load('fball4.png')]
shootFireL = [pygame.image.load('fball1L.png'), pygame.image.load('fball2L.png'), pygame.image.load('fball3L.png'), pygame.image.load('fball4L.png')]
jumpSquat = pygame.image.load('jumpsquat.png')
jumpSquatL = pygame.image.load('jumpsquatL.png')
air = pygame.image.load('air.png')
airL = pygame.image.load('airL.png')
crouchStart = []
crouchStartL = []

for filename in glob.glob('crouchStart/*R.png'):
    im = pygame.image.load(filename)
    crouchStart.append(im)

for filename in glob.glob('crouchStart/*L.png'):
    im = pygame.image.load(filename)
    crouchStartL.append(im)

for filename in glob.glob('crouchingpng/*.png'):
    im = pygame.image.load(filename)
    crouching.append(im)
for filename in glob.glob('crouchingpngL/*.png'):
    im = pygame.image.load(filename)
    crouchingL.append(im)


clock = pygame.time.Clock()


class Rotator:
    def __init__(self, screen_rect, master_image):
        self.screen_rect = screen_rect
        self.master_image = master_image
        self.image = self.master_image.copy()
        self.rect = self.image.get_rect(center=self.screen_rect.center)
        self.delay = 10
        self.timer = 0.0
        self.angle = 0

    def new_angle(self):
        self.angle += 6
        self.angle %= 360

    def rotate(self):
        self.new_angle()
        self.image = pygame.transform.rotate(self.master_image, self.angle)
        self.rect = self.image.get_rect(center=self.screen_rect.center)

    def update(self):
        if pygame.time.get_ticks() - self.timer > self.delay:
            self.timer = pygame.time.get_ticks()
            self.rotate()

    def draw(self, win):
        win.blit(self.image, self.rect)

class physics():

    def Bounciness(self, y):
        if self.y >= floor:
            if self.vCount >= -8:
                neg = 1
                if self.vCount < 0:
                    neg = -1
                self.y -= (self.vCount ** 2) * .5 * neg
                self.vCount -= 1

            else:
                self.vCount = 10
        else:
            self.y += self.gravity


class character(object):

    def __init__(self, x, y, width, height):
        self.x = x
        self.y = y
        self.width = width
        self.height = height
        self.vel = 5
        self.isJump = False
        self.jumpCount = 10
        self.left = False
        self.right = False
        self.isRight = True
        self.walkCount = 0
        self.standing = True
   #     self.isDrift = False
        self.walking = False
        self.jumpSquat = False
        self.aniCount = 0
        self.attacking = False
        self.crouching = False
        self.crouchStart = False
        self.action = False

    def draw(self, win):
        if self.walkCount + 1 >= 24:
            self.walkCount = 0

        if self.crouchStart:
            if self.isRight:
                if self.aniCount + 1 >= 3:
                    self.aniCount = 0
                win.blit(crouchStart[self.aniCount], (self.x, self.y))
                self.aniCount += 1
            else:
                if self.aniCount + 1 >= 3:
                    self.aniCount = 0
                win.blit(crouchStartL[self.aniCount], (self.x, self.y))
                self.aniCount += 1

        if self.crouching:
            if self.isRight:
                if self.aniCount + 1 >= 84:
                    self.aniCount = 0
                win.blit(crouching[self.aniCount], (self.x, self.y))
                self.aniCount += 1
            else:
                if self.aniCount + 1 >= 84:
                    self.aniCount = 0
                win.blit(crouchingL[self.aniCount], (self.x, self.y))
                self.aniCount += 1

        if not self.isJump and self.standing:
            if self.attacking:
                if self.aniCount + 1 >= 80:
                    self.aniCount = 0
                win.blit(shootFire[self.aniCount // 20], (self.x, self.y))
                self.aniCount += 1

        if self.jumpSquat:
            if self.isRight:
                win.blit(jumpSquat, (self.x, self.y))
            else:
                win.blit(jumpSquatL, (self.x, self.y))

        if not self.action and not self.standing and not self.isJump and not self.jumpSquat:
            self.walking = True
        else:
            self.walking = False

        if self.isJump:
            if self.isRight:
                win.blit(air, (self.x, self.y))
            else:
                win.blit(airL, (self.x, self.y))
        if self.walking:
            if self.left and not self.isJump:
                win.blit(walkLeft[self.walkCount//4], (self.x, self.y))
                self.walkCount += 1
                self.isRight = False
            elif self.right and not self.isJump:
                win.blit(walkRight[self.walkCount//4], (self.x, self.y))
                self.walkCount += 1
                self.isRight = True

        if self.standing and not self.isJump and not self.jumpSquat and not self.action:
            if self.right:
                win.blit(char, (self.x, self.y))
                self.isRight = True
            elif self.left:
                win.blit(charL, (self.x, self.y))
                self.isRight = False



class projectile(object):
    def __init__(self, x, y, radius, color, facing):
        self.x = x
        self.y = y
        self.radius = radius
        self.color = color
        self.facing = facing
        self.vel = 8 * facing
        self.aniCount = 0
        self.projectile_active = False
        self.gravity = 5
        self.vertical_V = 0
        self.vCount = 8
        self.bounce = False


    def draw(self, win):
        if self.aniCount + 1 >= 64:
            self.aniCount = 0
        if self.projectile_active:
            self.aniCount += 1
        win.blit(fireBall[self.aniCount//8], (self.x, self.y))
        ##     pygame.draw.circle(win, self.color, (self.x, self.y), self.radius)

    def Bounciness(self, y):
        if self.y >= floor:
            self.bounce = True
            self.vCount = self.vCount
        if not self.bounce:
            self.y += self.gravity
        else:
        ##    if self.y >= floor or self.vCount < 8:
            if self.vCount >= -8:
                neg = 1
                if self.vCount < 0:
                     neg = -1
                self.y -= (self.vCount ** 2) * .3 * neg
                self.vCount -= 1
                self.bounce = True
            else:
                self.vCount = 8
                self.bounce = False

def redrawGameWindow():

    win.blit(bg, (0, 0))
    mario.draw(win)
    for bullet in bullets:
        bullet.draw(win)
    pygame.display.update()


# mainloop
attacking = False
cr = True
js = False
flag = True
doubleFire = True
mario = character(screenWidth//2, screenHeight//2 - 60, 65, 82)
bullets = []
run = True
canDown = True

while run:
    clock.tick(60)

    for event in pygame.event.get():
        if event.type == pygame.QUIT:
            run = False

    for bullet in bullets:
        if 580 > bullet.x > 0:
            bullet.projectile_active = True
            bullet.x += bullet.vel
            bullet.Bounciness(bullet.y)
        else:
            bullets.pop(bullets.index(bullet))
            bullet.projectile_active = False

    keys = pygame.key.get_pressed()
    if keys[pygame.K_SPACE]:

      #  if not attacking:
      #      if mario.attacking and not mario.isJump:
      #          attacking = True
     #           mario.standing = False
      #          NeutralBTime = pygame.time.get_ticks()
     #           mario.walkCount = 0
      #  else:
      #      if (pygame.time.get_ticks() - NeutralBTime) >= 80 and attacking:
      #          attacking = False
      #          mario.attacking = False
        if flag:
        #    mario.attacking = True
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

    if not mario.jumpSquat and not mario.attacking and not mario.crouchStart:
        if (keys[pygame.K_LEFT] or keys[pygame.K_a]) and mario.x > mario.vel and not mario.action:
            mario.x -= mario.vel
            mario.left = True
            mario.right = False
            mario.standing = False
            mario.walking = True
            mario.crouching = False
            cr = True
        elif (keys[pygame.K_RIGHT] or keys[pygame.K_d]) and mario.x < screenWidth - mario.width and not mario.action:
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
    if not js:
        if keys[pygame.K_UP] or keys[pygame.K_w]:
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
            mario.candown = False
            if mario.jumpCount >= -10:
                neg = 1
                if mario.jumpCount < 0:
                    neg = -1
                mario.y -= (mario.jumpCount ** 2) * 0.3 * neg
                mario.jumpCount -= 1
            else:
                canDown = True
                js = False
                mario.standing = True
                mario.isJump = False
                mario.jumpCount = 10
    if not mario.jumpSquat and not mario.isJump and not keys[pygame.K_UP]:
        if (keys[pygame.K_s] or keys[pygame.K_DOWN]) and not mario.isJump and canDown:
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



    redrawGameWindow()

pygame.quit()
