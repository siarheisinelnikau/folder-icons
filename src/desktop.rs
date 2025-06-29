//! Reactive, single-window egui host. CPU rasterization, Win32 GDI presentation.
use crate::App;
use anyhow::{Context, Result};
use egui_software_backend::{BufferMutRef, ColorFieldOrder, EguiSoftwareRender};
use std::{collections::VecDeque, num::NonZeroU32, path::PathBuf, rc::Rc, time::Instant};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    window::{Window, WindowId},
};

enum Event {
    Repaint(Instant, u64),
    Accessibility(egui_winit::accesskit_winit::Event),
}
impl From<egui_winit::accesskit_winit::Event> for Event {
    fn from(value: egui_winit::accesskit_winit::Event) -> Self {
        Self::Accessibility(value)
    }
}

struct Host {
    // Surface must be released before its window/context.
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    surface_context: Option<softbuffer::Context<Rc<Window>>>,
    window: Option<Rc<Window>>,
    input: Option<egui_winit::State>,
    context: egui::Context,
    app: App,
    proxy: EventLoopProxy<Event>,
    renderer: EguiSoftwareRender,
    pixels: Vec<u32>,
    size: (u32, u32),
    dirty: bool,
    deadline: Option<Instant>,
    frame_times: VecDeque<f64>,
    error: Option<String>,
    frames: u64,
    presents: u64,
    render_ns: u64,
    present_ns: u64,
    diagnostic: Option<Diagnostic>,
    repaint_causes: std::collections::BTreeMap<String, u64>,
    window_events: std::collections::BTreeMap<String, u64>,
    last_cursor: Option<winit::dpi::PhysicalPosition<f64>>,
    input_events: u64,
}

struct Diagnostic {
    output: PathBuf,
    phase: u8,
    next: Instant,
    baseline: (Instant, f64, u64, u64, u64, u64, u64),
    samples: Vec<serde_json::Value>,
    interfered: bool,
    expires: Instant,
}

fn process_cpu() -> f64 {
    #[cfg(windows)]
    {
        use windows_sys::Win32::{
            Foundation::FILETIME,
            System::Threading::{GetCurrentProcess, GetProcessTimes},
        };
        let zero = || FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        let (mut created, mut exited, mut kernel, mut user) = (zero(), zero(), zero(), zero());
        // SAFETY: GetCurrentProcess returns a pseudo handle; all output pointers are valid.
        if unsafe {
            GetProcessTimes(
                GetCurrentProcess(),
                &mut created,
                &mut exited,
                &mut kernel,
                &mut user,
            )
        } != 0
        {
            let ticks = |t: FILETIME| ((t.dwHighDateTime as u64) << 32) | t.dwLowDateTime as u64;
            return (ticks(kernel) + ticks(user)) as f64 / 10_000_000.;
        }
    }
    f64::NAN
}

pub fn diagnose(output: PathBuf, fixture: Option<PathBuf>) -> Result<()> {
    anyhow::ensure!(
        output.is_absolute(),
        "Diagnostic output path must be absolute"
    );
    run_internal(Vec::new(), None, Some((output, fixture)))
}

pub fn run(paths: Vec<PathBuf>, update_error: Option<PathBuf>) -> Result<()> {
    run_internal(paths, update_error, None)
}
fn run_internal(
    paths: Vec<PathBuf>,
    update_error: Option<PathBuf>,
    diagnostic: Option<(PathBuf, Option<PathBuf>)>,
) -> Result<()> {
    let event_loop = EventLoop::<Event>::with_user_event().build()?;
    let proxy = event_loop.create_proxy();
    let context = egui::Context::default();
    let wake = proxy.clone();
    context.set_request_repaint_callback(move |request| {
        if let Some(when) = Instant::now().checked_add(request.delay) {
            let _ = wake.send_event(Event::Repaint(when, request.current_cumulative_pass_nr));
        }
    });
    let app = if let Some((_, fixture)) = &diagnostic {
        crate::capture::fixture(
            context.clone(),
            fixture.as_deref(),
            if fixture.is_some() {
                "populated"
            } else {
                "empty"
            },
        )?
    } else {
        App::new(context.clone(), paths, update_error, false)
    };
    let mut host = Host {
        surface: None,
        surface_context: None,
        window: None,
        input: None,
        context,
        app,
        proxy,
        renderer: EguiSoftwareRender::new(ColorFieldOrder::Bgra),
        pixels: Vec::new(),
        size: (0, 0),
        dirty: true,
        deadline: None,
        frame_times: VecDeque::new(),
        error: None,
        frames: 0,
        presents: 0,
        render_ns: 0,
        present_ns: 0,
        diagnostic: diagnostic.map(|(output, _)| Diagnostic {
            output,
            phase: 0,
            next: Instant::now() + std::time::Duration::from_secs(1),
            baseline: (Instant::now(), process_cpu(), 0, 0, 0, 0, 0),
            samples: Vec::new(),
            interfered: false,
            expires: Instant::now() + std::time::Duration::from_secs(30),
        }),
        repaint_causes: std::collections::BTreeMap::new(),
        window_events: std::collections::BTreeMap::new(),
        last_cursor: None,
        input_events: 0,
    };
    event_loop.run_app(&mut host)?;
    if let Some(error) = host.error {
        anyhow::bail!(error);
    }
    Ok(())
}

impl Host {
    fn visible(&self) -> bool {
        self.window.as_ref().is_some_and(|w| {
            !w.is_minimized().unwrap_or(false)
                && w.is_visible().unwrap_or(false)
                && w.inner_size().width > 0
                && w.inner_size().height > 0
        })
    }
    fn schedule(&mut self, when: Instant) {
        self.deadline = Some(self.deadline.map_or(when, |old| old.min(when)));
    }
    fn wake(&mut self, el: &ActiveEventLoop) {
        self.app.logic(&self.context);
        if self.app.restarting {
            el.exit();
            return;
        }
        if self.visible() {
            self.dirty = true;
            if let Some(w) = &self.window {
                w.request_redraw();
            }
        }
    }
    fn paint(&mut self) -> Result<()> {
        if !self.visible() {
            return Ok(());
        }
        let window = self.window.as_ref().context("Missing window")?;
        let size = window.inner_size();
        let resized = self.size != (size.width, size.height);
        let surface = self.surface.as_mut().context("Missing surface")?;
        if resized {
            surface
                .resize(
                    NonZeroU32::new(size.width).context("Zero width")?,
                    NonZeroU32::new(size.height).context("Zero height")?,
                )
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            self.size = (size.width, size.height);
            self.pixels
                .resize(size.width as usize * size.height as usize, 0);
        }
        if self.dirty || resized {
            self.deadline = None;
            let started = Instant::now();
            let input = self.input.as_mut().context("Missing input")?;
            let mut raw = input.take_egui_input(window);
            if let Some(info) = raw.viewports.get_mut(&egui::ViewportId::ROOT) {
                egui_winit::update_viewport_info(info, &self.context, window, false);
            }
            let output = self.context.run(raw, |ctx| self.app.ui(ctx));
            if self.diagnostic.is_some() {
                for cause in self.context.repaint_causes() {
                    *self.repaint_causes.entry(format!("{cause}")).or_default() += 1;
                }
            }
            input.handle_platform_output(window, output.platform_output);
            let primitives = self
                .context
                .tessellate(output.shapes, output.pixels_per_point);
            self.pixels.fill(0x001b1b1b);
            self.renderer.render(
                &mut BufferMutRef::new(
                    bytemuck::cast_slice_mut(&mut self.pixels),
                    size.width as usize,
                    size.height as usize,
                ),
                &primitives,
                &output.textures_delta,
                output.pixels_per_point,
            );
            self.frames += 1;
            self.render_ns += started.elapsed().as_nanos() as u64;
            self.frame_times
                .push_back(started.elapsed().as_secs_f64() * 1000.);
            if self.frame_times.len() > 120 {
                self.frame_times.pop_front();
            }
            let mut sorted: Vec<_> = self.frame_times.iter().copied().collect();
            sorted.sort_by(f64::total_cmp);
            self.app.render_timing = format!(
                "UI + raster: median {:.1} ms · p95 {:.1} ms ({} samples; excludes presentation)",
                sorted[sorted.len() / 2],
                sorted[(sorted.len() - 1) * 95 / 100],
                sorted.len()
            );
            if let Some(viewport) = output.viewport_output.get(&egui::ViewportId::ROOT)
                && let Some(when) = Instant::now().checked_add(viewport.repaint_delay)
            {
                self.schedule(when);
            }
            self.dirty = false;
        }
        // An OS expose/paint can reuse the last bitmap without rebuilding the UI.
        let presented = Instant::now();
        let mut buffer = self
            .surface
            .as_mut()
            .context("Missing surface")?
            .buffer_mut()
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        buffer.copy_from_slice(&self.pixels);
        buffer
            .present()
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        self.presents += 1;
        self.present_ns += presented.elapsed().as_nanos() as u64;
        Ok(())
    }

    fn diagnostic_step(&mut self, el: &ActiveEventLoop) {
        let Some(diag) = &mut self.diagnostic else {
            return;
        };
        let now = Instant::now();
        if now < diag.next {
            return;
        }
        let cpu = process_cpu();
        let elapsed = now.duration_since(diag.baseline.0).as_secs_f64();
        let core_percent = (cpu - diag.baseline.1) * 100. / elapsed;
        let cpu_count = std::thread::available_parallelism().map_or(1, |n| n.get());
        let mut label = match diag.phase {
            0 => "startup",
            1 => "visible_idle",
            2 => "hidden_idle",
            3 => "restore",
            _ => "return_to_idle",
        };
        let input = self.input_events - diag.baseline.6;
        let interrupted = input > 0 && matches!(diag.phase, 1 | 4);
        let settling = !interrupted
            && matches!(diag.phase, 1 | 4)
            && (1..=8).contains(&(self.frames - diag.baseline.2))
            && core_percent <= 5.;
        if interrupted {
            diag.interfered = true;
            label = "interaction_during_idle_sample";
        } else if settling {
            label = "settling_after_invalidation";
        }
        diag.samples.push(serde_json::json!({"state":label,"seconds":elapsed,"input_events":input,"cpu_one_core_percent":core_percent,"cpu_normalized_percent":core_percent/cpu_count as f64,"frames":self.frames-diag.baseline.2,"presents":self.presents-diag.baseline.3,"ui_raster_ms":(self.render_ns-diag.baseline.4) as f64/1e6,"present_ms":(self.present_ns-diag.baseline.5) as f64/1e6}));
        let failed = !interrupted
            && !settling
            && matches!(diag.phase, 1 | 2 | 4)
            && (self.frames != diag.baseline.2
                || self.presents != diag.baseline.3
                || !core_percent.is_finite()
                || core_percent > 5.);
        if (diag.phase == 4 && !interrupted && !settling) || failed || now >= diag.expires {
            let report = serde_json::json!({"pid":std::process::id(),"exe":std::env::current_exe().ok(),"version":crate::VERSION,"backend":"egui_software_backend CPU + softbuffer Win32 GDI","logical_cpus_exposed":cpu_count,"gpu_telemetry":"unavailable; Windows composition is outside the app renderer","gate_passed":!failed&&!interrupted&&!settling&&diag.phase==4,"input_interference":diag.interfered,"samples":diag.samples,"repaint_causes":self.repaint_causes,"window_events":self.window_events});
            let result = (|| -> Result<()> {
                if let Some(parent) = diag.output.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                folder_icons::platform::atomic_write(
                    &diag.output,
                    &serde_json::to_vec_pretty(&report)?,
                )?;
                Ok(())
            })();
            if let Err(error) = result {
                self.error = Some(format!("Cannot save diagnostics: {error:#}"));
            }
            el.exit();
            diag.next = now + std::time::Duration::from_secs(60);
            return;
        }
        if interrupted || settling {
            diag.baseline = (
                now,
                cpu,
                self.frames,
                self.presents,
                self.render_ns,
                self.present_ns,
                self.input_events,
            );
            diag.next = now + std::time::Duration::from_secs(if diag.phase == 1 { 5 } else { 3 });
            return;
        }
        if diag.phase == 1
            && let Some(w) = &self.window
        {
            w.set_visible(false);
        }
        if diag.phase == 2 {
            if let Some(w) = &self.window {
                w.set_visible(true);
                w.request_redraw();
            }
            self.dirty = true;
        }
        diag.phase += 1;
        diag.baseline = (
            now,
            cpu,
            self.frames,
            self.presents,
            self.render_ns,
            self.present_ns,
            self.input_events,
        );
        diag.next = now
            + std::time::Duration::from_secs(match diag.phase {
                1 | 2 => 5,
                3 => 1,
                _ => 3,
            });
    }
}

impl ApplicationHandler<Event> for Host {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let result = (|| -> Result<()> {
            let artwork =
                folder_icons::render::render(&folder_icons::render::Style::default(), None);
            let icon = winit::window::Icon::from_rgba(artwork.into_raw(), 256, 256)?;
            let window = Rc::new(
                el.create_window(
                    Window::default_attributes()
                        .with_visible(false)
                        .with_active(self.diagnostic.is_none())
                        .with_title("FolderIcons")
                        .with_inner_size(winit::dpi::LogicalSize::new(
                            crate::ui::DEFAULT_SIZE[0],
                            crate::ui::DEFAULT_SIZE[1],
                        ))
                        .with_min_inner_size(winit::dpi::LogicalSize::new(
                            crate::ui::MIN_SIZE[0],
                            crate::ui::MIN_SIZE[1],
                        ))
                        .with_window_icon(Some(icon)),
                )?,
            );
            let surface_context = softbuffer::Context::new(window.clone())
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let surface = softbuffer::Surface::new(&surface_context, window.clone())
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let mut input = egui_winit::State::new(
                self.context.clone(),
                egui::ViewportId::ROOT,
                window.as_ref(),
                Some(window.scale_factor() as f32),
                window.theme(),
                None,
            );
            input.init_accesskit(el, &window, self.proxy.clone());
            self.surface = Some(surface);
            self.surface_context = Some(surface_context);
            self.input = Some(input);
            window.set_visible(true);
            self.window = Some(window);
            self.wake(el);
            Ok(())
        })();
        if let Err(error) = result {
            self.error = Some(format!("Cannot initialize CPU window: {error:#}"));
            el.exit();
        }
    }
    fn user_event(&mut self, el: &ActiveEventLoop, event: Event) {
        match event {
            Event::Repaint(when, pass) => {
                self.app.logic(&self.context);
                let current = self.context.cumulative_pass_nr();
                if current == pass || current == pass + 1 {
                    self.schedule(when);
                }
                if self.app.restarting {
                    el.exit();
                }
            }
            Event::Accessibility(event) => {
                use egui_winit::accesskit_winit::WindowEvent;
                match event.window_event {
                    WindowEvent::InitialTreeRequested => self.context.enable_accesskit(),
                    WindowEvent::ActionRequested(request) => {
                        if let Some(input) = &mut self.input {
                            input.on_accesskit_action_request(request);
                        }
                    }
                    WindowEvent::AccessibilityDeactivated => {}
                }
                self.wake(el);
            }
        }
    }
    fn window_event(&mut self, el: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let Some(window) = &self.window else {
            return;
        };
        if window.id() != id {
            return;
        }
        // Native Explorer drops feed the bounded file worker. No disk access or
        // folder mutation is performed by the window/input callback itself.
        match &event {
            WindowEvent::HoveredFile(_) => {
                if !self.app.drop_hover {
                    self.app.drop_hover = true;
                    self.wake(el);
                }
                return;
            }
            WindowEvent::HoveredFileCancelled => {
                if self.app.drop_hover {
                    self.app.drop_hover = false;
                    self.wake(el);
                }
                return;
            }
            WindowEvent::DroppedFile(path) => {
                self.app.queue_folders(vec![path.clone()]);
                self.wake(el);
                return;
            }
            _ => {}
        }
        match &event {
            WindowEvent::CursorMoved { position, .. } => {
                if self.last_cursor == Some(*position) {
                    if self.diagnostic.is_some() {
                        *self
                            .window_events
                            .entry("duplicate_pointer_move".into())
                            .or_default() += 1;
                    }
                    return;
                }
                self.last_cursor = Some(*position);
                self.input_events += 1;
            }
            WindowEvent::MouseInput { .. }
            | WindowEvent::MouseWheel { .. }
            | WindowEvent::KeyboardInput { .. } => {
                self.input_events += 1;
            }
            WindowEvent::CursorLeft { .. }
            | WindowEvent::CursorEntered { .. }
            | WindowEvent::Resized(_)
            | WindowEvent::ScaleFactorChanged { .. }
            | WindowEvent::Moved(_) => self.last_cursor = None,
            _ => {}
        }
        if self.diagnostic.is_some() {
            let label = match &event {
                WindowEvent::CursorMoved { .. } => "pointer_move",
                WindowEvent::MouseInput { .. } => "pointer_button",
                WindowEvent::MouseWheel { .. } => "scroll",
                WindowEvent::KeyboardInput { .. } => "keyboard",
                WindowEvent::RedrawRequested => "paint",
                WindowEvent::Focused(_) => "focus",
                WindowEvent::Resized(_) => "resize",
                _ => "other",
            };
            *self.window_events.entry(label.into()).or_default() += 1;
        }
        let response = self
            .input
            .as_mut()
            .expect("Input initialized with window")
            .on_window_event(window, &event);
        match event {
            WindowEvent::CloseRequested => {
                self.app.logic(&self.context);
                if self.app.local_busy || self.app.installing {
                    self.app.message =
                        "Wait for the current operation to finish before closing.".into();
                    self.wake(el);
                } else {
                    el.exit();
                }
            }
            WindowEvent::RedrawRequested => {
                if let Err(error) = self.paint() {
                    self.error = Some(format!("CPU rendering failed: {error:#}"));
                    el.exit();
                }
            }
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => self.wake(el),
            _ => {
                if response.repaint {
                    self.wake(el);
                }
            }
        }
    }
    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        self.diagnostic_step(el);
        if el.exiting() {
            return;
        }
        if self
            .deadline
            .is_some_and(|deadline| deadline <= Instant::now())
        {
            self.deadline = None;
            self.wake(el);
        }
        if self
            .app
            .next_work_deadline()
            .is_some_and(|deadline| deadline <= Instant::now())
        {
            self.wake(el);
        }
        // Never leave the loop polling when no repaint deadline remains.
        let next = match (self.deadline, self.diagnostic.as_ref().map(|d| d.next)) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        let next = match (next, self.app.next_work_deadline()) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        el.set_control_flow(next.map_or(ControlFlow::Wait, ControlFlow::WaitUntil));
    }
    fn exiting(&mut self, _el: &ActiveEventLoop) {
        self.context.set_request_repaint_callback(|_| {});
    }
}
