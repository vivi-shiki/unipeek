//! Headless: render the model to a PNG, and time render-only vs render+readback.
use unipeek::{camera::Orbit, gpu::Gpu, model::Model, renderer::Renderer, wgpu};
use std::{path::Path, time::Instant};

fn main() -> anyhow::Result<()> {
    let a: Vec<String> = std::env::args().collect();
    let (fbx, tex_dir, out) = (&a[1], &a[2], &a[3]);
    let w: u32 = a.get(4).map(|s| s.parse().unwrap()).unwrap_or(1024);
    let h: u32 = a.get(5).map(|s| s.parse().unwrap()).unwrap_or(1024);
    let yaw: f32 = a.get(6).map(|s| s.parse().unwrap()).unwrap_or(0.0);

    let t = Instant::now();
    let model = Model::load_fbx(fbx)?;
    println!("fbx: {} tris in {:?}", model.triangles(), t.elapsed());
    let gpu = Gpu::new(Gpu::instance(wgpu::Backends::DX12 | wgpu::Backends::VULKAN), None)?;
    println!("adapter: {} ({:?})", gpu.adapter.get_info().name, gpu.adapter.get_info().backend);
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let t = Instant::now();
    let mut renderer = Renderer::new(&gpu.device, &gpu.queue, format, &model, Path::new(tex_dir));
    println!("textures + pipelines: {:?}", t.elapsed());

    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    let padded = (w * 4).div_ceil(256) * 256;
    let buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: (padded * h) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut cam = Orbit::fit(model.min, model.max);
    cam.yaw = yaw;
    let mut pixels = vec![0u8; (w * h * 4) as usize];
    let wait = || gpu.device.poll(wgpu::PollType::wait_indefinitely()).map(|_| ());

    let n = 100;
    let mut render_only = 0.0;
    let mut with_readback = 0.0;
    for i in 0..n + 10 {
        cam.yaw = yaw + i as f32 * 0.01;
        let t = Instant::now();
        let mut enc = gpu.device.create_command_encoder(&Default::default());
        renderer.render(&gpu.device, &gpu.queue, &mut enc, &view, w, h, &cam);
        gpu.queue.submit([enc.finish()]);
        wait()?;
        if i >= 10 {
            render_only += t.elapsed().as_secs_f64() * 1e3;
        }
    }
    for i in 0..n + 10 {
        cam.yaw = yaw + (n - 1 - i.min(n - 1)) as f32 * 0.01 * 0.0;
        let t = Instant::now();
        let mut enc = gpu.device.create_command_encoder(&Default::default());
        renderer.render(&gpu.device, &gpu.queue, &mut enc, &view, w, h, &cam);
        enc.copy_texture_to_buffer(
            target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(padded), rows_per_image: Some(h) },
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        gpu.queue.submit([enc.finish()]);
        buf.map_async(wgpu::MapMode::Read, .., |_| {});
        wait()?;
        {
            let m = buf.get_mapped_range(..)?;
            for y in 0..h as usize {
                let (row, pr) = (w as usize * 4, padded as usize);
                pixels[y * row..(y + 1) * row].copy_from_slice(&m[y * pr..y * pr + row]);
            }
        }
        buf.unmap();
        if i >= 10 {
            with_readback += t.elapsed().as_secs_f64() * 1e3;
        }
    }
    println!(
        "{w}x{h}: render {:.3} ms/frame, render+readback {:.3} ms/frame (+{:.3})",
        render_only / n as f64,
        with_readback / n as f64,
        (with_readback - render_only) / n as f64
    );
    image::save_buffer(out, &pixels, w, h, image::ColorType::Rgba8)?;
    println!("wrote {out}");
    Ok(())
}
