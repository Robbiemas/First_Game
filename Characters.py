import pygame
import math

class Character(object):

    def __init__(self, x, y, players):
        self.x = x
        self.prevX = 0
        self.xVelocity = 0
        self.y = y
        self.prevY = 0
        self.yVelocity = 0
        self.isRight = True
        self.width = 0
        self.height = 0
        """
        
        
        self.weight = weight
        self.jumps = jumps     
        self.runSpeed = runspeed
        self.walkSpeed = walkspeed
        self.fallSpeed = fallspeed
        self.fastFallSpeed = ffspeed
        self.dashLength = dashlength
        self.rollLength = rolllength
        self.airDodgeLength = airdodgelength
        self.jumpHeight = jumpheight
        self.jumpSquatNumber = jumpsquatnumber
        """
        self.lives = 4
        self.collision = 0
        self.grounded = 0
        players.append(self)

        self.air = False
        self.standing = True
        self.walking = False
        self.walkSlow = False
        self.walkMiddle = False
        self.walkFast = False
        self.dash = False
        self.running = False
        self.canRun = True

        self.jumpSquat = False
        self.crouchStart = False
        self.crouching = False
        self.landingLag = False
        self.landing = False

        self.actionable = True
        self.attacking = False
        self.sliding = False
        self.freeFall = False

        self.airDodge = False
        self.blocking = False
        self.dodge = False
        self.roll = False

        self.ftilt = False
        self.utilt = False
        self.dtilt = False

        self.fsmash = False
        self.usmash = False
        self.dsmash = False

        self.fair = False
        self.bair = False
        self.uair = False
        self.dair = False
        self.nair = False

        #  Keys
        self.jumpkey = 0
        self.canJump = 0
        self.jumpWait = 0
        self.akey = 0
        self.grabkey = 0
        self.specialkey = 0
        self.blockkey = 0
        self.menukey = 0
        self.upkey = 0
        self.downkey = 0
        self.leftkey = 0
        self.rightkey = 0
        self.mainstickx = 0

        self.main_stick = [0, 0]
        self.c_stick = [0, 0]
        self.r_trigger = 0
        self.l_trigger = 0

        # counts

        self.aniCount = 0
        self.jumpCount = 0
        self.jsCount = 0            # jump squat counter
        self.walkCount = 0
        self.xCount = 0
        self.gCount = 1
        self.dodgeCount = 0
        self.landLagCount = 0
        self.dropCount = 0

        # timers
        self.timer = 0

    def set_prev_cords(self):
        self.prevX = self.x
        self.prevY = self.y

    # Checks if current ecb is below or
    def collision_check(self, platform):
        if (self.ecb()[1] >= platform.y - 1) and \
                (platform.x <= self.ecb()[0] <= platform.x + platform.w) and not self.collision and\
                (self.prev_ecb()[1] < platform.y and platform.x <= self.prev_ecb()[0] <= platform.x + platform.w):
            return True
        else:
            return False

    def get_grounded(self):
        return self.grounded

    def is_grounded(self):
        self.grounded = 1
        self.reset_y_velocity()
        self.gCount = 1
        # if ecb one pixel higher than platform or stage floor set ground to true

    def reset_ground(self):
        self.grounded = 0

    def gravity(self, gWeight):
        if self.yVelocity <= self.fallSpeed:
            self.gCount *= 1 + gWeight
            self.change_velocity(self.yVelocity + self.gCount)

    def call_x(self):
        return self.x

    def call_y(self):
        return self.y

    def move_x(self):
        self.x += self.xVelocity

    def move_y(self):
        self.y += self.yVelocity

    def change_velocity(self, vel):  # checks if given velocity is over the ff speed then sets new velocity
        #    self.y = self.y - vel * .05
        if vel > self.fastFallSpeed:
            self.yVelocity = self.fastFallSpeed
        else:
            self.yVelocity = vel

    def reset_x_velocity(self):
        self.xVelocity = 0

    def reset_y_velocity(self):
        self.yVelocity = 0

    def changeY(self, newY):
        self.y = newY

    def check_death(self, stage):
        if stage.stageName == 'first':
            if self.y > stage.lowerbound:
                self.lives += -1
                self.reset_x_velocity()
                self.reset_y_velocity()
            if self.y < stage.upperbound:
                self.lives += -1
                self.reset_x_velocity()
                self.reset_y_velocity()
            if self.x > stage.rightbound:
                self.lives += -1
                self.reset_x_velocity()
                self.reset_y_velocity()
            if self.x < stage.leftbound:
                self.lives += -1
                self.reset_x_velocity()
                self.reset_y_velocity()
        #  respawn
        #    self.changeY()
        #    self.changeX()

    def offset(self, offx, offy):
        self.x = offx
        self.y = offy

    def player_collision(self, player):
        if player.y - player.height < self.y <= player.y:
            if player.x + player.width / 2 + self.width / 2 < self.x < player.x - player.width / 2 - self.width / 2:
                if player.x < self.x:
                    self.xVelocity += 0.3
                else:
                    self.xVelocity += -0.3

    def new_game(self):
        self.lives = 4
        self.x = self.spawn[0]
        self.y = self.spawn[1]

    def jump(self, bool):
        self.jumpkey = bool

    def air_friction(self):
        if abs(self.xVelocity) > 0:
            if self.main_stick[0] == 0:
                self.xVelocity *= 1 - self.airFriction


    def drift(self, xjoyvalue): # defines how far you drift while holding the stick midair
        if -0.1 >= xjoyvalue >= -1.0 and self.xVelocity > -1 * self.airSpeed:  # left, velocity isn't at max airspeed
            self.xVelocity -= 5*self.airAccelBase                      # accel at base speed first
            self.xVelocity += 5*(self.airAccelAdd * xjoyvalue)         # add accel based on x value

        if 0.1 <= xjoyvalue <= 1.0 and self.xVelocity < self.airSpeed:  # right, velocity isn't at max airspeed
            self.xVelocity += 5*self.airAccelBase                      # accel at base speed first
            self.xVelocity += 5*(self.airAccelAdd * xjoyvalue)         # add accel based on x value


    def walk(self, xjoyvalue):
        if -0.1 >= xjoyvalue >= -1.0:  # left
            if -0.23 >= xjoyvalue >= -0.32:
                self.walkSlow = True
                self.walkMiddle = False
                self.walkFast = False
                self.xVelocity = -self.walkSpeed / 3
            if -0.33 >= xjoyvalue >= -0.53:
                self.walkMiddle = True
                self.walkSlow = False
                self.walkFast = False
                self.xVelocity = -self.walkSpeed / 2
            if xjoyvalue < -0.53:
                self.walkFast = True
                self.walkSlow = False
                self.walkMiddle = False
                self.xVelocity = self.walkSpeed * xjoyvalue
            self.isRight = False
        if 0.1 <= xjoyvalue <= 1.0:  # right
            if 0.23 <= xjoyvalue <= 0.32:
                self.walkSlow = True
                self.walkMiddle = False
                self.walkFast = False
                self.xVelocity = self.walkSpeed / 3
            if 0.33 <= xjoyvalue <= 0.53:
                self.walkMiddle = True
                self.walkSlow = False
                self.walkFast = False
                self.xVelocity = self.walkSpeed / 3
            if xjoyvalue > 0.53:
                self.walkFast = True
                self.walkSlow = False
                self.walkMiddle = False
                self.xVelocity = self.walkSpeed * xjoyvalue
            self.isRight = True
          #  else:
           #     self.xVelocity -= 0.20

    def run(self, xjoyvalue):
        if -0.2 >= xjoyvalue >= -1.0:  # left
            self.xVelocity = -(self.runSpeed)
            self.isRight = False

        if 0.2 <= xjoyvalue <= 1.0:  # right
            self.xVelocity = self.runSpeed
            self.isRight = True

    def apply_traction(self, vel):
        vel *= 1 - self.traction
        if abs(vel) < 0.5:
            vel = 0
        self.xVelocity = vel

    def fast_fall(self, yjoyvalue):
        if (0 <= self.yVelocity < self.fastFallSpeed) and yjoyvalue >= 0.8:
            self.yVelocity = self.fastFallSpeed

    def air_dodge(self):
        if self.main_stick == [0, 0]:
            self.xVelocity = 0
            self.yVelocity = 0
        else:
            self.xVelocity = self.angle_to_trajectory(self.trajectory_to_Angle())[0] * self.airDodgeLength
            self.yVelocity = self.angle_to_trajectory(self.trajectory_to_Angle())[1] * self.airDodgeLength
        self.airDodge = True
        self.air = False
        self.actionable = False

    def angle_to_trajectory(self, angle):
        yValue = -(math.sin(math.radians(angle)))
        xValue = (math.cos(math.radians(angle)))
        return [xValue, yValue]

    def trajectory_to_Angle(self):
        angle = math.degrees(-math.atan2(self.main_stick[1], self.main_stick[0]))
        angle %= 360
        return angle


    def crouch(self, yjoyvalue):
        if yjoyvalue > 0.3 and self.grounded:
            self.reset_x_velocity()

    def is_dead(self):
        if self.lives <= 0:
            return True
        return False

    def draw_lives(self, win, xpos, ypos):
        for i in range(0, self.lives):
            pygame.draw.rect(win, (255, 0, 0), (xpos, ypos, 10, 10))
            xpos += 15

        """
    def draw(self, win):
        x = self.x - (self.width // 2)
        y = self.y - self.height

        #  if self.walkCount + 1 >= "walk animation frame #":
        #      self.walkCount = 0

        # setting animations
        if self.crouching:
            if self.isRight:
                if self.aniCount + 1 >= "crouch animation frames":
                    self.aniCount = 0
                win.blit("crouch folder with pngs"[self.aniCount], (x, y))
                self.aniCount += 1
            else:
                if self.aniCount + 1 >= "crouch animation frames":
                    self.aniCount = 0
                win.blit(pygame.transform.flip("crouch folder with pngs"[self.aniCount], True, False), (x, y))
                self.aniCount += 1
        if self.standing:
            pygame.draw.rect(win, (255, 0, 0), (x + scroll[0], y + scroll[1], self.width, self.height))
            # actual code
        """
        """
            if self.isRight:
                if self.aniCount + 1 >= "standing animation frames":
                    self.aniCount = 0
                win.blit("stand folder"[self.aniCount], (x, y))
                self.aniCount += 1
            else:
                if self.aniCount + 1 >= "standing animation frames":
                    self.aniCount = 0
                win.blit(pygame.transform.flip("stand folder"[self.aniCount], True, False), (x, y))
                self.aniCount += 1
        """
        """
        if self.air:
            pygame.draw.rect(win, (255, 0, 0), (x + scroll[0], y + scroll[1], self.width, self.height))
            # actual code
        """


        """
        if self.isRight:
            if self.aniCount + 1 >= "air animation frames":
                self.aniCount = 0
            win.blit("air folder"[self.aniCount], (x, y))
            self.aniCount += 1
        else:
            if self.aniCount + 1 >= "air animation frames":
                self.aniCount = 0
            win.blit(pygame.transform.flip("air folder"[self.aniCount], True, False), (x, y))
            self.aniCount += 1
        """
        """
        if self.jumpSquat:
            pygame.draw.rect(win, (255, 0, 0), (x + scroll[0], y + 30 + scroll[1], self.width, self.height))
            # actual code
        """
        """
            if self.isRight:
                if self.aniCount + 1 >= "jumpquat animation frames":
                    self.aniCount = 0
                win.blit("jumpSquat folder"[self.aniCount], (x, y))
                self.aniCount += 1
            else:
                if self.aniCount + 1 >= "jumpsquat animation frames":
                    self.aniCount = 0
                win.blit("jumpSquat folder"[self.aniCount], (x, y))
                self.aniCount += 1
        """
        """
        if self.landingLag:
            pygame.draw.rect(win, (255, 0, 0), (x + scroll[0], y + 12 + scroll[1], self.width, self.height))
        """
        """
            # actual code
            if self.isRight:
                if self.aniCount + 1 >= "landing animation frames":
                    self.aniCount = 0
                win.blit("landingLag folder with pngs"[self.aniCount], (x, y))
                self.aniCount += 1
            else:
                if self.aniCount + 1 >= "landing animation frames":
                    self.aniCount = 0
                win.blit(pygame.transform.flip("landingLag folder with pngs"[self.aniCount], True, False), (x, y))
                self.aniCount += 1
        """
        """

        if self.crouchStart:
            if self.isRight:
                if self.aniCount + 1 >= "crouch start animation frames":
                    self.aniCount = 0
                win.blit("crouch start folder with pngs"[self.aniCount], (x, y))
                self.aniCount += 1
            else:
                if self.aniCount + 1 >= "crouch start animation frames":
                    self.aniCount = 0
                win.blit(pygame.transform.flip("crouch start folder with pngs"[self.aniCount], True, False), (x, y))
                self.aniCount += 1
        if self.dash:
            if self.isRight:
                if self.aniCount + 1 >= "dash animation frames":
                    self.aniCount = 0
                win.blit("dash folder"[self.aniCount], (x, y))
                self.aniCount += 1
            else:
                if self.aniCount + 1 >= "dash animation frames":
                    self.aniCount = 0
                win.blit(pygame.transform.flip("dash folder"[self.aniCount], True, False), (x, y))
                self.aniCount += 1

        if self.walking:
            pygame.draw.rect(win, (255, 0, 0), (x + scroll[0], y + scroll[1], self.width, self.height))
            # actual code
        """
        """
            # if holding past .30 on x stick, else do slow walk
            if self.isRight:
                if self.aniCount + 1 >= "walk animation frames":
                    self.aniCount = 0
                win.blit("walk folder"[self.aniCount], (x, y))
                self.aniCount += 1
            else:
                if self.aniCount + 1 >= "walk animation frames":
                    self.aniCount = 0
                win.blit(pygame.transform.flip("walk folder"[self.aniCount], True, False), (x, y))
                self.aniCount += 1
        """
        """

        if self.running:
            pygame.draw.rect(win, (175, 0, 0), (x + scroll[0], y + scroll[1], self.width, self.height))
            # actual code
        """
        """
            if self.isRight:
                if self.aniCount + 1 >= "run animation frames":
                    self.aniCount = 0
                win.blit("run folder"[self.aniCount], (x, y))
                self.aniCount += 1
            else:
                if self.aniCount + 1 >= "run animation frames":
                    self.aniCount = 0
                win.blit(pygame.transform.flip("run folder"[self.aniCount], True, False), (x, y))
                self.aniCount += 1
        """
        """

        if self.blocking:
            if self.isRight:
                if self.aniCount + 1 >= "block animation frames":
                    self.aniCount = 0
                win.blit("block folder"[self.aniCount], (x, y))
                self.aniCount += 1
            else:
                if self.aniCount + 1 >= "block animation frames":
                    self.aniCount = 0
                win.blit(pygame.transform.flip("block folder"[self.aniCount], True, False), (x, y))
                self.aniCount += 1
        if self.roll:
            if self.isRight:
                if self.aniCount + 1 >= "roll animation frames":
                    self.aniCount = 0
                win.blit("roll folder"[self.aniCount], (x, y))
                self.aniCount += 1
            else:
                if self.aniCount + 1 >= "roll animation frames":
                    self.aniCount = 0
                win.blit(pygame.transform.flip("roll folder"[self.aniCount], True, False), (x, y))
                self.aniCount += 1
        if self.dodge:
            if self.isRight:
                if self.aniCount + 1 >= "dodge animation frames":
                    self.aniCount = 0
                win.blit("dodge folder"[self.aniCount], (x, y))
                self.aniCount += 1
            else:
                if self.aniCount + 1 >= "dodge animation frames":
                    self.aniCount = 0
                win.blit(pygame.transform.flip("dodge folder"[self.aniCount], True, False), (x, y))
                self.aniCount += 1

        if self.uair:
            if self.isRight:
                if self.aniCount + 1 >= "uair animation frames":
                    self.aniCount = 0
                win.blit("uair folder"[self.aniCount], (x, y))
                self.aniCount += 1
            else:
                if self.aniCount + 1 >= "uair animation frames":
                    self.aniCount = 0
                win.blit(pygame.transform.flip("uair folder"[self.aniCount], True, False), (x, y))
                self.aniCount += 1
        if self.fair:
            if self.isRight:
                if self.aniCount + 1 >= "fair animation frames":
                    self.aniCount = 0
                win.blit("air folder"[self.aniCount], (x, y))
                self.aniCount += 1
            else:
                if self.aniCount + 1 >= "fair animation frames":
                    self.aniCount = 0
                win.blit(pygame.transform.flip("fair folder"[self.aniCount], True, False), (x, y))
                self.aniCount += 1
        if self.dair:
            if self.isRight:
                if self.aniCount + 1 >= "dair animation frames":
                    self.aniCount = 0
                win.blit("dair folder"[self.aniCount], (x, y))
                self.aniCount += 1
            else:
                if self.aniCount + 1 >= "dair animation frames":
                    self.aniCount = 0
                win.blit(pygame.transform.flip("dair folder"[self.aniCount], True, False), (x, y))
                self.aniCount += 1
        if self.bair:
            if self.isRight:
                if self.aniCount + 1 >= "bair animation frames":
                    self.aniCount = 0
                win.blit("bair folder"[self.aniCount], (x, y))
                self.aniCount += 1
            else:
                if self.aniCount + 1 >= "bair animation frames":
                    self.aniCount = 0
                win.blit(pygame.transform.flip("bair folder"[self.aniCount], True, False), (x, y))
                self.aniCount += 1
        if self.nair:
            if self.isRight:
                if self.aniCount + 1 >= "nair animation frames":
                    self.aniCount = 0
                win.blit("nair folder"[self.aniCount], (x, y))
                self.aniCount += 1
            else:
                if self.aniCount + 1 >= "nair animation frames":
                    self.aniCount = 0
                win.blit(pygame.transform.flip("nair folder"[self.aniCount], True, False), (x, y))
                self.aniCount += 1

        if self.ftilt:
            if self.isRight:
                if self.aniCount + 1 >= "ftilt animation frames":
                    self.aniCount = 0
                win.blit("ftilt folder"[self.aniCount], (x, y))
                self.aniCount += 1
            else:
                if self.aniCount + 1 >= "ftilt animation frames":
                    self.aniCount = 0
                win.blit(pygame.transform.flip("ftilt folder"[self.aniCount], True, False), (x, y))
                self.aniCount += 1
        if self.utilt:
            if self.isRight:
                if self.aniCount + 1 >= "utilt animation frames":
                    self.aniCount = 0
                win.blit("utilt folder"[self.aniCount], (x, y))
                self.aniCount += 1
            else:
                if self.aniCount + 1 >= "utilt animation frames":
                    self.aniCount = 0
                win.blit(pygame.transform.flip("utilt folder"[self.aniCount], True, False), (x, y))
                self.aniCount += 1
        if self.dtilt:
            if self.isRight:
                if self.aniCount + 1 >= "dtilt animation frames":
                    self.aniCount = 0
                win.blit("dtilt folder"[self.aniCount], (x, y))
                self.aniCount += 1
            else:
                if self.aniCount + 1 >= "dtilt animation frames":
                    self.aniCount = 0
                win.blit(pygame.transform.flip("dtilt folder"[self.aniCount], True, False), (x, y))
                self.aniCount += 1
    """

    def choosechar(self, char):
        if char == "DolphinMole":
            self.dolphinmole()

    def dolphinmole(self):
        self.width = 56
        self.height = 142
        self.weight = 30
        self.runSpeed = 12
        self.walkSpeed = 6
        self.airSpeed = 8
        self.airAccelBase = 0.02
        self.airAccelAdd = 0.06
        self.airFriction = 0.06
        self.traction = 0.12
        self.jumps = 2
        self.jumpCount = 2
        self.js = 5                         # jump squat frames
        self.fallSpeed = 16
        self.fastFallSpeed = 18
        self.dashLength = 150
        self.rollLength = 200
        self.airDodgeLength = 26
        self.airDodgeResistance = 0.80
        self.jumpHeight = -24
        self.shortHop = -18
        self.airJumpHeight = -28
        self.jumpSquatNumber = 3
        self.gWeight = 0.04


    def ecb(self):
        ecb_bot = (self.x, self.y)
        ecb_top = (self.x, self.y - self.height)
        ecb_left = (self.x - self.width / 2, self.y - self.height / 2)
        ecb_right = (self.x + self.width / 2 - 1, self.y - self.height / 2)
        return ecb_bot

    def draw_ecb(self, win):
        ecb_bot = (self.x, self.y)
        ecb_top = (self.x, self.y - self.height)
        ecb_left = (self.x - self.width / 2, self.y - self.height / 2)
        ecb_right = (self.x + self.width / 2 - 1, self.y - self.height / 2)
        pygame.draw.polygon(win, (255, 165, 0), (ecb_top, ecb_right, ecb_bot, ecb_left))

    def prev_ecb(self):
        ecb_bot = (self.prevX, self.prevY)
        ecb_top = (self.prevX, self.prevY - self.height)
        ecb_left = (self.prevX - self.width / 2, self.prevY - self.height / 2)
        ecb_right = (self.prevX + self.width / 2 - 1, self.prevY - self.height / 2)
        return ecb_bot

    def draw_prev_ecb(self, win):
        ecb_bot = (self.prevX, self.prevY)
        ecb_top = (self.prevX, self.prevY - self.height)
        ecb_left = (self.prevX - self.width / 2, self.prevY - self.height / 2)
        ecb_right = (self.prevX + self.width / 2 - 1, self.prevY - self.height / 2)
        pygame.draw.polygon(win, (235, 155, 0), (ecb_top, ecb_right, ecb_bot, ecb_left))


"""
class CharSelect(object):
    def __init__(self, width, height, weight, runspeed, walkspeed, jumps, fallspeed, ffspeed, dashlength, rolllength, airdodgelength, jumpheight, jumpsquatnumber):
        self.width = width
        self.height = height
        self.weight = weight
        self.runSpeed = runspeed
        self.walkSpeed = walkspeed
        self.jumps = jumps
        self.fallSpeed = fallspeed
        self.fastFallSpeed = ffspeed
        self.dashLength = dashlength
        self.rollLength = rolllength
        self.airDodgeLength = airdodgelength
        self.jumpHeight = jumpheight
        self.jumpSquatNumber = jumpsquatnumber

    def choosechar(self, char):
        if char == "DolphinMole":
            self.dolphinmole()

    def dolphinmole(self):
        self.width = 56
        self.height = 142
        self.weight = 30
        self.runSpeed = 10
        self.walkSpeed = 8
        self.jumps = 2
        self.fallSpeed = 5
        self.fastFallSpeed = 8
        self.dashLength = 150
        self.rollLength = 200
        self.airDodgeLength = 200
        self.jumpHeight = 200
        self.jumpSquatNumber = 5

"""
