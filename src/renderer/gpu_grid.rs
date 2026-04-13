//! GPU instanced glyph renderer for the terminal grid.
//!
//! Replaces per-pixel CPU alpha blending with a single instanced draw call.
//! Each visible text character becomes one `GlyphInstance` — a screen-space
//! quad textured from a packed R8Unorm glyph atlas. The vertex shader
//! generates 6 vertices per quad (two triangles) from `vertex_index`;
//! the fragment shader samples the atlas alpha and applies the foreground
//! colour with standard alpha blending.
//!
//! The CPU pixel buffer is still uploaded first (backgrounds, cursors, box
//! drawing) via `write_texture`. The render pass uses `LoadOp::Load` so the
//! GPU glyphs composite on top without clearing the base layer.

use std::collections::HashMap;

use bytemuck::{Pod, Zeroable};

use crate::renderer::glyph_atlas::GlyphAtlas;

const ATLAS_SIZE: u32 = 2048;
const INITIAL_INSTANCE_CAP: u32 = 16384;

const SHADER_SRC: &str = r#"
struct Uniforms {
    screen_size: vec2<f32>,
};

@group(0) @binding(0) var atlas_tex: texture_2d<f32>;
@group(0) @binding(1) var atlas_samp: sampler;
@group(0) @binding(2) var<uniform> uniforms: Uniforms;

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(
    @builtin(vertex_index) vi: u32,
    @location(0) pos: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) uv_pos: vec2<f32>,
    @location(3) uv_size: vec2<f32>,
    @location(4) color: vec4<f32>,
) -> VsOut {
    var corners = array<vec2<f32>, 6>(
        vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(0.0, 1.0),
        vec2(1.0, 0.0), vec2(1.0, 1.0), vec2(0.0, 1.0),
    );
    let c = corners[vi];
    let px = pos + c * size;
    let ndc = vec2(
        px.x / uniforms.screen_size.x * 2.0 - 1.0,
        1.0 - px.y / uniforms.screen_size.y * 2.0,
    );
    var out: VsOut;
    out.position = vec4(ndc, 0.0, 1.0);
    out.uv = uv_pos + c * uv_size;
    out.color = color;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let a = textureSample(atlas_tex, atlas_samp, in.uv).r;
    return vec4(in.color.rgb, in.color.a * a);
}
"#;

/// Glyph descriptor destined for GPU instanced text rendering.
///
/// Produced by `grid::draw()` for terminal cells and by `block_renderer::draw()`
/// for command block output. The GPU renderer resolves atlas UVs and bearing
/// offsets when building the instance buffer.
#[derive(Clone)]
pub struct CellGlyph {
    pub px: usize,
    pub py: usize,
    pub ch: char,
    pub fg: (u8, u8, u8),
    pub font_size: f32,
    pub line_height: f32,
    pub bold: bool,
    pub italic: bool,
}

/// Per-instance data uploaded to the GPU vertex buffer.
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct GlyphInstance {
    pos: [f32; 2],
    size: [f32; 2],
    uv_pos: [f32; 2],
    uv_size: [f32; 2],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Uniforms {
    screen_size: [f32; 2],
    _pad: [f32; 2],
}

#[derive(Debug, Hash, Eq, PartialEq, Clone, Copy)]
struct GpuGlyphKey {
    ch: char,
    size_cp: u32,
    bold: bool,
    italic: bool,
}

impl GpuGlyphKey {
    fn new(ch: char, font_size: f32, bold: bool, italic: bool) -> Self {
        Self {
            ch,
            size_cp: (font_size * 100.0) as u32,
            bold,
            italic,
        }
    }
}

struct AtlasRegion {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    bearing_x: i32,
    bearing_y: i32,
}

/// GPU instanced glyph renderer.
///
/// Owns the wgpu render pipeline, a packed R8Unorm glyph atlas texture,
/// and a dynamically-sized instance buffer. Call [`render`] each frame
/// with the list of [`CellGlyph`]s collected by the grid drawer.
pub struct GpuGridRenderer {
    pipeline: wgpu::RenderPipeline,
    _bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    atlas_texture: wgpu::Texture,
    _atlas_view: wgpu::TextureView,
    _sampler: wgpu::Sampler,
    uniform_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    instance_cap: u32,

    regions: HashMap<GpuGlyphKey, AtlasRegion>,
    shelf_x: u32,
    shelf_y: u32,
    shelf_height: u32,
    /// Reusable instances buffer to avoid per-frame allocation.
    instance_staging: Vec<GlyphInstance>,
}

impl GpuGridRenderer {
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let atlas_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glyph_atlas"),
            size: wgpu::Extent3d {
                width: ATLAS_SIZE,
                height: ATLAS_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let atlas_view = atlas_texture.create_view(&Default::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("glyph_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_grid_uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("glyph_instances"),
            size: (INITIAL_INSTANCE_CAP as u64) * std::mem::size_of::<GlyphInstance>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("gpu_grid_bgl"),
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
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gpu_grid_bg"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: uniform_buffer.as_entire_binding(),
                },
            ],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("gpu_grid_shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER_SRC.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("gpu_grid_pl"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("gpu_grid_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<GlyphInstance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 0,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 8,
                            shader_location: 1,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 16,
                            shader_location: 2,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 24,
                            shader_location: 3,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 32,
                            shader_location: 4,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            multiview_mask: None,
            cache: None,
        });

        Self {
            pipeline,
            _bind_group_layout: bind_group_layout,
            bind_group,
            atlas_texture,
            _atlas_view: atlas_view,
            _sampler: sampler,
            uniform_buffer,
            instance_buffer,
            instance_cap: INITIAL_INSTANCE_CAP,
            regions: HashMap::with_capacity(512),
            shelf_x: 0,
            shelf_y: 0,
            shelf_height: 0,
            instance_staging: Vec::with_capacity(INITIAL_INSTANCE_CAP as usize),
        }
    }

    /// Build instances for `cell_glyphs`, upload to GPU, and execute the
    /// render pass that composites text on top of the already-uploaded
    /// CPU pixel buffer.
    pub fn render(
        &mut self,
        cell_glyphs: &[CellGlyph],
        cpu_atlas: &mut GlyphAtlas,
        font_system: &mut cosmic_text::FontSystem,
        swash_cache: &mut cosmic_text::SwashCache,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_view: &wgpu::TextureView,
        screen_w: f32,
        screen_h: f32,
        scissor: Option<(u32, u32, u32, u32)>,
    ) {
        if cell_glyphs.is_empty() {
            return;
        }

        let atlas_inv = 1.0 / ATLAS_SIZE as f32;

        self.instance_staging.clear();

        for cg in cell_glyphs {
            self.ensure_glyph(
                cg.ch,
                cg.font_size,
                cg.line_height,
                cg.bold,
                cg.italic,
                cpu_atlas,
                font_system,
                swash_cache,
                queue,
            );

            let key = GpuGlyphKey::new(cg.ch, cg.font_size, cg.bold, cg.italic);
            let region = match self.regions.get(&key) {
                Some(r) => r,
                None => continue,
            };

            let gx = cg.px as f32 + region.bearing_x as f32;
            let gy = cg.py as f32 + region.bearing_y as f32;

            self.instance_staging.push(GlyphInstance {
                pos: [gx, gy],
                size: [region.w as f32, region.h as f32],
                uv_pos: [region.x as f32 * atlas_inv, region.y as f32 * atlas_inv],
                uv_size: [region.w as f32 * atlas_inv, region.h as f32 * atlas_inv],
                color: [
                    cg.fg.0 as f32 / 255.0,
                    cg.fg.1 as f32 / 255.0,
                    cg.fg.2 as f32 / 255.0,
                    1.0,
                ],
            });
        }

        if self.instance_staging.is_empty() {
            return;
        }

        let needed = self.instance_staging.len() as u32;
        if needed > self.instance_cap {
            let new_cap = needed.next_power_of_two();
            self.instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("glyph_instances"),
                size: (new_cap as u64) * std::mem::size_of::<GlyphInstance>() as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.instance_cap = new_cap;
        }

        queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(&self.instance_staging));

        let uniforms = Uniforms {
            screen_size: [screen_w, screen_h],
            _pad: [0.0; 2],
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("gpu_grid_encoder"),
        });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("gpu_grid_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: surface_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });

            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
            if let Some((sx, sy, sw, sh)) = scissor {
                pass.set_scissor_rect(sx, sy, sw, sh);
            }
            pass.draw(0..6, 0..needed);
        }

        queue.submit(std::iter::once(encoder.finish()));
    }

    /// Ensure that the glyph for `ch` at `font_size` is packed into the
    /// GPU atlas texture, rasterizing via the CPU atlas if necessary.
    fn ensure_glyph(
        &mut self,
        ch: char,
        font_size: f32,
        cell_height: f32,
        bold: bool,
        italic: bool,
        cpu_atlas: &mut GlyphAtlas,
        font_system: &mut cosmic_text::FontSystem,
        swash_cache: &mut cosmic_text::SwashCache,
        queue: &wgpu::Queue,
    ) {
        let key = GpuGlyphKey::new(ch, font_size, bold, italic);
        if self.regions.contains_key(&key) {
            return;
        }

        let raster = match cpu_atlas.get_or_rasterize(
            ch,
            font_size,
            cell_height,
            bold,
            italic,
            font_system,
            swash_cache,
        ) {
            Some(g) => g,
            None => return,
        };

        let gw = raster.width as u32;
        let gh = raster.height as u32;
        if gw == 0 || gh == 0 {
            return;
        }

        let (ax, ay) = match self.shelf_pack(gw, gh) {
            Some(pos) => pos,
            None => {
                self.reset_atlas(queue);
                match self.shelf_pack(gw, gh) {
                    Some(pos) => pos,
                    None => return,
                }
            }
        };

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.atlas_texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x: ax, y: ay, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            &raster.alphas,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(gw),
                rows_per_image: Some(gh),
            },
            wgpu::Extent3d {
                width: gw,
                height: gh,
                depth_or_array_layers: 1,
            },
        );

        self.regions.insert(
            key,
            AtlasRegion {
                x: ax,
                y: ay,
                w: gw,
                h: gh,
                bearing_x: raster.bearing_x,
                bearing_y: raster.bearing_y,
            },
        );
    }

    /// Shelf-based rectangle packing. Returns `(x, y)` in the atlas or
    /// `None` if the atlas is full.
    fn shelf_pack(&mut self, w: u32, h: u32) -> Option<(u32, u32)> {
        if w > ATLAS_SIZE || h > ATLAS_SIZE {
            return None;
        }

        if self.shelf_x + w <= ATLAS_SIZE {
            let x = self.shelf_x;
            let y = self.shelf_y;
            if y + h.max(self.shelf_height) > ATLAS_SIZE {
                return None;
            }
            self.shelf_x += w + 1;
            self.shelf_height = self.shelf_height.max(h);
            return Some((x, y));
        }

        self.shelf_y += self.shelf_height + 1;
        self.shelf_x = 0;
        self.shelf_height = 0;

        if self.shelf_y + h > ATLAS_SIZE {
            return None;
        }

        let x = self.shelf_x;
        let y = self.shelf_y;
        self.shelf_x = w + 1;
        self.shelf_height = h;
        Some((x, y))
    }

    fn reset_atlas(&mut self, queue: &wgpu::Queue) {
        log::warn!("GPU glyph atlas full — clearing and rebuilding");
        self.regions.clear();
        self.shelf_x = 0;
        self.shelf_y = 0;
        self.shelf_height = 0;

        let zeros = vec![0u8; (ATLAS_SIZE * ATLAS_SIZE) as usize];
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.atlas_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &zeros,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(ATLAS_SIZE),
                rows_per_image: Some(ATLAS_SIZE),
            },
            wgpu::Extent3d {
                width: ATLAS_SIZE,
                height: ATLAS_SIZE,
                depth_or_array_layers: 1,
            },
        );
    }

    /// Clear the GPU atlas cache (e.g. after font change). The texture is
    /// zeroed on the next `ensure_glyph` call that triggers a full reset.
    pub fn clear_atlas(&mut self) {
        self.regions.clear();
        self.shelf_x = 0;
        self.shelf_y = 0;
        self.shelf_height = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyph_key_equality() {
        let a = GpuGlyphKey::new('A', 16.0, false, false);
        let b = GpuGlyphKey::new('A', 16.0, false, false);
        let c = GpuGlyphKey::new('B', 16.0, false, false);
        let d = GpuGlyphKey::new('A', 14.0, false, false);
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_ne!(a, d);
    }

    #[test]
    fn glyph_instance_layout() {
        assert_eq!(std::mem::size_of::<GlyphInstance>(), 48);
        assert_eq!(std::mem::align_of::<GlyphInstance>(), 4);
    }

    #[test]
    fn uniforms_layout() {
        assert_eq!(std::mem::size_of::<Uniforms>(), 16);
    }

    #[test]
    fn shelf_pack_simple() {
        let mut r = ShelfPackerTest::new();
        let p1 = r.shelf_pack(10, 20);
        assert_eq!(p1, Some((0, 0)));
        let p2 = r.shelf_pack(10, 20);
        assert_eq!(p2, Some((11, 0)));
    }

    #[test]
    fn shelf_pack_new_shelf() {
        let mut r = ShelfPackerTest::new();
        r.shelf_pack(ATLAS_SIZE - 5, 30);
        let p2 = r.shelf_pack(10, 20);
        assert_eq!(p2, Some((0, 31)));
    }

    #[test]
    fn shelf_pack_overflow() {
        let mut r = ShelfPackerTest::new();
        assert_eq!(r.shelf_pack(ATLAS_SIZE + 1, 10), None);
        assert_eq!(r.shelf_pack(10, ATLAS_SIZE + 1), None);
    }

    struct ShelfPackerTest {
        shelf_x: u32,
        shelf_y: u32,
        shelf_height: u32,
    }

    impl ShelfPackerTest {
        fn new() -> Self {
            Self {
                shelf_x: 0,
                shelf_y: 0,
                shelf_height: 0,
            }
        }

        fn shelf_pack(&mut self, w: u32, h: u32) -> Option<(u32, u32)> {
            if w > ATLAS_SIZE || h > ATLAS_SIZE {
                return None;
            }
            if self.shelf_x + w <= ATLAS_SIZE {
                let x = self.shelf_x;
                let y = self.shelf_y;
                if y + h.max(self.shelf_height) > ATLAS_SIZE {
                    return None;
                }
                self.shelf_x += w + 1;
                self.shelf_height = self.shelf_height.max(h);
                return Some((x, y));
            }
            self.shelf_y += self.shelf_height + 1;
            self.shelf_x = 0;
            self.shelf_height = 0;
            if self.shelf_y + h > ATLAS_SIZE {
                return None;
            }
            let x = self.shelf_x;
            let y = self.shelf_y;
            self.shelf_x = w + 1;
            self.shelf_height = h;
            Some((x, y))
        }
    }

    #[test]
    fn cell_glyph_fields() {
        let cg = CellGlyph {
            px: 100,
            py: 200,
            ch: 'A',
            fg: (255, 128, 0),
            font_size: 26.0,
            line_height: 30.0,
            bold: false,
            italic: false,
        };
        assert_eq!(cg.ch, 'A');
        assert_eq!(cg.fg.1, 128);
    }

    #[test]
    fn instance_buffer_growth_is_power_of_two() {
        let n: u32 = 20000;
        let cap = n.next_power_of_two();
        assert_eq!(cap, 32768);
        assert!(cap >= n);
    }

    #[test]
    fn atlas_region_stores_bearing() {
        let r = AtlasRegion {
            x: 10,
            y: 20,
            w: 12,
            h: 22,
            bearing_x: 1,
            bearing_y: -15,
        };
        assert_eq!(r.bearing_y, -15);
    }

    #[test]
    fn shader_source_is_non_empty() {
        assert!(SHADER_SRC.len() > 100);
        assert!(SHADER_SRC.contains("vs_main"));
        assert!(SHADER_SRC.contains("fs_main"));
    }
}
