use crate::state::Vec2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EcbDiamond {
    pub top: Vec2,
    pub right: Vec2,
    pub bottom: Vec2,
    pub left: Vec2,
}

impl EcbDiamond {
    pub const fn from_bottom_center_and_size(bottom_center: Vec2, width: i32, height: i32) -> Self {
        let half_width = width / 2;
        let half_height = height / 2;

        Self {
            top: Vec2 {
                x: bottom_center.x,
                y: bottom_center.y + height,
            },
            right: Vec2 {
                x: bottom_center.x + half_width,
                y: bottom_center.y + half_height,
            },
            bottom: bottom_center,
            left: Vec2 {
                x: bottom_center.x - half_width,
                y: bottom_center.y + half_height,
            },
        }
    }

    pub const fn points(self) -> [Vec2; 4] {
        [self.top, self.right, self.bottom, self.left]
    }
}
