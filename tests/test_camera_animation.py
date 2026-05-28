import pygame


class RenderPlayerStub:
    character = "DolphinMole"
    state = "guardOff"
    aniCount = 0
    isRight = True
    x = 100
    y = 200
    width = 32
    height = 32
    shielding = False
    shieldHP = 60

    def draw_lives(self, _win, _x, _y):
        pass


def _surface(size):
    frame = pygame.Surface((size, size), pygame.SRCALPHA)
    frame.fill((255, 255, 255, 255))
    return frame


def _assert_state_renders_with_size(monkeypatch, state, expected_size):
    import Camera

    frames = [[_surface(10)] for _ in range(13)]
    frames[Camera.d["standing"]] = [_surface(16)]
    frames[Camera.d["landingLag"]] = [_surface(8)]
    frames[Camera.d["blocking"]] = [_surface(12)]
    monkeypatch.setattr(Camera, "DMAni", frames, raising=False)

    player = RenderPlayerStub()
    player.state = state
    window = pygame.Surface((320, 240), pygame.SRCALPHA)

    Camera.get_mask(player)
    Camera.draw_char(window, player, 0, [0, 0])

    assert player.mask.count() == expected_size * expected_size


def test_guard_off_uses_existing_render_animation(monkeypatch):
    _assert_state_renders_with_size(monkeypatch, "guardOff", 12)


def test_landing_fall_special_falls_back_to_standing_animation(monkeypatch):
    _assert_state_renders_with_size(monkeypatch, "landingFallSpecial", 16)


def test_unknown_state_falls_back_to_standing_animation(monkeypatch):
    _assert_state_renders_with_size(monkeypatch, "futureMeleeState", 16)


def test_empty_mapped_state_falls_back_to_standing_animation(monkeypatch):
    import Camera

    monkeypatch.setitem(Camera.d, "emptyMappedState", Camera.d["landingLag"])

    frames = [[_surface(10)] for _ in range(13)]
    frames[Camera.d["standing"]] = [_surface(16)]
    frames[Camera.d["landingLag"]] = []
    monkeypatch.setattr(Camera, "DMAni", frames, raising=False)

    player = RenderPlayerStub()
    player.state = "emptyMappedState"
    window = pygame.Surface((320, 240), pygame.SRCALPHA)

    Camera.get_mask(player)
    Camera.draw_char(window, player, 0, [0, 0])

    assert player.mask.count() == 16 * 16


def test_depleted_shield_still_draws_while_shield_state_is_active(monkeypatch):
    import Camera

    frames = [[_surface(10)] for _ in range(13)]
    shield = pygame.Surface((4, 4), pygame.SRCALPHA)
    shield.fill((255, 0, 0, 255))
    frames[Camera.d["shield"]] = [shield]
    monkeypatch.setattr(Camera, "DMAni", frames, raising=False)

    player = RenderPlayerStub()
    player.shielding = True
    player.shieldHP = 0
    window = pygame.Surface((320, 240), pygame.SRCALPHA)

    Camera.draw_shield(window, player, [0, 0])

    assert window.get_at((114, 168)) == pygame.Color(255, 0, 0, 255)
