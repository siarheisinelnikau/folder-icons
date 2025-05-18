use crate::{
    Action, App, Network, REPOSITORY, VERSION, Work,
    render::{self, Badge},
};
use egui::{Color32, RichText, Stroke, Vec2};

pub const INK: Color32 = Color32::from_rgb(31, 48, 68);
pub const MUTED: Color32 = Color32::from_rgb(91, 108, 127);
pub const ACCENT: Color32 = Color32::from_rgb(27, 116, 105);
pub const BORDER: Color32 = Color32::from_rgb(215, 224, 232);
pub const CANVAS: Color32 = Color32::from_rgb(244, 247, 250);

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
    style.spacing.item_spacing = Vec2::new(10., 10.);
    style.spacing.button_padding = Vec2::new(14., 8.);
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
                    .inner_margin(egui::Margin::symmetric(24, 18)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new("FolderIcons").size(27.).strong());
                        secondary(ui, "Make important folders easy to find.");
                    });
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
                    .inner_margin(egui::Margin::symmetric(24, 12)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if self.local_busy {
                        ui.spinner();
                    }
                    ui.add(
                        egui::Label::new(RichText::new(&self.message).size(13.).color(MUTED))
                            .wrap(),
                    );
                });
            });
        egui::SidePanel::left("design")
            .exact_width(312.)
            .resizable(false)
            .frame(egui::Frame::new().fill(Color32::WHITE).inner_margin(20))
            .show(ctx, |ui| {
                if self.installing || self.restarting {
                    ui.disable();
                }
                egui::ScrollArea::vertical()
                    .id_salt("design-scroll")
                    .show(ui, |ui| {
                        section(ui, "Your folder style");
                        egui::Frame::new()
                            .fill(CANVAS)
                            .corner_radius(12)
                            .inner_margin(10)
                            .show(ui, |ui| {
                                if let Some(texture) = &self.texture {
                                    ui.add(
                                        egui::Image::new(texture)
                                            .fit_to_exact_size(Vec2::splat(180.)),
                                    );
                                } else {
                                    ui.allocate_space(Vec2::splat(180.));
                                }
                            });
                        ui.add_space(4.);
                        section(ui, "Color");
                        let before = self.style.clone();
                        ui.horizontal(|ui| {
                            for (name, color) in render::THEMES {
                                let fill = Color32::from_rgb(color[0], color[1], color[2]);
                                let selected = self.style.color == color;
                                let response = ui
                                    .add_sized(
                                        [34., 34.],
                                        egui::Button::new(if selected { "•" } else { "" })
                                            .fill(fill)
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
                        });
                        ui.horizontal(|ui| {
                            ui.color_edit_button_srgb(&mut self.style.color);
                            secondary(ui, "Choose a custom color");
                        });
                        ui.add_space(6.);
                        section(ui, "Badge");
                        for row in Badge::ALL.chunks(3) {
                            ui.horizontal(|ui| {
                                for badge in row {
                                    ui.selectable_value(
                                        &mut self.style.badge,
                                        *badge,
                                        badge.label(),
                                    );
                                }
                            });
                        }
                        if before != self.style {
                            self.invalidate();
                        }
                        ui.add_space(6.);
                        section(ui, "Photo cover");
                        secondary(ui, "Use a PNG or JPEG from your computer.");
                        if let Some(photo) = &self.photo {
                            ui.add(
                                egui::Label::new(
                                    photo.file_name().unwrap_or_default().to_string_lossy(),
                                )
                                .truncate(),
                            );
                        }
                        ui.horizontal(|ui| {
                            if ui
                                .add_enabled(!self.local_busy, egui::Button::new("Choose photo…"))
                                .clicked()
                            {
                                self.send(Work::PickPhoto);
                            }
                            if self.photo.is_some() && ui.button("Remove").clicked() {
                                self.photo = None;
                                self.invalidate();
                            }
                        });
                        ui.small(RichText::new("Your source image stays untouched.").color(MUTED));
                    });
            });
        egui::CentralPanel::default().frame(egui::Frame::new().fill(CANVAS).inner_margin(24)).show(ctx,|ui|{
            if self.installing||self.restarting {ui.disable();}
            ui.horizontal(|ui|{
                ui.heading("Choose your folders");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{secondary(ui,format!("{} selected",self.paths.len()));});
            });
            secondary(ui,"Select folders, customize their look, then review your changes.");
            ui.add_space(8.);
            ui.horizontal(|ui|{
                if ui.add_enabled(!self.local_busy,egui::Button::new("+ Add folders…")).clicked(){self.send(Work::PickFolders);}
                if !self.paths.is_empty()&&ui.add_enabled(!self.local_busy,egui::Button::new("Clear list")).clicked(){self.paths.clear();self.plans.clear();self.plan_errors.clear();}
            });
            ui.add_space(4.);
            if self.paths.is_empty() {
                egui::Frame::new().fill(Color32::WHITE).stroke(Stroke::new(1.,BORDER)).corner_radius(10).inner_margin(24).show(ui,|ui|{
                    ui.set_min_width(ui.available_width());
                    ui.add_space(18.);section(ui,"A clearer desktop starts here");
                    secondary(ui,"Add one folder or a whole group. Only folders you select will change.");
                    ui.add_space(16.);
                    ui.label("1   Choose folders");ui.label("2   Pick a color, badge or cover");ui.label("3   Review and apply");
                    ui.add_space(18.);
                });
            } else {
                let mut remove=None;
                egui::ScrollArea::vertical().id_salt("folders").max_height(164.).show(ui,|ui|{
                    for (index,path) in self.paths.iter().enumerate() {
                        egui::Frame::new().fill(Color32::WHITE).stroke(Stroke::new(1.,BORDER)).corner_radius(8).inner_margin(12).show(ui,|ui|{
                            ui.set_min_width(ui.available_width());
                            ui.horizontal(|ui|{
                                ui.vertical(|ui|{
                                    ui.set_max_width((ui.available_width()-60.).max(180.));
                                    ui.label(RichText::new(path.file_name().unwrap_or_default().to_string_lossy()).strong());
                                    ui.add(egui::Label::new(RichText::new(path.display().to_string()).size(12.).color(MUTED)).truncate()).on_hover_text(path.display().to_string());
                                });
                                if ui.add_enabled(!self.local_busy,egui::Button::new("×")).on_hover_text("Remove from selection").clicked(){remove=Some(index);}
                            });
                        });
                    }
                });
                if let Some(index)=remove {self.paths.remove(index);self.plans.clear();self.plan_errors.clear();}
                ui.add_space(8.);
                if self.plans.is_empty() { ui.horizontal(|ui|{
                    let ready=!self.local_busy&&!self.installing&&!self.paths.is_empty();
                    if primary(ui,"Review changes",ready&&self.ico.is_some()&&self.preview_revision==self.revision).clicked(){self.plan_action=Action::Apply;self.send(Work::Plan(self.revision,self.paths.clone(),Action::Apply,self.ico.clone()));}
                    if ui.add_enabled(ready,egui::Button::new("Restore original…")).clicked(){self.plan_action=Action::Restore;self.send(Work::Plan(self.revision,self.paths.clone(),Action::Restore,None));}
                });
                ui.small(RichText::new("Nothing changes until you confirm. Original appearances are backed up.").color(MUTED)); }
            }
            ui.add_space(12.);
            egui::ScrollArea::vertical().id_salt("review").show(ui,|ui|{
                if !self.plans.is_empty() {
                    egui::Frame::new().fill(Color32::WHITE).stroke(Stroke::new(1.,BORDER)).corner_radius(10).inner_margin(18).show(ui,|ui|{
                        section(ui,if self.plan_action==Action::Apply {"Ready to apply"}else{"Restore original appearances"});
                        secondary(ui,format!("{} folders · Selected folders only, without recursion",self.plans.len()));
                        for plan in &self.plans {ui.label(friendly_path(&plan.folder));}
                        ui.collapsing("Files and recovery details",|ui|{ui.label(match self.plan_action {Action::Apply=>"Update the folder icon entries in desktop.ini, preserve other settings, and save the icon and original backup in .foldericons.",Action::Restore=>"Restore the exact original desktop.ini and appearance attributes, then remove owned icons and the backup after success."});});
                        ui.horizontal(|ui|{
                            if primary(ui,if self.plan_action==Action::Apply {"Apply to folders"}else{"Restore folders"},!self.local_busy&&!self.installing).clicked(){let plans=std::mem::take(&mut self.plans);self.send(Work::Execute(plans));}
                            if ui.button("Cancel").clicked(){self.plans.clear();self.plan_errors.clear();}
                        });
                    });
                }
                for error in &self.plan_errors {ui.colored_label(Color32::from_rgb(163,47,57),error);}
                for result in &self.report {ui.label(result);}
            });
        });
        if self.settings_open {
            self.settings_ui(ctx);
        }
    }

    fn settings_ui(&mut self, ctx: &egui::Context) {
        let mut open = self.settings_open;
        egui::Window::new("Settings & updates").open(&mut open).default_width(540.).default_height(520.).default_pos([360.,100.]).resizable(false).vscroll(true).show(ctx,|ui|{
            section(ui,"FolderIcons");secondary(ui,format!("Version {VERSION} · Windows x64 · CPU rendering"));
            ui.add_space(8.);ui.separator();section(ui,"Updates");
            let toggle=ui.add_enabled(self.settings_ready&&!self.local_busy,egui::Checkbox::new(&mut self.settings.check_on_startup,"Check for updates on startup"));
            if toggle.changed(){self.send(Work::SaveSettings(self.settings.clone()));}
            secondary(ui,"Checks run quietly. Downloading and installing always require your action.");
            if ui.add_enabled(!self.net_busy,egui::Button::new("Check for updates")).clicked(){self.check(false);}
            ui.label(&self.update_status);
            if let Some((done,total))=self.progress {ui.add(egui::ProgressBar::new(done as f32/total.max(1) as f32).text(format!("{} / {} KB",done/1024,total/1024)));}
            if let Some(update)=self.update.clone() {
                secondary(ui,"The app will close, install the verified update and restart. Settings and folder backups are preserved.");
                if primary(ui,"Download and install",!self.net_busy&&!self.local_busy&&self.plans.is_empty()).clicked()&&self.network.try_send(Network::Install(update)).is_ok(){self.net_busy=true;self.installing=true;self.update_status="Downloading and verifying update…".into();}
            }
            ui.add_space(8.);ui.separator();section(ui,"Explorer shortcut");
            secondary(ui,"Add “Customize with FolderIcons” to a folder's right-click menu. The shortcut opens this editor; it never applies changes automatically.");
            ui.horizontal(|ui|{
                if ui.add_enabled(!self.local_busy,egui::Button::new("Enable context menu")).clicked(){self.send(Work::Context(true));}
                if ui.add_enabled(!self.local_busy,egui::Button::new("Remove shortcut")).clicked(){self.send(Work::Context(false));}
            });
            ui.small(RichText::new("Windows 11: look under Show more options. Keep the EXE in place after enabling.").color(MUTED));
            ui.add_space(8.);ui.separator();
            ui.horizontal(|ui|{ui.hyperlink_to("Project repository",REPOSITORY);ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{if ui.button("Done").clicked(){self.settings_open=false;}});});
            ui.collapsing("Diagnostics",|ui|{ui.label("Renderer: CPU rasterization / Windows GDI presentation");ui.label(&self.render_timing);});
        });
        self.settings_open = self.settings_open && open;
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
