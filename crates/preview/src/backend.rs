//! The presentation paths under test. Each one shows frames from the same `Renderer`.
use anyhow::{Context, Result, bail};
use unipeek::{camera::Orbit, gpu::Gpu, model::Model, renderer::Renderer, wgpu};
use std::{ffi::c_void, path::Path, time::Instant};
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
    pub fn new(visual: *mut c_void, model: &Model, textures: &Path) -> Result<Self> {
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
        let renderer = Renderer::new(&gpu.device, &gpu.queue, format, model, textures);
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
    pub fn new(hwnd: isize, model: &Model, textures: &Path) -> Result<Self> {
        unsafe {
            let device: IDCompositionDevice =
                DCompositionCreateDevice2(None::<&IUnknown>).context("DCompositionCreateDevice2")?;
            // GPUI already owns the topmost=false target of this window.
            let target = device
                .CreateTargetForHwnd(HWND(hwnd as *mut c_void), true)
                .context("CreateTargetForHwnd(topmost = true)")?;
            let visual = device.CreateVisual()?;
            target.SetRoot(&visual)?;
            let surface = VisualSurface::new(visual.as_raw(), model, textures)?;
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

/// Option A: offscreen render, read back, hand BGRA bytes to GPUI.
pub struct Readback {
    pub gpu: Gpu,
    pub renderer: Renderer,
    target: Option<(u32, u32, wgpu::Texture, wgpu::TextureView, wgpu::Buffer)>,
}

impl Readback {
    pub fn new(model: &Model, textures: &Path) -> Result<Self> {
        let gpu = Gpu::new(Gpu::instance(wgpu::Backends::DX12), None)?;
        eprintln!("[readback] adapter={}", gpu.adapter.get_info().name);
        // GPUI's RenderImage is BGRA.
        let renderer = Renderer::new(&gpu.device, &gpu.queue, wgpu::TextureFormat::Bgra8UnormSrgb, model, textures);
        Ok(Self { gpu, renderer, target: None })
    }

    /// Blocking render + readback. Returns tightly packed BGRA pixels.
    pub fn draw(&mut self, w: u32, h: u32, camera: &Orbit) -> Result<Vec<u8>> {
        let (w, h) = (w.max(1), h.max(1));
        let padded = (w * 4).div_ceil(256) * 256;
        if !matches!(&self.target, Some((tw, th, ..)) if (*tw, *th) == (w, h)) {
            let texture = self.gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("readback target"),
                size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Bgra8UnormSrgb,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let view = texture.create_view(&Default::default());
            let buffer = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("readback staging"),
                size: (padded * h) as u64,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            self.target = Some((w, h, texture, view, buffer));
        }
        let (_, _, texture, view, buffer) = self.target.as_ref().unwrap();
        let mut enc = self.gpu.device.create_command_encoder(&Default::default());
        self.renderer.render(&self.gpu.device, &self.gpu.queue, &mut enc, view, w, h, camera);
        enc.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(padded), rows_per_image: Some(h) },
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        self.gpu.queue.submit([enc.finish()]);
        buffer.map_async(wgpu::MapMode::Read, .., |_| {});
        self.gpu.device.poll(wgpu::PollType::wait_indefinitely())?;
        let row = w as usize * 4;
        let mut pixels = vec![0u8; row * h as usize];
        {
            let mapped = buffer.get_mapped_range(..)?;
            for y in 0..h as usize {
                let src = y * padded as usize;
                pixels[y * row..(y + 1) * row].copy_from_slice(&mapped[src..src + row]);
            }
        }
        buffer.unmap();
        Ok(pixels)
    }
}
