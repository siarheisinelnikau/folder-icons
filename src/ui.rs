use crate::{
    Action, App, REPOSITORY, VERSION, Work,
    render::{self, Badge},
};
use egui::{Color32, RichText, Stroke, Vec2};

pub const INK: Color32 = Color32::from_rgb(31, 48, 68);
pub const MUTED: Color32 = Color32::from_rgb(91, 108, 127);
pub const ACCENT: Color32 = Color32::from_rgb(27, 116, 105);
pub const BORDER: Color32 = Color32::from_rgb(215, 224, 232);
pub const CANVAS: Color32 = Color32::from_rgb(244, 247, 250);
pub const DEFAULT_SIZE: [f64; 2] = [780., 590.];
pub const MIN_SIZE: [f64; 2] = [740., 560.];

pub fn configure(ctx: &egui::Context) {
    // Use a locally installed Windows fallback for localized folder names. No font
    // is redistributed; this bounded read happens before the native window exists.
    if let Some(windows) = std::env::var_os("WINDIR") {
        let path = std::path::PathBuf::from(windows).join("Fonts/msyh.ttc");
        if std::fs::metadata(&path).is_ok_and(|m| m.len() <= 32 * 1024 * 1024)
            && let Ok(bytes) = std::fs::read(path)
        {
            let mut fonts = egui::FontDefinitions::default();
            fonts.font_data.insert(
                "system-cjk".into(),
                std::sync::Arc::new(egui::FontData::from_owned(bytes)),
            );
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .push("system-cjk".into());
            ctx.set_fonts(fonts);
        }
    }
    let mut style = egui::Style {
        visuals: egui::Visuals::light(),
        ..Default::default()
    };
    style.visuals.override_text_color = Some(INK);
    style.visuals.panel_fill = CANVAS;
    style.visuals.window_fill = Color32::WHITE;
    style.visuals.faint_bg_color = Color32::from_rgb(234, 240, 244);
    style.visuals.extreme_bg_color = Color32::WHITE;
    style.visuals.selection.bg_fill = Color32::from_rgb(212, 237, 232);
    style.visuals.selection.stroke = Stroke::new(1., ACCENT);
    style.visuals.widgets.inactive.bg_fill = Color32::WHITE;
    style.visuals.widgets.inactive.weak_bg_fill = Color32::WHITE;
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(1., BORDER);
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(228, 240, 237);
    style.visuals.widgets.active.bg_fill = Color32::from_rgb(212, 237, 232);
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1., BORDER);
    style.visuals.widgets.noninteractive.bg_fill = CANVAS;
    style.spacing.item_spacing = Vec2::new(8., 8.);
    style.spacing.button_padding = Vec2::new(12., 7.);
    style.spacing.interact_size = Vec2::new(36., 32.);
    style.spacing.window_margin = egui::Margin::same(20);
    style.spacing.scroll = egui::style::ScrollStyle::solid();
    for (kind, size) in [
        (egui::TextStyle::Heading, 23.),
        (egui::TextStyle::Body, 15.),
        (egui::TextStyle::Button, 14.),
        (egui::TextStyle::Small, 12.),
    ] {
        style
            .text_styles
            .insert(kind, egui::FontId::proportional(size));
    }
    ctx.set_style(style);
}
fn section(ui: &mut egui::Ui, title: &str) {
    ui.label(RichText::new(title).strong().size(16.));
}
fn secondary(ui: &mut egui::Ui, text: impl Into<String>) {
    ui.label(RichText::new(text.into()).color(MUTED));
}
fn primary(ui: &mut egui::Ui, label: &str, enabled: bool) -> egui::Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(RichText::new(label).color(Color32::WHITE))
            .fill(ACCENT)
            .min_size(Vec2::new(150., 38.)),
    )
}

impl App {
    pub(crate) fn ui(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("header")
            .frame(
                egui::Frame::new()
                    .fill(Color32::WHITE)
                    .inner_margin(egui::Margin::symmetric(16, 10)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("FolderIcons").size(23.).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Settings & updates").clicked() {
                            self.settings_open = true;
                        }
                    });
                });
                if let Some(update) = &self.update {
                    ui.horizontal(|ui| {
                        ui.label(format!("Version {} is available", update.version));
                        if ui.link("View update").clicked() {
                            self.settings_open = true;
                        }
                    });
                }
            });
        egui::TopBottomPanel::bottom("status")
            .frame(
                egui::Frame::new()
                    .fill(Color32::WHITE)
                    .inner_margin(egui::Margin::symmetric(16, 8)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if self.local_busy {
                        ui.spinner();
                    }
                    ui.add(
                        egui::Label::new(RichText::new(&self.message).size(13.).color(MUTED))
                            .truncate(),
                    )
                    .on_hover_text(&self.message);
                });
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(CANVAS).inner_margin(16))
            .show(ctx, |ui| {
                if self.installing || self.restarting {
                    ui.disable();
                }
                egui::ScrollArea::vertical()
                    .id_salt("editor-content")
                    .show(ui, |ui| {
                        if self.plans.is_empty() {
                            self.folder_selection_ui(ui);
                            self.appearance_ui(ui);
                            ui.horizontal(|ui| {
                                let ready = !self.local_busy
                                    && !self.installing
                                    && self.pending_paths.is_empty()
                                    && !self.paths.is_empty();
                                if primary(
                                    ui,
                                    "Review changes",
                                    ready
                                        && self.ico.is_some()
                                        && self.preview_revision == self.revision,
                                )
                                .clicked()
                                {
                                    self.plan_action = Action::Apply;
                                    self.send(Work::Plan(
                                        self.plan_generation,
                                        self.paths.clone(),
                                        Action::Apply,
                                        self.ico.clone(),
                                    ));
                                }
                                if ui
                                    .add_enabled(ready, egui::Button::new("Restore original…"))
                                    .clicked()
                                {
                                    self.plan_action = Action::Restore;
                                    self.send(Work::Plan(
                                        self.plan_generation,
                                        self.paths.clone(),
                                        Action::Restore,
                                        None,
                                    ));
                                }
                            });
                            ui.small(
                                RichText::new(
                                    "Review before applying. Original appearances are backed up.",
                                )
                                .color(MUTED),
                            );
                        } else {
                            self.review_ui(ui);
                        }
                        for error in &self.plan_errors {
                            ui.colored_label(Color32::from_rgb(163, 47, 57), error);
                        }
                        for result in &self.report {
                            ui.label(result);
                        }
                    });
            });
        if self.settings_open {
            self.settings_ui(ctx);
        }
        if self.symbol_picker {
            self.symbol_picker_ui(ctx);
        }
        if self.drop_hover {
            let rect = ctx.content_rect().shrink(8.);
            let painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Tooltip,
                egui::Id::new("folder-drop-target"),
            ));
            painter.rect_filled(
                rect,
                10.,
                Color32::from_rgba_unmultiplied(236, 248, 245, 248),
            );
            painter.rect_stroke(rect, 10., Stroke::new(2., ACCENT), egui::StrokeKind::Inside);
            painter.text(
                rect.center() - Vec2::new(0., 14.),
                egui::Align2::CENTER_CENTER,
                if self.installing || self.restarting {
                    "Wait for installation to finish"
                } else {
                    "Drop folders to add them"
                },
                egui::FontId::proportional(24.),
                INK,
            );
            painter.text(
                rect.center() + Vec2::new(0., 22.),
                egui::Align2::CENTER_CENTER,
                "Folders are added to the selection. No files are moved.",
                egui::FontId::proportional(15.),
                MUTED,
            );
        }
    }

    fn folder_selection_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            section(ui, "Folders");
            secondary(ui, format!("{} selected", self.paths.len()));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if !self.paths.is_empty() {
                    if ui
                        .add_enabled(!self.local_busy, egui::Button::new("+ Add folders…"))
                        .clicked()
                    {
                        self.send(Work::PickFolders);
                    }
                    if ui
                        .add_enabled(!self.local_busy, egui::Button::new("Clear list"))
                        .clicked()
                    {
                        self.paths.clear();
                        self.plans.clear();
                        self.plan_errors.clear();
                    }
                }
            });
        });
        egui::Frame::new()
            .fill(Color32::WHITE)
            .stroke(Stroke::new(1., BORDER))
            .corner_radius(8)
            .inner_margin(12)
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                if self.paths.is_empty() {
                    secondary(ui, "Drop folders here, or choose them below.");
                    if primary(ui, "+ Add folders…", !self.local_busy).clicked() {
                        self.send(Work::PickFolders);
                    }
                } else {
                    let mut remove = None;
                    egui::ScrollArea::vertical()
                        .id_salt("folders")
                        .max_height(132.)
                        .show(ui, |ui| {
                            for (index, path) in self.paths.iter().enumerate() {
                                if index > 0 {
                                    ui.separator();
                                }
                                ui.horizontal(|ui| {
                                    let text_width = (ui.available_width() - 42.).max(160.);
                                    ui.vertical(|ui| {
                                        ui.set_width(text_width);
                                        ui.spacing_mut().item_spacing.y = 2.;
                                        ui.add(
                                            egui::Label::new(
                                                RichText::new(
                                                    path.file_name()
                                                        .unwrap_or_default()
                                                        .to_string_lossy(),
                                                )
                                                .strong(),
                                            )
                                            .truncate(),
                                        );
                                        ui.add(
                                            egui::Label::new(
                                                RichText::new(friendly_path(path))
                                                    .size(12.)
                                                    .color(MUTED),
                                            )
                                            .truncate(),
                                        )
                                        .on_hover_text(friendly_path(path));
                                    });
                                    if ui
                                        .add_enabled(!self.local_busy, egui::Button::new("×"))
                                        .on_hover_text("Remove from selection")
                                        .clicked()
                                    {
                                        remove = Some(index);
                                    }
                                });
                            }
                        });
                    if let Some(index) = remove {
                        self.paths.remove(index);
                        self.plans.clear();
                        self.plan_errors.clear();
                    }
                }
            });
    }

    fn appearance_ui(&mut self, ui: &mut egui::Ui) {
        section(ui, "Appearance");
        egui::Frame::new()
            .fill(Color32::WHITE)
            .stroke(Stroke::new(1., BORDER))
            .corner_radius(8)
            .inner_margin(14)
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    if let Some(texture) = &self.texture {
                        ui.add(egui::Image::new(texture).fit_to_exact_size(Vec2::splat(148.)));
                    } else {
                        ui.allocate_space(Vec2::splat(148.));
                    }
                    ui.add_space(12.);
                    ui.vertical(|ui| {
                        ui.set_width(ui.available_width());
                        let before = self.style.clone();
                        ui.horizontal(|ui| {
                            ui.add_sized([48., 32.], egui::Label::new("Color"));
                            for (name, color) in render::THEMES {
                                let selected = self.style.color == color;
                                let response = ui
                                    .add_sized(
                                        [32., 32.],
                                        egui::Button::new(if selected { "•" } else { "" })
                                            .fill(Color32::from_rgb(color[0], color[1], color[2]))
                                            .stroke(Stroke::new(
                                                if selected { 2. } else { 0. },
                                                INK,
                                            )),
                                    )
                                    .on_hover_text(name);
                                response.widget_info(|| {
                                    egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name)
                                });
                                if response.clicked() {
                                    self.style.color = color;
                                }
                            }
                            ui.color_edit_button_srgb(&mut self.style.color)
                                .on_hover_text("Custom color");
                        });
                        ui.horizontal(|ui| {
                            let label = ui.add_sized([48., 32.], egui::Label::new("Symbol"));
                            let text = if self.style.badge == Badge::None {
                                "Choose symbol…".into()
                            } else {
                                format!("{}…", self.style.badge.label())
                            };
                            if ui
                                .add_sized([150., 32.], egui::Button::new(text))
                                .labelled_by(label.id)
                                .clicked()
                            {
                                self.symbol_picker = true;
                            }
                        });
                        if before != self.style {
                            self.invalidate();
                        }
                        ui.horizontal(|ui| {
                            ui.add_sized([48., 32.], egui::Label::new("Cover"));
                            if ui
                                .add_enabled(!self.local_busy, egui::Button::new("Choose photo…"))
                                .on_hover_text("PNG or JPEG; source image stays untouched")
                                .clicked()
                            {
                                self.send(Work::PickPhoto);
                            }
                            if self.photo.is_some() && ui.button("Remove").clicked() {
                                self.photo = None;
                                self.invalidate();
                            }
                        });
                        if let Some(photo) = &self.photo {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(
                                        photo.file_name().unwrap_or_default().to_string_lossy(),
                                    )
                                    .size(12.)
                                    .color(MUTED),
                                )
                                .truncate(),
                            );
                        }
                    });
                });
            });
    }

    fn review_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading(if self.plan_action == Action::Apply {
                "Review changes"
            } else {
                "Restore originals"
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Back").clicked() {
                    self.plans.clear();
                    self.plan_errors.clear();
                }
            });
        });
        if self.plans.is_empty() {
            return;
        }
        egui::Frame::new().fill(Color32::WHITE).stroke(Stroke::new(1.,BORDER)).corner_radius(8).inner_margin(14).show(ui,|ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                if self.plan_action==Action::Apply && let Some(texture)=&self.texture {ui.add(egui::Image::new(texture).fit_to_exact_size(Vec2::splat(88.)));}
                ui.vertical(|ui| {
                    section(ui,if self.plan_action==Action::Apply {"Ready to apply"}else{"Restore original appearances"});
                    secondary(ui,format!("{} folders · No subfolders affected",self.plans.len()));
                    ui.small(if self.plan_action==Action::Apply {"Your original folder appearances are preserved."}else{"Restore the appearance saved before the first change."});
                });
            });
            ui.separator();
            egui::ScrollArea::vertical().id_salt("review-folders").max_height(200.).show(ui,|ui| {
                for plan in &self.plans {
                    ui.label(RichText::new(plan.folder.file_name().unwrap_or_default().to_string_lossy()).strong());
                    ui.add(egui::Label::new(RichText::new(friendly_path(&plan.folder)).size(13.).color(MUTED)).wrap());
                }
            });
            ui.collapsing("Files and recovery details",|ui| {ui.label(match self.plan_action {Action::Apply=>"Update folder icon entries in desktop.ini, preserve other settings, and save the icon and original backup in .foldericons.",Action::Restore=>"Restore the exact original desktop.ini and appearance attributes, then remove owned icons and the backup after success."});});
        });
        ui.horizontal(|ui| {
            if primary(
                ui,
                if self.plan_action == Action::Apply {
                    "Apply to folders"
                } else {
                    "Restore folders"
                },
                !self.local_busy && !self.installing,
            )
            .clicked()
            {
                let plans = std::mem::take(&mut self.plans);
                self.send(Work::Execute(plans));
            }
            if ui.button("Cancel").clicked() {
                self.plans.clear();
                self.plan_errors.clear();
            }
        });
    }

    fn settings_ui(&mut self, ctx: &egui::Context) {
        let mut open = self.settings_open;
        egui::Window::new("Settings & updates").open(&mut open).default_width(480.).default_height(400.).max_height((ctx.content_rect().height()-160.).max(300.)).anchor(egui::Align2::CENTER_CENTER,Vec2::ZERO).resizable(false).vscroll(true).show(ctx,|ui|{
            secondary(ui,format!("Version {VERSION} · Windows x64 · CPU rendering"));
            ui.add_space(8.);ui.separator();section(ui,"Updates");
            let toggle=ui.add_enabled(self.settings_ready&&!self.local_busy,egui::Checkbox::new(&mut self.settings.check_on_startup,"Check for updates on startup"));
            if toggle.changed(){self.send(Work::SaveSettings(self.settings.clone()));}
            secondary(ui,"Downloads and installation need your confirmation.");
            if ui.add_enabled(!self.net_busy,egui::Button::new("Check for updates")).clicked(){self.check(false);}
            ui.label(&self.update_status);
            if let Some((done,total))=self.progress {ui.add(egui::ProgressBar::new(done as f32/total.max(1) as f32).text(format!("{} / {} KB",done/1024,total/1024)));}
            if let Some(update)=self.update.clone() {
                secondary(ui,"The app will close, install the verified update and restart. Settings and folder backups are preserved.");
                if primary(ui,"Download and install",!self.net_busy&&!self.local_busy&&self.plans.is_empty()).clicked()&&self.network.as_ref().is_some_and(|network| network.download(update).is_ok()){self.net_busy=true;self.installing=true;self.update_status="Downloading and verifying update…".into();}
            }
            ui.add_space(8.);ui.separator();section(ui,"Explorer shortcut");
            secondary(ui,"Open this editor from a folder’s right-click menu.");
            ui.horizontal(|ui|{
                if ui.add_enabled(!self.local_busy,egui::Button::new("Enable context menu")).clicked(){self.send(Work::Context(true));}
                if ui.add_enabled(!self.local_busy,egui::Button::new("Remove shortcut")).clicked(){self.send(Work::Context(false));}
            });
            ui.small(RichText::new("Windows 11: Show more options. Keep the EXE in place.").color(MUTED));
            ui.add_space(8.);ui.separator();
            ui.horizontal(|ui|{ui.hyperlink_to("Project repository",REPOSITORY);ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{if ui.button("Done").clicked(){self.settings_open=false;}});});
            ui.collapsing("Diagnostics",|ui|{ui.label("Renderer: CPU rasterization / Windows GDI presentation");ui.label(&self.render_timing);});
        });
        self.settings_open = self.settings_open && open;
    }

    fn symbol_picker_ui(&mut self, ctx: &egui::Context) {
        let mut open = self.symbol_picker;
        egui::Window::new("Folder symbols")
            .open(&mut open)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .fixed_size(Vec2::new(
                464.,
                if self.symbol_category == 1 {
                    296.
                } else {
                    230.
                },
            ))
            .resizable(false)
            .vscroll(true)
            .max_height((ctx.content_rect().height() - 140.).max(260.))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    for (index, (name, _)) in Badge::GROUPS.iter().enumerate() {
                        ui.selectable_value(&mut self.symbol_category, index, *name);
                    }
                });
                ui.add_space(4.);
                if self.symbols.is_empty() {
                    ui.spinner();
                    secondary(ui, "Preparing original symbols…");
                } else {
                    egui::Grid::new(("symbol-choices", self.symbol_category))
                        .spacing([8., 8.])
                        .show(ui, |ui| {
                            for (index, badge) in
                                Badge::GROUPS[self.symbol_category].1.iter().enumerate()
                            {
                                if let Some((_, texture)) =
                                    self.symbols.iter().find(|(b, _)| b == badge)
                                {
                                    let image = egui::Image::new(texture)
                                        .fit_to_exact_size(Vec2::splat(36.));
                                    if ui
                                        .add_sized(
                                            [140., 58.],
                                            egui::Button::image_and_text(image, badge.label())
                                                .selected(self.style.badge == *badge)
                                                .image_tint_follows_text_color(false),
                                        )
                                        .clicked()
                                    {
                                        self.style.badge = *badge;
                                        self.invalidate();
                                        self.symbol_picker = false;
                                    }
                                }
                                if index % 3 == 2 {
                                    ui.end_row();
                                }
                            }
                        });
                }
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("No symbol").clicked() {
                        self.style.badge = Badge::None;
                        self.invalidate();
                        self.symbol_picker = false;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Done").clicked() {
                            self.symbol_picker = false;
                        }
                    });
                });
            });
        self.symbol_picker = self.symbol_picker && open;
    }
}

fn friendly_path(path: &std::path::Path) -> String {
    let text = path.display().to_string();
    if let Some(unc) = text.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{unc}")
    } else {
        text.strip_prefix(r"\\?\").unwrap_or(&text).to_owned()
    }
}
