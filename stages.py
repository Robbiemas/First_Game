from platform import Platform


class Stage():
    def __init__(self, name):
        self.stageName = name
        if name == 'first':
            self.lowerbound = 1400
            self.upperbound = -250
            self.rightbound = 2250
            self.leftbound = -330
        self.floor = ((290, 710), (1620, 710))

    def load_platforms(self):
        platforms = set()
        if self.stageName == 'first':
            left = Platform(470, 530, 246, 10, platforms, 0)
            right = Platform(1196, 530, 246, 10, platforms, 0)
            top = Platform(833, 408, 246, 10, platforms, 0)
            floor = Platform(290, 710, 1360, 18, platforms, 1)
            return platforms

    def spawn_position(self, player):
        if player == 1:
            return 590, 480
        if player == 2:
            return 1320, 480
