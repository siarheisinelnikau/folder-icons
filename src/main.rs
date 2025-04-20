#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
use folder_icons::{
    REPOSITORY, VERSION,
    engine::{self, Action, FolderPlan},
    platform,
    render::{self, Badge, Style},
    settings::{self, Settings},
    updates,
};
use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver, SyncSender},
    time::{Duration, Instant},
};

enum Work {
    LoadSettings(Option<PathBuf>),
    SaveSettings(Settings),
    PickFolders,
    PickPhoto,
    Preview(u64, Style, Option<PathBuf>),
    Plan(u64, Vec<PathBuf>, Action, Option<Vec<u8>>),
    Execute(Vec<FolderPlan>),
    Context(bool),
}
enum ResultEvent {
    Settings(Settings, Option<String>, Option<String>),
    Saved,
    Paths(Vec<PathBuf>),
    Photo(Option<PathBuf>),
    Preview(u64, image::RgbaImage, Vec<u8>),
    Plan(u64, Vec<FolderPlan>, Vec<String>),
    Executed(Vec<String>),
    Message(String),
    Error(String),
}
enum Network {
    Check(bool),
    Install(updates::Available),
}
enum NetEvent {
    Checked(bool, Result<Option<updates::Available>, String>),
    Progress(u64, u64),
    Ready,
    Error(String),
}

fn main() -> eframe::Result {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.first().is_some_and(|x| x == "--finish-update") {
        if let Err(e) = updates::finish_install(&args[1..]) {
            // Helper has no console in release mode; leave a local diagnostic for recovery.
            if let Ok(exe) = std::env::current_exe() {
                let diagnostic = exe.with_extension("error.txt");
                let _ = std::fs::write(&diagnostic, format!("{e:#}"));
                // Reopen the preserved/replaced application to surface the failure in-app.
                if let Some(target) = args.get(2).map(PathBuf::from)
                    && target.is_absolute()
                    && target.parent() == exe.parent()
                    && target != exe
                    && target.is_file()
                {
                    let mut command = std::process::Command::new(target);
                    command.arg("--update-error").arg(diagnostic);
                    #[cfg(windows)]
                    {
                        use std::os::windows::process::CommandExt;
                        command.creation_flags(0x08000000);
                    }
                    let _ = command.spawn();
                }
            }
            std::process::exit(1);
        }
        return Ok(());
    }
    let update_error = if args.first().is_some_and(|x| x == "--update-error") {
        args.get(1).map(PathBuf::from)
    } else {
        None
    };
    let paths = args
        .into_iter()
        .skip(if update_error.is_some() { 2 } else { 0 })
        .filter(|x| !x.to_string_lossy().starts_with("--"))
        .map(PathBuf::from)
        .take(128)
        .collect();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("FolderIcons")
            .with_inner_size([1060., 740.])
            .with_min_inner_size([820., 620.]),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "FolderIcons",
        options,
        Box::new(move |cc| Ok(Box::new(App::new(cc, paths, update_error)))),
    )
}

struct App {
    work: SyncSender<Work>,
    results: Receiver<ResultEvent>,
    network: SyncSender<Network>,
    net_results: Receiver<NetEvent>,
    local_busy: bool,
    net_busy: bool,
    installing: bool,
    restarting: bool,
    settings: Settings,
    settings_ready: bool,
    settings_open: bool,
    paths: Vec<PathBuf>,
    style: Style,
    photo: Option<PathBuf>,
    revision: u64,
    preview_revision: u64,
    dirty_since: Option<Instant>,
    texture: Option<egui::TextureHandle>,
    ico: Option<Vec<u8>>,
    plans: Vec<FolderPlan>,
    plan_errors: Vec<String>,
    plan_action: Action,
    report: Vec<String>,
    message: String,
    update: Option<updates::Available>,
    update_status: String,
    progress: Option<(u64, u64)>,
}
impl App {
    fn new(
        cc: &eframe::CreationContext<'_>,
        paths: Vec<PathBuf>,
        update_error: Option<PathBuf>,
    ) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        let (tx, rx) = mpsc::sync_channel(2);
        let (rtx, rrx) = mpsc::sync_channel(8);
        let ctx = cc.egui_ctx.clone();
        std::thread::Builder::new()
            .name("foldericons-files".into())
            .spawn(move || {
                let mut photo_cache: Option<(PathBuf, image::RgbaImage)> = None;
                while let Ok(job) = rx.recv() {
                    let event = run_work(job, &mut photo_cache)
                        .unwrap_or_else(|e| ResultEvent::Error(format!("{e:#}")));
                    if rtx.send(event).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                }
            })
            .expect("Cannot start file worker");
        let (ntx, nrx) = mpsc::sync_channel(1);
        let (etx, erx) = mpsc::sync_channel(8);
        let ctx = cc.egui_ctx.clone();
        std::thread::Builder::new()
            .name("foldericons-updates".into())
            .spawn(move || {
                while let Ok(job) = nrx.recv() {
                    let event = match job {
                        Network::Check(startup) => NetEvent::Checked(
                            startup,
                            updates::check().map_err(|e| format!("{e:#}")),
                        ),
                        Network::Install(update) => {
                            let mut last = Instant::now() - Duration::from_secs(1);
                            let result = updates::download(&update, |done, total| {
                                if last.elapsed() >= Duration::from_millis(150) || done == total {
                                    let _ = etx.try_send(NetEvent::Progress(done, total));
                                    ctx.request_repaint();
                                    last = Instant::now();
                                }
                            })
                            .and_then(|stage| updates::start_install(&stage));
                            match result {
                                Ok(()) => NetEvent::Ready,
                                Err(e) => NetEvent::Error(format!("{e:#}")),
                            }
                        }
                    };
                    if etx.send(event).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                }
            })
            .expect("Cannot start update worker");
        tx.try_send(Work::LoadSettings(update_error))
            .expect("Initial queue is empty");
        Self {
            work: tx,
            results: rrx,
            network: ntx,
            net_results: erx,
            local_busy: true,
            net_busy: false,
            installing: false,
            restarting: false,
            settings: Settings::default(),
            settings_ready: false,
            settings_open: false,
            paths,
            style: Style::default(),
            photo: None,
            revision: 1,
            preview_revision: 0,
            dirty_since: Some(Instant::now()),
            texture: None,
            ico: None,
            plans: vec![],
            plan_errors: vec![],
            plan_action: Action::Apply,
            report: vec![],
            message: "Choose folders, design an icon, then review an Apply plan.".into(),
            update: None,
            update_status: "Not checked yet".into(),
            progress: None,
        }
    }
    fn send(&mut self, job: Work) {
        if self.installing || self.restarting {
            return;
        }
        if matches!(&job, Work::Plan(..)) {
            self.plans.clear();
            self.plan_errors.clear();
        }
        match self.work.try_send(job) {
            Ok(()) => self.local_busy = true,
            Err(e) => self.message = format!("Worker unavailable: {e}"),
        }
    }
    fn check(&mut self, startup: bool) {
        if !self.net_busy && self.network.try_send(Network::Check(startup)).is_ok() {
            self.net_busy = true;
            self.update_status = "Checking releases…".into();
        }
    }
    fn invalidate(&mut self) {
        self.revision += 1;
        self.dirty_since = Some(Instant::now());
        self.ico = None;
        self.plans.clear();
        self.plan_errors.clear();
    }
    fn receive(&mut self, ctx: &egui::Context) {
        while let Ok(event) = self.results.try_recv() {
            self.local_busy = false;
            match event {
                ResultEvent::Settings(settings, error, update_error) => {
                    self.settings = settings;
                    self.settings_ready = true;
                    if let Some(error) = error {
                        self.message = error;
                    }
                    if let Some(error) = update_error {
                        self.message = format!("Previous update failed: {error}");
                        self.settings_open = true;
                    }
                    if self.settings.check_on_startup {
                        self.check(true);
                    }
                }
                ResultEvent::Saved => self.message = "Settings saved.".into(),
                ResultEvent::Paths(paths) => {
                    for path in paths {
                        if self.paths.len() < 128 && !self.paths.contains(&path) {
                            self.paths.push(path);
                        }
                    }
                    self.plans.clear();
                    self.plan_errors.clear();
                }
                ResultEvent::Photo(path) => {
                    if path.is_some() {
                        self.photo = path;
                        self.invalidate();
                    }
                }
                ResultEvent::Preview(revision, img, ico) => {
                    if revision == self.revision {
                        let color = egui::ColorImage::from_rgba_unmultiplied(
                            [img.width() as usize, img.height() as usize],
                            img.as_raw(),
                        );
                        self.texture = Some(ctx.load_texture(
                            "folder-preview",
                            color,
                            egui::TextureOptions::LINEAR,
                        ));
                        self.ico = Some(ico);
                        self.preview_revision = revision;
                    }
                }
                ResultEvent::Plan(revision, plans, errors) => {
                    if revision == self.revision {
                        self.plans = plans;
                        self.plan_errors = errors;
                        self.report.clear();
                        self.message =
                            "Review the exact folders below, then confirm the plan.".into();
                    } else {
                        self.message = "Design changed while planning. Build a new plan.".into();
                    }
                }
                ResultEvent::Executed(report) => {
                    self.report = report;
                    self.plans.clear();
                    self.plan_errors.clear();
                    self.message = "Batch completed. Review each result below.".into();
                }
                ResultEvent::Message(message) => self.message = message,
                ResultEvent::Error(error) => {
                    self.message = error;
                    self.plans.clear();
                }
            }
        }
        while let Ok(event) = self.net_results.try_recv() {
            match event {
                NetEvent::Checked(startup, result) => {
                    self.net_busy = false;
                    match result {
                        Ok(Some(update)) => {
                            self.update_status = format!("Version {} is available", update.version);
                            self.update = Some(update);
                        }
                        Ok(None) => {
                            self.update = None;
                            self.update_status = "You are up to date.".into();
                        }
                        Err(error) => {
                            self.update_status = error;
                            if !startup {
                                self.settings_open = true;
                            }
                        }
                    }
                }
                NetEvent::Progress(done, total) => self.progress = Some((done, total)),
                NetEvent::Ready => {
                    self.installing = false;
                    self.restarting = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                NetEvent::Error(error) => {
                    self.installing = false;
                    self.net_busy = false;
                    self.progress = None;
                    self.update_status = error;
                    self.settings_open = true;
                }
            }
        }
        if !self.local_busy
            && !self.installing
            && !self.restarting
            && let Some(since) = self.dirty_since
        {
            if since.elapsed() >= Duration::from_millis(160) {
                self.dirty_since = None;
                self.send(Work::Preview(
                    self.revision,
                    self.style.clone(),
                    self.photo.clone(),
                ));
            } else {
                ctx.request_repaint_after(Duration::from_millis(160) - since.elapsed());
            }
        }
    }
    fn settings_ui(&mut self, ctx: &egui::Context) {
        let mut open = self.settings_open;
        egui::Window::new("Settings & updates").open(&mut open).default_width(530.).resizable(true).show(ctx,|ui|{
            ui.heading("FolderIcons");ui.label(format!("Installed version: {VERSION} · Windows x64 portable"));
            ui.hyperlink_to("Canonical repository (planned)",REPOSITORY);
            ui.separator();
            let response=ui.add_enabled(self.settings_ready&&!self.local_busy,egui::Checkbox::new(&mut self.settings.check_on_startup,"Check for updates on startup"));
            if response.changed(){self.send(Work::SaveSettings(self.settings.clone()));}
            ui.label("Startup checks run in the background. Downloads and installation require your action.");
            if ui.add_enabled(!self.net_busy,egui::Button::new("Check for updates")).clicked(){self.check(false);}
            ui.label(&self.update_status);
            if let Some((done,total))=self.progress {ui.add(egui::ProgressBar::new(done as f32/total.max(1) as f32).text(format!("Downloading: {} / {} KB",done/1024,total/1024)));}
            if let Some(update)=self.update.clone() {
                ui.label("Installation verifies SHA-256, closes this app, replaces its EXE and restarts. Folder backups and settings are preserved.");
                if ui.add_enabled(!self.net_busy&&!self.local_busy&&self.plans.is_empty(),egui::Button::new("Download and install")).clicked()
                    && self.network.try_send(Network::Install(update)).is_ok() {
                    self.net_busy=true;self.installing=true;self.update_status="Downloading and verifying update…".into();
                }
            }
            ui.separator();ui.heading("Explorer integration");
            ui.label("Optional per-user context menu. No administrator rights required. Keep the executable at its current location after enabling.");
            ui.horizontal(|ui|{
                if ui.add_enabled(!self.local_busy,egui::Button::new("Enable context menu")).clicked(){self.send(Work::Context(true));}
                if ui.add_enabled(!self.local_busy,egui::Button::new("Remove context menu")).clicked(){self.send(Work::Context(false));}
            });
            ui.label("On Windows 11, this command may appear under Show more options. It opens FolderIcons with the selected folder; it never applies an icon automatically.");
        });
        self.settings_open = open;
    }
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.receive(ctx);
        if ctx.input(|i| i.viewport().close_requested()) && (self.local_busy || self.installing) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.message = "Wait for the current operation to finish before closing.".into();
        }
    }

    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if self.installing || self.restarting {
            root.disable();
        }
        let ctx = root.ctx().clone();
        egui::Panel::top("header").show(root, |ui| {
            ui.add_space(8.);
            ui.horizontal(|ui| {
                ui.heading("FolderIcons");
                ui.label("Give every folder a clear identity");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Settings & updates").clicked() {
                        self.settings_open = true;
                    }
                });
            });
            ui.add_space(8.);
            if let Some(update) = &self.update {
                ui.horizontal(|ui| {
                    ui.colored_label(
                        egui::Color32::LIGHT_GREEN,
                        format!("FolderIcons {} is available", update.version),
                    );
                    if ui.button("View update").clicked() {
                        self.settings_open = true;
                    }
                });
            }
        });
        egui::Panel::bottom("status").show(root, |ui| {
            ui.horizontal(|ui| {
                if self.local_busy {
                    ui.spinner();
                }
                ui.label(&self.message);
            });
        });
        egui::Panel::left("design").exact_size(292.).show(root,|ui|{
            egui::ScrollArea::vertical().id_salt("design_scroll").show(ui,|ui|{
            ui.heading("Design");
            if let Some(texture)=&self.texture {ui.add(egui::Image::new(texture).fit_to_exact_size(egui::vec2(240.,240.)));}
            else {ui.allocate_space(egui::vec2(240.,240.));}
            if self.preview_revision!=self.revision {ui.label("Preparing live preview…");}
            let before=self.style.clone();
            ui.label("Original themes");
            ui.horizontal_wrapped(|ui|{for (name,color) in render::THEMES {
                if ui.add(egui::Button::new(name).fill(egui::Color32::from_rgb(color[0],color[1],color[2]))).clicked(){self.style.color=color;}
            }});
            ui.horizontal(|ui|{ui.label("Custom color");ui.color_edit_button_srgb(&mut self.style.color);});
            egui::ComboBox::from_label("Badge").selected_text(self.style.badge.label()).show_ui(ui,|ui|{for badge in Badge::ALL {ui.selectable_value(&mut self.style.badge,badge,badge.label());}});
            if before!=self.style {self.invalidate();}
            ui.separator();ui.label("Photo cover · PNG or JPEG");
            if let Some(photo)=&self.photo {ui.label(photo.file_name().unwrap_or_default().to_string_lossy());}
            ui.horizontal(|ui|{
                if ui.add_enabled(!self.local_busy,egui::Button::new("Choose photo…")).clicked(){self.send(Work::PickPhoto);}
                if ui.add_enabled(self.photo.is_some(),egui::Button::new("Remove")).clicked(){self.photo=None;self.invalidate();}
            });
            ui.small("Photos are embedded in the generated icon. The original photo stays untouched. Each ICO includes sizes from 16 to 256 px.");
            });
        });
        egui::CentralPanel::default().show(root,|ui|{
            ui.horizontal(|ui|{ui.heading("Folders");ui.label(format!("{} / 128 selected",self.paths.len()));});
            ui.horizontal(|ui|{
                if ui.add_enabled(!self.local_busy,egui::Button::new("Add folders…")).clicked(){self.send(Work::PickFolders);}
                if ui.add_enabled(!self.local_busy&&!self.paths.is_empty(),egui::Button::new("Clear list")).clicked(){self.paths.clear();self.plans.clear();self.plan_errors.clear();}
            });
            ui.label("Changes affect only the selected folders, without recursion.");
            let mut remove=None;
            egui::ScrollArea::vertical().id_salt("folders").max_height(170.).show(ui,|ui|{
                for (index,path) in self.paths.iter().enumerate(){ui.horizontal(|ui|{if ui.add_enabled(!self.local_busy,egui::Button::new("×")).clicked(){remove=Some(index);}ui.label(path.display().to_string());});}
            });
            if let Some(index)=remove {self.paths.remove(index);self.plans.clear();self.plan_errors.clear();}
            ui.separator();
            ui.horizontal(|ui|{
                let ready=!self.local_busy&&!self.installing&&!self.paths.is_empty();
                if ui.add_enabled(ready&&self.ico.is_some()&&self.preview_revision==self.revision,egui::Button::new("Build Apply plan")).clicked(){self.plan_action=Action::Apply;self.send(Work::Plan(self.revision,self.paths.clone(),Action::Apply,self.ico.clone()));}
                if ui.add_enabled(ready,egui::Button::new("Build Restore plan")).clicked(){self.plan_action=Action::Restore;self.send(Work::Plan(self.revision,self.paths.clone(),Action::Restore,None));}
            });
            egui::ScrollArea::vertical().id_salt("plan").show(ui,|ui|{
                if !self.plans.is_empty() {
                    ui.heading(match self.plan_action {Action::Apply=>"Review Apply plan",Action::Restore=>"Review Restore plan"});
                    for plan in &self.plans {ui.group(|ui|{
                        ui.label(plan.folder.display().to_string());
                        match plan.action {
                            Action::Apply=>{ui.small(if plan.had_ini {"Preserve existing settings; replace icon entries in desktop.ini."}else{"Create Unicode desktop.ini."});ui.small("Store the icon and original backup in .foldericons; mark the folder read-only for Explorer customization.");}
                            Action::Restore=>{ui.small("Restore the original desktop.ini bytes and appearance attributes; remove owned generated icons and the backup after success.");}
                        }
                    });}
                    ui.label("Each folder is independent. Errors are reported per folder; successful folders stay applied. A changed folder is skipped until a new plan is built.");
                    let label=if self.plan_action==Action::Apply {"Apply reviewed plan"}else{"Restore reviewed plan"};
                    ui.horizontal(|ui|{
                        if ui.add_enabled(!self.local_busy&&!self.installing,egui::Button::new(label)).clicked(){let plans=std::mem::take(&mut self.plans);self.send(Work::Execute(plans));}
                        if ui.button("Discard plan").clicked(){self.plans.clear();self.plan_errors.clear();}
                    });
                }
                for error in &self.plan_errors {ui.colored_label(egui::Color32::LIGHT_RED,error);}
                for result in &self.report {ui.label(result);}
                if self.paths.is_empty() {ui.add_space(30.);ui.heading("Start with a folder");ui.label("Add one or several folders. Choose a color, badge or photo cover, then review exactly what will be written before applying.");}
            });
        });
        if self.settings_open {
            self.settings_ui(&ctx);
        }
    }
}

fn run_work(
    job: Work,
    photo_cache: &mut Option<(PathBuf, image::RgbaImage)>,
) -> anyhow::Result<ResultEvent> {
    Ok(match job {
        Work::LoadSettings(update_error) => {
            let diagnostic = update_error.and_then(|path| {
                let exe = std::env::current_exe().ok()?;
                if path.parent() != exe.parent()
                    || !path
                        .file_name()?
                        .to_str()?
                        .starts_with(".FolderIcons-helper-")
                    || path.extension()?.to_str()? != "txt"
                {
                    return None;
                }
                platform::read_optional(&path, 64 * 1024)
                    .ok()
                    .flatten()
                    .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            });
            match settings::load() {
                Ok(s) => ResultEvent::Settings(s, None, diagnostic),
                Err(e) => ResultEvent::Settings(
                    Settings {
                        check_on_startup: false,
                    },
                    Some(format!(
                        "Cannot load settings; startup checks disabled for this session: {e:#}"
                    )),
                    diagnostic,
                ),
            }
        }
        Work::SaveSettings(s) => {
            settings::save(&s)?;
            ResultEvent::Saved
        }
        Work::PickFolders => ResultEvent::Paths(
            rfd::FileDialog::new()
                .set_title("Select folders to customize")
                .pick_folders()
                .unwrap_or_default(),
        ),
        Work::PickPhoto => ResultEvent::Photo(
            rfd::FileDialog::new()
                .set_title("Choose photo cover")
                .add_filter("Photos", &["png", "jpg", "jpeg"])
                .pick_file(),
        ),
        Work::Preview(revision, style, photo) => {
            if let Some(path) = photo {
                if photo_cache.as_ref().map(|(p, _)| p) != Some(&path) {
                    *photo_cache = Some((path.clone(), render::load_photo(&path)?));
                }
            } else {
                *photo_cache = None;
            }
            let img = render::render(&style, photo_cache.as_ref().map(|(_, i)| i));
            let ico = render::ico(&img)?;
            ResultEvent::Preview(revision, img, ico)
        }
        Work::Plan(revision, paths, action, ico) => {
            platform::require_windows()?;
            let mut plans = Vec::new();
            let mut errors = Vec::new();
            for (path, result) in engine::batch_plan(&paths, action, ico.as_deref()) {
                match result {
                    Ok(plan) => plans.push(plan),
                    Err(e) => errors.push(format!("Skipped {}: {e:#}", path.display())),
                }
            }
            ResultEvent::Plan(revision, plans, errors)
        }
        Work::Execute(plans) => {
            platform::require_windows()?;
            let mut report = Vec::new();
            for plan in plans {
                match engine::execute(&plan) {
                    Ok(()) => {
                        platform::notify_folder(&plan.folder);
                        report.push(format!(
                            "OK · {:?} · {}",
                            plan.action,
                            plan.folder.display()
                        ));
                    }
                    Err(e) => report.push(format!("FAILED · {} · {e:#}", plan.folder.display())),
                }
            }
            ResultEvent::Executed(report)
        }
        Work::Context(enable) => {
            platform::context_menu(enable)?;
            ResultEvent::Message(
                if enable {
                    "Explorer context menu enabled."
                } else {
                    "Explorer context menu removed."
                }
                .into(),
            )
        }
    })
}
