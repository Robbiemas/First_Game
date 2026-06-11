use crate::{
    layout::{bounded_child_height, responsive_split_layout},
    move_keyframes::MoveKeyframesStateSource,
    state_graphs::{StateGraphCanvasView, StateGraphDocument, StateGraphSelection},
    template::render_spreadsheet_table,
    theme::{devtool_theme, status_palette},
    AppSection, LedgerTabTemplate, MoveKeyframesPanel, ParityLedgerApp, StateGraphsPanel,
    ThemeMode,
};
use eframe::egui;
use mole_runtime::{
    LegacySpriteCue, RenderCapsule, RenderColor, RenderPolygon, RenderRect, RenderScene,
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

pub fn render_app(ui: &mut egui::Ui, app: &mut ParityLedgerApp) {
    apply_app_visuals(ui);

    egui::Panel::top("devtool_header")
        .resizable(false)
        .show_inside(ui, |ui| {
            ui.add_space(2.0);
            render_outer_tabs(ui, app);
            ui.add_space(2.0);
        });

    egui::Panel::bottom("devtool_footer")
        .resizable(false)
        .show_inside(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(format!("Section: {}", app.selected_section.label()));
                ui.separator();
                ui.label(
                    "CLI-first contract: GUI surfaces mirror agent commands and Rust artifacts.",
                );
                ui.separator();
                ui.label("Layout: adaptive split panes, mobile sub-tabs, bounded data panes.");
            });
        });

    egui::CentralPanel::default().show_inside(ui, |ui| {
        render_active_section(ui, app);
    });
}

fn apply_app_visuals(ui: &mut egui::Ui) {
    let app_theme = devtool_theme(ThemeMode::Light);
    let mut visuals = egui::Visuals::light();
    visuals.panel_fill = app_theme.app_background;
    visuals.window_fill = app_theme.workbench_fill;
    visuals.override_text_color = Some(app_theme.accent_text);
    visuals.widgets.active.bg_fill = app_theme.accent_fill;
    visuals.widgets.hovered.bg_fill = app_theme.panel_fill;
    ui.ctx().set_visuals(visuals);
}

fn render_active_section(ui: &mut egui::Ui, app: &mut ParityLedgerApp) {
    match app.selected_section {
        AppSection::ParityLedger => render_parity_ledger(ui, app),
        AppSection::StateGraphs => render_state_graphs(ui, app),
        AppSection::EcbCoverage => render_ecb_coverage(ui, app),
        AppSection::InputTrace => render_input_trace(ui, app),
        AppSection::SlippiReplay => render_slippi_replay(ui, app),
        AppSection::MoveKeyframes => render_move_keyframes(ui, app),
    }
}

fn render_outer_tabs(ui: &mut egui::Ui, app: &mut ParityLedgerApp) {
    ui.horizontal_wrapped(|ui| {
        ui.heading("Mole Game Dev Tool");
        ui.separator();
        let titles = app.section_titles();
        for (index, title) in titles.iter().enumerate() {
            let section = match index {
                0 => AppSection::StateGraphs,
                1 => AppSection::ParityLedger,
                2 => AppSection::EcbCoverage,
                3 => AppSection::InputTrace,
                4 => AppSection::SlippiReplay,
                _ => AppSection::MoveKeyframes,
            };
            let selected = app.selected_section == section;
            if ui.selectable_label(selected, *title).clicked() {
                app.select_section(section);
            }
        }
    });
}

fn render_parity_ledger(ui: &mut egui::Ui, app: &mut ParityLedgerApp) {
    ui.heading(app.title());
    ui.label(format!(
        "Registry dual-surface: {}",
        app.view_model.registry.dual_surface
    ));
    ui.label(format!(
        "Active ledger tabs: {}",
        app.view_model.registry.active_tab_count
    ));
    ui.label(format!(
        "Planned ledger tabs: {}",
        app.view_model.registry.planned_tab_count
    ));
    ui.separator();
    ui.label(&app.parity_ledger.summary);
    ui.separator();

    ui.horizontal_wrapped(|ui| {
        let titles = app
            .parity_ledger
            .tabs
            .iter()
            .map(|tab| tab.label.clone())
            .collect::<Vec<_>>();
        for (index, title) in titles.iter().enumerate() {
            let selected = app.selected_ledger_tab == index;
            if ui.selectable_label(selected, title.as_str()).clicked() {
                app.select_ledger_tab(index);
            }
        }
    });

    ui.separator();
    if let Some(tab) = app.parity_ledger.tabs.get(app.selected_ledger_tab) {
        let template = LedgerTabTemplate::from(tab);
        let mut selected_row = app.selected_ledger_row;
        let mut pending_selection = selected_row;
        render_sheet_tab(
            ui,
            app.theme(),
            &template,
            &mut selected_row,
            |clicked_row| {
                pending_selection = clicked_row;
            },
        );
        if pending_selection != selected_row {
            selected_row = pending_selection;
        }
        app.selected_ledger_row = selected_row;
    }
}

fn render_ecb_coverage(ui: &mut egui::Ui, app: &mut ParityLedgerApp) {
    ui.heading("ECB Coverage");
    ui.label(app.ecb_coverage.summary());
    ui.separator();

    let template = LedgerTabTemplate::from(&app.ecb_coverage);
    let mut selected_row = app.selected_ecb_row;
    let mut pending_selection = selected_row;
    render_sheet_tab(
        ui,
        app.theme(),
        &template,
        &mut selected_row,
        |clicked_row| {
            pending_selection = clicked_row;
        },
    );
    if pending_selection != selected_row {
        selected_row = pending_selection;
    }
    app.selected_ecb_row = selected_row;
}

fn render_sheet_tab(
    ui: &mut egui::Ui,
    theme: ThemeMode,
    tab: &LedgerTabTemplate,
    selected_row: &mut usize,
    on_select: impl FnMut(usize),
) {
    ui.label(&tab.summary);
    ui.separator();
    render_spreadsheet_table(ui, theme, tab, selected_row, on_select);
}

fn render_state_graphs(ui: &mut egui::Ui, app: &mut ParityLedgerApp) {
    ui.heading("State Graphs");
    ui.label(app.state_graphs.summary());
    ui.horizontal_wrapped(|ui| {
        ui.label(format!("Layout: {}", app.state_graph_canvas.layout_path));
        let save_label = if app.state_graph_layout_dirty {
            "Save Layout*"
        } else {
            "Save Layout"
        };
        if ui.button(save_label).clicked() {
            let _ = app.save_state_graph_layout();
        }
        if let Some(status) = app.state_graph_layout_status.as_ref() {
            ui.label(status);
        }
    });
    ui.separator();
    render_state_graph_subtabs(ui, app);
    ui.separator();
    match app.selected_state_graph_panel {
        StateGraphsPanel::Graphs => render_state_graph_canvas_pair(ui, app),
        StateGraphsPanel::Selection => render_state_graph_selection_detail(ui, app),
        StateGraphsPanel::Missing => render_state_graph_missing_sheet(ui, app),
    }
}

fn render_state_graph_subtabs(ui: &mut egui::Ui, app: &mut ParityLedgerApp) {
    ui.horizontal_wrapped(|ui| {
        for (panel, label) in [
            (StateGraphsPanel::Graphs, "Graphs"),
            (StateGraphsPanel::Selection, "Selection"),
            (StateGraphsPanel::Missing, "Missing"),
        ] {
            if ui
                .selectable_label(app.selected_state_graph_panel == panel, label)
                .clicked()
            {
                app.selected_state_graph_panel = panel;
            }
        }
    });
}

fn render_state_graph_missing_sheet(ui: &mut egui::Ui, app: &mut ParityLedgerApp) {
    let template = LedgerTabTemplate::from(&app.state_graphs);
    let mut selected_row = app.selected_state_graph_row;
    let mut pending_selection = selected_row;
    render_sheet_tab(
        ui,
        app.theme(),
        &template,
        &mut selected_row,
        |clicked_row| {
            pending_selection = clicked_row;
        },
    );
    if pending_selection != selected_row {
        selected_row = pending_selection;
    }
    app.selected_state_graph_row = selected_row;
}

fn render_state_graph_canvas_pair(ui: &mut egui::Ui, app: &mut ParityLedgerApp) {
    if app.state_graph_canvas.graphs.is_empty() {
        ui.label("No state graph canvas data available.");
        return;
    }

    let available_width = ui.available_width();
    let available_height = ui.available_height();
    let pane_gap = ui.spacing().item_spacing.x;
    let layout = responsive_split_layout(
        available_width,
        available_height,
        app.state_graph_canvas.graphs.len(),
        pane_gap,
    );
    let graphs = app.state_graph_canvas.graphs.clone();
    if layout.stacked {
        let selected = app
            .selected_state_graph_pane
            .min(graphs.len().saturating_sub(1));
        app.selected_state_graph_pane = selected;
        ui.horizontal_wrapped(|ui| {
            for (index, graph) in graphs.iter().enumerate() {
                if ui
                    .selectable_label(index == selected, graph.title.as_str())
                    .clicked()
                {
                    app.selected_state_graph_pane = index;
                }
            }
        });
        if let Some(graph) = graphs.get(app.selected_state_graph_pane) {
            ui.allocate_ui_with_layout(
                egui::vec2(layout.left_width, layout.body_height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| render_state_graph_canvas(ui, app, graph),
            );
        }
    } else {
        ui.horizontal_top(|ui| {
            for (index, graph) in graphs.iter().enumerate() {
                let width = if index == 0 {
                    layout.left_width
                } else {
                    layout.right_width
                };
                ui.allocate_ui_with_layout(
                    egui::vec2(width, layout.body_height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| render_state_graph_canvas(ui, app, graph),
                );
            }
        });
    }
}

fn render_state_graph_canvas(
    ui: &mut egui::Ui,
    app: &mut ParityLedgerApp,
    graph: &StateGraphDocument,
) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.vertical(|ui| {
            let theme = app.theme();
            ui.horizontal(|ui| {
                ui.heading(&graph.title);
                let view = app
                    .state_graph_canvas_views
                    .entry(graph.id.clone())
                    .or_insert_with(|| StateGraphCanvasView::from_graph_zoom(graph.zoom));
                ui.label(format!(
                    "{} nodes | {} edges | zoom {:.2}x",
                    graph.nodes.len(),
                    graph.edges.len(),
                    view.zoom
                ));
                if ui.small_button("-").clicked() {
                    view.zoom_by(0.85);
                }
                if ui.small_button("+").clicked() {
                    view.zoom_by(1.15);
                }
                if ui.small_button("Reset").clicked() {
                    *view = StateGraphCanvasView::from_graph_zoom(graph.zoom);
                }
            });
            if !graph.description.is_empty() {
                ui.label(&graph.description);
            }
            let canvas_height =
                bounded_child_height(ui.available_height(), 120.0, ui.available_height());
            let desired_size = egui::vec2(ui.available_width().max(1.0), canvas_height);
            let (rect, response) =
                ui.allocate_exact_size(desired_size, egui::Sense::click_and_drag());
            let canvas_rect = rect.shrink(16.0);
            let canvas_size = [canvas_rect.width() as f64, canvas_rect.height() as f64];
            if response.dragged_by(egui::PointerButton::Secondary)
                || response.dragged_by(egui::PointerButton::Middle)
            {
                if let Some(view) = app.state_graph_canvas_views.get_mut(&graph.id) {
                    let delta = ui.input(|input| input.pointer.delta());
                    view.pan_by([delta.x as f64, delta.y as f64]);
                }
            }
            let view = *app
                .state_graph_canvas_views
                .entry(graph.id.clone())
                .or_insert_with(|| StateGraphCanvasView::from_graph_zoom(graph.zoom));
            if response.dragged_by(egui::PointerButton::Primary) {
                if app.state_graph_active_drag_node.is_none() {
                    if let Some(pointer) = response.interact_pointer_pos() {
                        let point = [
                            (pointer.x - canvas_rect.left()) as f64,
                            (pointer.y - canvas_rect.top()) as f64,
                        ];
                        if let Some(StateGraphSelection::Node { graph_id, id }) =
                            graph.selection_at_canvas_point(&view, canvas_size, point)
                        {
                            app.state_graph_active_drag_node = Some((graph_id.clone(), id.clone()));
                            app.state_graph_selection =
                                Some(StateGraphSelection::Node { graph_id, id });
                        }
                    }
                }
                if let Some((active_graph_id, active_node_id)) =
                    app.state_graph_active_drag_node.clone()
                {
                    if active_graph_id == graph.id {
                        let delta = ui.input(|input| input.pointer.delta());
                        if delta != egui::Vec2::ZERO {
                            if let Some(graph_delta) = graph.graph_delta_from_canvas_delta(
                                &view,
                                canvas_size,
                                [delta.x as f64, delta.y as f64],
                            ) {
                                if let Some(current) = app
                                    .state_graph_canvas
                                    .graph(&active_graph_id)
                                    .and_then(|graph| graph.node_position(&active_node_id))
                                {
                                    if app
                                        .state_graph_canvas
                                        .set_node_position(
                                            &active_graph_id,
                                            &active_node_id,
                                            [
                                                current[0] + graph_delta[0],
                                                current[1] + graph_delta[1],
                                            ],
                                        )
                                        .is_ok()
                                    {
                                        app.state_graph_layout_dirty = true;
                                        app.state_graph_layout_status =
                                            Some("Layout has unsaved changes.".to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if !response.dragged_by(egui::PointerButton::Primary) {
                app.state_graph_active_drag_node = None;
            }
            if response.clicked() {
                if let Some(pointer) = response.interact_pointer_pos() {
                    let point = [
                        (pointer.x - canvas_rect.left()) as f64,
                        (pointer.y - canvas_rect.top()) as f64,
                    ];
                    app.state_graph_selection =
                        graph.selection_at_canvas_point(&view, canvas_size, point);
                    if app.state_graph_selection.is_some() {
                        app.selected_state_graph_panel = StateGraphsPanel::Selection;
                    }
                }
            }
            let painter = ui.painter_at(rect);
            let theme_palette = devtool_theme(theme);
            painter.rect_filled(rect, 4.0, theme_palette.workbench_fill);
            painter.rect_stroke(
                rect,
                4.0,
                egui::Stroke::new(1.0, theme_palette.panel_border),
                egui::StrokeKind::Inside,
            );
            draw_state_graph_document(
                &painter,
                canvas_rect,
                theme,
                graph,
                &view,
                app.state_graph_selection.as_ref(),
            );
        });
    });
}

fn render_state_graph_selection_detail(ui: &mut egui::Ui, app: &ParityLedgerApp) {
    let Some(selection) = app.state_graph_selection.as_ref() else {
        ui.label("Select a graph node or edge to inspect its source, status, and parity metadata.");
        return;
    };
    ui.separator();
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.heading("Selection");
        match selection {
            StateGraphSelection::Node { graph_id, id } => {
                ui.label(format!("Node: {id} ({graph_id})"));
            }
            StateGraphSelection::Edge { graph_id, index } => {
                ui.label(format!("Edge #{index} ({graph_id})"));
            }
        }
        if let Some(detail) = app.state_graph_selection_detail(selection) {
            let mut detail_text = detail;
            ui.add(
                egui::TextEdit::multiline(&mut detail_text)
                    .font(egui::TextStyle::Monospace)
                    .desired_rows(8)
                    .interactive(false),
            );
        }
    });
}

fn draw_state_graph_document(
    painter: &egui::Painter,
    rect: egui::Rect,
    theme: ThemeMode,
    graph: &StateGraphDocument,
    view: &StateGraphCanvasView,
    selected: Option<&StateGraphSelection>,
) {
    if graph.bounds().is_none() {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "No nodes.",
            egui::FontId::proportional(14.0),
            devtool_theme(theme).muted_text,
        );
        return;
    }
    let positions = graph
        .nodes
        .iter()
        .map(|node| {
            (
                node.id.as_str(),
                graph_node_screen_pos(rect, graph, view, node.pos),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let node_size = egui::vec2(94.0, 42.0);

    for (edge_index, edge) in graph.edges.iter().enumerate() {
        let (Some(from), Some(to)) = (
            positions.get(edge.from.as_str()),
            positions.get(edge.to.as_str()),
        ) else {
            continue;
        };
        let palette = status_palette(theme, Some(&edge.status), false);
        let is_selected = matches!(
            selected,
            Some(StateGraphSelection::Edge { graph_id, index })
                if graph_id == &graph.id && *index == edge_index
        );
        painter.line_segment(
            [*from, *to],
            egui::Stroke::new(
                if is_selected { 3.0 } else { 1.5 },
                palette
                    .border
                    .gamma_multiply(if is_selected { 1.0 } else { 0.85 }),
            ),
        );
        let midpoint = egui::pos2((from.x + to.x) * 0.5, (from.y + to.y) * 0.5);
        painter.circle_filled(midpoint, 3.0, palette.border);
    }

    for node in &graph.nodes {
        let Some(center) = positions.get(node.id.as_str()).copied() else {
            continue;
        };
        let palette = status_palette(theme, Some(&node.status), false);
        let is_selected = matches!(
            selected,
            Some(StateGraphSelection::Node { graph_id, id })
                if graph_id == &graph.id && id == &node.id
        );
        let node_rect = egui::Rect::from_center_size(center, node_size);
        painter.rect_filled(node_rect, 4.0, palette.fill);
        painter.rect_stroke(
            node_rect,
            4.0,
            egui::Stroke::new(if is_selected { 3.0 } else { 1.5 }, palette.border),
            egui::StrokeKind::Inside,
        );
        painter.text(
            node_rect.center_top() + egui::vec2(0.0, 8.0),
            egui::Align2::CENTER_TOP,
            if node.label.is_empty() {
                node.id.as_str()
            } else {
                node.label.as_str()
            },
            egui::FontId::proportional(11.0),
            palette.text,
        );
        painter.text(
            node_rect.center_bottom() - egui::vec2(0.0, 14.0),
            egui::Align2::CENTER_BOTTOM,
            &node.status,
            egui::FontId::proportional(9.0),
            palette.accent_text,
        );
    }
}

fn graph_node_screen_pos(
    rect: egui::Rect,
    graph: &StateGraphDocument,
    view: &StateGraphCanvasView,
    pos: [f64; 2],
) -> egui::Pos2 {
    let local = graph
        .graph_position_to_canvas(view, [rect.width() as f64, rect.height() as f64], pos)
        .unwrap_or([0.0, 0.0]);
    egui::pos2(rect.left() + local[0] as f32, rect.top() + local[1] as f32)
}

fn render_move_keyframes(ui: &mut egui::Ui, app: &mut ParityLedgerApp) {
    ui.heading("Move Keyframes");
    render_move_keyframes_browser_controls(ui, app);
    ui.separator();

    let mut selected_row = app
        .selected_move_keyframe_row
        .min(app.move_keyframes.keyframes.len().saturating_sub(1));
    app.move_keyframes_editor
        .set_selected_frame_index(selected_row);

    let available_width = ui.available_width();
    let available_height = ui.available_height();
    let layout = responsive_split_layout(
        available_width,
        available_height,
        2,
        ui.spacing().item_spacing.x,
    );

    if layout.stacked {
        render_move_keyframes_mobile_tabs(ui, app);
        ui.separator();
        match app.selected_move_keyframes_panel {
            MoveKeyframesPanel::Preview => {
                render_move_keyframes_viewport_panel(ui, app, &mut selected_row);
                ui.add_space(6.0);
                render_move_keyframes_strip_panel(
                    ui,
                    app.theme(),
                    &app.move_keyframes,
                    &mut selected_row,
                );
            }
            MoveKeyframesPanel::Details => {
                render_move_keyframes_details_panel(ui, app, selected_row);
            }
            MoveKeyframesPanel::Data => {
                render_move_keyframes_data_panel(ui, app, &mut selected_row);
            }
        }
    } else {
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(layout.left_width, layout.body_height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    render_move_keyframes_viewport_panel(ui, app, &mut selected_row);
                    ui.add_space(6.0);
                    render_move_keyframes_strip_panel(
                        ui,
                        app.theme(),
                        &app.move_keyframes,
                        &mut selected_row,
                    );
                },
            );

            ui.separator();

            ui.allocate_ui_with_layout(
                egui::vec2(layout.right_width, layout.body_height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    render_move_keyframes_details_panel(ui, app, selected_row);
                },
            );
        });
    }

    app.move_keyframes_editor
        .set_selected_frame_index(selected_row);
    app.selected_move_keyframe_row = selected_row;
}

fn render_move_keyframes_mobile_tabs(ui: &mut egui::Ui, app: &mut ParityLedgerApp) {
    ui.horizontal_wrapped(|ui| {
        for (panel, label) in [
            (MoveKeyframesPanel::Preview, "Preview"),
            (MoveKeyframesPanel::Details, "Details"),
            (MoveKeyframesPanel::Data, "Table"),
        ] {
            if ui
                .selectable_label(app.selected_move_keyframes_panel == panel, label)
                .clicked()
            {
                app.selected_move_keyframes_panel = panel;
            }
        }
    });
}

fn render_move_keyframes_browser_controls(ui: &mut egui::Ui, app: &mut ParityLedgerApp) {
    ui.horizontal_wrapped(|ui| {
        let column_width = ((ui.available_width() - ui.spacing().item_spacing.x) * 0.5)
            .clamp(320.0, ui.available_width());
        ui.allocate_ui_with_layout(
            egui::vec2(column_width, 0.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| render_move_keyframes_target_controls(ui, app),
        );
        ui.allocate_ui_with_layout(
            egui::vec2(column_width, 0.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| render_move_keyframes_import_controls(ui, app),
        );
    });
}

fn render_move_keyframes_target_controls(ui: &mut egui::Ui, app: &mut ParityLedgerApp) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.vertical(|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("Character");
                let selected_character = app.selected_move_keyframe_character_label();
                let characters = app.move_keyframe_characters.clone();
                let mut pending_character: Option<String> = None;
                egui::ComboBox::from_id_salt("move_keyframes_character")
                    .selected_text(selected_character)
                    .show_ui(ui, |ui| {
                        for character in characters {
                            let enabled_label = if character.populated {
                                character.label.clone()
                            } else {
                                format!("{} (empty)", character.label)
                            };
                            if ui
                                .selectable_label(
                                    app.selected_move_keyframe_character_id == character.id,
                                    enabled_label,
                                )
                                .clicked()
                            {
                                pending_character = Some(character.id);
                            }
                        }
                    });
                ui.label("State");
                let selected_state = app.selected_move_keyframe_state_label();
                let states = app.move_keyframe_states.clone();
                let mut pending_state: Option<String> = None;
                egui::ComboBox::from_id_salt("move_keyframes_state")
                    .selected_text(selected_state)
                    .show_ui(ui, |ui| {
                        for state in states {
                            let marker = match state.source {
                                MoveKeyframesStateSource::MaterializedArtifact => "",
                                MoveKeyframesStateSource::SourceManifest => " (manifest)",
                            };
                            if ui
                                .selectable_label(
                                    app.selected_move_keyframe_state == state.state,
                                    format!("{}{}", state.label, marker),
                                )
                                .clicked()
                            {
                                pending_state = Some(state.state);
                            }
                        }
                    });
                if let Some(character_id) = pending_character {
                    if let Err(error) = app.select_move_keyframe_character(&character_id) {
                        app.move_keyframes_status = Some(error);
                    }
                } else if let Some(state) = pending_state {
                    if let Err(error) = app.select_move_keyframe_state(&state) {
                        app.move_keyframes_status = Some(error);
                    }
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.label(app.move_keyframes.summary());
                if let Some(path) = app.move_keyframes_editor.artifact_path() {
                    ui.separator();
                    ui.label(format!("Artifact: {}", path.display()));
                }
            });
            if let Some(status) = &app.move_keyframes_status {
                ui.label(status);
            }
        });
    });
}

fn render_move_keyframes_import_controls(ui: &mut egui::Ui, app: &mut ParityLedgerApp) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.vertical(|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("Import From");
                let sources = app.move_keyframe_import_sources();
                let selected_source = app.selected_move_keyframe_import_source_label();
                let mut pending_source: Option<&'static str> = None;
                egui::ComboBox::from_id_salt("move_keyframes_import_source")
                    .selected_text(selected_source)
                    .show_ui(ui, |ui| {
                        for (id, label) in sources {
                            if ui
                                .selectable_label(
                                    app.selected_move_keyframe_import_source_id == id,
                                    label,
                                )
                                .clicked()
                            {
                                pending_source = Some(id);
                            }
                        }
                    });
                if let Some(source_id) = pending_source {
                    app.select_move_keyframe_import_source(source_id);
                }
                if ui.button("Import All States").clicked() {
                    match app.run_move_keyframe_import_pipeline() {
                        Ok(()) => {}
                        Err(error) => app.move_keyframes_status = Some(error),
                    }
                }
            });
            ui.label(format!(
                "Target: {} | Source: {} | Method: Mole CLI all-states import",
                app.selected_move_keyframe_character_label(),
                app.selected_move_keyframe_import_source_label()
            ));
        });
    });
}

fn render_move_keyframes_details_panel(
    ui: &mut egui::Ui,
    app: &mut ParityLedgerApp,
    selected_row: usize,
) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.vertical(|ui| {
            ui.heading("State Details");
            ui.label(format!(
                "{} / {}",
                app.selected_move_keyframe_character_label(),
                app.selected_move_keyframe_state_label()
            ));
            ui.separator();
            ui.label(format!("State id: {}", app.move_keyframes.state));
            ui.label(format!("Total frames: {}", app.move_keyframes.summary.total_frames));
            ui.label(format!("Materialized keyframes: {}", app.move_keyframes.keyframes.len()));
            ui.label(format!("Sources: {}", app.move_keyframes.sources.len()));
            ui.label(format!("Gaps: {}", app.move_keyframes.gaps.len()));
            if let Some(pose_tree) = app.move_keyframes_editor.pose_tree() {
                ui.label(format!(
                    "Figatree root: {} | joints: {}",
                    pose_tree.root,
                    pose_tree.joints.len()
                ));
            }
            if app.move_keyframes.keyframes.is_empty() {
                ui.separator();
                ui.label("This state is listed in the compact source manifest but has not been expanded into editable keyframes yet.");
                return;
            }
            ui.separator();
            if let Some(frame) = app.move_keyframes.keyframes.get(selected_row) {
                ui.label(format!("Selected frame: {}", frame.frame));
                ui.label(format!("Hitboxes: {}", frame.hitboxes.len()));
                ui.label(format!("Hurtboxes: {}", frame.hurtboxes.len()));
                ui.label(format!("Body volumes: {}", frame.body_volumes.len()));
                ui.label(format!(
                    "Interpolates from previous: {}",
                    frame.interpolates_from_previous
                ));
            }
        });
    });
}

fn render_move_keyframes_data_panel(
    ui: &mut egui::Ui,
    app: &mut ParityLedgerApp,
    selected_row: &mut usize,
) {
    let template = LedgerTabTemplate::from(&app.move_keyframes);
    let mut pending_selection = *selected_row;
    render_sheet_tab(ui, app.theme(), &template, selected_row, |clicked_row| {
        pending_selection = clicked_row;
    });
    if pending_selection != *selected_row {
        *selected_row = pending_selection;
    }
}

fn render_input_trace(ui: &mut egui::Ui, app: &mut ParityLedgerApp) {
    ui.heading("Input Trace");
    ui.label(app.input_trace.summary());
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.vertical(|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("Artifact");
                ui.add(
                    egui::TextEdit::singleline(&mut app.input_trace_path)
                        .desired_width(ui.available_width().min(520.0)),
                );
            });
            ui.horizontal_wrapped(|ui| {
                ui.label("Player");
                ui.add(
                    egui::DragValue::new(&mut app.input_trace_player_number)
                        .range(1..=4)
                        .speed(1),
                );
                ui.label("Start");
                ui.add(egui::DragValue::new(&mut app.input_trace_start).speed(1));
                ui.label("End");
                ui.add(egui::DragValue::new(&mut app.input_trace_end).speed(1));
                if ui.button("Refresh").clicked() {
                    match app.reload_input_trace() {
                        Ok(()) => {}
                        Err(error) => app.input_trace_status = Some(error),
                    }
                }
            });
            if let Some(status) = &app.input_trace_status {
                ui.label(status);
            }
        });
    });
    ui.separator();

    let template = LedgerTabTemplate::from(&app.input_trace);
    let mut selected_row = app.selected_input_trace_row;
    let mut pending_selection = selected_row;
    render_sheet_tab(
        ui,
        app.theme(),
        &template,
        &mut selected_row,
        |clicked_row| {
            pending_selection = clicked_row;
        },
    );
    if pending_selection != selected_row {
        selected_row = pending_selection;
    }
    app.selected_input_trace_row = selected_row;
    ui.separator();
    egui::CollapsingHeader::new("Raw Input Export")
        .default_open(false)
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .max_height(bounded_child_height(ui.available_height(), 120.0, 260.0))
                .show(ui, |ui| {
                    ui.monospace(&app.input_trace.raw_export_text);
                });
        });
}

fn render_slippi_replay(ui: &mut egui::Ui, app: &mut ParityLedgerApp) {
    ui.heading("Slippi Replay");
    ui.label(app.slippi_replay.summary());
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.vertical(|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("Artifact");
                ui.add(
                    egui::TextEdit::singleline(&mut app.slippi_replay_path)
                        .desired_width(ui.available_width().min(520.0)),
                );
            });
            ui.horizontal_wrapped(|ui| {
                ui.label("Player");
                ui.add(
                    egui::DragValue::new(&mut app.slippi_replay_player_number)
                        .range(1..=4)
                        .speed(1),
                );
                ui.label("Start");
                ui.add(egui::DragValue::new(&mut app.slippi_replay_start).speed(1));
                ui.label("End");
                ui.add(egui::DragValue::new(&mut app.slippi_replay_end).speed(1));
                ui.label("Rows");
                ui.add(
                    egui::DragValue::new(&mut app.slippi_replay_max_frames)
                        .range(1..=10_000)
                        .speed(1),
                );
                if ui.button("Refresh").clicked() {
                    match app.reload_slippi_replay() {
                        Ok(()) => {}
                        Err(error) => app.slippi_replay_status = Some(error),
                    }
                }
                let has_diff = app.slippi_replay.first_diff_index().is_some();
                if ui
                    .add_enabled(has_diff, egui::Button::new("First Diff"))
                    .clicked()
                {
                    let _ = app.select_first_slippi_replay_diff();
                }
            });
            if let Some(status) = &app.slippi_replay_status {
                ui.label(status);
            }
        });
    });
    ui.separator();

    let template = LedgerTabTemplate::from(&app.slippi_replay);
    let mut selected_row = app.selected_slippi_replay_row;
    let mut pending_selection = selected_row;
    render_sheet_tab(
        ui,
        app.theme(),
        &template,
        &mut selected_row,
        |clicked_row| {
            pending_selection = clicked_row;
        },
    );
    if pending_selection != selected_row {
        selected_row = pending_selection;
    }
    app.selected_slippi_replay_row = selected_row;
    ui.separator();
    egui::CollapsingHeader::new("Trace Report")
        .default_open(false)
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .max_height(bounded_child_height(ui.available_height(), 120.0, 260.0))
                .show(ui, |ui| {
                    ui.monospace(&app.slippi_replay.trace_report_text);
                });
        });
}

fn render_move_keyframes_strip_panel(
    ui: &mut egui::Ui,
    theme: ThemeMode,
    surface: &crate::move_keyframes::MoveKeyframesSurface,
    selected_row: &mut usize,
) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.heading("Frames");
                ui.label("attack frames are tinted");
            });
            ui.separator();
            let palette = move_keyframe_preview_palette(theme);
            let total_frames = surface
                .summary
                .total_frames
                .max(surface.keyframes.len())
                .max(1);
            let available_width = ui.available_width().max(1.0);
            let tile_height = 22.0;
            let (rect, response) = ui.allocate_exact_size(
                egui::vec2(available_width, tile_height),
                egui::Sense::click(),
            );
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 3.0, palette.background);
            painter.rect_stroke(
                rect,
                3.0,
                egui::Stroke::new(1.0, palette.border),
                egui::StrokeKind::Inside,
            );
            let selected_frame_number = surface
                .keyframes
                .get(*selected_row)
                .map(|frame| frame.frame)
                .unwrap_or(1);
            let cell_width = rect.width() / total_frames as f32;
            for frame_number in 1..=total_frames {
                let x0 = rect.left() + (frame_number - 1) as f32 * cell_width;
                let x1 = rect.left() + frame_number as f32 * cell_width;
                let cell_rect = egui::Rect::from_min_max(
                    egui::pos2(x0, rect.top()),
                    egui::pos2((x1 - 1.0).max(x0), rect.bottom()),
                );
                let keyed = surface
                    .keyframes
                    .iter()
                    .any(|frame| frame.frame == frame_number);
                let attack = move_keyframe_frame_has_active_hitbox(surface, frame_number);
                let selected = frame_number == selected_frame_number;
                let fill = move_keyframe_timeline_fill(theme, selected, attack, keyed);
                painter.rect_filled(cell_rect, 1.0, fill);
                if selected {
                    painter.rect_stroke(
                        cell_rect,
                        1.0,
                        egui::Stroke::new(1.5, palette.hitbox),
                        egui::StrokeKind::Inside,
                    );
                } else if keyed && cell_width >= 5.0 {
                    painter.rect_stroke(
                        cell_rect,
                        1.0,
                        egui::Stroke::new(1.0, palette.border),
                        egui::StrokeKind::Inside,
                    );
                }
            }
            if response.clicked() {
                if let Some(pos) = response.interact_pointer_pos() {
                    let frame_number = (((pos.x - rect.left()) / cell_width).floor() as usize + 1)
                        .clamp(1, total_frames);
                    if let Some(index) = nearest_move_keyframe_index(surface, frame_number) {
                        *selected_row = index;
                    }
                }
            }
        });
    });
}

fn render_move_keyframes_viewport_panel(
    ui: &mut egui::Ui,
    app: &mut ParityLedgerApp,
    selected_row: &mut usize,
) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.vertical(|ui| {
            ui.heading("Selected Frame Runtime Preview");
            let theme = app.theme();
            ui.horizontal_wrapped(|ui| {
                let previous_clicked = ui
                    .add_enabled(*selected_row > 0, egui::Button::new("<"))
                    .clicked();
                if previous_clicked && app.move_keyframes_editor.select_previous_frame() {
                    *selected_row = app.move_keyframes_editor.selected_frame_index();
                }
                let next_enabled = selected_row.saturating_add(1) < app.move_keyframes.keyframes.len();
                let next_clicked = ui
                    .add_enabled(next_enabled, egui::Button::new(">"))
                    .clicked();
                if next_clicked && app.move_keyframes_editor.select_next_frame() {
                    *selected_row = app.move_keyframes_editor.selected_frame_index();
                }
                if let Some(frame) = app.move_keyframes.keyframes.get(*selected_row) {
                    ui.label(format!(
                        "Frame {} | Hitboxes: {} | Hurtboxes: {} | Body volumes: {} | Interpolates: {}",
                        frame.frame,
                        frame.hitboxes.len(),
                        frame.hurtboxes.len(),
                        frame.body_volumes.len(),
                        frame.interpolates_from_previous
                    ));
                } else {
                    ui.label("No keyframe selected.");
                }
            });
            ui.separator();

            let preview_height =
                bounded_child_height(ui.available_height(), 120.0, ui.available_height());
            let desired_size = egui::vec2(ui.available_width().max(1.0), preview_height);
            let (rect, _) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
            let painter = ui.painter_at(rect);
            let palette = move_keyframe_preview_palette(theme);
            if let Some(preview) = app
                .move_keyframes_editor
                .runtime_preview(rect.width().max(1.0) as u32, rect.height().max(1.0) as u32)
            {
                let fit = move_keyframe_preview_fit(rect, &preview.scene);
                let workspace_root = app.workspace_root().to_path_buf();
                draw_move_keyframe_runtime_scene(
                    ui.ctx(),
                    &painter,
                    rect,
                    &preview.scene,
                    &palette,
                    fit,
                    &workspace_root,
                    &mut app.move_keyframe_texture_cache,
                );
            } else {
                painter.rect_filled(rect, 6.0, palette.background);
                painter.rect_stroke(
                    rect,
                    6.0,
                    egui::Stroke::new(1.0, palette.border),
                    egui::StrokeKind::Inside,
                );
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "No runtime preview available.",
                    egui::FontId::proportional(16.0),
                    palette.border,
                );
            }
        });
    });
}

fn move_keyframe_timeline_fill(
    theme: ThemeMode,
    selected: bool,
    attack: bool,
    keyed: bool,
) -> egui::Color32 {
    let palette = move_keyframe_preview_palette(theme);
    match (selected, attack, keyed, theme) {
        (true, _, _, ThemeMode::Dark) => egui::Color32::from_rgb(16, 117, 177),
        (true, _, _, ThemeMode::Light) => egui::Color32::from_rgb(191, 219, 254),
        (false, true, _, ThemeMode::Dark) => {
            egui::Color32::from_rgba_premultiplied(92, 32, 32, 220)
        }
        (false, true, _, ThemeMode::Light) => {
            egui::Color32::from_rgba_premultiplied(248, 215, 215, 255)
        }
        (false, false, true, ThemeMode::Dark) => egui::Color32::from_rgb(30, 64, 175),
        (false, false, true, ThemeMode::Light) => egui::Color32::from_rgb(219, 234, 254),
        (false, false, false, ThemeMode::Dark) => palette.border.gamma_multiply(0.35),
        (false, false, false, ThemeMode::Light) => palette.border.gamma_multiply(0.22),
    }
}

fn nearest_move_keyframe_index(
    surface: &crate::move_keyframes::MoveKeyframesSurface,
    frame_number: usize,
) -> Option<usize> {
    surface
        .keyframes
        .iter()
        .enumerate()
        .min_by_key(|(_, frame)| frame.frame.abs_diff(frame_number))
        .map(|(index, _)| index)
}

fn move_keyframe_frame_has_active_hitbox(
    surface: &crate::move_keyframes::MoveKeyframesSurface,
    frame_number: usize,
) -> bool {
    surface
        .summary
        .active_hitbox_windows
        .iter()
        .any(|window| move_keyframe_window_contains(window, frame_number))
        || surface
            .keyframes
            .iter()
            .any(|frame| frame.frame == frame_number && !frame.hitboxes.is_empty())
}

fn move_keyframe_window_contains(window: &serde_json::Value, frame_number: usize) -> bool {
    let start = window
        .get("start")
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(usize::MAX);
    let end = window
        .get("end")
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or_default();
    start <= frame_number && frame_number <= end
}

fn draw_move_keyframe_runtime_scene(
    ctx: &egui::Context,
    painter: &egui::Painter,
    rect: egui::Rect,
    scene: &RenderScene,
    palette: &MoveKeyframePreviewPalette,
    fit: MoveKeyframePreviewFit,
    asset_root: &Path,
    texture_cache: &mut BTreeMap<String, egui::TextureHandle>,
) {
    painter.rect_filled(rect, 6.0, render_color(scene.background));
    painter.rect_stroke(
        rect,
        6.0,
        egui::Stroke::new(1.0, palette.border),
        egui::StrokeKind::Inside,
    );

    draw_render_rect(painter, scene.stage, fit, egui::Color32::TRANSPARENT);
    let sprite_path = move_keyframe_sprite_asset_path(asset_root, scene.player_sprites[0]);
    let sprite_drawn = sprite_path
        .as_deref()
        .and_then(|path| load_move_keyframe_texture(ctx, path, texture_cache))
        .is_some_and(|texture| {
            draw_render_texture(
                painter,
                &texture,
                scene.players[0],
                fit,
                scene.player_sprites[0].flip_x,
            );
            true
        });
    if move_keyframe_draws_player_rect(scene, sprite_path.is_some() || sprite_drawn) {
        draw_render_rect(painter, scene.players[0], fit, egui::Color32::TRANSPARENT);
    }
    if let Some(hurtboxes) = scene.player_hurtbox_pills.first() {
        for hurtbox in hurtboxes.iter().copied() {
            draw_render_capsule(painter, hurtbox, fit);
        }
    }
    if let Some(hitboxes) = scene.player_hitbox_pills.first() {
        for hitbox in hitboxes.iter().copied() {
            draw_render_capsule(painter, hitbox, fit);
        }
    }
    draw_render_polygon(painter, scene.player_ecbs[0], fit);
}

fn load_move_keyframe_texture(
    ctx: &egui::Context,
    path: &Path,
    texture_cache: &mut BTreeMap<String, egui::TextureHandle>,
) -> Option<egui::TextureHandle> {
    let key = path.to_string_lossy().to_string();
    if !texture_cache.contains_key(&key) {
        let image = image::ImageReader::open(path)
            .ok()?
            .decode()
            .ok()?
            .to_rgba8();
        let size = [image.width() as usize, image.height() as usize];
        let pixels = image.into_raw();
        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);
        let texture = ctx.load_texture(
            format!("move-keyframe-preview:{key}"),
            color_image,
            egui::TextureOptions::LINEAR,
        );
        texture_cache.insert(key.clone(), texture);
    }
    texture_cache.get(&key).cloned()
}

fn draw_render_texture(
    painter: &egui::Painter,
    texture: &egui::TextureHandle,
    rect: RenderRect,
    fit: MoveKeyframePreviewFit,
    flip_x: bool,
) {
    let min = fit.apply(egui::pos2(rect.x as f32, rect.y as f32));
    let max = fit.apply(egui::pos2(
        (rect.x + rect.width as i32) as f32,
        (rect.y + rect.height as i32) as f32,
    ));
    let uv = if flip_x {
        egui::Rect::from_min_max(egui::pos2(1.0, 0.0), egui::pos2(0.0, 1.0))
    } else {
        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0))
    };
    painter.image(
        texture.id(),
        egui::Rect::from_two_pos(min, max),
        uv,
        egui::Color32::WHITE,
    );
}

fn draw_render_rect(
    painter: &egui::Painter,
    rect: RenderRect,
    fit: MoveKeyframePreviewFit,
    override_color: egui::Color32,
) {
    let color = render_color(rect.color);
    let min = fit.apply(egui::pos2(rect.x as f32, rect.y as f32));
    let max = fit.apply(egui::pos2(
        (rect.x + rect.width as i32) as f32,
        (rect.y + rect.height as i32) as f32,
    ));
    let egui_rect = egui::Rect::from_two_pos(min, max);
    let color = if override_color == egui::Color32::TRANSPARENT {
        color
    } else {
        override_color
    };
    painter.rect_filled(egui_rect, 0.0, color.gamma_multiply(0.24));
    painter.rect_stroke(
        egui_rect,
        0.0,
        egui::Stroke::new(1.0, color),
        egui::StrokeKind::Inside,
    );
}

fn draw_render_capsule(
    painter: &egui::Painter,
    capsule: RenderCapsule,
    fit: MoveKeyframePreviewFit,
) {
    let color = render_color(capsule.color);
    let a = fit.apply(egui::pos2(capsule.a.x as f32, capsule.a.y as f32));
    let b = fit.apply(egui::pos2(capsule.b.x as f32, capsule.b.y as f32));
    let radius = (capsule.radius.max(1) as f32 * fit.scale).max(1.0);
    let wireframe = move_keyframe_capsule_wireframe(a, b, radius);
    let stroke = egui::Stroke::new(1.25, color);
    if let Some((left, right)) = wireframe.sides {
        painter.line_segment(left, stroke);
        painter.line_segment(right, stroke);
        painter.line_segment([a, b], egui::Stroke::new(1.0, color.gamma_multiply(0.55)));
    }
    painter.circle_stroke(a, wireframe.radius, stroke);
    if b != a {
        painter.circle_stroke(b, wireframe.radius, stroke);
    }
    painter.circle_filled(a, 1.5, color);
    if b != a {
        painter.circle_filled(b, 1.5, color);
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct MoveKeyframeCapsuleWireframe {
    radius: f32,
    sides: Option<([egui::Pos2; 2], [egui::Pos2; 2])>,
}

fn move_keyframe_capsule_wireframe(
    a: egui::Pos2,
    b: egui::Pos2,
    radius: f32,
) -> MoveKeyframeCapsuleWireframe {
    let radius = radius.max(1.0);
    let delta = b - a;
    let length = delta.length();
    if length <= f32::EPSILON {
        return MoveKeyframeCapsuleWireframe {
            radius,
            sides: None,
        };
    }
    let normal = egui::vec2(-delta.y / length, delta.x / length) * radius;
    MoveKeyframeCapsuleWireframe {
        radius,
        sides: Some(([a + normal, b + normal], [a - normal, b - normal])),
    }
}

fn draw_render_polygon(
    painter: &egui::Painter,
    polygon: RenderPolygon,
    fit: MoveKeyframePreviewFit,
) {
    let color = render_color(polygon.color);
    let points = polygon
        .points
        .into_iter()
        .map(|point| fit.apply(egui::pos2(point.x as f32, point.y as f32)))
        .collect::<Vec<_>>();
    let Ok(points): Result<[egui::Pos2; 4], _> = points.try_into() else {
        return;
    };
    let stroke = egui::Stroke::new(1.0, color);
    for line in move_keyframe_ecb_wireframe_lines(points) {
        painter.line_segment(line, stroke);
    }
}

fn move_keyframe_ecb_wireframe_lines(points: [egui::Pos2; 4]) -> Vec<[egui::Pos2; 2]> {
    vec![
        [points[0], points[1]],
        [points[1], points[2]],
        [points[2], points[3]],
        [points[3], points[0]],
        [points[0], points[2]],
        [points[3], points[1]],
    ]
}

#[derive(Clone, Copy, Debug)]
struct MoveKeyframePreviewFit {
    scale: f32,
    offset: egui::Vec2,
}

impl MoveKeyframePreviewFit {
    fn apply(self, point: egui::Pos2) -> egui::Pos2 {
        egui::pos2(
            point.x * self.scale + self.offset.x,
            point.y * self.scale + self.offset.y,
        )
    }
}

fn move_keyframe_preview_fit(rect: egui::Rect, scene: &RenderScene) -> MoveKeyframePreviewFit {
    let mut bounds: Option<(f32, f32, f32, f32)> = None;
    let mut include = |x: f32, y: f32| {
        bounds = Some(match bounds {
            Some((min_x, min_y, max_x, max_y)) => {
                (min_x.min(x), min_y.min(y), max_x.max(x), max_y.max(y))
            }
            None => (x, y, x, y),
        });
    };

    let player = scene.players[0];
    include(player.x as f32, player.y as f32);
    include(
        (player.x + player.width as i32) as f32,
        (player.y + player.height as i32) as f32,
    );

    for point in scene.player_ecbs[0].points {
        include(point.x as f32, point.y as f32);
    }
    if let Some(hurtboxes) = scene.player_hurtbox_pills.first() {
        for hurtbox in hurtboxes {
            include(hurtbox.a.x as f32, hurtbox.a.y as f32);
            include(hurtbox.b.x as f32, hurtbox.b.y as f32);
        }
    }
    if let Some(hitboxes) = scene.player_hitbox_pills.first() {
        for hitbox in hitboxes {
            include(hitbox.a.x as f32, hitbox.a.y as f32);
            include(hitbox.b.x as f32, hitbox.b.y as f32);
        }
    }
    include(player.x as f32, scene.stage.y as f32);
    include(
        (player.x + player.width as i32) as f32,
        scene.stage.y as f32,
    );

    let Some((min_x, min_y, max_x, max_y)) = bounds else {
        return MoveKeyframePreviewFit {
            scale: 1.0,
            offset: egui::Vec2::ZERO,
        };
    };

    let padding = 28.0;
    let content_width = (max_x - min_x).max(1.0);
    let content_height = (max_y - min_y).max(1.0);
    let scale_x = (rect.width() - padding * 2.0) / content_width;
    let scale_y = (rect.height() - padding * 2.0) / content_height;
    let scale = scale_x.min(scale_y).clamp(0.35, 4.0);
    let center = egui::pos2((min_x + max_x) * 0.5, (min_y + max_y) * 0.5);
    MoveKeyframePreviewFit {
        scale,
        offset: egui::vec2(
            rect.center().x - center.x * scale,
            rect.center().y - center.y * scale,
        ),
    }
}

fn render_color(color: RenderColor) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a)
}

#[derive(Clone, Copy)]
struct MoveKeyframePreviewPalette {
    background: egui::Color32,
    border: egui::Color32,
    hitbox: egui::Color32,
}

fn move_keyframe_preview_palette(theme: ThemeMode) -> MoveKeyframePreviewPalette {
    match theme {
        ThemeMode::Dark => MoveKeyframePreviewPalette {
            background: egui::Color32::from_rgb(10, 14, 24),
            border: egui::Color32::from_rgb(51, 65, 85),
            hitbox: egui::Color32::from_rgb(248, 113, 113),
        },
        ThemeMode::Light => MoveKeyframePreviewPalette {
            background: egui::Color32::from_rgb(247, 250, 255),
            border: egui::Color32::from_rgb(191, 219, 254),
            hitbox: egui::Color32::from_rgb(220, 38, 38),
        },
    }
}

fn move_keyframe_sprite_asset_path(root: &Path, cue: LegacySpriteCue) -> Option<PathBuf> {
    let path = root.join(cue.relative_path());
    path.is_file().then_some(path)
}

fn move_keyframe_draws_player_rect(scene: &RenderScene, sprite_asset_available: bool) -> bool {
    !sprite_asset_available
        && scene.player_hurtbox_pills.first().is_none_or(Vec::is_empty)
        && scene.player_hitbox_pills.first().is_none_or(Vec::is_empty)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mole_core::MotionState;
    use mole_runtime::RenderFrame;
    use std::path::Path;

    fn workspace_root() -> PathBuf {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        manifest_dir
            .parent()
            .and_then(Path::parent)
            .expect("workspace root")
            .to_path_buf()
    }

    #[test]
    fn move_keyframe_preview_uses_sprite_asset_instead_of_player_rect_when_available() {
        let mut frame = RenderFrame::from_world(&mole_core::World::for_two_players_on_stage(
            mole_core::StageProfile::dev_flat_test(),
        ));
        frame.player_motion_states[0] = MotionState::Turn;
        frame.player_animation_frames[0] = 1;
        let scene = RenderScene::from_frame_on_stage(
            &frame,
            &mole_core::StageProfile::dev_flat_test(),
            640,
            360,
        );

        let asset_path =
            move_keyframe_sprite_asset_path(&workspace_root(), scene.player_sprites[0])
                .expect("turning sprite asset");

        assert!(asset_path.ends_with(Path::new("DolphinMole/turning/Standing1.png")));
        assert!(!move_keyframe_draws_player_rect(&scene, true));
    }

    #[test]
    fn move_keyframe_capsules_are_projected_as_wireframe_sides_not_filled_pills() {
        let wireframe =
            move_keyframe_capsule_wireframe(egui::pos2(10.0, 20.0), egui::pos2(30.0, 20.0), 4.0);

        let (top, bottom) = wireframe.sides.expect("non-zero capsule has sides");
        assert_eq!(wireframe.radius, 4.0);
        assert_eq!(top, [egui::pos2(10.0, 24.0), egui::pos2(30.0, 24.0)]);
        assert_eq!(bottom, [egui::pos2(10.0, 16.0), egui::pos2(30.0, 16.0)]);

        let circle =
            move_keyframe_capsule_wireframe(egui::pos2(5.0, 5.0), egui::pos2(5.0, 5.0), 3.0);
        assert_eq!(circle.radius, 3.0);
        assert!(circle.sides.is_none());
    }

    #[test]
    fn move_keyframe_ecb_wireframe_uses_decomp_top_right_bottom_left_axes() {
        let points = [
            egui::pos2(10.0, 0.0),
            egui::pos2(20.0, 10.0),
            egui::pos2(10.0, 20.0),
            egui::pos2(0.0, 10.0),
        ];

        let lines = move_keyframe_ecb_wireframe_lines(points);

        assert_eq!(lines.len(), 6);
        assert_eq!(lines[0], [points[0], points[1]]);
        assert_eq!(lines[3], [points[3], points[0]]);
        assert_eq!(lines[4], [points[0], points[2]]);
        assert_eq!(lines[5], [points[3], points[1]]);
    }
}
