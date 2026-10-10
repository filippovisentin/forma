//! GPU display of document geometry with wgpu: cameras, scene buffers, shaded
//! surfaces with edges, curves, construction-plane grids and selection highlight.
//! Each viewport renders into its own texture ([`View`]), so the same code drives the
//! egui viewports and headless screenshots (tests, CLI, agents). No windowing here.

mod camera;
mod scene;

pub use camera::{Camera, StandardView};
pub use glam;
pub use scene::{
    grid_lines, grid_spacing, model_center, GridPlane, LineVertex, MeshVertex, Scene, SceneCache,
    EDGE_COLOR, HIGHLIGHT,
};
pub use wgpu;

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

/// Colour format of the rendered image (what PNGs are written from).
pub const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
/// Format of the view handed to egui (`Renderer::render`'s return value).
pub const DISPLAY_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const SAMPLES: u32 = 4;
/// Viewport background (linear RGB), close to Rhino's default grey.
const BACKGROUND: wgpu::Color = wgpu::Color {
    r: 0.352,
    g: 0.352,
    b: 0.352,
    a: 1.0,
};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniforms {
    view_proj: [[f32; 4]; 4],
    eye: [f32; 4],
}

struct Targets {
    size: (u32, u32),
    msaa: wgpu::TextureView,
    depth: wgpu::TextureView,
    output: wgpu::Texture,
    output_view: wgpu::TextureView,
    /// Same pixels viewed as non-sRGB (gamma-encoded values), as egui expects.
    display_view: wgpu::TextureView,
}

/// GPU buffers of one [`Scene`].
#[derive(Default)]
struct Buffers {
    mesh_vertices: Option<wgpu::Buffer>,
    mesh_indices: Option<wgpu::Buffer>,
    index_count: u32,
    lines: Option<wgpu::Buffer>,
    line_count: u32,
}

impl Buffers {
    fn new(device: &wgpu::Device, s: &Scene) -> Buffers {
        Buffers {
            mesh_vertices: buffer(
                device,
                "mesh vertices",
                bytemuck::cast_slice(&s.mesh_vertices),
                wgpu::BufferUsages::VERTEX,
            ),
            mesh_indices: buffer(
                device,
                "mesh indices",
                bytemuck::cast_slice(&s.mesh_indices),
                wgpu::BufferUsages::INDEX,
            ),
            index_count: s.mesh_indices.len() as u32,
            lines: buffer(
                device,
                "lines",
                bytemuck::cast_slice(&s.line_vertices),
                wgpu::BufferUsages::VERTEX,
            ),
            line_count: s.line_vertices.len() as u32,
        }
    }
}

fn buffer(
    device: &wgpu::Device,
    label: &str,
    contents: &[u8],
    usage: wgpu::BufferUsages,
) -> Option<wgpu::Buffer> {
    (!contents.is_empty()).then(|| {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents,
            usage,
        })
    })
}

/// How a viewport draws objects (Rhino's display modes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayMode {
    Wireframe,
    Shaded,
}

impl DisplayMode {
    pub const ALL: [DisplayMode; 2] = [DisplayMode::Wireframe, DisplayMode::Shaded];

    pub fn name(self) -> &'static str {
        match self {
            DisplayMode::Wireframe => "Wireframe",
            DisplayMode::Shaded => "Shaded",
        }
    }
}

/// One viewport's render target and camera uniforms.
pub struct View {
    uniforms: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    targets: Option<Targets>,
}

/// Shared pipelines and scene buffers; draws any number of [`View`]s.
pub struct Renderer {
    mesh_pipeline: wgpu::RenderPipeline,
    line_pipeline: wgpu::RenderPipeline,
    grid_pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    scene: Buffers,
    highlight: Buffers,
    grids: [Buffers; 3],
    pub show_grid: bool,
}

impl Renderer {
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("forma shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("forma uniforms"),
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
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("forma layout"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });

        let mesh_attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x4];
        let line_attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4];
        let pipeline = |label: &str,
                        vs: &str,
                        fs: &str,
                        stride: usize,
                        attrs: &[wgpu::VertexAttribute],
                        topology: wgpu::PrimitiveTopology,
                        bias: wgpu::DepthBiasState,
                        write_depth: bool,
                        compare: wgpu::CompareFunction| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some(vs),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: stride as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: attrs,
                    })],
                },
                primitive: wgpu::PrimitiveState {
                    topology,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(write_depth),
                    depth_compare: Some(compare),
                    stencil: Default::default(),
                    bias,
                }),
                multisample: wgpu::MultisampleState {
                    count: SAMPLES,
                    ..Default::default()
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: COLOR_FORMAT,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        // Surfaces are pushed slightly back so curves and edges on them stay visible.
        let mesh_pipeline = pipeline(
            "forma mesh",
            "vs_mesh",
            "fs_mesh",
            std::mem::size_of::<MeshVertex>(),
            &mesh_attrs,
            wgpu::PrimitiveTopology::TriangleList,
            wgpu::DepthBiasState {
                constant: 2,
                slope_scale: 1.5,
                clamp: 0.0,
            },
            true,
            wgpu::CompareFunction::LessEqual,
        );
        let line_pipeline = pipeline(
            "forma lines",
            "vs_line",
            "fs_line",
            std::mem::size_of::<LineVertex>(),
            &line_attrs,
            wgpu::PrimitiveTopology::LineList,
            Default::default(),
            true,
            wgpu::CompareFunction::LessEqual,
        );
        // The grid never hides anything (like Rhino, objects below it stay visible).
        let grid_pipeline = pipeline(
            "forma grid",
            "vs_line",
            "fs_line",
            std::mem::size_of::<LineVertex>(),
            &line_attrs,
            wgpu::PrimitiveTopology::LineList,
            Default::default(),
            false,
            wgpu::CompareFunction::LessEqual,
        );

        Renderer {
            mesh_pipeline,
            line_pipeline,
            grid_pipeline,
            bind_group_layout: bgl,
            scene: Buffers::default(),
            highlight: Buffers::default(),
            grids: Default::default(),
            show_grid: true,
        }
    }

    /// A new viewport target.
    pub fn new_view(&self, device: &wgpu::Device) -> View {
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("forma view uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("forma view"),
            layout: &self.bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniforms.as_entire_binding(),
            }],
        });
        View {
            uniforms,
            bind_group,
            targets: None,
        }
    }

    /// Upload the document scene and rebuild the grids around it. Returns the grid
    /// spacing (the same on all three planes).
    pub fn set_scene(&mut self, device: &wgpu::Device, scene: &Scene, min_grid_extent: f64) -> f64 {
        self.scene = Buffers::new(device, scene);
        let mut spacing = 1.0;
        for p in GridPlane::ALL {
            let (lines, s) = grid_lines(scene, p, min_grid_extent);
            spacing = s;
            self.grids[p.index()] = Buffers {
                lines: buffer(
                    device,
                    "grid",
                    bytemuck::cast_slice(&lines),
                    wgpu::BufferUsages::VERTEX,
                ),
                line_count: lines.len() as u32,
                ..Default::default()
            };
        }
        spacing
    }

    /// Upload the selection highlight (empty scene clears it).
    pub fn set_highlight(&mut self, device: &wgpu::Device, scene: &Scene) {
        self.highlight = Buffers::new(device, scene);
    }

    fn ensure_targets(view: &mut View, device: &wgpu::Device, size: (u32, u32)) {
        if view.targets.as_ref().is_some_and(|t| t.size == size) {
            return;
        }
        let extent = wgpu::Extent3d {
            width: size.0,
            height: size.1,
            depth_or_array_layers: 1,
        };
        let tex = |label: &str, format, samples, usage, view_formats: &[wgpu::TextureFormat]| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: extent,
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats,
            })
        };
        let msaa = tex(
            "msaa",
            COLOR_FORMAT,
            SAMPLES,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
            &[],
        );
        let depth = tex(
            "depth",
            DEPTH_FORMAT,
            SAMPLES,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
            &[],
        );
        let output = tex(
            "viewport",
            COLOR_FORMAT,
            1,
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            &[DISPLAY_FORMAT],
        );
        let display_view = output.create_view(&wgpu::TextureViewDescriptor {
            format: Some(DISPLAY_FORMAT),
            ..Default::default()
        });
        view.targets = Some(Targets {
            size,
            msaa: msaa.create_view(&Default::default()),
            depth: depth.create_view(&Default::default()),
            output_view: output.create_view(&Default::default()),
            output,
            display_view,
        });
    }

    /// Render a view and return its texture in [`DISPLAY_FORMAT`] (gamma-encoded
    /// values, as egui expects for native textures).
    #[allow(clippy::too_many_arguments)]
    pub fn render<'v>(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view: &'v mut View,
        size: (u32, u32),
        camera: &Camera,
        grid: GridPlane,
        mode: DisplayMode,
    ) -> &'v wgpu::TextureView {
        let size = (size.0.max(1), size.1.max(1));
        Self::ensure_targets(view, device, size);
        let aspect = size.0 as f64 / size.1 as f64;
        let eye = if camera.ortho {
            let d = -camera.back();
            [d.x as f32, d.y as f32, d.z as f32, 1.0]
        } else {
            let e = camera.eye();
            [e.x as f32, e.y as f32, e.z as f32, 0.0]
        };
        let u = Uniforms {
            view_proj: camera.view_proj(aspect).to_cols_array_2d(),
            eye,
        };
        queue.write_buffer(&view.uniforms, 0, bytemuck::bytes_of(&u));

        let targets = view.targets.as_ref().expect("targets");
        let mut enc = device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("forma viewport"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &targets.msaa,
                    depth_slice: None,
                    resolve_target: Some(&targets.output_view),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(BACKGROUND),
                        store: wgpu::StoreOp::Discard,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &targets.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &view.bind_group, &[]);
            let draw_lines_with =
                |pass: &mut wgpu::RenderPass<'_>, b: &Buffers, p: &wgpu::RenderPipeline| {
                    if let Some(l) = &b.lines {
                        pass.set_pipeline(p);
                        pass.set_vertex_buffer(0, l.slice(..));
                        pass.draw(0..b.line_count, 0..1);
                    }
                };
            let draw_lines = |pass: &mut wgpu::RenderPass<'_>, b: &Buffers| {
                draw_lines_with(pass, b, &self.line_pipeline);
            };
            let draw_mesh_with =
                |pass: &mut wgpu::RenderPass<'_>, b: &Buffers, p: &wgpu::RenderPipeline| {
                    if let (Some(v), Some(i)) = (&b.mesh_vertices, &b.mesh_indices) {
                        pass.set_pipeline(p);
                        pass.set_vertex_buffer(0, v.slice(..));
                        pass.set_index_buffer(i.slice(..), wgpu::IndexFormat::Uint32);
                        pass.draw_indexed(0..b.index_count, 0, 0..1);
                    }
                };
            if self.show_grid {
                let b = &self.grids[grid.index()];
                if let Some(l) = &b.lines {
                    pass.set_pipeline(&self.grid_pipeline);
                    pass.set_vertex_buffer(0, l.slice(..));
                    pass.draw(0..b.line_count, 0..1);
                }
            }
            let draw_mesh = |pass: &mut wgpu::RenderPass<'_>, b: &Buffers| {
                draw_mesh_with(pass, b, &self.mesh_pipeline);
            };
            match mode {
                DisplayMode::Wireframe => {
                    draw_lines(&mut pass, &self.scene);
                    draw_lines(&mut pass, &self.highlight);
                }
                DisplayMode::Shaded => {
                    draw_mesh(&mut pass, &self.scene);
                    draw_lines(&mut pass, &self.scene);
                    // Same geometry drawn again: equal depth passes LessEqual, so it lands on top.
                    draw_mesh(&mut pass, &self.highlight);
                    draw_lines(&mut pass, &self.highlight);
                }
            }
        }
        queue.submit([enc.finish()]);
        &view.targets.as_ref().expect("targets").display_view
    }

    /// Copy a view's last image back to the CPU as tightly packed RGBA8 (sRGB).
    pub fn read_pixels(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view: &View,
    ) -> Option<Vec<u8>> {
        let t = view.targets.as_ref()?;
        let (w, h) = t.size;
        let row = w * 4;
        let padded =
            row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: (padded * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = device.create_command_encoder(&Default::default());
        enc.copy_texture_to_buffer(
            t.output.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        queue.submit([enc.finish()]);
        buffer.map_async(wgpu::MapMode::Read, .., |_| {});
        device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            })
            .ok()?;
        let data = buffer.slice(..).get_mapped_range().ok()?;
        let mut out = Vec::with_capacity((row * h) as usize);
        for y in 0..h as usize {
            let start = y * padded as usize;
            out.extend_from_slice(&data[start..start + row as usize]);
        }
        Some(out)
    }
}

/// Grid plane matching a standard view (Rhino's CPlanes).
pub fn grid_plane_for(view: StandardView) -> GridPlane {
    match view {
        StandardView::Front => GridPlane::XZ,
        StandardView::Right => GridPlane::YZ,
        _ => GridPlane::XY,
    }
}

/// A GPU device without a window, for screenshots and tests.
pub fn headless_device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: None,
        ..Default::default()
    }))
    .ok()?;
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()
}

/// Render a document to RGBA8 pixels without a window. Returns `None` when no GPU
/// (or software rasteriser) is available.
pub fn screenshot(
    doc: &forma_doc::Document,
    view: StandardView,
    size: (u32, u32),
) -> Option<Vec<u8>> {
    let (device, queue) = headless_device()?;
    let scene = Scene::from_document(doc);
    let mut r = Renderer::new(&device);
    r.set_scene(&device, &scene, 10.0);
    r.show_grid = std::env::var_os("FORMA_NO_GRID").is_none();
    let mut v = r.new_view(&device);
    let mut cam = Camera::view(view);
    cam.fit(scene.min, scene.max, size.0 as f64 / size.1 as f64);
    r.render(
        &device,
        &queue,
        &mut v,
        size,
        &cam,
        grid_plane_for(view),
        DisplayMode::Shaded,
    );
    r.read_pixels(&device, &queue, &v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use forma_doc::{Document, Geometry};
    use forma_geom::{Mesh, Point3};

    /// Renders a red square (above the grid plane) seen from the top and checks the
    /// centre pixel. Skipped when no GPU or software rasteriser is available.
    #[test]
    fn renders_a_mesh_headless() {
        let mut doc = Document::new();
        doc.layers[0].color = [200, 0, 0];
        let mut t = doc.begin();
        t.add(Geometry::Mesh(Mesh {
            positions: vec![
                Point3::new(0.0, 0.0, 10.0),
                Point3::new(100.0, 0.0, 10.0),
                Point3::new(100.0, 100.0, 10.0),
                Point3::new(0.0, 100.0, 10.0),
            ],
            normals: vec![],
            triangles: vec![[0, 1, 2], [0, 2, 3]],
        }));
        t.commit();
        let Some(px) = screenshot(&doc, StandardView::Top, (64, 48)) else {
            eprintln!("skipping: no GPU adapter");
            return;
        };
        assert_eq!(px.len(), 64 * 48 * 4);
        let c = &px[(24 * 64 + 32) * 4..(24 * 64 + 32) * 4 + 4];
        assert!(
            c[0] > c[1] + 40 && c[0] > c[2] + 40,
            "centre pixel {c:?} should be reddish"
        );
        let corner = &px[0..4];
        assert!(
            corner[0].abs_diff(corner[1]) < 20,
            "corner {corner:?} should be background grey"
        );
    }
}
