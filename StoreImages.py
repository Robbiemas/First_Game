import pygame
import glob
import json

DolphinMoleAnimations = []

def gather_images(character, state):        # gather images
    converted_images = []                   # array for converted images

    for filename in glob.glob(character + '/' + state + '/*.png'):      # for all png's of character state
        im = pygame.image.load(filename).convert_alpha()                # path converted image as im
        im_mask = pygame.mask.from_surface(im)
        converted_images.append(im)                                     # append im onto converted images array
        print(converted_images, character, state)
    return converted_images                                             # return array of converted images


def store_images(character, state):         # store images
    global DolphinMoleAnimations
    image_list = gather_images(character, state)            # take array of converted images of character state
    character_animation = str(character + "Animations")
    if str(character_animation) == "DolphinMoleAnimations":
        DolphinMoleAnimations.append(image_list)                             # append array state list per character


# order should match camera dictionary
animations = ['standing', 'running', 'dashing', 'walking', 'air', "landingLag", "airDodge", "jumpSquat", "freeFall",
              "turning", "runTurn", 'blocking', 'shield']
characters = ["DolphinMole", "NewBadGuy"]


def run_image_store():
    for character in characters:        # per character store an array of state animations
        for animation in animations:
            store_images(character, animation)
