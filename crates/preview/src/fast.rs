//! Option C: gpui-fast's window composition. GPUI owns the DirectComposition tree and
//! stacks our visual between its base scene and its deferred/overlay surface.
use crate::backend::{Rect, VisualSurface};
use anyhow::{Context, Result, anyhow};
use gpui_kit::{Bounds, DevicePixels, Window, WindowCompositionSurface, point, size};
use unipeek::Scene;
use std::time::Instant;
use windows::core::{IUnknown, Interface};

pub struct FastLayer {
    attachment: WindowCompositionSurface,
    pub surface: VisualSurface,
    rect: Rect,
}

impl FastLayer {
    pub fn new(window: &Window, scene: &Scene) -> Result<Self> {
        let t = Instant::now();
        let composition = window.enable_window_composition().context("enable_window_composition")?;
        let attachment = composition.create_native_surface().context("create_native_surface")?;
        let visual = attachment
            .platform_surface()?
            .platform_handle()?
            .downcast::<IUnknown>()
            .map_err(|_| anyhow!("platform handle is not an IUnknown (windows crate version mismatch?)"))?;
        let surface = VisualSurface::new(visual.as_raw(), scene)?;
        eprintln!("[fast] composition surface + renderer ready in {:?}", t.elapsed());
        Ok(Self { attachment, surface, rect: Rect::default() })
    }

    pub fn place(&mut self, rect: Rect) -> Result<Option<f64>> {
        if rect == self.rect {
            return Ok(None);
        }
        let t = Instant::now();
        // Configure first: wgpu only calls SetContent, and set_bounds commits the tree.
        let resized = self.surface.resize(rect.w, rect.h);
        self.attachment.platform_surface()?.set_bounds(Bounds {
            origin: point(DevicePixels(rect.x), DevicePixels(rect.y)),
            size: size(DevicePixels(rect.w as i32), DevicePixels(rect.h as i32)),
        })?;
        self.rect = rect;
        Ok(resized.then(|| t.elapsed().as_secs_f64() * 1e3))
    }
}
