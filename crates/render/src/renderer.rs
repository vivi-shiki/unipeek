//! Minimal forward renderer: opaque parts, then alpha-blended parts. MSAA 4x.
use crate::{
    camera::Orbit,
    model::{Model, Vertex},
    texture,
};
use bytemuck::{Pod, Zeroable};
use std::path::Path;
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

struct Draw {
    first: u32,
    count: u32,
    bind: usize,
    blend: bool,
}

pub struct Renderer {
    opaque: wgpu::RenderPipeline,
    blend: wgpu::RenderPipeline,
    globals: wgpu::Buffer,
    globals_bind: wgpu::BindGroup,
    vbuf: wgpu::Buffer,
    binds: Vec<wgpu::BindGroup>,
    draws: Vec<Draw>,
    format: wgpu::TextureFormat,
    targets: Option<(u32, u32, wgpu::TextureView, wgpu::TextureView)>,
    pub clear: wgpu::Color,
}

impl Renderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        model: &Model,
        texture_dir: &Path,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let g_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let t_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
            ],
        });
        let globals_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &g_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&g_layout), Some(&t_layout)],
            immediate_size: 0,
        });
        let pipeline = |blend: bool| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: size_of::<Vertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2],
                    })],
                },
                primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH,
                    depth_write_enabled: Some(!blend),
                    depth_compare: Some(wgpu::CompareFunction::Less),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: SAMPLES,
                    mask: !0,
                    alpha_to_coverage_enabled: !blend,
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(if blend {
                            wgpu::BlendState::ALPHA_BLENDING
                        } else {
                            wgpu::BlendState::REPLACE
                        }),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let (opaque, blend) = (pipeline(false), pipeline(true));

        let vbuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("vertices"),
            contents: bytemuck::cast_slice(&model.vertices),
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

        // One texture per distinct material; light grey fallback when nothing matches.
        let mut names: Vec<String> = Vec::new();
        let mut binds = Vec::new();
        let mut draws = Vec::new();
        for part in &model.parts {
            let bind = match names.iter().position(|n| n == &part.material) {
                Some(i) => i,
                None => {
                    let mips = texture::guess_texture(texture_dir, &part.material)
                        .and_then(|p| texture::load_mips(&p).map_err(|e| eprintln!("texture {p:?}: {e}")).ok())
                        .unwrap_or_else(|| {
                            eprintln!("no texture for material {:?}, using grey", part.material);
                            vec![image::RgbaImage::from_pixel(4, 4, image::Rgba([230, 230, 230, 255]))]
                        });
                    let tex = device.create_texture(&wgpu::TextureDescriptor {
                        label: Some(&part.material),
                        size: wgpu::Extent3d {
                            width: mips[0].width(),
                            height: mips[0].height(),
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: mips.len() as u32,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Rgba8UnormSrgb,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                        view_formats: &[],
                    });
                    for (level, mip) in mips.iter().enumerate() {
                        queue.write_texture(
                            wgpu::TexelCopyTextureInfo {
                                texture: &tex,
                                mip_level: level as u32,
                                origin: wgpu::Origin3d::ZERO,
                                aspect: wgpu::TextureAspect::All,
                            },
                            mip,
                            wgpu::TexelCopyBufferLayout {
                                offset: 0,
                                bytes_per_row: Some(4 * mip.width()),
                                rows_per_image: None,
                            },
                            wgpu::Extent3d { width: mip.width(), height: mip.height(), depth_or_array_layers: 1 },
                        );
                    }
                    binds.push(device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: None,
                        layout: &t_layout,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: wgpu::BindingResource::TextureView(&tex.create_view(&Default::default())),
                            },
                            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&sampler) },
                        ],
                    }));
                    names.push(part.material.clone());
                    names.len() - 1
                }
            };
            draws.push(Draw {
                first: part.first,
                count: part.count,
                bind,
                blend: part.material.to_lowercase().contains("alpha"),
            });
        }

        Renderer {
            opaque,
            blend,
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
        for blend in [false, true] {
            pass.set_pipeline(if blend { &self.blend } else { &self.opaque });
            for d in self.draws.iter().filter(|d| d.blend == blend) {
                pass.set_bind_group(1, &self.binds[d.bind], &[]);
                pass.draw(d.first..d.first + d.count, 0..1);
            }
        }
    }
}
