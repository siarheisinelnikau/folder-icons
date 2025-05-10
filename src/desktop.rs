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
    Repaint(Instant),
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
}

pub fn run(paths: Vec<PathBuf>, update_error: Option<PathBuf>) -> Result<()> {
    let event_loop = EventLoop::<Event>::with_user_event().build()?;
    let proxy = event_loop.create_proxy();
    let context = egui::Context::default();
    let wake = proxy.clone();
    context.set_request_repaint_callback(move |request| {
        if let Some(when) = Instant::now().checked_add(request.delay) {
            let _ = wake.send_event(Event::Repaint(when));
        }
    });
    let app = App::new(context.clone(), paths, update_error);
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
            let started = Instant::now();
            let input = self.input.as_mut().context("Missing input")?;
            let raw = input.take_egui_input(window);
            let output = self.context.run(raw, |ctx| self.app.ui(ctx));
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
            self.frame_times
                .push_back(started.elapsed().as_secs_f64() * 1000.);
            if self.frame_times.len() > 120 {
                self.frame_times.pop_front();
            }
            let mut sorted: Vec<_> = self.frame_times.iter().copied().collect();
            sorted.sort_by(f64::total_cmp);
            self.app.render_timing = format!(
                "CPU frame: median {:.1} ms · p95 {:.1} ms ({} samples)",
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
        Ok(())
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
                        .with_title("FolderIcons")
                        .with_inner_size(winit::dpi::LogicalSize::new(1060., 740.))
                        .with_min_inner_size(winit::dpi::LogicalSize::new(820., 620.))
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
            Event::Repaint(when) => {
                self.app.logic(&self.context);
                self.schedule(when);
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
        if self
            .deadline
            .is_some_and(|deadline| deadline <= Instant::now())
        {
            self.deadline = None;
            self.wake(el);
        }
        // Never leave the loop polling when no repaint deadline remains.
        el.set_control_flow(
            self.deadline
                .map_or(ControlFlow::Wait, ControlFlow::WaitUntil),
        );
    }
    fn exiting(&mut self, _el: &ActiveEventLoop) {
        self.context.set_request_repaint_callback(|_| {});
    }
}
