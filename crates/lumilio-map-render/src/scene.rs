use crate::{Camera, Error, Frame, Grid, Tile};
use std::collections::BTreeMap;
use wgpu::util::DeviceExt;

struct Cached {
    texture: wgpu::Texture,
    used: u64,
}
pub struct Scene {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    textures: BTreeMap<String, Cached>,
    clock: u64,
    limit: usize,
}

fn bytes(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_ne_bytes())
        .collect()
}
fn error(value: impl ToString) -> Error {
    Error::Render(value.to_string())
}

impl Scene {
    pub fn new() -> Result<Self, Error> {
        pollster::block_on(Self::create())
    }
    async fn create() -> Result<Self, Error> {
        let instance = wgpu::Instance::default();
        let adapter = match instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
        {
            Ok(adapter) => adapter,
            Err(_) => instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    force_fallback_adapter: true,
                    ..Default::default()
                })
                .await
                .map_err(|_| Error::NoGpu)?,
        };
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("world-map"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::default(),
                trace: wgpu::Trace::Off,
            })
            .await
            .map_err(error)?;
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("map-tile"),
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
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("map"),
            source: wgpu::ShaderSource::Wgsl(include_str!("map.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("map"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: 16,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x4,
                        offset: 0,
                        shader_location: 0,
                    }],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Bgra8Unorm,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        Ok(Self {
            device,
            queue,
            pipeline,
            layout,
            sampler,
            textures: BTreeMap::new(),
            clock: 0,
            limit: 256 * 1024 * 1024,
        })
    }
    fn texture(
        &self,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
    ) -> wgpu::Texture {
        self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("map"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
    }
    pub fn render(
        &mut self,
        camera: Camera,
        tiles: &[Tile],
        grid: Grid,
        width: u32,
        height: u32,
    ) -> Result<Frame, Error> {
        if width == 0
            || height == 0
            || width > 4096
            || height > 4096
            || !camera.x.is_finite()
            || !camera.z.is_finite()
            || !camera.blocks_per_pixel.is_finite()
            || camera.blocks_per_pixel <= 0.0
            || tiles.iter().any(|tile| {
                tile.rgba.len() != 256 * 256 * 4
                    || !tile.x.is_finite()
                    || !tile.z.is_finite()
                    || !tile.span.is_finite()
                    || tile.span <= 0.0
            })
        {
            return Err(Error::InvalidInput);
        }
        self.clock += 1;
        for tile in tiles {
            if !self.textures.contains_key(&tile.id) {
                let texture = self.texture(
                    256,
                    256,
                    wgpu::TextureFormat::Rgba8Unorm,
                    wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                );
                self.queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    &tile.rgba,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(1024),
                        rows_per_image: Some(256),
                    },
                    wgpu::Extent3d {
                        width: 256,
                        height: 256,
                        depth_or_array_layers: 1,
                    },
                );
                self.textures.insert(
                    tile.id.clone(),
                    Cached {
                        texture,
                        used: self.clock,
                    },
                );
            }
            self.textures.get_mut(&tile.id).unwrap().used = self.clock;
        }
        let target = self.texture(
            width,
            height,
            wgpu::TextureFormat::Bgra8Unorm,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        let target_view = target.create_view(&Default::default());
        let min_x = camera.x - f64::from(width) * camera.blocks_per_pixel / 2.0;
        let min_z = camera.z - f64::from(height) * camera.blocks_per_pixel / 2.0;
        // vec3 padding aligns to 16 bytes; the WGSL uniform occupies 48 bytes.
        let uniform = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: &bytes(&[
                    min_x as f32,
                    min_z as f32,
                    camera.blocks_per_pixel as f32,
                    u8::from(grid.chunks) as f32,
                    u8::from(grid.regions) as f32,
                    0.,
                    0.,
                    0.,
                    0.,
                    0.,
                    0.,
                    0.,
                ]),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let mut draws = Vec::new();
        for tile in tiles {
            let rect = [
                ((tile.x - min_x) / (f64::from(width) * camera.blocks_per_pixel) * 2.0 - 1.0)
                    as f32,
                (1.0 - (tile.z - min_z) / (f64::from(height) * camera.blocks_per_pixel) * 2.0)
                    as f32,
                (tile.span / (f64::from(width) * camera.blocks_per_pixel) * 2.0) as f32,
                (-tile.span / (f64::from(height) * camera.blocks_per_pixel) * 2.0) as f32,
            ];
            let vertex = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: None,
                    contents: &bytes(&rect),
                    usage: wgpu::BufferUsages::VERTEX,
                });
            let view = self.textures[&tile.id]
                .texture
                .create_view(&Default::default());
            let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: uniform.as_entire_binding(),
                    },
                ],
            });
            draws.push((vertex, group));
        }
        let padded = (width * 4).div_ceil(256) * 256;
        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: u64::from(padded) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.18,
                            g: 0.18,
                            b: 0.18,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            for (vertex, group) in &draws {
                pass.set_bind_group(0, group, &[]);
                pass.set_vertex_buffer(0, vertex.slice(..));
                pass.draw(0..6, 0..1);
            }
        }
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &staging,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);
        let slice = staging.slice(..);
        let (send, receive) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = send.send(result);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(error)?;
        receive.recv().map_err(error)?.map_err(error)?;
        let data = slice.get_mapped_range().map_err(error)?;
        let mut bgra = Vec::with_capacity((width * height * 4) as usize);
        for row in data.chunks_exact(padded as usize) {
            bgra.extend_from_slice(&row[..(width * 4) as usize]);
        }
        drop(data);
        staging.unmap();
        while self.textures.len() * 256 * 256 * 4 > self.limit {
            let oldest = self
                .textures
                .iter()
                .min_by_key(|(_, entry)| entry.used)
                .map(|(id, _)| id.clone())
                .unwrap();
            self.textures.remove(&oldest);
        }
        Ok(Frame {
            width,
            height,
            bgra,
        })
    }
}
