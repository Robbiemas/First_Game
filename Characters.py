import pygame
import math

X_TAP_START_THRESHOLD = 0.28
Y_TAP_START_THRESHOLD = 0.28
DASH_INPUT_THRESHOLD = 0.80
FAST_FALL_INPUT_THRESHOLD = 0.80
DASH_TAP_WINDOW = 2
FAST_FALL_TAP_WINDOW = 2
EXPIRED_TAP_TIMER = 0xFE
ESCAPE_AIR_SOURCE_DEADZONE = 20 / 127
ESCAPE_AIR_SOURCE_ACTION_FRAMES = 15
ESCAPE_AIR_SOURCE_DECAY = 0.90

CAPTAIN_FALCON_STATS = {
    "weight": 104,
    "initial_dash": 2.0,
    "run_speed": 2.3,
    "dash_frames": 15,
    "dash_accel_base": 0.01,
    "dash_accel_add": 0.15,
    "walk_speed": 0.85,
    "traction": 0.08,
    "air_friction": 0.01,
    "air_speed": 1.12,
    "air_accel_base": 0.02,
    "air_accel_add": 0.04,
    "gravity": 0.13,
    "fall_speed": 2.9,
    "fast_fall_speed": 3.5,
    "jumpsquat_frames": 4,
    "full_hop_height": 38.52,
    "short_hop_height": 14.85,
    "double_jump_height": 28.56,
    "empty_landing_lag": 4,
}

CAPTAIN_FALCON_FRAME_DATA = {
    "nair": {"first_active": 7, "last_active": 29, "total": 44, "landing_lag": 15},
    "uair": {"first_active": 6, "last_active": 13, "total": 33, "iasa": 30, "landing_lag": 15},
    "bair": {"first_active": 10, "last_active": 17, "total": 35, "iasa": 29, "landing_lag": 18},
    "fair": {"first_active": 14, "last_active": 30, "total": 39, "iasa": 36, "landing_lag": 19},
    "dair": {"first_active": 16, "last_active": 20, "total": 44, "iasa": 38, "landing_lag": 24},
    "jab1": {"first_active": 3, "last_active": 5, "total": 21, "iasa": 16},
    "jab2": {"first_active": 4, "last_active": 6, "total": 19, "iasa": 18},
    "jab3": {"first_active": 6, "last_active": 12, "total": 31, "iasa": 23},
    "dash_attack": {"first_active": 7, "last_active": 16, "total": 39, "iasa": 38},
    "fsmash": {"first_active": 18, "last_active": 21, "total": 64, "iasa": 60},
    "usmash": {"first_active": 21, "last_active": 28, "total": 54, "iasa": 40},
    "dsmash": {"first_active": 19, "last_active": 32, "total": 49, "iasa": 45},
    "ftilt": {"first_active": 9, "last_active": 11, "total": 29},
    "utilt": {"first_active": 17, "last_active": 21, "total": 39, "iasa": 38},
    "dtilt": {"first_active": 10, "last_active": 15, "total": 35, "iasa": 35},
    "falcon_punch": {"first_active": 52, "last_active": 56, "total": 99, "iasa": 65},
    "raptor_boost": {"first_active": 15, "last_active": 34, "total": 79},
    "falcon_kick": {"first_active": 14, "last_active": 32, "total": 64},
    "falcon_dive": {"first_active": 13, "last_active": 33, "total": 64},
    "grab": {"first_active": 7, "last_active": 8, "total": 30},
    "dash_grab": {"first_active": 11, "last_active": 12, "total": 40},
    "spotdodge": {"first_active": 3, "last_active": 20, "total": 32},
    "airdodge": {"total": 49},
    "roll_backward": {"first_active": 4, "last_active": 19, "total": 31},
    "roll_forward": {"first_active": 4, "last_active": 19, "total": 31},
}


def _simulated_jump_height(initial_velocity, gravity_step):
    y = 0
    velocity = initial_velocity
    highest_point = 0

    for _ in range(240):
        velocity += gravity_step
        y += velocity
        highest_point = min(highest_point, y)
        if velocity >= 0:
            break

    return -highest_point


def vertical_velocity_for_jump_height(height, gravity, scale):
    target_height = height * scale
    gravity_step = gravity * scale
    low = -max(1.0, target_height + gravity_step)
    high = 0.0

    while _simulated_jump_height(low, gravity_step) < target_height:
        low *= 2

    for _ in range(48):
        mid = (low + high) / 2
        if _simulated_jump_height(mid, gravity_step) < target_height:
            high = mid
        else:
            low = mid

    return (low + high) / 2


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
        self.landingFallSpecial = False
        self.landing = False
        self.endDash = False
        self.endLag = False

        self.actionable = True
        self.attacking = False
        self.sliding = False
        self.freeFall = False
        self.fallSpecial = False

        self.airDodge = False
        self.blocking = False
        self.guardOff = False
        self.canBlock = True
        self.shielding = False
        self.shieldTurn = False
        self.guardOffFrames = 15
        self.guardOffCount = 0
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
        self.jump_released_during_squat = False
        self.jumpWait = 0
        self.akey = 0
        self.grabkey = 0
        self.specialkey = 0
        self.blockkey = 0
        self.block_pressed = 0
        self.l_shieldkey = 0
        self.r_shieldkey = 0
        self.l_shield_pressed = 0
        self.r_shield_pressed = 0
        self.l_trigger_digital = 0
        self.r_trigger_digital = 0
        self.l_trigger_digital_pressed = 0
        self.r_trigger_digital_pressed = 0
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
        self.xTapDirection = 0
        self.xTapTimer = EXPIRED_TAP_TIMER
        self.yTapDirection = 0
        self.yTapTimer = EXPIRED_TAP_TIMER
        self.melee_x_tap_timer = None
        self.melee_y_tap_timer = None
        self.melee_dash_direction = None
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
        'landingFallSpecial': 'landingFallSpecial',
        'endDash': 'endDash',
        'hitstun': 'hitstun',
        'freeFall': 'freefall',
        'fallSpecial': 'fallSpecial',
        'airDodge': 'airDodge',
        'blocking': 'blocking',
        'guardOff': 'guardOff',
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

    def set_hit_boxes(self, attack, scroll):
        if self.isRight:
            o = 1
        else:
            o = -1
        if getattr(self, "character", None) == "DolphinMole":
            if attack == "jab":
                jab1 = self.frameData["jab1"]
                self.attackCount += 1

                # on frame 3 append jab 1 hitbox to self.hitboxes
                if self.attackCount == jab1["first_active"]:     #(pos, radius, type, damage, angle, baseKnockback, knockbackScaling, fixed)
                    self.hitBoxes.append(Hitbox(((self.x - scroll) + 40*o, (self.y - scroll) - 80), 10, type, 3, 90, 20, 100, False))
                # after 2 frames clear self.hitboxes
                if self.attackCount > jab1["last_active"]:
                    self.hitBoxes = []
                # after 19 frames end jab 1
                if self.attackCount == jab1["total"]:
                    # stop jab
                    self.attackCount = 0

    def check_hit(self, HITBOXES):
        for hitbox in HITBOXES:
            result = self.mask.overlap(hitbox, )

    def block(self):
        self.shieldHP += self.shieldDepletionRate
        self.shielding = True
        self.actionable = False
        self.apply_traction(self.xVelocity)

        if not self.blockkey:
            self.set_state("guardOff")
            self.shielding = False
            self.actionable = False
            self.canDash = False
            self.canJump = False
            self.canBlock = True
            self.guardOffCount = 0
            self.dodgeCount = 0
            self.shieldTurn = False
            return

        if self.isRight and self.main_stick[0] <= -0.23:
            self.shieldTurn = True
            self.dodgeCount = 0

        if not self.isRight and self.main_stick[0] >= 0.23:
            self.shieldTurn = True
            self.dodgeCount = 0

        if self.shieldTurn:
            # activate powershield availability
            self.dodgeCount += 1
            if self.dodgeCount == 5:
                self.dodgeCount = 0
                self.turnAround()
                self.shieldTurn = False

    def guard_off(self):
        self.shielding = False
        self.actionable = False
        self.canDash = False
        self.canBlock = True
        self.canJump = True
        self.apply_traction(self.xVelocity)
        self.guardOffCount += 1

        if self.guardOffCount >= self.guardOffFrames:
            self.set_state("standing")
            self.actionable = True
            self.canDash = True
            self.canBlock = True
            self.guardOffCount = 0
            self.dodgeCount = 0

    def turnAround(self):
        if self.isRight:
            self.isRight = False
        else:
            self.isRight = True

    def update_x_tap_timer(self, stick):
        if stick >= X_TAP_START_THRESHOLD:
            direction = 1
        elif stick <= -X_TAP_START_THRESHOLD:
            direction = -1
        else:
            direction = 0

        if direction == 0:
            self.xTapDirection = 0
            self.xTapTimer = EXPIRED_TAP_TIMER
        elif direction != self.xTapDirection:
            self.xTapDirection = direction
            self.xTapTimer = 0
        else:
            self.xTapTimer = min(self.xTapTimer + 1, EXPIRED_TAP_TIMER)

    def has_fresh_x_tap(self, stick):
        if stick >= DASH_INPUT_THRESHOLD:
            direction = 1
        elif stick <= -DASH_INPUT_THRESHOLD:
            direction = -1
        else:
            return False

        if self.melee_x_tap_timer is not None:
            return self.melee_dash_direction == direction

        return self.xTapDirection == direction and self.xTapTimer < DASH_TAP_WINDOW

    def update_y_tap_timer(self, stick):
        if stick >= Y_TAP_START_THRESHOLD:
            direction = 1
        elif stick <= -Y_TAP_START_THRESHOLD:
            direction = -1
        else:
            direction = 0

        if direction == 0:
            self.yTapDirection = 0
            self.yTapTimer = EXPIRED_TAP_TIMER
        elif direction != self.yTapDirection:
            self.yTapDirection = direction
            self.yTapTimer = 0
        else:
            self.yTapTimer = min(self.yTapTimer + 1, EXPIRED_TAP_TIMER)

    def has_fresh_down_tap(self, stick):
        return (
            stick >= FAST_FALL_INPUT_THRESHOLD
            and self.yTapDirection == 1
            and self.yTapTimer < FAST_FALL_TAP_WINDOW
        )

    def consume_y_tap(self):
        self.yTapTimer = EXPIRED_TAP_TIMER

    def turn(self, stick):
        fresh_x_tap = self.has_fresh_x_tap(stick)

        if self.turnCount < 2:              # if on frame 1 or 2

            if self.turnCount == 0:

                if abs(stick) >= 0.80:           # smash turn
                    self.canDash = True
                    self.smashTurn = True
                    self.turnAround()

                if 0.28 <= abs(stick) < 0.80:                 # tilt turn
                    self.tiltTurn = True
                    self.canDash = False

                if self.dashTurn and self.smashTurn:

                    if self.isRight:
                        self.xVelocity = self.initialDash * 6
                    else:
                        self.xVelocity = -self.initialDash * 6
                    self.set_state("dashing")  # DASH
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

            if self.smashTurn and self.turnCount == 1 and fresh_x_tap: # and self.smashFlag:    # if turn 2 and still in smashturn with flag
                if self.isRight:
                    self.xVelocity = self.initialDash * 6
                else:
                    self.xVelocity = -self.initialDash * 6
                self.set_state("dashing")                               # DASH
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
                if abs(stick) >= 0.8 and self.smashFlag and fresh_x_tap:  # smash turn
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
        gravity = gWeight * self.multiplier
        self.gCount = gravity
        if self.yVelocity < self.fallSpeed:
            self.yVelocity = min(self.yVelocity + gravity, self.fallSpeed)


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


    def max_run_velocity(self):
        return self.runSpeed * self.multiplier

    def max_walk_velocity(self):
        return self.walkSpeed * self.multiplier

    def ground_friction(self):
        return getattr(self, "groundFriction", self.traction * self.multiplier)

    def ground_accel_and_target(self, stick, max_velocity):
        if abs(stick) < 0.1:
            return 0, 0

        accel = (stick * self.dashAccelBase) + math.copysign(self.dashAccelAdd, stick)
        target_velocity = stick * max_velocity
        return accel * self.multiplier, target_velocity

    def approach_ground_velocity(self, accel, target_velocity, friction=None, max_velocity=None):
        if friction is None:
            friction = self.ground_friction()

        if target_velocity == 0:
            self.apply_traction(self.xVelocity)
            return

        if not (self.xVelocity * accel < 0):
            if accel > 0 and self.xVelocity + accel > target_velocity:
                accel = -friction
                if self.xVelocity + accel < target_velocity:
                    accel = target_velocity - self.xVelocity
            elif accel < 0 and self.xVelocity + accel < target_velocity:
                accel = friction
                if self.xVelocity + accel > target_velocity:
                    accel = target_velocity - self.xVelocity

        self.xVelocity += accel
        if max_velocity is not None:
            self.xVelocity = max(-max_velocity, min(self.xVelocity, max_velocity))

    def walk(self, xjoyvalue):
        stick = xjoyvalue
        if abs(stick) < 0.1:
            self.apply_traction(self.xVelocity)
            return

        max_walk_velocity = self.max_walk_velocity()
        target_velocity = stick * max_walk_velocity
        walk_init_velocity = getattr(self, "walkInitVelocity", max_walk_velocity * 0.10)
        walk_accel = getattr(self, "walkAccel", max_walk_velocity * 0.05)
        accel = (stick * walk_init_velocity) + (math.copysign(walk_accel, stick))

        self.approach_ground_velocity(
            accel,
            target_velocity,
            self.ground_friction(),
            max_walk_velocity,
        )

        self.isRight = stick > 0
        walk_ratio = abs(self.xVelocity) / max_walk_velocity if max_walk_velocity else 0
        self.walkSlow = walk_ratio < (1 / 3)
        self.walkMiddle = (1 / 3) <= walk_ratio < (2 / 3)
        self.walkFast = walk_ratio >= (2 / 3)
          #  else:
           #     self.xVelocity -= 0.20

    def dash(self, xjoyvalue):
        stick = xjoyvalue
        opposite_stick = (self.isRight and stick < 0) or (not self.isRight and stick > 0)
        opposite_smash_turn = opposite_stick and self.has_fresh_x_tap(stick)

        if not opposite_smash_turn:
            accel, target_velocity = self.ground_accel_and_target(stick, self.max_run_velocity())
            self.approach_ground_velocity(
                accel,
                target_velocity,
                self.ground_friction(),
                self.max_run_velocity(),
            )

        if self.dashCount >= self.dashFrames:  # if dashFrames elapsed
            if (self.isRight and stick >= 0.64) or (not self.isRight and stick <= -0.64):   # if dash is over, run
                self.set_state("running")
                self.dashCount = 0
            elif abs(stick) >= 0.1:
                self.isRight = stick > 0
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
        elif opposite_smash_turn and self.dashCount < self.dashFrames:
            self.set_state("standing")
            self.xVelocity = 0
            self.dashCount = 0
            self.actionable = True
            self.canDash = True
            self.dashTurn = True
            self.smashTurn = True
            #self.turnCount += 1

        self.dashCount += 1
        if self.dashCount == 4:
            self.canDash = True


    def run(self, xjoyvalue):
        stick = xjoyvalue
        same_direction = (stick > 0 and self.isRight) or (stick < 0 and not self.isRight)
        if not same_direction:
            self.apply_traction(self.xVelocity)
            return

        accel, target_velocity = self.ground_accel_and_target(stick, self.max_run_velocity())
        if target_velocity:
            velocity_fraction = self.xVelocity / target_velocity
            if 0 < velocity_fraction < 1:
                accel *= 1 - velocity_fraction

        self.approach_ground_velocity(
            accel,
            target_velocity,
            self.ground_friction(),
            self.max_run_velocity(),
        )

    def apply_traction(self, vel):
        vel *= 1 - self.traction
        if abs(vel) < 0.5:
            vel = 0
        self.xVelocity = vel

    def fast_fall(self, yjoyvalue):
        if (0 <= self.yVelocity < self.fastFallSpeed) and self.has_fresh_down_tap(yjoyvalue):
            self.yVelocity = self.fastFallSpeed
            self.consume_y_tap()

    def air_dodge(self):
        self.dodgeCount += 1

        if self.dodgeCount == 1:
            stick_x = self.main_stick[0]
            stick_y = self.main_stick[1]
            deadzone = getattr(self, "airDodgeDeadzone", ESCAPE_AIR_SOURCE_DEADZONE)
            if abs(stick_x) < deadzone and abs(stick_y) < deadzone:
                self.xVelocity = 0
                self.yVelocity = 0
            else:
                magnitude = math.hypot(stick_x, stick_y)
                force = getattr(self, "airDodgeForce", self.airDodgeLength)
                self.xVelocity = force * stick_x / magnitude
                self.yVelocity = force * stick_y / magnitude
            self.actionable = False
        elif self.dodgeCount < getattr(self, "airDodgeActionFrames", ESCAPE_AIR_SOURCE_ACTION_FRAMES):
            decay = getattr(self, "airDodgeDecay", ESCAPE_AIR_SOURCE_DECAY)
            self.xVelocity *= decay
            self.yVelocity *= decay
        else:
            self.xVelocity = 0
            self.yVelocity = 0

        if self.dodgeCount >= getattr(self, "airDodgeFrames", getattr(self, "airDodgeActionFrames", ESCAPE_AIR_SOURCE_ACTION_FRAMES)):
            self.set_state("fallSpecial")
            self.dodgeCount = 0

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
        stats = CAPTAIN_FALCON_STATS
        self.frameData = {
            name: values.copy()
            for name, values in CAPTAIN_FALCON_FRAME_DATA.items()
        }
        self.width = 56
        self.height = 142
        self.weight = stats["weight"]
        self.runSpeed = stats["run_speed"]
        self.walkSpeed = stats["walk_speed"]
        self.airSpeed = stats["air_speed"] * self.multiplier
        self.airAccelBase = stats["air_accel_base"]
        self.airAccelAdd = stats["air_accel_add"]
        self.airFriction = stats["air_friction"]
        self.traction = stats["traction"]
        self.walkInitVelocity = self.walkSpeed * self.multiplier * 0.10
        self.walkAccel = self.walkSpeed * self.multiplier * 0.05
        self.groundFriction = self.traction * self.multiplier
        self.jumps = 2
        self.jumpCount = 2
        self.js = stats["jumpsquat_frames"]
        self.landingLagFrames = stats["empty_landing_lag"]
        self.landingFallSpecialFrames = 10
        self.fallSpeed = stats["fall_speed"] * self.multiplier
        self.fastFallSpeed = stats["fast_fall_speed"] * self.multiplier
        self.dashFrames = stats["dash_frames"]
        self.initialDash = stats["initial_dash"]
        self.dashAccelBase = stats["dash_accel_base"]
        self.dashAccelAdd = stats["dash_accel_add"]
        self.rollLength = 200
        self.airDodgeFrames = self.frameData["airdodge"]["total"]
        self.airDodgeLength = 26
        self.airDodgeForce = self.airDodgeLength
        self.airDodgeDeadzone = ESCAPE_AIR_SOURCE_DEADZONE
        self.airDodgeActionFrames = ESCAPE_AIR_SOURCE_ACTION_FRAMES
        self.airDodgeDecay = ESCAPE_AIR_SOURCE_DECAY
        self.airDodgeResistance = 0.80
        self.fullHopHeight = stats["full_hop_height"]
        self.shortHopHeight = stats["short_hop_height"]
        self.doubleJumpHeight = stats["double_jump_height"]
        self.gWeight = stats["gravity"]
        self.fullHopVelocity = vertical_velocity_for_jump_height(
            self.fullHopHeight,
            self.gWeight,
            self.multiplier,
        )
        self.shortHopVelocity = vertical_velocity_for_jump_height(
            self.shortHopHeight,
            self.gWeight,
            self.multiplier,
        )
        self.doubleJumpVelocity = vertical_velocity_for_jump_height(
            self.doubleJumpHeight,
            self.gWeight,
            self.multiplier,
        )
        self.jumpHeight = self.fullHopVelocity
        self.shortHop = self.shortHopVelocity
        self.airJumpHeight = self.doubleJumpVelocity
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



