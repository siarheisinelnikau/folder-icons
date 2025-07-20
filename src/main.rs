#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod capture;
mod desktop;
mod ui;
use folder_icons::{
    REPOSITORY, VERSION,
    engine::{self, Action, FolderPlan},
    platform,
    render::{self, Style},
    selection,
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
    InspectPaths(Vec<PathBuf>, Vec<PathBuf>, usize),
    SymbolCatalog,
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
    Selection(selection::Intake),
    Symbols(Vec<(render::Badge, image::RgbaImage)>),
    Photo(Option<PathBuf>),
    Preview(u64, image::RgbaImage, Vec<u8>),
    Plan(u64, Vec<FolderPlan>, Vec<String>),
    Executed(Vec<String>),
    Message(String),
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
    if args.first().is_some_and(|x| x == "--apply-update") {
        return rust_auto_updator::apply_update(&updates::config().map_err(anyhow::Error::msg)?)
            .map_err(anyhow::Error::msg);
    }
    let cleanup = if args.first().is_some_and(|x| x == "--cleanup-update") {
        anyhow::ensure!(args.len() == 2, "Invalid update cleanup arguments");
        Some(PathBuf::from(&args[1]))
    } else {
        None
    };
    let update_error = if args.first().is_some_and(|x| x == "--update-error") {
        args.get(1).map(PathBuf::from)
    } else {
        None
    };
    let paths = args
        .into_iter()
        .skip(if update_error.is_some() || cleanup.is_some() {
            2
        } else {
            0
        })
        .filter(|x| !x.to_string_lossy().starts_with("--"))
        .map(PathBuf::from)
        .take(128)
        .collect();
    desktop::run(paths, update_error, cleanup)
}

struct App {
    work: SyncSender<Work>,
    results: Receiver<ResultEvent>,
    network: Option<updates::Service>,
    check_is_startup: bool,
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
    update: Option<Box<updates::Available>>,
    update_status: String,
    progress: Option<(u64, u64)>,
    render_timing: String,
    pending_paths: Vec<PathBuf>,
    pending_overflow: usize,
    pending_since: Option<Instant>,
    plan_generation: u64,
    drop_hover: bool,
    symbol_picker: bool,
    symbol_category: usize,
    symbols: Vec<(render::Badge, egui::TextureHandle)>,
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
        let ctx = context.clone();
        let network = if isolated {
            Err("Update checks are disabled in preview mode.".into())
        } else {
            updates::config()
                .and_then(|config| updates::Service::new(config, move || ctx.request_repaint()))
        };
        let update_status = network
            .as_ref()
            .err()
            .cloned()
            .unwrap_or_else(|| "Not checked yet".into());
        if !isolated {
            tx.try_send(Work::LoadSettings(update_error))
                .expect("Initial queue is empty");
        }
        Self {
            work: tx,
            results: rrx,
            network: network.ok(),
            check_is_startup: false,
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
            update_status,
            progress: None,
            render_timing: "No frame samples yet".into(),
            pending_paths: Vec::new(),
            pending_overflow: 0,
            pending_since: None,
            plan_generation: 1,
            drop_hover: false,
            symbol_picker: false,
            symbol_category: 0,
            symbols: Vec::new(),
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
        let catalog = matches!(&job, Work::SymbolCatalog);
        match self.work.try_send(job) {
            Ok(()) => self.local_busy = true,
            Err(e) => {
                if catalog {
                    self.symbol_picker = false;
                }
                self.message = format!("Worker unavailable: {e}");
            }
        }
    }
    fn check(&mut self, startup: bool) {
        if !self.net_busy
            && self
                .network
                .as_ref()
                .is_some_and(|network| network.check().is_ok())
        {
            self.check_is_startup = startup;
            self.net_busy = true;
            self.update_status = "Checking releases…".into();
        }
    }
    fn invalidate(&mut self) {
        self.revision += 1;
        self.dirty_since = Some(Instant::now());
        self.ico = None;
        self.invalidate_plan();
    }
    fn invalidate_plan(&mut self) {
        self.plan_generation += 1;
        self.plans.clear();
        self.plan_errors.clear();
    }
    fn queue_folders(&mut self, paths: Vec<PathBuf>) {
        self.drop_hover = false;
        if self.installing || self.restarting {
            self.message = "Wait for installation to finish before adding folders.".into();
            return;
        }
        if paths.is_empty() {
            return;
        }
        self.settings_open = false;
        self.symbol_picker = false;
        self.invalidate_plan();
        for path in paths {
            if self.pending_paths.len() < selection::MAX_PENDING {
                self.pending_paths.push(path);
            } else {
                self.pending_overflow = self.pending_overflow.saturating_add(1);
            }
        }
        self.pending_since = Some(Instant::now());
        self.message = "Checking folders…".into();
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
                    self.queue_folders(paths);
                }
                ResultEvent::Selection(intake) => {
                    self.message = intake.message();
                    if !intake.folders.is_empty() {
                        self.paths.extend(intake.folders);
                        self.report.clear();
                        self.invalidate_plan();
                    }
                }
                ResultEvent::Symbols(images) => self.install_symbols(ctx, images),
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
                    if revision == self.plan_generation {
                        self.plans = plans;
                        self.plan_errors = errors;
                        self.report.clear();
                        self.message =
                            "Review the exact folders below, then confirm the plan.".into();
                    } else {
                        self.message =
                            "Selection or design changed. Review the changes again.".into();
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
        if !self.local_busy
            && !self.pending_paths.is_empty()
            && !self.installing
            && !self.restarting
        {
            let elapsed = self
                .pending_since
                .map_or(Duration::ZERO, |since| since.elapsed());
            if elapsed >= Duration::from_millis(80) {
                let paths = std::mem::take(&mut self.pending_paths);
                let overflow = std::mem::take(&mut self.pending_overflow);
                self.pending_since = None;
                self.send(Work::InspectPaths(paths, self.paths.clone(), overflow));
            } else {
                ctx.request_repaint_after(Duration::from_millis(80) - elapsed);
            }
        }
        while let Some(event) = self
            .network
            .as_ref()
            .and_then(|network| network.events.try_recv().ok())
        {
            match event {
                updates::Event::Checked(result) => {
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
                            if !self.check_is_startup {
                                self.settings_open = true;
                            }
                        }
                    }
                }
                updates::Event::Progress { received, total } => {
                    self.progress = Some((received, total))
                }
                updates::Event::Ready(result) => {
                    let result = result.and_then(|mut prepared| updates::handoff(&mut prepared));
                    self.installing = false;
                    self.net_busy = false;
                    match result {
                        Ok(()) => self.restarting = true,
                        Err(error) => {
                            self.progress = None;
                            self.update_status = error;
                            self.settings_open = true;
                        }
                    }
                }
            }
        }
        if !self.local_busy
            && self.symbol_picker
            && self.symbols.is_empty()
            && !self.installing
            && !self.restarting
        {
            self.send(Work::SymbolCatalog);
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
    fn install_symbols(
        &mut self,
        ctx: &egui::Context,
        images: Vec<(render::Badge, image::RgbaImage)>,
    ) {
        self.symbols = images
            .into_iter()
            .map(|(badge, img)| {
                let texture = ctx.load_texture(
                    format!("symbol-{badge:?}"),
                    egui::ColorImage::from_rgba_unmultiplied(
                        [img.width() as usize, img.height() as usize],
                        img.as_raw(),
                    ),
                    egui::TextureOptions::LINEAR,
                );
                (badge, texture)
            })
            .collect();
    }
    fn next_work_deadline(&self) -> Option<Instant> {
        if self.local_busy || self.installing || self.restarting {
            return None;
        }
        let intake = if self.pending_paths.is_empty() {
            None
        } else {
            self.pending_since.map(|t| t + Duration::from_millis(80))
        };
        let preview = self.dirty_since.map(|t| t + Duration::from_millis(160));
        if self.symbol_picker && self.symbols.is_empty() {
            return Some(Instant::now());
        }
        match (intake, preview) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
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
        Work::InspectPaths(paths, existing, overflow) => {
            ResultEvent::Selection(selection::inspect(&paths, &existing, overflow))
        }
        Work::SymbolCatalog => ResultEvent::Symbols(render::symbol_catalog()),
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

#[cfg(test)]
mod intake_tests {
    use super::*;
    fn root() -> tempfile::TempDir {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".test-output");
        std::fs::create_dir_all(&path).unwrap();
        tempfile::tempdir_in(path).unwrap()
    }
    fn settle(app: &mut App, ctx: &egui::Context) {
        let until = Instant::now() + Duration::from_secs(3);
        while app.local_busy || !app.pending_paths.is_empty() {
            assert!(Instant::now() < until, "Folder worker did not settle");
            app.logic(ctx);
            std::thread::park_timeout(Duration::from_millis(5));
        }
    }
    #[test]
    fn drop_queued_during_preview_keeps_the_preview_and_deduplicates_the_next_batch() {
        let root = root();
        let folder = root.path().join("旅行 Project");
        std::fs::create_dir(&folder).unwrap();
        let ctx = egui::Context::default();
        let mut app = App::new(ctx.clone(), vec![], None, true);
        app.dirty_since = None;
        app.send(Work::Preview(app.revision, app.style.clone(), None));
        app.queue_folders(vec![folder.clone()]);
        settle(&mut app, &ctx);
        assert_eq!(app.paths, [std::fs::canonicalize(&folder).unwrap()]);
        assert!(app.ico.is_some());
        assert_eq!(app.preview_revision, app.revision);
        app.queue_folders(vec![folder.join(".")]);
        settle(&mut app, &ctx);
        assert_eq!(app.paths.len(), 1);
        assert!(app.message.contains("already selected"));
        assert_eq!(std::fs::read_dir(folder).unwrap().count(), 0);
    }
    #[test]
    fn selection_change_rejects_an_in_flight_plan_and_has_a_bounded_queue() {
        let root = root();
        let first = root.path().join("First");
        let second = root.path().join("Second");
        std::fs::create_dir(&first).unwrap();
        std::fs::create_dir(&second).unwrap();
        let ctx = egui::Context::default();
        let mut app = App::new(ctx.clone(), vec![first.clone()], None, true);
        app.dirty_since = None;
        let ico = render::ico(&render::render(&app.style, None)).unwrap();
        app.send(Work::Plan(
            app.plan_generation,
            vec![first],
            Action::Apply,
            Some(ico),
        ));
        app.queue_folders(vec![second]);
        settle(&mut app, &ctx);
        assert!(app.plans.is_empty());
        assert_eq!(app.paths.len(), 2);
        app.queue_folders(vec![root.path().join("unused"); selection::MAX_PENDING + 7]);
        assert_eq!(app.pending_paths.len(), selection::MAX_PENDING);
        assert_eq!(app.pending_overflow, 7);
    }
    #[test]
    fn symbol_catalog_loads_on_the_worker_then_stops_scheduling_work() {
        let ctx = egui::Context::default();
        let mut app = App::new(ctx.clone(), vec![], None, true);
        app.dirty_since = None;
        app.symbol_picker = true;
        assert!(app.next_work_deadline().is_some());
        app.logic(&ctx);
        settle(&mut app, &ctx);
        assert_eq!(app.symbols.len(), 17);
        assert!(app.symbols.iter().any(|(b, _)| *b == render::Badge::Code));
        assert!(app.next_work_deadline().is_none());
        app.logic(&ctx);
        assert!(!app.local_busy);
    }
}
