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
        self.shieldDepletionRate = -0.28
        self.shieldRegenRate = 0.07
        self.state = "air"
        self.multiplier = 6
        self.hitBoxes = []
        players.append(self)

        self.air = False
        self.standing = True
        self.walking = False
        self.walkSlow = False
        self.walkMiddle = False
        self.walkFast = False
        self.dashing = False
        self.running = False
        self.canDash = True
        self.turning = False
        self.smashTurn = False
        self.tiltTurn = False
        self.runTurn = False
        self.dashTurn = False
        self.fromDash = False
        #self.canRun = True
        self.smashFlag = False
        self.resetFlag = False
        self.releasePause = True


        self.jumpSquat = False
        self.crouchStart = False
        self.crouching = False
        self.landingLag = False
        self.landing = False
        self.endDash = False
        self.endLag = False

        self.actionable = True
        self.attacking = False
        self.sliding = False
        self.freeFall = False

        self.airDodge = False
        self.blocking = False
        self.canBlock = True
        self.shielding = False
        self.shieldTurn = False
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
        self.dashCount = 0
        self.dashLagCount = 0
        self.turnCount = 0
        self.lagCount = 0
        self.attackCount = 0
        self.shieldHP = 60

        # timers
        self.timer = 0

    d = {
        'standing': 'standing',
        'running': 'running',
        'dashing': 'dashing',
        'air': 'air',
        'walking': 'walking',
        'jumpSquat': 'jumpSquat',
        'crouchStart': 'crouchStart',
        'crouching': 'crouching',
        'landingLag': 'landingLag',
        'endDash': 'endDash',
        'hitstun': 'hitstun',
        'freeFall': 'freefall',
        'airDodge': 'airDodge',
        'blocking': 'blocking',
        'shieldstun': 'shieldstun',
        'dodge': 'dodge',
        'roll': 'roll',
        'turning': 'turning',
        'runTurn': 'runTurn',
        'ftilt': 24,
        'utilt': 25,
        'dtilt': 26,
        'fsmash': 27,
        'usmash': 28,
        'dsmash': 29,
        'fair': 30,
        'bair': 31,
        'uair': 32,
        'dair': 33,
        'nair': 34,
        'jab': 35,
        'special': 37,
        'uspecial': 38,
        'dspecial': 39,
        'fspecial': 40

    }

    def set_state(self, state):  # sets state with reference state, disables all other states
        for ref in self.d:  # for each state in dictionary of states
            if str(state) == ref:  # if reference state == dictionary state in for loop

                setattr(self, state, True)
                #self.d[ref] = True  # set self.(reference state) = True
                self.state = ref# str(self.d[ref])
            else:
                setattr(self, ref, False)
                #if self.d[ref]:
                #    self.d[ref] = False  # else set self.(reference state) = False
        self.aniCount = 0
        print(self.state)

    def set_hit_boxes(self, attack, scroll):
        if self.isRight:
            o = 1
        else:
            o = -1
        if self.choosechar == "DolphinMole":
            if attack == "jab":
                self.attackCount += 1

                # on frame 3 append jab 1 hitbox to self.hitboxes
                if self.attackCount == 3:     #(pos, radius, type, damage, angle, baseKnockback, knockbackScaling, fixed)
                    self.hitBoxes.append(Hitbox(((self.x - scroll) + 40*o, (self.y - scroll) - 80), 10, type, 3, 90, 20, 100, False))
                # after 2 frames clear self.hitboxes
                if self.attackCount == 5:
                    self.hitBoxes = []
                # after 19 frames end jab 1
                if self.attackCount == 19:
                    # stop jab
                    self.attackCount = 0

    def check_hit(self, HITBOXES):
        for hitbox in HITBOXES:
            result = self.mask.overlap(hitbox, )

    def block(self):
        release = False
        self.shieldHP += self.shieldDepletionRate
        self.shielding = True
        self.actionable = False
        self.apply_traction(self.xVelocity)

        if self.canBlock:
            release = True
            self.dodgeCount += 1

        if self.isRight and self.main_stick[0] <= -0.23:
            self.shieldTurn = True
            self.dodgeCount = 0

        if not self.isRight and self.main_stick[0] >= 0.23:
            self.shieldTurn = True
            self.dodgeCount = 0

        if self.shieldTurn:
            # activate powershield availability
            release = False
            self.dodgeCount += 1
            if self.dodgeCount == 5:
                self.dodgeCount = 0
                self.turnAround()
                self.shieldTurn = False

        if release and self.dodgeCount > 15:
            self.set_state("standing")
            self.shielding = False
            self.actionable = True
            self.dodgeCount = 0
            self.canDash = True

    def turnAround(self):
        if self.isRight:
            self.isRight = False
        else:
            self.isRight = True

    def turn(self, stick):
        if self.turnCount < 2:              # if on frame 1 or 2

            if self.turnCount == 0:

                if abs(stick) >= 0.80:           # smash turn
                    self.canDash = True
                    self.smashTurn = True
                    self.turnAround()

                if 0.28 <= abs(stick) < -0.80:                 # tilt turn
                    self.tiltTurn = True
                    self.canDash = False

                if self.dashTurn and self.smashTurn:

                    if self.isRight:
                        self.xVelocity = self.initialDash * 6
                    else:
                        self.xVelocity = -self.initialDash * 6
                    self.set_state("dashing")  # DASH
                    print("smash")
                    self.smashFlag = False
                    self.smashTurn = False
                    self.tiltTurn = False
                    self.turnCount = -1
                    self.dashTurn = False

            if self.turnCount == 1:
                if abs(stick) >= 0.80:           # smash turn
                    if not self.smashTurn:
                        self.turnAround()
                    self.canDash = True
                    self.smashTurn = True
                    self.tiltTurn = False

                if abs(stick) < 0.80:                 # tilt turn
                    self.tiltTurn = True
                    self.canDash = False

            if self.smashTurn and self.turnCount == 1: # and self.smashFlag:    # if turn 2 and still in smashturn with flag
                if self.isRight:
                    self.xVelocity = self.initialDash * 6
                else:
                    self.xVelocity = -self.initialDash * 6
                self.set_state("dashing")                               # DASH
                print("smash")
                self.smashFlag = False
                self.smashTurn = False
                self.tiltTurn = False
                self.turnCount = -1

        if self.turnCount == 2:
           # if (stick == 0 and self.resetFlag) or (self.tiltTurn and self.resetFlag):
             #   self.set_state("standing")
             #   self.turnCount = 0
              #  self.canDash = True

            if abs(stick) > 0.28:
                self.canDash = False
                if abs(stick) >= 0.64:
                    self.smashFlag = True

        if 2 < self.turnCount <= self.turnFrames:
            if self.turnCount == self.tiltFrames and not self.smashTurn:
                self.turnAround()

            if self.tiltFrames < self.turnCount <= self.turnFrames:
                if abs(stick) >= 0.8 and self.smashFlag:  # smash turn
                    # self.canDash = True
                    if self.isRight:

                        self.xVelocity = self.initialDash * 6
                    else:
                        self.xVelocity = -self.initialDash * 6

                    self.set_state("dashing")  # DASH
                    self.turnCount = -1
                    self.tiltFlag = False
                    self.smashTurn = False
                    # self.canDash = False

        self.turnCount += 1

        if self.turnCount >= 12:
            self.set_state("standing")
            self.turnCount = 0
            self.tiltTurn = False
            self.smashTurn = False
            self.canDash = True



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


    def move_x(self):
        self.x += self.xVelocity

    def move_y(self):
        self.y += self.yVelocity

    def changeY(self, newY):
        self.y = newY

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
            self.xVelocity -= 10*self.airAccelBase                      # accel at base speed first
            self.xVelocity += 10*(self.airAccelAdd * xjoyvalue)         # add accel based on x value

        if 0.1 <= xjoyvalue <= 1.0 and self.xVelocity < self.airSpeed:  # right, velocity isn't at max airspeed
            self.xVelocity += 10*self.airAccelBase                      # accel at base speed first
            self.xVelocity += 10*(self.airAccelAdd * xjoyvalue)         # add accel based on x value


    def walk(self, xjoyvalue):
        m = self.multiplier
        if -0.1 >= xjoyvalue >= -1.0:  # left
            if 0 > xjoyvalue >= -0.32 and (self.xVelocity > (-self.walkSpeed/3) * m):
                self.walkSlow = True
                self.walkMiddle = False
                self.walkFast = False
                self.xVelocity += -self.walkSpeed
            if -0.33 >= xjoyvalue >= -0.53 and (self.xVelocity > (-self.walkSpeed * 2/3) * m):
                self.walkMiddle = True
                self.walkSlow = False
                self.walkFast = False
                self.xVelocity += -self.walkSpeed
            if xjoyvalue < -0.53 and (self.xVelocity > (-self.walkSpeed) * m):
                self.walkFast = True
                self.walkSlow = False
                self.walkMiddle = False
                self.xVelocity += self.walkSpeed * xjoyvalue
            self.isRight = False
        if 0.1 <= xjoyvalue <= 1.0:  # right
            if 0 < xjoyvalue <= 0.32 and (self.xVelocity < (self.walkSpeed/3) * m):
                self.walkSlow = True
                self.walkMiddle = False
                self.walkFast = False
                self.xVelocity += self.walkSpeed
            if 0.33 <= xjoyvalue <= 0.53 and (self.xVelocity < (self.walkSpeed * 2/3) * m):
                self.walkMiddle = True
                self.walkSlow = False
                self.walkFast = False
                self.xVelocity += self.walkSpeed
            if xjoyvalue > 0.53 and (self.xVelocity < (self.walkSpeed) * m):
                self.walkFast = True
                self.walkSlow = False
                self.walkMiddle = False
                self.xVelocity += self.walkSpeed * xjoyvalue
            self.isRight = True
          #  else:
           #     self.xVelocity -= 0.20

    def dash(self, xjoyvalue):
        stick = xjoyvalue

        if abs(stick) > 0.64:
            if self.xVelocity > 0:           
                if stick > 0 and (abs(self.xVelocity) < self.runSpeed * 6):
                    self.xVelocity *= 1 + self.dashAccelBase  # accel at base speed first
                    self.xVelocity *= 1 + (self.dashAccelAdd * stick)  # add accel based on x value
                elif abs(self.xVelocity) < self.runSpeed * 6:
                    self.xVelocity *= 1 - self.dashAccelBase
                    self.xVelocity *= 1 + (self.dashAccelAdd * stick)  # add accel based on x value
            elif self.xVelocity < 0:
                if stick < 0 and (abs(self.xVelocity) < self.runSpeed * 6):
                    self.xVelocity *= 1 + self.dashAccelBase  # accel at base speed first
                    self.xVelocity *= 1 + (self.dashAccelAdd * -stick)  # add accel based on x value
                elif abs(self.xVelocity) < self.runSpeed * 6:
                    self.xVelocity *= 1 - self.dashAccelBase
                    self.xVelocity *= 1 + (self.dashAccelAdd * -stick)  # add accel based on x value

        """
        if -0.1 >= xjoyvalue >= -1.0 and (self.xVelocity > -self.runSpeed * 8) and not self.isRight:

            if -0.64 >= xjoyvalue >= -0.76 and (self.xVelocity * 0.85 > 8 * -self.runSpeed):
                self.xVelocity *= 1 + self.dashAccelBase  # accel at base speed first
                self.xVelocity *= 1 + (self.dashAccelAdd * -stick)  # add accel based on x value
            if xjoyvalue < -0.76 and (self.xVelocity > 8 * (-self.runSpeed)):
                self.xVelocity *= 1 + self.dashAccelBase  # accel at base speed first
                self.xVelocity *= 1 + (self.dashAccelAdd * -stick)  # add accel based on x value

            #self.isRight = False

        if 0.1 <= xjoyvalue <= 1.0 and (self.xVelocity < self.runSpeed * 8) and self.isRight:  # right, velocity isn't at max airspeed

            if 0.64 <= xjoyvalue <= 0.76 and (self.xVelocity * 0.85 < 8 * self.runSpeed):
                self.xVelocity *= 1 + self.dashAccelBase  # accel at base speed first
                self.xVelocity *= 1 + (self.dashAccelAdd * xjoyvalue)  # add accel based on x value
            if xjoyvalue > 0.76 and (self.xVelocity < 8 * self.runSpeed):
                self.xVelocity *= 1 + self.dashAccelBase  # accel at base speed first
                self.xVelocity *= 1 + (self.dashAccelAdd * xjoyvalue)  # add accel based on x value
        """


        if self.dashCount >= self.dashFrames:  # if dashFrames elapsed
            if (self.isRight and stick >= 0.64) or (not self.isRight and stick <= -0.64):   # if dash is over, run
                self.set_state("running")
                self.dashCount = 0
            elif 0 < abs(stick) < 0.64:
                self.set_state("walking")
                self.actionable = True
                self.dashCount = 0
                #self.turnCount = 2
            else:
                self.set_state("standing")
                self.actionable = True
                self.dashCount = 0
                #self.turnCount = 2
                self.actionable = True
        elif ((self.isRight and stick <= -0.80) or (not self.isRight and stick >= 0.80)) and ((self.canDash and self.xCount < self.dashFrames) or self.dashCount == 1):
            self.set_state("standing")
            self.xVelocity = 0
            self.dashCount = 0
            self.actionable = True
            self.dashTurn = True
            self.smashTurn = True
            #self.turnCount += 1

        self.dashCount += 1
        if self.dashCount == 4:
            self.canDash = True


    def run(self, xjoyvalue):
        speed = self.xVelocity
        if -0.1 >= xjoyvalue >= -1.0 and not self.isRight:  # left
            if -0.64 >= xjoyvalue >= -0.76 and (self.xVelocity > 6 * (-self.runSpeed * 0.90)):
                self.xVelocity += -self.runSpeed * xjoyvalue * 0.85
            if xjoyvalue < -0.76 and (self.xVelocity > 6 * (-self.runSpeed)):
                self.xVelocity += self.runSpeed * xjoyvalue
            #self.isRight = False
        if 0.1 <= xjoyvalue <= 1.0 and self.isRight:  # right
            if 0.64 <= xjoyvalue <= 0.76 and (self.xVelocity < 6 * (self.runSpeed * 0.90)):
                self.xVelocity += self.runSpeed * xjoyvalue * 0.85
            if xjoyvalue > 0.76 and (self.xVelocity < 6 * self.runSpeed):
                self.xVelocity += self.runSpeed * xjoyvalue
            #self.isRight = True
        if not self.runTurn and (self.runSpeed == self.initialDash):
            self.xVelocity = speed



    def apply_traction(self, vel):
        vel *= 1 - self.traction
        if abs(vel) < 0.5:
            vel = 0
        self.xVelocity = vel

    def fast_fall(self, yjoyvalue):
        if (0 <= self.yVelocity < self.fastFallSpeed) and yjoyvalue >= 0.8:
            self.yVelocity = self.fastFallSpeed

    def air_dodge(self):
        self.dodgeCount += 1

        if self.dodgeCount == 1:
            if self.main_stick == [0, 0]:
                self.xVelocity = 0
                self.yVelocity = 0
            else:
                self.xVelocity = self.angle_to_trajectory(self.trajectory_to_Angle())[0] * self.airDodgeLength
                self.yVelocity = self.angle_to_trajectory(self.trajectory_to_Angle())[1] * self.airDodgeLength
            self.actionable = False
        else:
            if self.xVelocity < 0:
                self.xVelocity = -((math.sqrt(abs(self.xVelocity)) - 0.2) ** 2)
            else:
                self.xVelocity = (math.sqrt(self.xVelocity) - 0.2) ** 2
            if self.yVelocity < 0:
                self.yVelocity = -((math.sqrt(abs(self.yVelocity)) - 0.2) ** 2)
            else:
                self.yVelocity = (math.sqrt(self.yVelocity) - 0.2) ** 2

        if self.dodgeCount >= 40:
            self.set_state("freeFall")
            self.dodgeCount = 0
            self.xVelocity = 0

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

    def choosechar(self, char):
        if char == "DolphinMole":
            self.dolphinmole()

    def dolphinmole(self):
        self.character = 'DolphinMole'
        self.width = 56
        self.height = 142
        self.weight = 30
        self.runSpeed = 2.3
        self.walkSpeed = 0.85
        self.airSpeed = 8
        self.airAccelBase = 0.02
        self.airAccelAdd = 0.06
        self.airFriction = 0.06
        self.traction = 0.08
        self.jumps = 2
        self.jumpCount = 2
        self.js = 5                         # jump squat frames
        self.fallSpeed = 16
        self.fastFallSpeed = 18
        self.dashFrames = 15
        self.initialDash = 2
        self.dashAccelBase = 0.10
        self.dashAccelAdd = 0.15
        self.rollLength = 200
        self.airDodgeLength = 26
        self.airDodgeResistance = 0.80
        self.jumpHeight = -28
        self.shortHop = -18
        self.airJumpHeight = -28
        self.gWeight = 0.02
        self.turnFrames = 11
        self.tiltFrames = 5


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


class Hitbox(object):
    def __init__(self, win, pos, radius, type, damage, angle, baseKnockback, knockbackScaling, fixed):
        self.pos = pos
        self.radius = radius
        self.type = type
        self.damage = damage
        self.angle = angle
        self.baseKnockback = baseKnockback
        self.knockbackScaling = knockbackScaling
        self.fixed = fixed

        self.mask = pygame.mask.from_surface(pygame.draw.circle(win, (255, 0, 0), pos, radius, 1))



