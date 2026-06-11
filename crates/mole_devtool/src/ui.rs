use crate::{
    layout::{bounded_child_height, responsive_split_layout},
    state_graphs::{StateGraphCanvasView, StateGraphDocument, StateGraphSelection},
    template::render_spreadsheet_table,
    theme::{devtool_theme, status_palette},
    AppSection, LedgerTabTemplate, MoveKeyframesPanel, ParityLedgerApp, StateGraphsPanel,
    ThemeMode,
};
use eframe::egui;
use mole_core::Vec2 as CoreVec2;
use mole_runtime::{RenderCapsule, RenderColor, RenderPolygon, RenderRect, RenderScene};
use std::collections::BTreeMap;

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
    ui.label(app.move_keyframes.summary());
    if let Some(pose_tree) = app.move_keyframes_editor.pose_tree() {
        ui.label(format!(
            "Figatree root: {} | Joints: {} | 3D transforms preserved: true",
            pose_tree.root,
            pose_tree.joints.len()
        ));
    }
    if let Some(path) = app
        .move_keyframes_editor
        .artifact_path()
        .map(|path| path.display().to_string())
    {
        ui.horizontal(|ui| {
            ui.label(format!("Artifact: {}", path));
            let save_clicked = ui
                .add_enabled(
                    app.move_keyframes_editor.is_dirty(),
                    egui::Button::new("Save"),
                )
                .clicked();
            if save_clicked {
                let _ = app.move_keyframes_editor.save_to_source();
            }
        });
    }
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
            }
            MoveKeyframesPanel::Inspector => {
                render_move_keyframes_choice_panel(ui, app, selected_row);
                ui.add_space(6.0);
                render_move_keyframes_inspector_panel(ui, app);
            }
            MoveKeyframesPanel::Data => {
                render_move_keyframes_strip_panel(
                    ui,
                    app.theme(),
                    &app.move_keyframes,
                    &mut selected_row,
                );
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
                    render_move_keyframes_choice_panel(ui, app, selected_row);
                    ui.add_space(6.0);
                    render_move_keyframes_inspector_panel(ui, app);
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
            (MoveKeyframesPanel::Inspector, "Inspector"),
            (MoveKeyframesPanel::Data, "Frames"),
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

fn render_move_keyframes_choice_panel(
    ui: &mut egui::Ui,
    app: &mut ParityLedgerApp,
    selected_row: usize,
) {
    let current_character = app.move_keyframes.target_character_label.clone();
    let current_state = app.move_keyframes.state.clone();
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.vertical(|ui| {
            ui.heading("Selection");
            ui.horizontal(|ui| {
                ui.label("Character");
                egui::ComboBox::from_id_salt("move_keyframes_character")
                    .selected_text(current_character.clone())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut app.move_keyframes.target_character_label, current_character.clone(), current_character.clone());
                    });
            });
            ui.horizontal(|ui| {
                ui.label("State");
                egui::ComboBox::from_id_salt("move_keyframes_state")
                    .selected_text(current_state.clone())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut app.move_keyframes.state, current_state.clone(), current_state.clone());
                    });
            });
            ui.label(format!(
                "Frame {} of {}",
                selected_row.saturating_add(1),
                app.move_keyframes.keyframes.len()
            ));
            ui.label("The Rust editor is currently backed by the materialized frame-data artifact for this state.");
        });
    });
}

fn render_move_keyframes_selected_handle_panel(ui: &mut egui::Ui, app: &mut ParityLedgerApp) {
    ui.heading("Selected Handle");
    let Some(selection) = app.move_keyframes_editor.selected_handle_detail() else {
        ui.label("Click a handle in the preview.");
        return;
    };

    ui.label(selection.label);
    ui.label(format!("{:?}", selection.kind));
    let mut x = selection.position[0];
    let mut y = selection.position[1];
    ui.horizontal(|ui| {
        ui.label("x");
        let x_changed = ui.add(egui::DragValue::new(&mut x).speed(0.1)).changed();
        ui.label("y");
        let y_changed = ui.add(egui::DragValue::new(&mut y).speed(0.1)).changed();
        ui.label(format!("z {:.2}", selection.position[2]));
        if x_changed || y_changed {
            let _ = app
                .move_keyframes_editor
                .drag_selected_handle([x - selection.position[0], y - selection.position[1]]);
        }
    });
}

fn render_move_keyframes_inspector_panel(ui: &mut egui::Ui, app: &mut ParityLedgerApp) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.vertical(|ui| {
            ui.heading("Selected Frame Inspector");

            let mut dirty = false;
            if let Some(frame) = app.move_keyframes_editor.selected_frame_mut() {
                ui.horizontal(|ui| {
                    ui.label("Frame #");
                    let mut frame_number = frame.frame as i64;
                    if ui
                        .add(
                            egui::DragValue::new(&mut frame_number)
                                .speed(1.0)
                                .range(0..=i64::MAX),
                        )
                        .changed()
                    {
                        frame.frame = frame_number.max(0) as usize;
                        dirty = true;
                    }
                    let mut interpolates_from_previous = frame.interpolates_from_previous;
                    if ui
                        .checkbox(
                            &mut interpolates_from_previous,
                            "Interpolates from previous",
                        )
                        .changed()
                    {
                        frame.interpolates_from_previous = interpolates_from_previous;
                        dirty = true;
                    }
                });
            } else {
                ui.label("No keyframe selected.");
            }

            if dirty {
                app.move_keyframes_editor.mark_dirty();
            }

            ui.separator();

            if let Some(frame) = app.move_keyframes_editor.selected_frame() {
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!(
                        "Pose keys: {}",
                        frame.pose.as_object().map_or(0, |o| o.len())
                    ));
                    ui.separator();
                    ui.label(format!("Hitboxes: {}", frame.hitboxes.len()));
                    ui.separator();
                    ui.label(format!("Hurtboxes: {}", frame.hurtboxes.len()));
                    ui.separator();
                    ui.label(format!("Body volumes: {}", frame.body_volumes.len()));
                });
                if let Some(pose_tree) = app.move_keyframes_editor.pose_tree() {
                    ui.label(format!(
                        "Figatree root: {} | joints: {}",
                        pose_tree.root,
                        pose_tree.joints.len()
                    ));
                }
                ui.separator();
                render_move_keyframes_selected_handle_panel(ui, app);
            }
        });
    });
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
            let frame_count = surface.keyframes.len().max(1);
            let available_width = ui.available_width().max(1.0);
            let gap = ui.spacing().item_spacing.x.max(2.0);
            let tile_width = ((available_width - gap * (frame_count.saturating_sub(1) as f32))
                / frame_count as f32)
                .clamp(4.0, 44.0);
            let tile_height = 28.0;

            ui.horizontal(|ui| {
                for (index, frame) in surface.keyframes.iter().enumerate() {
                    let selected = *selected_row == index;
                    let attack = !frame.hitboxes.is_empty();
                    let fill = move_keyframe_timeline_fill(theme, selected, attack);
                    let stroke = if selected {
                        egui::Stroke::new(1.5, palette.hitbox)
                    } else {
                        egui::Stroke::new(1.0, palette.border)
                    };
                    let label = if tile_width >= 14.0 {
                        format!("{}", frame.frame)
                    } else {
                        String::new()
                    };
                    let response = ui.add_sized(
                        egui::vec2(tile_width, tile_height),
                        egui::Button::new(label).fill(fill).stroke(stroke),
                    );
                    if response.clicked() {
                        *selected_row = index;
                    }
                    if index + 1 < frame_count {
                        ui.add_space(gap);
                    }
                }
            });
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
                draw_move_keyframe_runtime_scene(
                    &painter,
                    rect,
                    &preview.scene,
                    &palette,
                    fit,
                );
                let handles = app.move_keyframes_editor.handles_for_selected_frame();
                draw_move_keyframe_runtime_handles(
                    ui,
                    &painter,
                    rect,
                    &preview.scene,
                    fit,
                    app,
                    &handles,
                    &palette,
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

fn move_keyframe_timeline_fill(theme: ThemeMode, selected: bool, attack: bool) -> egui::Color32 {
    let palette = move_keyframe_preview_palette(theme);
    match (selected, attack, theme) {
        (true, _, ThemeMode::Dark) => egui::Color32::from_rgb(16, 117, 177),
        (true, _, ThemeMode::Light) => egui::Color32::from_rgb(191, 219, 254),
        (false, true, ThemeMode::Dark) => egui::Color32::from_rgba_premultiplied(92, 32, 32, 220),
        (false, true, ThemeMode::Light) => {
            egui::Color32::from_rgba_premultiplied(248, 215, 215, 255)
        }
        (false, false, ThemeMode::Dark) => palette.border.gamma_multiply(0.55),
        (false, false, ThemeMode::Light) => palette.border.gamma_multiply(0.35),
    }
}

fn draw_move_keyframe_runtime_scene(
    painter: &egui::Painter,
    rect: egui::Rect,
    scene: &RenderScene,
    palette: &MoveKeyframePreviewPalette,
    fit: MoveKeyframePreviewFit,
) {
    painter.rect_filled(rect, 6.0, render_color(scene.background));
    painter.rect_stroke(
        rect,
        6.0,
        egui::Stroke::new(1.0, palette.border),
        egui::StrokeKind::Inside,
    );

    draw_render_rect(painter, scene.stage, fit, egui::Color32::TRANSPARENT);
    draw_render_rect(painter, scene.players[0], fit, egui::Color32::TRANSPARENT);
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

fn draw_move_keyframe_runtime_handles(
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    _rect: egui::Rect,
    scene: &RenderScene,
    fit: MoveKeyframePreviewFit,
    app: &mut ParityLedgerApp,
    handles: &[crate::move_keyframes::MoveKeyframeHandle],
    palette: &MoveKeyframePreviewPalette,
) {
    let pixels_per_core_unit_milli = scene.transform.pixels_per_core_unit_milli.max(1) as f64;
    let world_delta_scale = 1_000_000.0 / (pixels_per_core_unit_milli * fit.scale as f64).max(1.0);

    for (index, handle) in handles.iter().enumerate() {
        let point = move_keyframe_handle_flattened_point(handle.position);
        let screen = scene.transform.world_to_screen(CoreVec2 {
            x: point.x,
            y: point.y,
        });
        let screen = fit.apply(egui::pos2(screen.x as f32, screen.y as f32));
        let handle_rect = egui::Rect::from_center_size(screen, egui::vec2(14.0, 14.0));
        let id = ui.id().with(("move_keyframe_handle", index, &handle.label));
        let response = ui.interact(handle_rect, id, egui::Sense::click_and_drag());
        let selected = app.move_keyframes_editor.selected_handle() == Some(&handle.kind);
        let color = if selected || app.move_keyframes_active_handle.as_ref() == Some(&handle.kind) {
            palette.hitbox
        } else if response.hovered() {
            palette.hurtbox
        } else {
            palette.border
        };
        painter.circle_filled(screen, 4.5, color);
        painter.circle_stroke(screen, 4.5, egui::Stroke::new(1.0, palette.background));

        if response.clicked() || response.drag_started() {
            let _ = app.move_keyframes_editor.select_handle(handle.kind.clone());
        }

        if response.drag_started() {
            app.move_keyframes_active_handle = Some(handle.kind.clone());
            app.move_keyframes_active_drag_delta = egui::Vec2::ZERO;
        }

        if app.move_keyframes_active_handle.as_ref() == Some(&handle.kind) && response.dragged() {
            let drag_delta = response.drag_delta();
            let incremental = drag_delta - app.move_keyframes_active_drag_delta;
            if incremental != egui::Vec2::ZERO {
                let world_delta = [
                    incremental.x as f64 * world_delta_scale,
                    -(incremental.y as f64) * world_delta_scale,
                ];
                let _ = app
                    .move_keyframes_editor
                    .drag_handle(handle.kind.clone(), world_delta);
                app.move_keyframes_active_drag_delta = drag_delta;
            }
        }

        if response.drag_stopped()
            && app.move_keyframes_active_handle.as_ref() == Some(&handle.kind)
        {
            app.move_keyframes_active_handle = None;
            app.move_keyframes_active_drag_delta = egui::Vec2::ZERO;
        }
    }
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
    painter.line_segment([a, b], egui::Stroke::new(radius, color));
    painter.circle_filled(a, radius.max(1.5), color.gamma_multiply(0.55));
    painter.circle_filled(b, radius.max(1.5), color.gamma_multiply(0.55));
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
    painter.add(egui::Shape::convex_polygon(
        points,
        color.gamma_multiply(0.22),
        egui::Stroke::new(1.0, color),
    ));
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

fn move_keyframe_handle_flattened_point(position: [f64; 3]) -> CoreVec2 {
    CoreVec2 {
        x: position[0].round() as i32,
        y: position[1].round() as i32,
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
    hurtbox: egui::Color32,
    hitbox: egui::Color32,
}

fn move_keyframe_preview_palette(theme: ThemeMode) -> MoveKeyframePreviewPalette {
    match theme {
        ThemeMode::Dark => MoveKeyframePreviewPalette {
            background: egui::Color32::from_rgb(10, 14, 24),
            border: egui::Color32::from_rgb(51, 65, 85),
            hurtbox: egui::Color32::from_rgb(74, 222, 128),
            hitbox: egui::Color32::from_rgb(248, 113, 113),
        },
        ThemeMode::Light => MoveKeyframePreviewPalette {
            background: egui::Color32::from_rgb(247, 250, 255),
            border: egui::Color32::from_rgb(191, 219, 254),
            hurtbox: egui::Color32::from_rgb(22, 163, 74),
            hitbox: egui::Color32::from_rgb(220, 38, 38),
        },
    }
}
