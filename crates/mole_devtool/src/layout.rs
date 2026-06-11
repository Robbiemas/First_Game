#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SplitLayout {
    pub stacked: bool,
    pub left_width: f32,
    pub right_width: f32,
    pub body_height: f32,
}

pub fn responsive_split_layout(
    available_width: f32,
    available_height: f32,
    pane_count: usize,
    gap: f32,
) -> SplitLayout {
    let width = available_width.max(1.0);
    let height = available_height.max(1.0);
    let stacked = pane_count > 1 && width < 760.0;
    let min_height = if stacked { 180.0 } else { 220.0 };
    let preferred_height = if stacked {
        height * 0.42
    } else {
        height * 0.62
    };
    let body_height = bounded_child_height_from_preferred(height, min_height, preferred_height);
    if stacked || pane_count <= 1 {
        SplitLayout {
            stacked,
            left_width: width,
            right_width: width,
            body_height,
        }
    } else {
        let usable = (width - gap).max(1.0);
        let pane = usable / 2.0;
        SplitLayout {
            stacked,
            left_width: pane,
            right_width: pane,
            body_height,
        }
    }
}

pub fn bounded_child_height(available_height: f32, preferred_min: f32, preferred_max: f32) -> f32 {
    let height = available_height.max(1.0);
    let min = preferred_min.min(preferred_max).max(1.0);
    let preferred = (height * 0.72).clamp(min, preferred_max.max(min));
    bounded_child_height_from_preferred(height, min, preferred)
}

fn bounded_child_height_from_preferred(
    available_height: f32,
    preferred_min: f32,
    preferred: f32,
) -> f32 {
    let height = available_height.max(1.0);
    if height < preferred_min {
        height
    } else {
        preferred.clamp(preferred_min, height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn responsive_split_layout_keeps_wide_graphs_in_a_50_50_row() {
        let layout = responsive_split_layout(1200.0, 700.0, 2, 12.0);

        assert!(!layout.stacked);
        assert_eq!(layout.left_width, 594.0);
        assert_eq!(layout.right_width, 594.0);
        assert!(layout.body_height <= 700.0);
    }

    #[test]
    fn responsive_split_layout_stacks_narrow_graphs_without_overflow_width() {
        let layout = responsive_split_layout(620.0, 600.0, 2, 12.0);

        assert!(layout.stacked);
        assert_eq!(layout.left_width, 620.0);
        assert_eq!(layout.right_width, 620.0);
        assert!(layout.body_height <= 600.0);
    }

    #[test]
    fn responsive_split_layout_never_requests_more_height_than_available() {
        let layout = responsive_split_layout(320.0, 120.0, 2, 12.0);

        assert_eq!(layout.body_height, 120.0);
    }

    #[test]
    fn bounded_child_height_never_exceeds_available_space() {
        assert_eq!(bounded_child_height(96.0, 210.0, 320.0), 96.0);
        assert!(bounded_child_height(500.0, 210.0, 320.0) <= 500.0);
    }
}
