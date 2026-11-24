//! Device setup shared by every front end.

pub struct Gpu {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl Gpu {
    /// `surface`: the surface the device must be able to present to, if any.
    pub fn new(instance: wgpu::Instance, surface: Option<&wgpu::Surface<'_>>) -> crate::Result<Gpu> {
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: surface,
            ..Default::default()
        }))
        .map_err(|e| crate::Error::NoGpu(e.to_string()))?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).map_err(|e| crate::Error::Gpu(e.to_string()))?;
        Ok(Gpu { instance, adapter, device, queue })
    }

    /// A device for offscreen rendering (thumbnails, readback viewports), without any window.
    /// Tries DX12 first on Windows, then Vulkan and Metal, then anything wgpu offers.
    pub fn headless() -> crate::Result<Gpu> {
        let mut errors = Vec::new();
        for backends in [wgpu::Backends::DX12, wgpu::Backends::VULKAN | wgpu::Backends::METAL, wgpu::Backends::all()] {
            match Gpu::new(Gpu::instance(backends), None) {
                Ok(gpu) => return Ok(gpu),
                Err(e) => errors.push(format!("{backends:?}: {e}")),
            }
        }
        Err(crate::Error::NoGpu(errors.join("; ")))
    }

    pub fn instance(backends: wgpu::Backends) -> wgpu::Instance {
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.backends = backends;
        wgpu::Instance::new(desc)
    }

    /// Copy a single-sample 4-byte-per-pixel texture back to the CPU, tightly packed.
    pub fn read_texture(&self, texture: &wgpu::Texture, w: u32, h: u32) -> crate::Result<Vec<u8>> {
        let padded = (w * 4).div_ceil(256) * 256;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: (padded * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = self.device.create_command_encoder(&Default::default());
        enc.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo { buffer: &buffer, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(padded), rows_per_image: Some(h) } },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        self.queue.submit([enc.finish()]);
        buffer.map_async(wgpu::MapMode::Read, .., |_| {});
        self.device.poll(wgpu::PollType::wait_indefinitely()).map_err(|e| crate::Error::Gpu(e.to_string()))?;
        let row = w as usize * 4;
        let mut pixels = vec![0u8; row * h as usize];
        {
            let mapped = buffer.get_mapped_range(..).map_err(|e| crate::Error::Gpu(e.to_string()))?;
            for y in 0..h as usize {
                let src = y * padded as usize;
                pixels[y * row..(y + 1) * row].copy_from_slice(&mapped[src..src + row]);
            }
        }
        buffer.unmap();
        Ok(pixels)
    }
}
