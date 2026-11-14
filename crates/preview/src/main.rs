//! Minimal test of ways to show a wgpu-rendered model inside a GPUI window on Windows.
//!
//!   preview <mode> <model.fbx> <texture dir> [WIDTHxHEIGHT]
//!
//! modes: `overlay`  - own topmost DirectComposition layer on upstream GPUI (zero-copy)
//!        `readback` - offscreen render, read back, show as a GPUI `RenderImage`
//!        `fast`     - gpui-fast composition surface (build with `--features fast`)
//!
//! env: LAB_SPIN=1 rotates continuously, LAB_SECS=n quits after n seconds.
mod backend;

use backend::{Readback, Rect, TopmostLayer};
use gpui_kit::{prelude::FluentBuilder, *};
use lab_render::{camera::Orbit, model::Model};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::{path::PathBuf, sync::Arc, time::Instant};

#[derive(Clone, Copy, PartialEq, Debug)]
enum Mode {
    Overlay,
    Readback,
    Fast,
}

#[derive(Default)]
struct Stats {
    frames: u32,
    draw_ms: f64,
    since: Option<Instant>,
    line: String,
}

struct Preview {
    mode: Mode,
    model: Model,
    textures: PathBuf,
    camera: Orbit,
    rect: Rect,
    dirty: bool,
    spin: bool,
    /// 0 = none, 1 = GPUI panel over the viewport, 2 = same, with the 3D layer hidden
    popup: u8,
    last_mouse: Option<Point<Pixels>>,
    layer: Option<TopmostLayer>,
    #[cfg(feature = "fast")]
    fast: Option<fast::FastLayer>,
    readback: Option<Readback>,
    image: Option<Arc<RenderImage>>,
    stats: Stats,
    started: Instant,
    quit_after: Option<f64>,
}

fn hwnd_of(window: &Window) -> anyhow::Result<isize> {
    match HasWindowHandle::window_handle(window)?.as_raw() {
        RawWindowHandle::Win32(h) => Ok(h.hwnd.get()),
        other => anyhow::bail!("not a Win32 window: {other:?}"),
    }
}

impl Preview {
    fn record(&mut self, draw_ms: f64) {
        let s = &mut self.stats;
        s.frames += 1;
        s.draw_ms += draw_ms;
        let since = *s.since.get_or_insert_with(Instant::now);
        let dt = since.elapsed().as_secs_f64();
        if dt >= 1.0 {
            s.line = format!(
                "{:?}  {}x{}  {:.0} fps  {:.2} ms/frame in backend",
                self.mode,
                self.rect.w,
                self.rect.h,
                s.frames as f64 / dt,
                s.draw_ms / s.frames.max(1) as f64
            );
            eprintln!("[stats] {}", s.line);
            *s = Stats { line: std::mem::take(&mut s.line), ..Default::default() };
        }
    }

    /// Called from the viewport canvas once layout is known (device pixels).
    fn present_zero_copy(&mut self, rect: Rect, window: &mut Window) -> anyhow::Result<()> {
        let moved = rect != self.rect;
        self.rect = rect;
        match self.mode {
            Mode::Overlay => {
                if self.layer.is_none() {
                    let t = Instant::now();
                    self.layer = Some(TopmostLayer::new(hwnd_of(window)?, &self.model, &self.textures)?);
                    eprintln!("[overlay] layer + renderer ready in {:?}", t.elapsed());
                    self.dirty = true;
                }
                let layer = self.layer.as_mut().unwrap();
                if let Some(ms) = layer.place(rect)? {
                    eprintln!("[overlay] resize to {}x{} took {ms:.2} ms", rect.w, rect.h);
                }
                layer.set_visible(self.popup != 2)?;
                if self.dirty || moved {
                    let t = Instant::now();
                    layer.surface.draw(&self.camera)?;
                    self.dirty = false;
                    self.record(t.elapsed().as_secs_f64() * 1e3);
                }
            }
            #[cfg(feature = "fast")]
            Mode::Fast => {
                if self.fast.is_none() {
                    self.fast = Some(fast::FastLayer::new(window, &self.model, &self.textures)?);
                    self.dirty = true;
                }
                let layer = self.fast.as_mut().unwrap();
                if let Some(ms) = layer.place(rect)? {
                    eprintln!("[fast] resize to {}x{} took {ms:.2} ms", rect.w, rect.h);
                }
                if self.dirty || moved {
                    let t = Instant::now();
                    layer.surface.draw(&self.camera)?;
                    self.dirty = false;
                    self.record(t.elapsed().as_secs_f64() * 1e3);
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Readback path: produce a fresh `RenderImage` for this frame when needed.
    fn refresh_image(&mut self, window: &mut Window) -> anyhow::Result<()> {
        if self.rect.w == 0 || !(self.dirty || self.image.is_none()) {
            return Ok(());
        }
        if self.readback.is_none() {
            self.readback = Some(Readback::new(&self.model, &self.textures)?);
        }
        let t = Instant::now();
        let (w, h) = (self.rect.w, self.rect.h);
        let pixels = self.readback.as_mut().unwrap().draw(w, h, &self.camera)?;
        let buffer = image::RgbaImage::from_raw(w, h, pixels).expect("size matches");
        let frames: smallvec::SmallVec<[image::Frame; 1]> = smallvec::smallvec![image::Frame::new(buffer)];
        let new = Arc::new(RenderImage::new(frames));
        // Every RenderImage gets its own atlas entry; release the previous one.
        if let Some(old) = self.image.replace(new) {
            window.drop_image(old).ok();
        }
        self.dirty = false;
        self.record(t.elapsed().as_secs_f64() * 1e3);
        Ok(())
    }
}

impl Render for Preview {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(limit) = self.quit_after
            && self.started.elapsed().as_secs_f64() > limit
        {
            cx.quit();
        }
        if self.spin {
            self.camera.yaw += 0.02;
            self.dirty = true;
            window.request_animation_frame();
        }
        if self.mode == Mode::Readback
            && let Err(e) = self.refresh_image(window)
        {
            eprintln!("readback failed: {e:#}");
        }

        let entity = cx.entity();
        let scale = window.scale_factor();
        let mode = self.mode;
        // The canvas only exists to learn the viewport's bounds after layout.
        let probe = canvas(
            move |bounds, window, cx| {
                let rect = Rect {
                    x: (bounds.origin.x / px(1.0) * scale).round() as i32,
                    y: (bounds.origin.y / px(1.0) * scale).round() as i32,
                    w: (bounds.size.width / px(1.0) * scale).round() as u32,
                    h: (bounds.size.height / px(1.0) * scale).round() as u32,
                };
                entity.update(cx, |this, cx| {
                    if mode == Mode::Readback {
                        if rect != this.rect {
                            this.rect = rect;
                            this.dirty = true;
                            cx.notify();
                        }
                    } else if let Err(e) = this.present_zero_copy(rect, window) {
                        eprintln!("present failed: {e:#}");
                    }
                });
            },
            |_, _, _, _| {},
        )
        .size_full();

        let viewport = div()
            .flex_1()
            .min_h_0()
            .relative()
            .bg(rgb(0x20242c))
            .when_some(self.image.clone().filter(|_| mode == Mode::Readback), |d, image| {
                d.child(img(ImageSource::Render(image)).absolute().top_0().left_0().size_full())
            })
            .child(probe)
            .on_mouse_down(MouseButton::Left, cx.listener(|this, e: &MouseDownEvent, _, _| this.last_mouse = Some(e.position)))
            .on_mouse_down(MouseButton::Middle, cx.listener(|this, e: &MouseDownEvent, _, _| this.last_mouse = Some(e.position)))
            .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _, cx| {
                let Some(button) = e.pressed_button else { return };
                let Some(last) = this.last_mouse.replace(e.position) else { return };
                let (dx, dy) = ((e.position.x - last.x) / px(1.0), (e.position.y - last.y) / px(1.0));
                match button {
                    MouseButton::Left => this.camera.rotate(dx, dy),
                    MouseButton::Middle => this.camera.pan(dx, dy),
                    _ => return,
                }
                this.dirty = true;
                cx.notify();
            }))
            .on_scroll(cx.listener(|this, e: &ScrollEvent, _, cx| {
                this.camera.zoom(e.delta.pixel_delta(px(40.0)).y / px(40.0));
                this.dirty = true;
                cx.notify();
            }));

        let button = |id: &'static str, label: String| {
            div().id(id).px_3().py_1().rounded_md().bg(rgb(0x3b4252)).text_color(rgb(0xeceff4)).cursor_pointer().child(label)
        };
        let toolbar = div()
            .h(px(40.0))
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .bg(rgb(0x2e3440))
            .text_color(rgb(0xd8dee9))
            .text_sm()
            .child(button("spin", format!("Spin: {}", if self.spin { "on" } else { "off" })).on_click(cx.listener(|this, _, _, cx| {
                this.spin = !this.spin;
                cx.notify();
            })))
            .child(button("popup", format!("Popup test: {}", ["off", "over 3D", "3D hidden"][self.popup as usize])).on_click(cx.listener(
                |this, _, _, cx| {
                    this.popup = (this.popup + 1) % 3;
                    cx.notify();
                },
            )))
            .child(button("reset", "Reset view".into()).on_click(cx.listener(|this, _, _, cx| {
                this.camera = Orbit::fit(this.model.min, this.model.max);
                this.dirty = true;
                cx.notify();
            })))
            .child(self.stats.line.clone());

        div()
            .size_full()
            .flex()
            .flex_col()
            .child(toolbar)
            .child(viewport)
            .when(self.popup != 0, |d| {
                d.child(deferred(
                    div()
                        .absolute()
                        .top(px(28.0))
                        .left(px(120.0))
                        .w(px(320.0))
                        .h(px(180.0))
                        .p_3()
                        .rounded_md()
                        .bg(rgb(0xbf616a))
                        .text_color(rgb(0xffffff))
                        .child("GPUI popup overlapping the 3D viewport"),
                ))
            })
    }
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    anyhow::ensure!(args.len() >= 4, "usage: preview <overlay|readback|fast> <model.fbx> <texture dir> [WxH]");
    let mode = match args[1].as_str() {
        "overlay" => Mode::Overlay,
        "readback" => Mode::Readback,
        "fast" if cfg!(feature = "fast") => Mode::Fast,
        "fast" => anyhow::bail!("rebuild with --features fast"),
        other => anyhow::bail!("unknown mode {other}"),
    };
    if mode == Mode::Overlay && cfg!(feature = "fast") {
        eprintln!("note: overlay mode is meant for upstream GPUI; this build uses gpui-fast");
    }
    let (w, h) = args
        .get(4)
        .and_then(|s| s.split_once('x'))
        .map(|(w, h)| (w.parse().unwrap(), h.parse().unwrap()))
        .unwrap_or((1100.0f32, 800.0f32));
    let t = Instant::now();
    let model = Model::load_fbx(&args[2])?;
    eprintln!("[model] {} triangles in {:?}", model.triangles(), t.elapsed());
    let textures = PathBuf::from(&args[3]);
    let spin = std::env::var("LAB_SPIN").is_ok();
    let quit_after = std::env::var("LAB_SECS").ok().and_then(|s| s.parse().ok());

    gpui_kit::application().run(move |cx| {
        gpui_kit::init(cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None, size(px(w), px(h)), cx))),
            ..Default::default()
        };
        gpui_kit::open_window(options, cx, |_, cx| {
            cx.new(|_| Preview {
                mode,
                camera: Orbit::fit(model.min, model.max),
                model,
                textures,
                rect: Rect::default(),
                dirty: true,
                spin,
                popup: std::env::var("LAB_POPUP").ok().and_then(|s| s.parse().ok()).unwrap_or(0),
                last_mouse: None,
                layer: None,
                #[cfg(feature = "fast")]
                fast: None,
                readback: None,
                image: None,
                stats: Stats::default(),
                started: Instant::now(),
                quit_after,
            })
        })
        .expect("failed to open window");
    });
    Ok(())
}

#[cfg(feature = "fast")]
mod fast;
