//! Forward renderer for a `Scene`: opaque and cutout parts, then inverted-hull outlines, then
//! alpha-blended parts. One pipeline per (pass, cull mode); MSAA 4x.
use crate::{
    camera::Orbit,
    model::Vertex,
    scene::Scene,
};
use bytemuck::{Pod, Zeroable};
use std::collections::BTreeMap;
use unipeek_unity::{
    material::{Cull, Look},
    shader::RenderMode,
};
use wgpu::util::DeviceExt;

const SAMPLES: u32 = 4;
const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    eye: [f32; 4],
    light: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct MaterialUniform {
    color: [f32; 4],
    shadow: [f32; 4],
    /// cutoff, mode (0 opaque, 1 cutout, 2 transparent), outline width, unused
    params: [f32; 4],
    outline: [f32; 4],
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Pass {
    Solid,
    Outline,
    Blend,
}

struct Draw {
    first: u32,
    count: u32,
    bind: usize,
    pass: Pass,
    cull: Option<wgpu::Face>,
}

pub struct Renderer {
    pipelines: BTreeMap<(Pass, u8), wgpu::RenderPipeline>,
    globals: wgpu::Buffer,
    globals_bind: wgpu::BindGroup,
    vbuf: wgpu::Buffer,
    binds: Vec<wgpu::BindGroup>,
    draws: Vec<Draw>,
    format: wgpu::TextureFormat,
    targets: Option<(u32, u32, wgpu::TextureView, wgpu::TextureView)>,
    pub clear: wgpu::Color,
}

fn cull_key(c: Option<wgpu::Face>) -> u8 {
    match c {
        None => 0,
        Some(wgpu::Face::Back) => 1,
        Some(wgpu::Face::Front) => 2,
    }
}

fn face(c: Cull) -> Option<wgpu::Face> {
    match c {
        Cull::Off => None,
        Cull::Back => Some(wgpu::Face::Back),
        Cull::Front => Some(wgpu::Face::Front),
    }
}

impl Renderer {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat, scene: &Scene) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniform_entry = |binding, visibility| wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
            count: None,
        };
        let g_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: None, entries: &[uniform_entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT)] });
        let m_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                uniform_entry(2, wgpu::ShaderStages::VERTEX_FRAGMENT),
            ],
        });
        let globals_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &g_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&g_layout), Some(&m_layout)],
            immediate_size: 0,
        });

        let vbuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("vertices"),
            contents: if scene.vertices.is_empty() { &[0u8; size_of::<Vertex>()] } else { bytemuck::cast_slice(&scene.vertices) },
            usage: wgpu::BufferUsages::VERTEX,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            anisotropy_clamp: 8,
            ..Default::default()
        });
        let white = vec![image::RgbaImage::from_pixel(4, 4, image::Rgba([255, 255, 255, 255]))];
        let binds: Vec<wgpu::BindGroup> = scene
            .materials
            .iter()
            .map(|m| {
                let mips = m.texture.as_deref().unwrap_or(&white);
                let view = upload(device, queue, &m.name, mips);
                let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("material"),
                    contents: bytemuck::bytes_of(&material_uniform(&m.look)),
                    usage: wgpu::BufferUsages::UNIFORM,
                });
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: None,
                    layout: &m_layout,
                    entries: &[
                        wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                        wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&sampler) },
                        wgpu::BindGroupEntry { binding: 2, resource: uniform.as_entire_binding() },
                    ],
                })
            })
            .collect();

        let mut draws = Vec::new();
        for part in &scene.parts {
            let look = &scene.materials[part.material].look;
            if look.draw_surface {
                let pass = if look.mode == RenderMode::Transparent { Pass::Blend } else { Pass::Solid };
                draws.push(Draw { first: part.first, count: part.count, bind: part.material, pass, cull: face(look.cull) });
            }
            if look.outline.is_some() {
                // Inverted hull: draw the extruded back faces only.
                draws.push(Draw { first: part.first, count: part.count, bind: part.material, pass: Pass::Outline, cull: Some(wgpu::Face::Front) });
            }
        }
        draws.sort_by_key(|d| d.pass);

        let mut pipelines = BTreeMap::new();
        for d in &draws {
            let key = (d.pass, cull_key(d.cull));
            if pipelines.contains_key(&key) {
                continue;
            }
            let blend = d.pass == Pass::Blend;
            let (vs, fs) = if d.pass == Pass::Outline { ("vs_outline", "fs_outline") } else { ("vs", "fs") };
            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some(vs),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: size_of::<Vertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2],
                    })],
                },
                primitive: wgpu::PrimitiveState { cull_mode: d.cull, ..Default::default() },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH,
                    depth_write_enabled: Some(!blend),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState { count: SAMPLES, mask: !0, alpha_to_coverage_enabled: false },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(if blend { wgpu::BlendState::ALPHA_BLENDING } else { wgpu::BlendState::REPLACE }),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            });
            pipelines.insert(key, pipeline);
        }

        Renderer {
            pipelines,
            globals,
            globals_bind,
            vbuf,
            binds,
            draws,
            format,
            targets: None,
            clear: wgpu::Color { r: 0.11, g: 0.12, b: 0.15, a: 1.0 },
        }
    }

    fn ensure_targets(&mut self, device: &wgpu::Device, w: u32, h: u32) {
        if matches!(&self.targets, Some((tw, th, ..)) if *tw == w && *th == h) {
            return;
        }
        let make = |format, label| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: SAMPLES,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        self.targets = Some((w, h, make(self.format, "msaa"), make(DEPTH, "depth")));
    }

    /// Render one frame into `target` (a single-sample view of `self.format`).
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        w: u32,
        h: u32,
        camera: &Orbit,
    ) {
        let (w, h) = (w.max(1), h.max(1));
        self.ensure_targets(device, w, h);
        let eye = camera.eye();
        // Key light follows the camera so the model is never back-lit.
        let light = (eye - camera.target).normalize() + glam::Vec3::new(-0.4, 0.7, 0.0);
        queue.write_buffer(
            &self.globals,
            0,
            bytemuck::bytes_of(&Globals {
                view_proj: camera.view_proj(w as f32 / h as f32).to_cols_array_2d(),
                eye: eye.extend(1.0).to_array(),
                light: light.normalize().extend(0.0).to_array(),
            }),
        );
        let (_, _, msaa, depth) = self.targets.as_ref().unwrap();
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: msaa,
                depth_slice: None,
                resolve_target: Some(target),
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(self.clear), store: wgpu::StoreOp::Discard },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth,
                depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_bind_group(0, &self.globals_bind, &[]);
        pass.set_vertex_buffer(0, self.vbuf.slice(..));
        for d in &self.draws {
            pass.set_pipeline(&self.pipelines[&(d.pass, cull_key(d.cull))]);
            pass.set_bind_group(1, &self.binds[d.bind], &[]);
            pass.draw(d.first..d.first + d.count, 0..1);
        }
    }
}

fn material_uniform(look: &Look) -> MaterialUniform {
    let mode = match look.mode {
        RenderMode::Opaque => 0.0,
        RenderMode::Cutout => 1.0,
        RenderMode::Transparent => 2.0,
    };
    let (width, outline) = look.outline.map_or((0.0, [0.0; 4]), |o| (o.width, o.color));
    let [r, g, b] = look.shadow_color;
    MaterialUniform { color: look.color, shadow: [r, g, b, 1.0], params: [look.cutoff, mode, width, 0.0], outline }
}

fn upload(device: &wgpu::Device, queue: &wgpu::Queue, label: &str, mips: &[image::RgbaImage]) -> wgpu::TextureView {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width: mips[0].width(), height: mips[0].height(), depth_or_array_layers: 1 },
        mip_level_count: mips.len() as u32,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (level, mip) in mips.iter().enumerate() {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &tex, mip_level: level as u32, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            mip,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * mip.width()), rows_per_image: None },
            wgpu::Extent3d { width: mip.width(), height: mip.height(), depth_or_array_layers: 1 },
        );
    }
    tex.create_view(&Default::default())
}
