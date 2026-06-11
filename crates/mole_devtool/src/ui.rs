use crate::{
    template::render_spreadsheet_table, theme::devtool_theme, AppSection, LedgerTabTemplate,
    ParityLedgerApp, ThemeMode,
};
use eframe::egui;
use mole_core::Vec2 as CoreVec2;
use mole_runtime::{RenderCapsule, RenderColor, RenderPolygon, RenderRect, RenderScene};

pub fn render_app(ui: &mut egui::Ui, app: &mut ParityLedgerApp) {
    let app_theme = devtool_theme(app.theme());
    let mut visuals = match app.theme() {
        ThemeMode::Dark => egui::Visuals::dark(),
        ThemeMode::Light => egui::Visuals::light(),
    };
    visuals.panel_fill = app_theme.app_background;
    visuals.window_fill = app_theme.workbench_fill;
    ui.ctx().set_visuals(visuals);

    ui.vertical(|ui| {
        render_outer_tabs(ui, app);
        ui.separator();
        match app.selected_section {
            AppSection::ParityLedger => render_parity_ledger(ui, app),
            AppSection::StateGraphs => render_state_graphs(ui, app),
            AppSection::EcbCoverage => render_ecb_coverage(ui, app),
            AppSection::InputTrace => render_input_trace(ui, app),
            AppSection::SlippiReplay => render_slippi_replay(ui, app),
            AppSection::MoveKeyframes => render_move_keyframes(ui, app),
        }
    });
}

fn render_outer_tabs(ui: &mut egui::Ui, app: &mut ParityLedgerApp) {
    ui.horizontal(|ui| {
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

        ui.separator();
        ui.label("Theme:");
        let dark_selected = app.theme() == ThemeMode::Dark;
        if ui.selectable_label(dark_selected, "Dark").clicked() {
            app.set_theme(ThemeMode::Dark);
        }
        let light_selected = app.theme() == ThemeMode::Light;
        if ui.selectable_label(light_selected, "Light").clicked() {
            app.set_theme(ThemeMode::Light);
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
    ui.separator();

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
    let right_width = (available_width * 0.33).clamp(300.0, 420.0);
    let left_width = (available_width - right_width - ui.spacing().item_spacing.x).max(320.0);

    if available_width < 980.0 {
        ui.vertical(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), (available_height * 0.58).max(360.0)),
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
            ui.add_space(6.0);
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), ui.available_height()),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    render_move_keyframes_choice_panel(ui, app, selected_row);
                    ui.add_space(6.0);
                    render_move_keyframes_inspector_panel(ui, app);
                },
            );
        });
    } else {
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(left_width, available_height),
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
                egui::vec2(right_width, available_height),
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
                .max_height(260.0)
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
                .max_height(260.0)
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
                .clamp(14.0, 44.0);
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
                    let label = format!("{}", frame.frame);
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

            let preview_height = (ui.available_height() * 0.44).clamp(210.0, 320.0);
            let desired_size = egui::vec2(ui.available_width().max(320.0), preview_height);
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
