#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod capture;
mod desktop;
mod ui;
use folder_icons::{
    REPOSITORY, VERSION,
    engine::{self, Action, FolderPlan},
    platform,
    render::{self, Style},
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

fn main() {
    std::panic::set_hook(Box::new(|info| write_error_log(&info.to_string())));
    if let Err(error) = run() {
        write_error_log(&format!("{error:#}"));
        std::process::exit(1);
    }
}

fn write_error_log(message: &str) {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::fs::write(exe.with_extension("error.log"), message);
    }
}

fn run() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.first().is_some_and(|a| a == "--capture") {
        return capture::run(&args[1..]);
    }
    if args.first().is_some_and(|a| a == "--diagnose") {
        anyhow::ensure!(
            (2..=3).contains(&args.len()),
            "--diagnose requires an output JSON and optional fixture root"
        );
        return desktop::diagnose(PathBuf::from(&args[1]), args.get(2).map(PathBuf::from));
    }
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
    desktop::run(paths, update_error)
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
    render_timing: String,
}
impl App {
    fn new(
        context: egui::Context,
        paths: Vec<PathBuf>,
        update_error: Option<PathBuf>,
        isolated: bool,
    ) -> Self {
        ui::configure(&context);
        let (tx, rx) = mpsc::sync_channel(2);
        let (rtx, rrx) = mpsc::sync_channel(8);
        let ctx = context.clone();
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
        let ctx = context.clone();
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
        if !isolated {
            tx.try_send(Work::LoadSettings(update_error))
                .expect("Initial queue is empty");
        }
        Self {
            work: tx,
            results: rrx,
            network: ntx,
            net_results: erx,
            local_busy: !isolated,
            net_busy: false,
            installing: false,
            restarting: false,
            settings: Settings::default(),
            settings_ready: isolated,
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
            render_timing: "No frame samples yet".into(),
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
}

impl App {
    fn logic(&mut self, ctx: &egui::Context) {
        self.receive(ctx);
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
