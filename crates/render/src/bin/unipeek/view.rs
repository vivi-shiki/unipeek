//! `unipeek view`: one package in a window of its own, for command line users and as a fallback.
//! Left drag turns, right or middle drag pans, the wheel zooms; R resets, 1-4 pick front, side,
//! back and 3/4 views, Esc closes. `UNIPEEK_VIEW_SECS=n` closes the window after n seconds.
use std::{sync::Arc, time::Instant};
use unipeek::{
    LoadOptions, Loaded, Loading, Scene,
    camera::Orbit,
    gpu::Gpu,
    renderer::Renderer,
    thumb::View,
    unity::Input,
    wgpu,
};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};

struct Shown {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    gpu: Gpu,
    renderer: Option<Renderer>,
}

struct App {
    location: String,
    loading: Loading,
    scene: Option<Scene>,
    orbit: Orbit,
    shown: Option<Shown>,
    drag: Option<(MouseButton, f64, f64)>,
    cursor: (f64, f64),
    started: Instant,
    quit_after: Option<f64>,
    error: Option<String>,
    status: i32,
    frames: u32,
}

pub fn run(location: &str) -> Result<i32, String> {
    let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App {
        location: location.to_string(),
        loading: Loading::start(Input::from_location(location), LoadOptions::default()),
        scene: None,
        orbit: Orbit::fit(unipeek::glam::Vec3::splat(-1.0), unipeek::glam::Vec3::splat(1.0)),
        shown: None,
        drag: None,
        cursor: (0.0, 0.0),
        started: Instant::now(),
        quit_after: std::env::var("UNIPEEK_VIEW_SECS").ok().and_then(|s| s.parse().ok()),
        error: None,
        status: 0,
        frames: 0,
    };
    event_loop.run_app(&mut app).map_err(|e| e.to_string())?;
    if app.quit_after.is_some() {
        let tris = app.scene.as_ref().map_or(0, Scene::triangles);
        eprintln!("view: {} frame(s), {tris} triangles, textures loaded: {}", app.frames, app.loading.is_done());
    }
    match app.error {
        Some(e) => Err(e),
        None => Ok(app.status),
    }
}

impl App {
    fn title(&self) -> String {
        let name = self.location.rsplit(['/', '\\']).next().unwrap_or(&self.location);
        match (&self.scene, self.loading.is_done()) {
            (None, _) => format!("{name} - loading {:.0}% - unipeek", self.loading.progress() * 100.0),
            (Some(s), false) => format!("{name} - {} triangles, loading textures - unipeek", s.triangles()),
            (Some(s), true) => {
                let prefab = s.report.prefab.as_deref().and_then(|p| p.rsplit('/').next()).unwrap_or("model");
                format!("{name} - {prefab} - {} triangles, {:?} - unipeek", s.triangles(), s.report.status)
            }
        }
    }

    fn take_loaded(&mut self, event_loop: &ActiveEventLoop) {
        let mut changed = false;
        while let Some(step) = self.loading.poll() {
            let (scene, keep_camera) = match step {
                Loaded::Untextured(s) => (*s, false),
                Loaded::Textured(s) => {
                    let keep = self.scene.is_some();
                    (*s, keep)
                }
                Loaded::Failed(e) => {
                    self.status = e.status().exit_code();
                    self.error = Some(e.to_string());
                    event_loop.exit();
                    return;
                }
            };
            if scene.is_empty() {
                self.status = unipeek::unity::Status::Unsupported.exit_code();
                self.error = Some(format!("nothing to show: {:?}", scene.report.degrades));
                event_loop.exit();
                return;
            }
            if !keep_camera {
                self.orbit = Orbit::fit(scene.min, scene.max);
                self.orbit.preset(View::ThreeQuarter);
            }
            if self.loading.is_done() {
                self.status = scene.report.status.exit_code().min(1);
                for d in &scene.report.degrades {
                    eprintln!("degraded: {}", serde_json::to_string(d).unwrap_or_default());
                }
            }
            if let Some(shown) = &mut self.shown {
                let mut r = Renderer::new(&shown.gpu.device, &shown.gpu.queue, shown.config.format, &scene);
                r.clear = background();
                shown.renderer = Some(r);
            }
            self.scene = Some(scene);
            changed = true;
        }
        if let Some(shown) = &self.shown {
            shown.window.set_title(&self.title());
            if changed {
                shown.window.request_redraw();
            }
        }
    }

    fn draw(&mut self) {
        let Some(shown) = &mut self.shown else { return };
        let Some(renderer) = &mut shown.renderer else { return };
        use wgpu::CurrentSurfaceTexture as T;
        let frame = match shown.surface.get_current_texture() {
            T::Success(f) | T::Suboptimal(f) => f,
            T::Outdated | T::Lost => {
                shown.surface.configure(&shown.gpu.device, &shown.config);
                return shown.window.request_redraw();
            }
            _ => return,
        };
        let view = frame.texture.create_view(&Default::default());
        let mut enc = shown.gpu.device.create_command_encoder(&Default::default());
        renderer.render(&shown.gpu.device, &shown.gpu.queue, &mut enc, &view, shown.config.width, shown.config.height, &self.orbit);
        shown.gpu.queue.submit([enc.finish()]);
        shown.window.pre_present_notify();
        shown.gpu.queue.present(frame);
        self.frames += 1;
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.shown.is_some() {
            return;
        }
        let attrs = Window::default_attributes().with_title(self.title()).with_inner_size(LogicalSize::new(900.0, 900.0));
        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                self.error = Some(format!("cannot open a window: {e}"));
                return event_loop.exit();
            }
        };
        let instance = Gpu::instance(wgpu::Backends::all());
        let surface = match instance.create_surface(window.clone()) {
            Ok(s) => s,
            Err(e) => {
                self.error = Some(format!("cannot draw into the window: {e}"));
                return event_loop.exit();
            }
        };
        let gpu = match Gpu::new(instance, Some(&surface)) {
            Ok(g) => g,
            Err(e) => {
                self.status = e.status().exit_code();
                self.error = Some(e.to_string());
                return event_loop.exit();
            }
        };
        let size = window.inner_size();
        let Some(mut config) = surface.get_default_config(&gpu.adapter, size.width.max(1), size.height.max(1)) else {
            self.error = Some("the window's surface is not supported by the GPU".into());
            return event_loop.exit();
        };
        let caps = surface.get_capabilities(&gpu.adapter);
        config.format = caps.formats.iter().copied().find(|f| f.is_srgb()).unwrap_or(config.format);
        surface.configure(&gpu.device, &config);
        let mut renderer = self.scene.as_ref().map(|s| Renderer::new(&gpu.device, &gpu.queue, config.format, s));
        if let Some(r) = &mut renderer {
            r.clear = background();
        }
        self.shown = Some(Shown { window, surface, config, gpu, renderer });
        // Poll the loader while it works; afterwards only input wakes the loop.
        event_loop.set_control_flow(ControlFlow::Poll);
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.quit_after.is_some_and(|s| self.started.elapsed().as_secs_f64() > s) {
            return event_loop.exit();
        }
        if !self.loading.is_done() || self.scene.is_none() {
            self.take_loaded(event_loop);
            std::thread::sleep(std::time::Duration::from_millis(10));
        } else if self.quit_after.is_none() {
            event_loop.set_control_flow(ControlFlow::Wait);
        } else {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let mut redraw = false;
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(shown) = &mut self.shown {
                    (shown.config.width, shown.config.height) = (size.width.max(1), size.height.max(1));
                    shown.surface.configure(&shown.gpu.device, &shown.config);
                    redraw = true;
                }
            }
            WindowEvent::RedrawRequested => self.draw(),
            WindowEvent::MouseInput { state, button, .. } => {
                self.drag = (state == ElementState::Pressed).then_some((button, self.cursor.0, self.cursor.1));
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = (position.x, position.y);
                if let Some((button, x, y)) = self.drag {
                    let (dx, dy) = ((position.x - x) as f32, (position.y - y) as f32);
                    match button {
                        MouseButton::Left => self.orbit.rotate(dx, dy),
                        _ => self.orbit.pan(dx, dy),
                    }
                    self.drag = Some((button, position.x, position.y));
                    redraw = true;
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let steps = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
                };
                self.orbit.zoom(steps);
                redraw = true;
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                match event.logical_key.as_ref() {
                    Key::Named(NamedKey::Escape) => event_loop.exit(),
                    Key::Character("r") | Key::Character("R") => self.orbit.reset(),
                    Key::Character("1") => self.orbit.preset(View::Front),
                    Key::Character("2") => self.orbit.preset(View::Side),
                    Key::Character("3") => self.orbit.preset(View::Back),
                    Key::Character("4") => self.orbit.preset(View::ThreeQuarter),
                    _ => {}
                }
                redraw = true;
            }
            _ => {}
        }
        if redraw && let Some(shown) = &self.shown {
            shown.window.request_redraw();
        }
    }
}

fn background() -> wgpu::Color {
    wgpu::Color { r: 0.012, g: 0.014, b: 0.019, a: 1.0 }
}
