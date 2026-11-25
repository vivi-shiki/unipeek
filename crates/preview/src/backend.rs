//! The presentation paths under test. Each one shows frames from the same `Renderer`.
use anyhow::{Context, Result, bail};
use unipeek::{Scene, camera::Orbit, gpu::Gpu, renderer::Renderer, wgpu};
use std::{ffi::c_void, time::Instant};
use windows::{
    Win32::{Foundation::HWND, Graphics::DirectComposition::*},
    core::{IUnknown, Interface},
};

/// Device-pixel rectangle of the 3D viewport inside the window's client area.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

/// Zero-copy: a wgpu swap chain bound to a DirectComposition visual.
pub struct VisualSurface {
    pub gpu: Gpu,
    pub renderer: Renderer,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
}

impl VisualSurface {
    /// `visual` must be a live `IDCompositionVisual`. The caller commits its DComp device
    /// after every `resize`, because wgpu only calls `SetContent`.
    pub fn new(visual: *mut c_void, scene: &Scene) -> Result<Self> {
        // CompositionVisual targets only exist on the DX12 backend.
        let instance = Gpu::instance(wgpu::Backends::DX12);
        let surface = unsafe { instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CompositionVisual(visual)) }
            .context("create_surface_unsafe(CompositionVisual)")?;
        let gpu = Gpu::new(instance, Some(&surface))?;
        let caps = surface.get_capabilities(&gpu.adapter);
        let format = caps.formats.iter().copied().find(|f| f.is_srgb()).unwrap_or(caps.formats[0]);
        let present_mode = if caps.present_modes.contains(&wgpu::PresentMode::Mailbox) {
            wgpu::PresentMode::Mailbox
        } else {
            wgpu::PresentMode::Fifo
        };
        eprintln!(
            "[surface] adapter={} format={format:?} present={present_mode:?} alpha={:?}",
            gpu.adapter.get_info().name,
            caps.alpha_modes
        );
        let mut config = surface.get_default_config(&gpu.adapter, 1, 1).context("surface not supported by adapter")?;
        config.format = format;
        config.present_mode = present_mode;
        // Force the first `resize` to configure.
        (config.width, config.height) = (0, 0);
        let renderer = Renderer::new(&gpu.device, &gpu.queue, format, scene);
        Ok(Self { gpu, renderer, surface, config })
    }

    /// Returns true when the swap chain was (re)configured.
    pub fn resize(&mut self, w: u32, h: u32) -> bool {
        let (w, h) = (w.max(1), h.max(1));
        if (self.config.width, self.config.height) == (w, h) {
            return false;
        }
        self.config.width = w;
        self.config.height = h;
        self.surface.configure(&self.gpu.device, &self.config);
        true
    }

    pub fn draw(&mut self, camera: &Orbit) -> Result<()> {
        use wgpu::CurrentSurfaceTexture as T;
        let frame = match self.surface.get_current_texture() {
            T::Success(f) | T::Suboptimal(f) => f,
            T::Outdated | T::Lost => {
                self.surface.configure(&self.gpu.device, &self.config);
                match self.surface.get_current_texture() {
                    T::Success(f) | T::Suboptimal(f) => f,
                    _ => bail!("surface unavailable after reconfigure"),
                }
            }
            _ => return Ok(()),
        };
        let view = frame.texture.create_view(&Default::default());
        let mut enc = self.gpu.device.create_command_encoder(&Default::default());
        self.renderer.render(
            &self.gpu.device,
            &self.gpu.queue,
            &mut enc,
            &view,
            self.config.width,
            self.config.height,
            camera,
        );
        self.gpu.queue.submit([enc.finish()]);
        self.gpu.queue.present(frame);
        Ok(())
    }
}

/// Option D: our own DComp device with a topmost target on GPUI's HWND.
pub struct TopmostLayer {
    device: IDCompositionDevice,
    _target: IDCompositionTarget,
    visual: IDCompositionVisual,
    pub surface: VisualSurface,
    rect: Rect,
    visible: bool,
}

impl TopmostLayer {
    pub fn new(hwnd: isize, scene: &Scene) -> Result<Self> {
        unsafe {
            let device: IDCompositionDevice =
                DCompositionCreateDevice2(None::<&IUnknown>).context("DCompositionCreateDevice2")?;
            // GPUI already owns the topmost=false target of this window.
            let target = device
                .CreateTargetForHwnd(HWND(hwnd as *mut c_void), true)
                .context("CreateTargetForHwnd(topmost = true)")?;
            let visual = device.CreateVisual()?;
            target.SetRoot(&visual)?;
            let surface = VisualSurface::new(visual.as_raw(), scene)?;
            Ok(Self { device, _target: target, visual, surface, rect: Rect::default(), visible: true })
        }
    }

    /// Move/resize the layer. Returns how long the swap-chain reconfigure took, if one happened.
    pub fn place(&mut self, rect: Rect) -> Result<Option<f64>> {
        if rect == self.rect {
            return Ok(None);
        }
        let t = Instant::now();
        let resized = self.surface.resize(rect.w, rect.h);
        unsafe {
            self.visual.SetOffsetX2(rect.x as f32)?;
            self.visual.SetOffsetY2(rect.y as f32)?;
            self.device.Commit()?;
        }
        self.rect = rect;
        Ok(resized.then(|| t.elapsed().as_secs_f64() * 1e3))
    }

    pub fn set_visible(&mut self, visible: bool) -> Result<()> {
        if visible != self.visible {
            self.visible = visible;
            unsafe {
                let v3: IDCompositionVisual3 = self.visual.cast()?;
                v3.SetOpacity2(if visible { 1.0 } else { 0.0 })?;
                self.device.Commit()?;
            }
        }
        Ok(())
    }
}
