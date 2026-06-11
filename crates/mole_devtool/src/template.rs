use crate::{
    layout::bounded_child_height,
    theme::{header_palette, status_palette},
    ParityLedgerSurfaceRow, ParityLedgerSurfaceTab, ThemeMode,
};
use eframe::egui;

#[derive(Debug, Clone)]
pub struct LedgerTabTemplate {
    pub title: String,
    pub summary: String,
    pub headers: Vec<String>,
    pub rows: Vec<LedgerTabTemplateRow>,
}

#[derive(Debug, Clone)]
pub struct LedgerTabTemplateRow {
    pub cells: Vec<String>,
    pub detail: String,
    pub status: Option<String>,
}

impl LedgerTabTemplate {
    pub fn renders_separate_status_column(&self) -> bool {
        !self.headers.iter().any(|header| header == "Status")
            && self.rows.iter().any(|row| row.status.is_some())
    }

    pub fn display_headers(&self) -> Vec<String> {
        let mut headers = self.headers.clone();
        if self.renders_separate_status_column() {
            headers.push("Status".to_string());
        }
        headers
    }
}

impl From<&ParityLedgerSurfaceTab> for LedgerTabTemplate {
    fn from(tab: &ParityLedgerSurfaceTab) -> Self {
        Self {
            title: tab.label.clone(),
            summary: tab.summary.clone(),
            headers: tab.headers.clone(),
            rows: tab.rows.iter().map(LedgerTabTemplateRow::from).collect(),
        }
    }
}

impl From<&ParityLedgerSurfaceRow> for LedgerTabTemplateRow {
    fn from(row: &ParityLedgerSurfaceRow) -> Self {
        Self {
            cells: row.cells.clone(),
            detail: row.detail.clone(),
            status: row.status.clone(),
        }
    }
}

pub fn render_spreadsheet_table(
    ui: &mut egui::Ui,
    theme: ThemeMode,
    template: &LedgerTabTemplate,
    selected_row: &mut usize,
    mut on_select: impl FnMut(usize),
) {
    if template.rows.is_empty() {
        ui.label("No rows available.");
        return;
    }

    *selected_row = (*selected_row).min(template.rows.len().saturating_sub(1));
    let row_index = *selected_row;

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .max_height(bounded_child_height(ui.available_height(), 160.0, 420.0))
        .show(ui, |ui| {
            let display_headers = template.display_headers();
            let column_widths = responsive_column_widths(&display_headers, ui.available_width());
            let separate_status_column = template.renders_separate_status_column();
            render_table_headers(ui, theme, &display_headers, &column_widths);
            for (index, row) in template.rows.iter().enumerate() {
                render_colored_row(
                    ui,
                    theme,
                    index,
                    row_index == index,
                    row,
                    separate_status_column,
                    &column_widths,
                    |clicked_row| {
                        *selected_row = clicked_row;
                        on_select(clicked_row);
                    },
                );
            }
        });

    ui.separator();
    if let Some(row) = template.rows.get(*selected_row) {
        ui.group(|ui| {
            if let Some(status) = &row.status {
                ui.label(format!("Status: {status}"));
            }
            ui.label(&row.detail);
        });
    }
}

fn render_table_headers(
    ui: &mut egui::Ui,
    theme: ThemeMode,
    headers: &[String],
    column_widths: &[f32],
) {
    let (fill, border, text) = header_palette(theme);
    ui.horizontal(|ui| {
        for (index, header) in headers.iter().enumerate() {
            let width = column_widths
                .get(index)
                .copied()
                .unwrap_or_else(|| base_cell_width(index));
            header_cell(ui, header, width, fill, border, text);
        }
    });
    ui.separator();
}

fn render_colored_row(
    ui: &mut egui::Ui,
    theme: ThemeMode,
    index: usize,
    selected: bool,
    row: &LedgerTabTemplateRow,
    separate_status_column: bool,
    column_widths: &[f32],
    mut on_select: impl FnMut(usize),
) {
    let palette = status_palette(theme, row.status.as_deref(), selected);
    let stroke = if selected {
        egui::Stroke::new(1.5, egui::Color32::from_rgb(96, 165, 250))
    } else {
        egui::Stroke::new(1.0, palette.border)
    };

    egui::Frame::NONE
        .fill(palette.fill)
        .stroke(stroke)
        .inner_margin(egui::Margin::symmetric(2, 2))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                for (cell_index, cell) in row.cells.iter().enumerate() {
                    let width = column_widths
                        .get(cell_index)
                        .copied()
                        .unwrap_or_else(|| base_cell_width(cell_index));
                    cell_frame(ui, width, palette.fill, palette.border, |ui| {
                        let response = ui.add_sized(
                            [inner_cell_width(width), 18.0],
                            egui::Button::new(egui::RichText::new(cell).color(palette.text))
                                .selected(selected && cell_index == 0),
                        );
                        if response.clicked() && cell_index == 0 {
                            on_select(index);
                        }
                    });
                }
                if separate_status_column {
                    let status = row.status.as_deref().unwrap_or("");
                    let status_width = column_widths.get(row.cells.len()).copied().unwrap_or(92.0);
                    cell_frame(ui, status_width, palette.fill, palette.border, |ui| {
                        ui.add_sized(
                            [inner_cell_width(status_width), 18.0],
                            egui::Label::new(
                                egui::RichText::new(status)
                                    .strong()
                                    .color(palette.accent_text),
                            ),
                        );
                    });
                }
            });
        });
    ui.add_space(4.0);
}

fn header_cell(
    ui: &mut egui::Ui,
    text: &str,
    width: f32,
    fill: egui::Color32,
    border: egui::Color32,
    text_color: egui::Color32,
) {
    cell_frame(ui, width, fill, border, |ui| {
        ui.add_sized(
            [inner_cell_width(width), 18.0],
            egui::Label::new(egui::RichText::new(text).strong().color(text_color)),
        );
    });
}

fn cell_frame(
    ui: &mut egui::Ui,
    width: f32,
    fill: egui::Color32,
    border: egui::Color32,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    egui::Frame::NONE
        .fill(fill)
        .stroke(egui::Stroke::new(1.0, border))
        .inner_margin(egui::Margin::symmetric(3, 2))
        .show(ui, |ui| {
            ui.set_min_width(width);
            add_contents(ui);
        });
}

fn responsive_column_widths(headers: &[String], available_width: f32) -> Vec<f32> {
    if headers.is_empty() {
        return Vec::new();
    }
    let gap_budget = 4.0 * headers.len().saturating_sub(1) as f32;
    let available = (available_width - gap_budget).max(1.0);
    let base = headers
        .iter()
        .enumerate()
        .map(|(index, header)| {
            if header == "Status" {
                92.0
            } else {
                base_cell_width(index).max(cell_width_by_text(header))
            }
        })
        .collect::<Vec<_>>();
    let base_total = base.iter().sum::<f32>();
    if base_total <= available {
        return base;
    }
    let minimums = headers
        .iter()
        .enumerate()
        .map(|(index, _)| min_cell_width(index))
        .collect::<Vec<_>>();
    let minimum_total = minimums.iter().sum::<f32>();
    if minimum_total >= available {
        let scale = (available / minimum_total).clamp(0.0, 1.0);
        return minimums.iter().map(|width| width * scale).collect();
    }
    let remaining = available - minimum_total;
    let flexible_total = base
        .iter()
        .zip(minimums.iter())
        .map(|(base, minimum)| (base - minimum).max(0.0))
        .sum::<f32>()
        .max(1.0);
    base.iter()
        .zip(minimums.iter())
        .enumerate()
        .map(|(_, (base, minimum))| {
            let flexible_width = (base - minimum).max(0.0);
            minimum + flexible_width * (remaining / flexible_total)
        })
        .collect()
}

fn base_cell_width(column_index: usize) -> f32 {
    match column_index {
        0 => 140.0,
        1 => 220.0,
        2 => 180.0,
        3 => 92.0,
        4 => 110.0,
        5 => 210.0,
        6 => 110.0,
        7 => 92.0,
        _ => 120.0,
    }
}

fn min_cell_width(column_index: usize) -> f32 {
    match column_index {
        0 => 64.0,
        1 => 76.0,
        2 => 68.0,
        _ => 56.0,
    }
}

fn inner_cell_width(width: f32) -> f32 {
    (width - 8.0).max(1.0)
}

fn cell_width_by_text(text: &str) -> f32 {
    match text {
        "Category" => 140.0,
        "Ledger Field" => 220.0,
        "Decomp Field" => 180.0,
        "Offset" => 92.0,
        "Decomp Value" => 110.0,
        "Rust Field" => 210.0,
        "Rust Value" => 110.0,
        "Status" => 92.0,
        "Field" => 220.0,
        "Source Field" => 180.0,
        "Value" => 110.0,
        "Kind" => 120.0,
        "Provenance" => 210.0,
        _ => 120.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ParityLedgerSurface, ParityLedgerSurfaceTab};
    use std::path::{Path, PathBuf};

    fn workspace_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("workspace root")
            .to_path_buf()
    }

    #[test]
    fn template_from_surface_tab_preserves_shape() {
        let surface = ParityLedgerSurface::load(workspace_root()).expect("surface loads");
        let tab: &ParityLedgerSurfaceTab = &surface.tabs[0];
        let template = LedgerTabTemplate::from(tab);

        assert_eq!(template.title, "Global Values");
        assert_eq!(template.rows.len(), tab.rows.len());
        assert_eq!(template.headers.len(), tab.headers.len());
        assert_eq!(template.rows[0].status.as_deref(), Some("match"));
    }

    #[test]
    fn table_template_does_not_duplicate_existing_status_column() {
        let template = LedgerTabTemplate {
            title: "Trace".to_string(),
            summary: "summary".to_string(),
            headers: vec!["Frame".to_string(), "Status".to_string()],
            rows: vec![LedgerTabTemplateRow {
                cells: vec!["1".to_string(), "match".to_string()],
                detail: "detail".to_string(),
                status: Some("match".to_string()),
            }],
        };

        assert!(!template.renders_separate_status_column());
        assert_eq!(template.display_headers(), vec!["Frame", "Status"]);
    }

    #[test]
    fn table_template_adds_status_column_for_semantic_status_only() {
        let template = LedgerTabTemplate {
            title: "State Graphs".to_string(),
            summary: "summary".to_string(),
            headers: vec!["Kind".to_string(), "Primary".to_string()],
            rows: vec![LedgerTabTemplateRow {
                cells: vec!["node".to_string(), "Wait".to_string()],
                detail: "detail".to_string(),
                status: Some("missing".to_string()),
            }],
        };

        assert!(template.renders_separate_status_column());
        assert_eq!(
            template.display_headers(),
            vec!["Kind", "Primary", "Status"]
        );
    }

    #[test]
    fn responsive_columns_fit_inside_narrow_available_width() {
        let headers = vec![
            "Frame".to_string(),
            "Pose".to_string(),
            "Hitboxes".to_string(),
            "Status".to_string(),
        ];

        let widths = responsive_column_widths(&headers, 280.0);

        assert_eq!(widths.len(), headers.len());
        assert!(
            widths.iter().sum::<f32>() <= 280.0,
            "responsive sheet columns should not request more width than the panel offers"
        );
    }

    #[test]
    fn responsive_columns_scale_below_minimums_when_the_panel_is_tiny() {
        let headers = vec![
            "Frame".to_string(),
            "Pose".to_string(),
            "Hitboxes".to_string(),
            "Status".to_string(),
        ];

        let widths = responsive_column_widths(&headers, 96.0);

        assert_eq!(widths.len(), headers.len());
        assert!(
            widths.iter().sum::<f32>() <= 96.0,
            "extremely narrow panels still bound the shared table primitive"
        );
        assert!(widths.iter().all(|width| inner_cell_width(*width) >= 1.0));
    }

    #[test]
    fn responsive_columns_keep_base_widths_when_space_is_available() {
        let headers = vec![
            "Frame".to_string(),
            "Pose".to_string(),
            "Hitboxes".to_string(),
        ];

        let widths = responsive_column_widths(&headers, 900.0);

        assert_eq!(widths, vec![140.0, 220.0, 180.0]);
    }
}
