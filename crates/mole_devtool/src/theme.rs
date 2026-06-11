use crate::ThemeMode;
use eframe::egui;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DevtoolTheme {
    pub app_background: egui::Color32,
    pub workbench_fill: egui::Color32,
    pub header_fill: egui::Color32,
    pub horizon_fill: egui::Color32,
    pub horizon_line: egui::Color32,
    pub title_text: egui::Color32,
    pub accent_fill: egui::Color32,
    pub accent_border: egui::Color32,
    pub accent_text: egui::Color32,
    pub panel_fill: egui::Color32,
    pub panel_border: egui::Color32,
    pub muted_text: egui::Color32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusPalette {
    pub fill: egui::Color32,
    pub border: egui::Color32,
    pub text: egui::Color32,
    pub accent_text: egui::Color32,
}

pub fn devtool_theme(theme: ThemeMode) -> DevtoolTheme {
    match theme {
        ThemeMode::Light => DevtoolTheme {
            app_background: egui::Color32::from_rgb(235, 239, 232),
            workbench_fill: egui::Color32::from_rgb(255, 255, 255),
            header_fill: egui::Color32::from_rgb(18, 24, 28),
            horizon_fill: egui::Color32::from_rgb(54, 94, 72),
            horizon_line: egui::Color32::from_rgb(80, 136, 94),
            title_text: egui::Color32::from_rgb(235, 239, 232),
            accent_fill: egui::Color32::from_rgb(231, 190, 86),
            accent_border: egui::Color32::from_rgb(88, 64, 31),
            accent_text: egui::Color32::from_rgb(26, 27, 24),
            panel_fill: egui::Color32::from_rgb(248, 250, 252),
            panel_border: egui::Color32::from_rgb(203, 213, 225),
            muted_text: egui::Color32::from_rgb(71, 85, 105),
        },
        ThemeMode::Dark => DevtoolTheme {
            app_background: egui::Color32::from_rgb(18, 24, 28),
            workbench_fill: egui::Color32::from_rgb(24, 33, 38),
            header_fill: egui::Color32::from_rgb(18, 24, 28),
            horizon_fill: egui::Color32::from_rgb(54, 94, 72),
            horizon_line: egui::Color32::from_rgb(80, 136, 94),
            title_text: egui::Color32::from_rgb(235, 239, 232),
            accent_fill: egui::Color32::from_rgb(231, 190, 86),
            accent_border: egui::Color32::from_rgb(88, 64, 31),
            accent_text: egui::Color32::from_rgb(26, 27, 24),
            panel_fill: egui::Color32::from_rgb(26, 37, 43),
            panel_border: egui::Color32::from_rgb(80, 136, 94),
            muted_text: egui::Color32::from_rgb(203, 213, 225),
        },
    }
}

pub fn header_palette(theme: ThemeMode) -> (egui::Color32, egui::Color32, egui::Color32) {
    let theme = devtool_theme(theme);
    (theme.horizon_fill, theme.horizon_line, theme.title_text)
}

pub fn status_palette(theme: ThemeMode, status: Option<&str>, selected: bool) -> StatusPalette {
    if selected {
        let theme = devtool_theme(theme);
        return StatusPalette {
            fill: theme.accent_fill,
            border: theme.accent_border,
            text: theme.accent_text,
            accent_text: theme.accent_text,
        };
    }

    match status {
        Some("match" | "aligned") => StatusPalette {
            fill: egui::Color32::from_rgb(220, 252, 231),
            border: egui::Color32::from_rgb(22, 163, 74),
            text: egui::Color32::from_rgb(5, 46, 22),
            accent_text: egui::Color32::from_rgb(5, 46, 22),
        },
        Some("partial") => StatusPalette {
            fill: egui::Color32::from_rgb(254, 249, 195),
            border: egui::Color32::from_rgb(202, 138, 4),
            text: egui::Color32::from_rgb(66, 32, 6),
            accent_text: egui::Color32::from_rgb(66, 32, 6),
        },
        Some("diff") => StatusPalette {
            fill: egui::Color32::from_rgb(255, 247, 237),
            border: egui::Color32::from_rgb(251, 146, 60),
            text: egui::Color32::from_rgb(67, 20, 7),
            accent_text: egui::Color32::from_rgb(194, 65, 12),
        },
        Some("mismatch") => StatusPalette {
            fill: egui::Color32::from_rgb(254, 226, 226),
            border: egui::Color32::from_rgb(220, 38, 38),
            text: egui::Color32::from_rgb(69, 10, 10),
            accent_text: egui::Color32::from_rgb(69, 10, 10),
        },
        Some("missing") => StatusPalette {
            fill: egui::Color32::from_rgb(254, 202, 202),
            border: egui::Color32::from_rgb(153, 27, 27),
            text: egui::Color32::from_rgb(69, 10, 10),
            accent_text: egui::Color32::from_rgb(69, 10, 10),
        },
        Some("derived" | "intentional") => StatusPalette {
            fill: egui::Color32::from_rgb(219, 234, 254),
            border: egui::Color32::from_rgb(37, 99, 235),
            text: egui::Color32::from_rgb(23, 37, 84),
            accent_text: egui::Color32::from_rgb(23, 37, 84),
        },
        Some("reference") => StatusPalette {
            fill: egui::Color32::from_rgb(238, 242, 247),
            border: egui::Color32::from_rgb(100, 116, 139),
            text: egui::Color32::from_rgb(15, 23, 42),
            accent_text: egui::Color32::from_rgb(15, 23, 42),
        },
        _ => {
            let theme = devtool_theme(theme);
            StatusPalette {
                fill: theme.panel_fill,
                border: theme.panel_border,
                text: theme.accent_text,
                accent_text: theme.muted_text,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_theme_keeps_pygame_frontend_identity_colors() {
        let theme = devtool_theme(ThemeMode::Light);

        assert_eq!(theme.header_fill, egui::Color32::from_rgb(18, 24, 28));
        assert_eq!(theme.horizon_fill, egui::Color32::from_rgb(54, 94, 72));
        assert_eq!(theme.horizon_line, egui::Color32::from_rgb(80, 136, 94));
        assert_eq!(theme.accent_fill, egui::Color32::from_rgb(231, 190, 86));
        assert_eq!(theme.accent_border, egui::Color32::from_rgb(88, 64, 31));
    }

    #[test]
    fn status_palette_matches_python_state_graph_styles() {
        let aligned = status_palette(ThemeMode::Light, Some("aligned"), false);
        let missing = status_palette(ThemeMode::Light, Some("missing"), false);
        let intentional = status_palette(ThemeMode::Light, Some("intentional"), false);

        assert_eq!(aligned.fill, egui::Color32::from_rgb(220, 252, 231));
        assert_eq!(aligned.border, egui::Color32::from_rgb(22, 163, 74));
        assert_eq!(missing.fill, egui::Color32::from_rgb(254, 202, 202));
        assert_eq!(missing.border, egui::Color32::from_rgb(153, 27, 27));
        assert_eq!(intentional.fill, egui::Color32::from_rgb(219, 234, 254));
        assert_eq!(intentional.border, egui::Color32::from_rgb(37, 99, 235));
    }

    #[test]
    fn diff_rows_keep_python_table_warning_tint_without_overloading_mismatch() {
        let diff = status_palette(ThemeMode::Light, Some("diff"), false);
        let mismatch = status_palette(ThemeMode::Light, Some("mismatch"), false);

        assert_eq!(diff.fill, egui::Color32::from_rgb(255, 247, 237));
        assert_eq!(diff.border, egui::Color32::from_rgb(251, 146, 60));
        assert_eq!(mismatch.fill, egui::Color32::from_rgb(254, 226, 226));
        assert_eq!(mismatch.border, egui::Color32::from_rgb(220, 38, 38));
    }
}
