//! Interactive viewport for a host UI: renders offscreen and hands back BGRA bytes, which a GPUI
//! host wraps in a `RenderImage`. The camera is an `Orbit` the host drives with mouse deltas.
//! Nothing here knows about windows or input events.
use crate::{
    Error, Result, Scene,
    camera::Orbit,
    gpu::Gpu,
    renderer::Renderer,
    scene::{LoadEvent, LoadOptions},
    thumb::Background,
};
use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
    mpsc,
};
use unipeek_unity::{Input, Report};

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;

/// A scene being loaded on a background thread: first untextured, then with textures.
pub struct Loading {
    rx: mpsc::Receiver<Loaded>,
    progress: Arc<AtomicU32>,
    done: bool,
}

/// One step of a `Loading`.
pub enum Loaded {
    /// Geometry with flat material colors; textures are still being decoded.
    Untextured(Box<Scene>),
    /// The finished scene.
    Textured(Box<Scene>),
    Failed(Error),
}

impl Loading {
    /// Start loading `input` on a new thread.
    pub fn start(input: Input, opts: LoadOptions) -> Loading {
        let (tx, rx) = mpsc::channel();
        let progress = Arc::new(AtomicU32::new(0));
        let p = progress.clone();
        std::thread::spawn(move || {
            let mut events = |e: LoadEvent<'_>| match e {
                LoadEvent::Progress(f) => p.store(f.to_bits(), Ordering::Relaxed),
                LoadEvent::Untextured(scene) => {
                    let _ = tx.send(Loaded::Untextured(Box::new(scene.clone())));
                }
            };
            let result = Scene::load_with(input, &opts, &mut events);
            p.store(1f32.to_bits(), Ordering::Relaxed);
            let _ = tx.send(match result {
                Ok(scene) => Loaded::Textured(Box::new(scene)),
                Err(e) => Loaded::Failed(e),
            });
        });
        Loading { rx, progress, done: false }
    }

    /// Fraction of the work done, 0 to 1.
    pub fn progress(&self) -> f32 {
        f32::from_bits(self.progress.load(Ordering::Relaxed))
    }

    /// The next step, if one arrived; never blocks.
    pub fn poll(&mut self) -> Option<Loaded> {
        let step = self.rx.try_recv().ok()?;
        self.done = matches!(step, Loaded::Textured(_) | Loaded::Failed(_));
        Some(step)
    }

    /// Wait for the next step.
    pub fn wait(&mut self) -> Option<Loaded> {
        let step = self.rx.recv().ok()?;
        self.done = matches!(step, Loaded::Textured(_) | Loaded::Failed(_));
        Some(step)
    }

    /// Whether the finished scene (or the failure) has been taken.
    pub fn is_done(&self) -> bool {
        self.done
    }
}

struct Target {
    w: u32,
    h: u32,
    padded: u32,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    buffer: wgpu::Buffer,
}

/// An offscreen viewport. Call `render` on every repaint; it only draws when the camera, the size,
/// the scene or the background changed, and returns `None` otherwise.
pub struct Viewport {
    device: wgpu::Device,
    queue: wgpu::Queue,
    width: u32,
    height: u32,
    renderer: Option<Renderer>,
    scene: Option<Scene>,
    orbit: Orbit,
    background: Background,
    drawn: Option<(Orbit, u32, u32, Background)>,
    target: Option<Target>,
    frame: Vec<u8>,
    loading: Option<Loading>,
    error: Option<Error>,
}

impl Viewport {
    pub fn offscreen(gpu: &Gpu, width: u32, height: u32) -> Viewport {
        Viewport {
            device: gpu.device.clone(),
            queue: gpu.queue.clone(),
            width: width.max(1),
            height: height.max(1),
            renderer: None,
            scene: None,
            orbit: Orbit::fit(glam::Vec3::splat(-1.0), glam::Vec3::splat(1.0)),
            background: Background::Color([28, 31, 38, 255]),
            drawn: None,
            target: None,
            frame: Vec::new(),
            loading: None,
            error: None,
        }
    }

    /// Size of the frame in physical pixels.
    pub fn resize(&mut self, width: u32, height: u32) {
        (self.width, self.height) = (width.max(1), height.max(1));
    }

    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Show `scene` and frame it from the front.
    pub fn set_scene(&mut self, scene: Scene) {
        self.orbit = Orbit::fit(scene.min, scene.max);
        self.replace_scene(scene);
    }

    /// Swap the scene without moving the camera (untextured to textured).
    fn replace_scene(&mut self, scene: Scene) {
        self.renderer = (!scene.is_empty()).then(|| Renderer::new(&self.device, &self.queue, FORMAT, &scene));
        self.scene = Some(scene);
        self.drawn = None;
    }

    /// Load `input` on a background thread. The untextured model appears first, then the textures.
    pub fn load(&mut self, input: Input, opts: LoadOptions) {
        self.loading = Some(Loading::start(input, opts));
        self.error = None;
    }

    /// Take finished loading steps. Returns true when the scene changed. Call it before `render`.
    pub fn update(&mut self) -> bool {
        let mut changed = false;
        while let Some(step) = self.loading.as_mut().and_then(Loading::poll) {
            match step {
                Loaded::Untextured(scene) => {
                    self.set_scene(*scene);
                }
                Loaded::Textured(scene) => {
                    if self.scene.is_none() {
                        self.set_scene(*scene);
                    } else {
                        self.replace_scene(*scene);
                    }
                }
                Loaded::Failed(e) => self.error = Some(e),
            }
            changed = true;
        }
        if self.loading.as_ref().is_some_and(Loading::is_done) {
            self.loading = None;
        }
        changed
    }

    /// Loading progress from 0 to 1, or `None` when nothing is loading.
    pub fn progress(&self) -> Option<f32> {
        self.loading.as_ref().map(Loading::progress)
    }

    /// Why the last `load` failed.
    pub fn error(&self) -> Option<&Error> {
        self.error.as_ref()
    }

    pub fn scene(&self) -> Option<&Scene> {
        self.scene.as_ref()
    }

    pub fn report(&self) -> Option<&Report> {
        self.scene.as_ref().map(|s| &s.report)
    }

    /// The camera. Changing it makes the next `render` draw.
    pub fn orbit(&mut self) -> &mut Orbit {
        &mut self.orbit
    }

    pub fn set_background(&mut self, background: Background) {
        self.background = background;
    }

    /// Draw again on the next `render` even if nothing changed.
    pub fn invalidate(&mut self) {
        self.drawn = None;
    }

    /// Render if anything changed and return the new frame: `width * height * 4` bytes, BGRA,
    /// sRGB, rows tightly packed, straight alpha. `None` when the last frame is still current or
    /// there is nothing to show.
    pub fn render(&mut self) -> Result<Option<&[u8]>> {
        let state = (self.orbit, self.width, self.height, self.background);
        if self.drawn == Some(state) || self.renderer.is_none() {
            return Ok(None);
        }
        self.draw()?;
        self.drawn = Some(state);
        Ok(Some(&self.frame))
    }

    /// The last rendered frame (empty before the first `render`).
    pub fn frame(&self) -> &[u8] {
        &self.frame
    }

    fn draw(&mut self) -> Result<()> {
        let (w, h) = (self.width, self.height);
        if !self.target.as_ref().is_some_and(|t| t.w == w && t.h == h) {
            let padded = (w * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("viewport"),
                size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let view = texture.create_view(&Default::default());
            let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("viewport readback"),
                size: (padded * h) as u64,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            self.target = Some(Target { w, h, padded, texture, view, buffer });
        }
        let t = self.target.as_ref().expect("created above");
        let renderer = self.renderer.as_mut().expect("checked by render");
        renderer.clear = crate::thumb::clear_color(self.background);
        let mut enc = self.device.create_command_encoder(&Default::default());
        renderer.render(&self.device, &self.queue, &mut enc, &t.view, w, h, &self.orbit);
        enc.copy_texture_to_buffer(
            t.texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo { buffer: &t.buffer, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(t.padded), rows_per_image: Some(h) } },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        self.queue.submit([enc.finish()]);
        t.buffer.map_async(wgpu::MapMode::Read, .., |_| {});
        self.device.poll(wgpu::PollType::wait_indefinitely()).map_err(|e| Error::Gpu(e.to_string()))?;
        let row = w as usize * 4;
        self.frame.resize(row * h as usize, 0);
        {
            let mapped = t.buffer.get_mapped_range(..).map_err(|e| Error::Gpu(e.to_string()))?;
            for y in 0..h as usize {
                let src = y * t.padded as usize;
                self.frame[y * row..(y + 1) * row].copy_from_slice(&mapped[src..src + row]);
            }
        }
        t.buffer.unmap();
        if self.background == Background::Transparent {
            crate::thumb::unpremultiply(&mut self.frame);
        }
        Ok(())
    }
}
