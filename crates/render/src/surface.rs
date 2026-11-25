//! Zero-copy viewport on Windows: the renderer presents straight into a DirectComposition visual
//! or a child window the host made, instead of reading frames back. Creation runs a self-test
//! (configure, draw, present one frame); when anything fails, `ViewportKind::open` falls back to
//! the readback `Viewport` and says why. Only raw handles cross this boundary, no window types.
use crate::{Error, Result, Scene, camera::Orbit, gpu::Gpu, renderer::Renderer, thumb::Background, viewport::Viewport};
use std::{ffi::c_void, num::NonZeroIsize};

/// Where a zero-copy viewport presents.
#[derive(Clone, Copy, Debug)]
pub enum Target {
    /// An `IDCompositionVisual` the host placed in its visual tree; the host commits its
    /// DirectComposition device after `resize`. DX12 only.
    CompositionVisual(*mut c_void),
    /// A child `HWND` the host created inside its window and keeps alive.
    Hwnd(isize),
}

/// A viewport that presents to a `Target` without copying frames to the CPU.
pub struct SurfaceViewport {
    gpu: Gpu,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    renderer: Option<Renderer>,
    orbit: Orbit,
    background: Background,
    drawn: Option<(Orbit, u32, u32, Background)>,
}

impl SurfaceViewport {
    /// Create the surface and present one cleared frame to prove the path works.
    ///
    /// # Safety
    /// The visual or window behind `target` must stay valid until this viewport is dropped.
    pub unsafe fn new(target: Target, width: u32, height: u32) -> Result<SurfaceViewport> {
        let (backends, raw) = match target {
            Target::CompositionVisual(v) if !v.is_null() => (wgpu::Backends::DX12, wgpu::SurfaceTargetUnsafe::CompositionVisual(v)),
            Target::Hwnd(h) => {
                let hwnd = NonZeroIsize::new(h).ok_or_else(|| Error::Gpu("null window handle".into()))?;
                let handle = wgpu::rwh::Win32WindowHandle::new(hwnd);
                (wgpu::Backends::DX12 | wgpu::Backends::VULKAN, wgpu::SurfaceTargetUnsafe::RawHandle { raw_display_handle: Some(wgpu::rwh::RawDisplayHandle::Windows(wgpu::rwh::WindowsDisplayHandle::new())), raw_window_handle: wgpu::rwh::RawWindowHandle::Win32(handle) })
            }
            Target::CompositionVisual(_) => return Err(Error::Gpu("null composition visual".into())),
        };
        let instance = Gpu::instance(backends);
        // SAFETY: the caller keeps the visual or window alive for the lifetime of the surface.
        let surface = unsafe { instance.create_surface_unsafe(raw) }.map_err(|e| Error::Gpu(format!("surface: {e}")))?;
        let gpu = Gpu::new(instance, Some(&surface))?;
        let caps = surface.get_capabilities(&gpu.adapter);
        let format = caps.formats.iter().copied().find(|f| f.is_srgb()).or(caps.formats.first().copied()).ok_or_else(|| Error::Gpu("the surface offers no format".into()))?;
        let mut config = surface.get_default_config(&gpu.adapter, width.max(1), height.max(1)).ok_or_else(|| Error::Gpu("the adapter cannot present to this surface".into()))?;
        config.format = format;
        if caps.present_modes.contains(&wgpu::PresentMode::Mailbox) {
            config.present_mode = wgpu::PresentMode::Mailbox;
        }
        if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::PreMultiplied) {
            config.alpha_mode = wgpu::CompositeAlphaMode::PreMultiplied;
        }
        surface.configure(&gpu.device, &config);
        let mut view = SurfaceViewport { gpu, surface, config, renderer: None, orbit: Orbit::fit(glam::Vec3::splat(-1.0), glam::Vec3::splat(1.0)), background: Background::Color([28, 31, 38, 255]), drawn: None };
        view.self_test()?;
        Ok(view)
    }

    /// Present one cleared frame and make sure the device survived it.
    fn self_test(&mut self) -> Result<()> {
        let frame = self.acquire()?;
        let view = frame.texture.create_view(&Default::default());
        let mut enc = self.gpu.device.create_command_encoder(&Default::default());
        enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("self test"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(crate::thumb::clear_color(self.background)), store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        self.gpu.queue.submit([enc.finish()]);
        self.gpu.queue.present(frame);
        self.gpu.device.poll(wgpu::PollType::wait_indefinitely()).map_err(|e| Error::Gpu(format!("self test: {e}")))?;
        Ok(())
    }

    fn acquire(&mut self) -> Result<wgpu::SurfaceTexture> {
        use wgpu::CurrentSurfaceTexture as T;
        match self.surface.get_current_texture() {
            T::Success(f) | T::Suboptimal(f) => Ok(f),
            T::Outdated | T::Lost => {
                self.surface.configure(&self.gpu.device, &self.config);
                match self.surface.get_current_texture() {
                    T::Success(f) | T::Suboptimal(f) => Ok(f),
                    _ => Err(Error::Gpu("the surface is unavailable".into())),
                }
            }
            _ => Err(Error::Gpu("no surface frame".into())),
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        let (w, h) = (width.max(1), height.max(1));
        if (self.config.width, self.config.height) != (w, h) {
            (self.config.width, self.config.height) = (w, h);
            self.surface.configure(&self.gpu.device, &self.config);
        }
    }

    pub fn set_scene(&mut self, scene: &Scene) {
        self.orbit = Orbit::fit(scene.min, scene.max);
        self.renderer = (!scene.is_empty()).then(|| Renderer::new(&self.gpu.device, &self.gpu.queue, self.config.format, scene));
        self.drawn = None;
    }

    pub fn orbit(&mut self) -> &mut Orbit {
        &mut self.orbit
    }

    pub fn set_background(&mut self, background: Background) {
        self.background = background;
    }

    pub fn describe(&self) -> String {
        self.gpu.describe()
    }

    /// Draw and present when anything changed. Returns whether a frame was presented.
    pub fn render(&mut self) -> Result<bool> {
        let state = (self.orbit, self.config.width, self.config.height, self.background);
        if self.drawn == Some(state) || self.renderer.is_none() {
            return Ok(false);
        }
        let frame = self.acquire()?;
        let view = frame.texture.create_view(&Default::default());
        let renderer = self.renderer.as_mut().expect("checked above");
        renderer.clear = crate::thumb::clear_color(self.background);
        let mut enc = self.gpu.device.create_command_encoder(&Default::default());
        renderer.render(&self.gpu.device, &self.gpu.queue, &mut enc, &view, self.config.width, self.config.height, &self.orbit);
        self.gpu.queue.submit([enc.finish()]);
        self.gpu.queue.present(frame);
        self.drawn = Some(state);
        Ok(true)
    }
}

/// A zero-copy viewport when the target works, otherwise the readback one.
pub enum ViewportKind {
    ZeroCopy(SurfaceViewport),
    /// The readback viewport and why zero-copy was not used.
    Readback(Viewport, String),
}

impl ViewportKind {
    /// Try zero-copy presentation to `target`; on any failure use a readback `Viewport` on `gpu`.
    ///
    /// # Safety
    /// As for `SurfaceViewport::new`.
    pub unsafe fn open(target: Target, gpu: &Gpu, width: u32, height: u32) -> ViewportKind {
        if std::env::var_os("UNIPEEK_NO_ZERO_COPY").is_some() {
            return ViewportKind::Readback(Viewport::offscreen(gpu, width, height), "UNIPEEK_NO_ZERO_COPY is set".into());
        }
        // SAFETY: forwarded from the caller.
        match unsafe { SurfaceViewport::new(target, width, height) } {
            Ok(v) => ViewportKind::ZeroCopy(v),
            Err(e) => ViewportKind::Readback(Viewport::offscreen(gpu, width, height), e.to_string()),
        }
    }
}
