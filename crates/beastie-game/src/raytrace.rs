//! Ordinary-GPU raster visibility and dynamic shadows with shared compute lighting.
use crate::ray_scene::{BvhNode, GeometryTriangle, GpuInstance, RayScene, TriangleSurface};
use bevy::{
    core_pipeline::FullscreenShader,
    ecs::schedule::ScheduleLabel,
    prelude::*,
    render::{
        RenderApp, RenderStartup,
        diagnostic::RecordDiagnostics,
        extract_resource::ExtractResourcePlugin,
        render_resource::{binding_types::*, *},
        renderer::{RenderContext, RenderDevice, RenderQueue, ViewQuery},
        view::{ExtractedView, ViewTarget},
    },
};
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct RayTracing;
#[derive(Resource, Clone, Default)]
pub struct RayReady(Arc<AtomicBool>);
impl RayReady {
    pub fn get(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}
/// Deterministic presentation clock and accessibility controls for water optics.
#[derive(Resource, Clone, Copy, Default, bevy::render::extract_resource::ExtractResource)]
struct WaterAppearance(Vec4);
fn update_water(frame: Res<crate::renderer::SceneFrame>, mut water: ResMut<WaterAppearance>) {
    let plan = &frame.plan;
    let seconds = if plan.reduced_motion {
        0.0
    } else {
        (plan.elapsed_ms.saturating_add(plan.simulation_remainder_ms) as f64 / 1000.0) as f32
    };
    water.0 = Vec4::new(
        seconds,
        if plan.reduced_flashes { 0.45 } else { 1.0 },
        0.0,
        0.0,
    );
}

#[cfg(test)]
mod water_tests {
    use super::*;

    #[test]
    fn water_does_not_jump_back_at_one_hour_and_honors_accessibility() {
        let world = beastie_core::WorldState::new(42, "Mop");
        let mut plan = beastie_view::plan(&world, &Default::default()).0;
        plan.elapsed_ms = 3_599_900;
        let mut app = App::new();
        app.insert_resource(crate::renderer::SceneFrame { plan })
            .init_resource::<WaterAppearance>()
            .add_systems(Update, update_water);
        app.update();
        let before = app.world().resource::<WaterAppearance>().0.x;
        app.world_mut()
            .resource_mut::<crate::renderer::SceneFrame>()
            .plan
            .elapsed_ms += 200;
        app.update();
        let after = app.world().resource::<WaterAppearance>().0.x;
        assert!((after - before - 0.2).abs() < 0.001);
        {
            let mut frame = app
                .world_mut()
                .resource_mut::<crate::renderer::SceneFrame>();
            frame.plan.reduced_motion = true;
            frame.plan.reduced_flashes = true;
        }
        app.update();
        let reduced = app.world().resource::<WaterAppearance>().0;
        assert_eq!(reduced.x, 0.0);
        assert!(reduced.y > 0.0 && reduced.y < 1.0);
        app.world_mut()
            .resource_mut::<crate::renderer::SceneFrame>()
            .plan
            .elapsed_ms += 10_000;
        app.update();
        assert_eq!(app.world().resource::<WaterAppearance>().0, reduced);
    }
}
pub(crate) fn probe_shader_source(
    probe: crate::args::RenderProbe,
) -> std::borrow::Cow<'static, str> {
    let source = include_str!("raytrace.wgsl");
    if probe == crate::args::RenderProbe::Full {
        return std::borrow::Cow::Borrowed(source);
    }
    std::borrow::Cow::Owned(source.replace(
        "const RENDER_PROBE:u32=0u;",
        &format!("const RENDER_PROBE:u32={}u;", probe as u32),
    ))
}

pub struct RayTracePlugin;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PrimaryVisibility {
    Compute,
    Packed,
    Wide,
}
impl PrimaryVisibility {
    fn select(size: UVec2, triangles: u32, instances: usize, texture_limit: u32) -> Self {
        if size.max_element() > texture_limit / 2 {
            Self::Compute
        } else if triangles < (1 << 24) && instances < 256 {
            Self::Packed
        } else {
            Self::Wide
        }
    }
    fn format(self) -> TextureFormat {
        if self == Self::Wide {
            TextureFormat::Rg32Uint
        } else {
            TextureFormat::R32Uint
        }
    }
    fn packed(self) -> bool {
        self == Self::Packed
    }
}

impl Plugin for RayTracePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WaterAppearance>()
            .add_systems(Update, update_water.after(crate::host::HostSet::Publish));
        app.add_plugins((
            ExtractResourcePlugin::<WaterAppearance>::default(),
            ExtractResourcePlugin::<RayScene>::default(),
            ExtractResourcePlugin::<crate::appearance::RenderAppearance>::default(),
        ));
        let ready = RayReady::default();
        app.insert_resource(ready.clone());
        let probe = app
            .world()
            .get_resource::<crate::args::RenderProbe>()
            .copied()
            .unwrap_or_default();
        let shader = app
            .world_mut()
            .resource_mut::<Assets<Shader>>()
            .add(Shader::from_wgsl(
                probe_shader_source(probe),
                "embedded://beastie/raytrace.wgsl",
            ));
        let compute_only = app
            .world_mut()
            .resource_mut::<Assets<Shader>>()
            .add(Shader::from_wgsl(
                probe_shader_source(probe).replace(
                    "const RASTER_PRIMARY:bool=true;",
                    "const RASTER_PRIMARY:bool=false;",
                ),
                "embedded://beastie/compute-primary.wgsl",
            ));
        let visibility = app
            .world_mut()
            .resource_mut::<Assets<Shader>>()
            .add(Shader::from_wgsl(
                include_str!("ray_visibility.wgsl"),
                "embedded://beastie/ray_visibility.wgsl",
            ));
        let blit = app
            .world_mut()
            .resource_mut::<Assets<Shader>>()
            .add(Shader::from_wgsl(
                include_str!("ray_blit.wgsl"),
                "embedded://beastie/ray_blit.wgsl",
            ));
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .insert_resource(ready)
                .insert_resource(RayShaders {
                    shadows: !matches!(
                        probe,
                        crate::args::RenderProbe::NoShadows | crate::args::RenderProbe::PrimaryOnly
                    ),
                    shader,
                    compute_only,
                    blit,
                    visibility,
                })
                .add_systems(RenderStartup, setup_pipeline)
                .add_systems(RayTracing, render);
        }
    }
}
#[derive(Resource)]
struct RayShaders {
    shadows: bool,
    compute_only: Handle<Shader>,
    visibility: Handle<Shader>,
    shader: Handle<Shader>,
    blit: Handle<Shader>,
}
#[derive(Resource)]
struct RayPipeline {
    shadow_depth: CachedRenderPipelineId,
    visibility_layout: BindGroupLayoutDescriptor,
    visibility: [CachedRenderPipelineId; 2],
    compute_only: CachedComputePipelineId,
    layout: BindGroupLayoutDescriptor,
    blit_layout: BindGroupLayoutDescriptor,
    compute: CachedComputePipelineId,
}
#[derive(ShaderType, Clone, Default)]
struct Params {
    world_from_clip: Mat4,
    size: UVec4,
    water: Vec4,
    roots: UVec4,
    shadow_roots: UVec4,
    cache_roots: UVec4,
    static_instances: [UVec4; 4],
}
#[cfg(test)]
mod visibility_tests {
    use super::*;
    #[test]
    fn visibility_limits_keep_indices_and_texture_extents_representable() {
        let size = UVec2::new(1920, 1080);
        assert_eq!(
            PrimaryVisibility::select(size, (1 << 24) - 1, 255, 4096),
            PrimaryVisibility::Packed
        );
        assert_eq!(
            PrimaryVisibility::select(size, 1 << 24, 255, 4096),
            PrimaryVisibility::Wide
        );
        assert_eq!(
            PrimaryVisibility::select(size, 100, 256, 4096),
            PrimaryVisibility::Wide
        );
        assert_eq!(
            PrimaryVisibility::select(UVec2::new(4096, 2160), 100, 1, 8192),
            PrimaryVisibility::Packed
        );
        assert_eq!(
            PrimaryVisibility::select(UVec2::new(4097, 2160), 100, 1, 8192),
            PrimaryVisibility::Compute
        );
    }
}

#[derive(ShaderType, Default)]
struct VisibilityParams {
    clip_from_world: Mat4,
}
#[derive(Default)]
struct FrameBuffers {
    shadow_map_params: UniformBuffer<crate::ray_shadow_maps::ShadowMapParams>,
    shadow_cameras: [UniformBuffer<VisibilityParams>; 12],
    shadow_maps: Option<(u32, Texture, TextureView, Vec<TextureView>)>,
    shadow_requested: Option<bool>,
    visibility_params: UniformBuffer<VisibilityParams>,
    visibility: Option<(
        UVec2,
        PrimaryVisibility,
        Texture,
        TextureView,
        Texture,
        TextureView,
    )>,
    triangles: Option<Buffer>,
    shadow_masks: Option<Buffer>,
    bounce_cache: Option<Buffer>,
    shadow_key: Option<(u64, UVec2, [u32; 16])>,
    surfaces: Option<Buffer>,
    nodes: Option<Buffer>,
    chunks: HashMap<u32, u64>,
    instances: StorageBuffer<Vec<GpuInstance>>,
    tlas: StorageBuffer<Vec<BvhNode>>,
    params: UniformBuffer<Params>,
    output: Option<(UVec2, Texture, TextureView)>,
    blit: Option<(TextureFormat, CachedRenderPipelineId)>,
}
fn setup_pipeline(mut commands: Commands, shaders: Res<RayShaders>, cache: Res<PipelineCache>) {
    let layout = BindGroupLayoutDescriptor::new(
        "ray scene",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (
                storage_buffer_read_only::<Vec<GeometryTriangle>>(false),
                storage_buffer_read_only::<Vec<BvhNode>>(false),
                storage_buffer_read_only::<Vec<GpuInstance>>(false),
                storage_buffer_read_only::<Vec<BvhNode>>(false),
                uniform_buffer::<Params>(false),
                texture_storage_2d(TextureFormat::Rgba16Float, StorageTextureAccess::WriteOnly),
                storage_buffer_read_only::<Vec<TriangleSurface>>(false),
                texture_2d(TextureSampleType::Uint),
                storage_buffer::<Vec<u32>>(false),
                storage_buffer::<Vec<u32>>(false),
                texture_2d_array(TextureSampleType::Depth),
                uniform_buffer::<crate::ray_shadow_maps::ShadowMapParams>(false),
            ),
        ),
    );
    let visibility_layout = BindGroupLayoutDescriptor::new(
        "visibility scene",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::VERTEX,
            (
                storage_buffer_read_only::<Vec<GeometryTriangle>>(false),
                storage_buffer_read_only::<Vec<GpuInstance>>(false),
                uniform_buffer::<VisibilityParams>(false),
            ),
        ),
    );
    let visibility_pipeline = |entry: &str, format| {
        cache.queue_render_pipeline(RenderPipelineDescriptor {
            label: Some("raster primary visibility".into()),
            layout: vec![visibility_layout.clone()],
            vertex: VertexState {
                shader: shaders.visibility.clone(),
                entry_point: Some("visibility_vertex".into()),
                ..default()
            },
            fragment: Some(FragmentState {
                shader: shaders.visibility.clone(),
                entry_point: Some(entry.to_owned().into()),
                targets: vec![Some(ColorTargetState {
                    format,
                    blend: None,
                    write_mask: ColorWrites::ALL,
                })],
                ..default()
            }),
            primitive: PrimitiveState {
                cull_mode: None,
                ..default()
            },
            depth_stencil: Some(DepthStencilState {
                format: TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(CompareFunction::Greater),
                stencil: default(),
                bias: default(),
            }),
            ..default()
        })
    };
    let shadow_depth = cache.queue_render_pipeline(RenderPipelineDescriptor {
        label: Some("dynamic shadow depth".into()),
        layout: vec![visibility_layout.clone()],
        vertex: VertexState {
            shader: shaders.visibility.clone(),
            entry_point: Some("visibility_vertex".into()),
            ..default()
        },
        fragment: None,
        primitive: PrimitiveState {
            cull_mode: None,
            ..default()
        },
        depth_stencil: Some(DepthStencilState {
            format: TextureFormat::Depth32Float,
            depth_write_enabled: Some(true),
            depth_compare: Some(CompareFunction::Greater),
            stencil: default(),
            bias: default(),
        }),
        ..default()
    });
    let visibility = [
        visibility_pipeline("visibility_fragment_packed", TextureFormat::R32Uint),
        visibility_pipeline("visibility_fragment", TextureFormat::Rg32Uint),
    ];
    let compute_only = cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some("compute primary fallback".into()),
        layout: vec![layout.clone()],
        shader: shaders.compute_only.clone(),
        entry_point: Some("trace_frame".into()),
        ..default()
    });
    let blit_layout = BindGroupLayoutDescriptor::new(
        "ray output",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (texture_2d(TextureSampleType::Float { filterable: false }),),
        ),
    );
    let compute = cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some("aquarium ray tracing".into()),
        layout: vec![layout.clone()],
        shader: shaders.shader.clone(),
        entry_point: Some("trace_frame".into()),
        ..default()
    });
    commands.insert_resource(RayPipeline {
        shadow_depth,
        visibility_layout,
        visibility,
        compute_only,
        layout,
        blit_layout,
        compute,
    });
}
#[allow(clippy::too_many_arguments)] // Render-world resources and the current camera.
fn render(
    view: ViewQuery<(&ViewTarget, &ExtractedView)>,
    ready: Res<RayReady>,
    appearance: Res<crate::appearance::RenderAppearance>,
    water: Res<WaterAppearance>,
    scene: Res<RayScene>,
    pipeline: Option<Res<RayPipeline>>,
    cache: Res<PipelineCache>,
    shaders: Res<RayShaders>,
    fullscreen: Res<FullscreenShader>,
    queue: Res<RenderQueue>,
    device: Res<RenderDevice>,
    gpu_timing: Option<Res<crate::ray_stats::ComputeGpuTiming>>,
    mut frame: Local<FrameBuffers>,
    mut ctx: RenderContext,
) {
    let (target, view) = view.into_inner();
    let Some(pipeline) = pipeline else {
        clear_target(target, &mut ctx);
        return;
    };
    if scene.instances.is_empty() {
        clear_target(target, &mut ctx);
        return;
    }
    let Some(format) = target.out_texture_view_format() else {
        return;
    };
    if frame.blit.as_ref().is_none_or(|(f, _)| *f != format) {
        frame.blit = Some((
            format,
            cache.queue_render_pipeline(RenderPipelineDescriptor {
                label: Some("ray radiance transfer".into()),
                layout: vec![pipeline.blit_layout.clone()],
                vertex: fullscreen.to_vertex_state(),
                fragment: Some(FragmentState {
                    shader: shaders.blit.clone(),
                    targets: vec![Some(ColorTargetState {
                        format,
                        blend: None,
                        write_mask: ColorWrites::ALL,
                    })],
                    ..default()
                }),
                ..default()
            }),
        ));
    }
    let Some(blit) = cache.get_render_pipeline(frame.blit.unwrap().1) else {
        clear_target(target, &mut ctx);
        return;
    };
    let size = view.viewport.zw();
    if size.x == 0 || size.y == 0 {
        return;
    }
    let primary = PrimaryVisibility::select(
        size,
        scene.triangle_count,
        scene.instances.len(),
        device.limits().max_texture_dimension_2d,
    );
    let compute_id = if primary == PrimaryVisibility::Compute {
        pipeline.compute_only
    } else {
        pipeline.compute
    };
    let Some(compute) = cache.get_compute_pipeline(compute_id) else {
        clear_target(target, &mut ctx);
        return;
    };
    let visibility_pipeline = if primary == PrimaryVisibility::Compute {
        None
    } else {
        let index = usize::from(primary == PrimaryVisibility::Wide);
        let Some(raster) = cache.get_render_pipeline(pipeline.visibility[index]) else {
            clear_target(target, &mut ctx);
            return;
        };
        Some(raster)
    };
    if frame.output.as_ref().is_none_or(|(s, _, _)| *s != size) {
        let texture = device.create_texture(&TextureDescriptor {
            label: Some("ray radiance"),
            size: Extent3d {
                width: size.x,
                height: size.y,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba16Float,
            usage: TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&TextureViewDescriptor::default());
        frame.output = Some((size, texture, view));
    }
    if frame
        .visibility
        .as_ref()
        .is_none_or(|(s, m, _, _, _, _)| *s != size || *m != primary)
    {
        let extent = if primary == PrimaryVisibility::Compute {
            UVec2::ONE
        } else {
            size * 2
        };
        let make = |format, label, usage| {
            device.create_texture(&TextureDescriptor {
                label: Some(label),
                size: Extent3d {
                    width: extent.x,
                    height: extent.y,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let texture = make(
            primary.format(),
            "primary visibility",
            TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
        );
        let depth = make(
            TextureFormat::Depth32Float,
            "primary depth",
            TextureUsages::RENDER_ATTACHMENT | TextureUsages::TRANSIENT,
        );
        let texture_view = texture.create_view(&default());
        let depth_view = depth.create_view(&default());
        frame.visibility = Some((size, primary, texture, texture_view, depth, depth_view));
    }
    let triangle_bytes = u64::from(scene.triangle_count) * GeometryTriangle::min_size().get();
    let surface_bytes = u64::from(scene.surface_count) * TriangleSurface::min_size().get();
    let node_bytes = u64::from(scene.node_count) * BvhNode::min_size().get();
    let grow_triangles = ensure_buffer(
        &mut frame.triangles,
        triangle_bytes,
        "ray triangles",
        &device,
    );
    let grow_surfaces = ensure_buffer(
        &mut frame.surfaces,
        surface_bytes,
        "ray triangle surfaces",
        &device,
    );
    let grow_nodes = ensure_buffer(&mut frame.nodes, node_bytes, "ray nodes", &device);
    if let Some(timing) = &gpu_timing {
        timing.observe_scene(
            frame.triangles.as_ref().unwrap().size()
                + frame.surfaces.as_ref().unwrap().size()
                + frame.nodes.as_ref().unwrap().size(),
            size,
        );
    }
    if grow_triangles || grow_surfaces || grow_nodes {
        frame.chunks.clear();
    }
    for chunk in &scene.geometry {
        if frame.chunks.get(&chunk.triangle_offset) == Some(&chunk.revision) {
            continue;
        }
        let geometry: Vec<_> = chunk
            .triangles
            .iter()
            .zip(chunk.surface_indices.iter())
            .map(|(triangle, &index)| triangle.geometry(chunk.surface_offset + index))
            .collect();
        let mut encoded = encase::StorageBuffer::new(Vec::new());
        encoded.write(&geometry).expect("triangle shader layout");
        queue.write_buffer(
            frame.triangles.as_ref().unwrap(),
            u64::from(chunk.triangle_offset) * GeometryTriangle::min_size().get(),
            encoded.as_ref(),
        );
        let mut encoded = encase::StorageBuffer::new(Vec::new());
        encoded
            .write(chunk.surfaces.as_ref())
            .expect("triangle surface shader layout");
        queue.write_buffer(
            frame.surfaces.as_ref().unwrap(),
            u64::from(chunk.surface_offset) * TriangleSurface::min_size().get(),
            encoded.as_ref(),
        );
        let mut encoded = encase::StorageBuffer::new(Vec::new());
        encoded
            .write(chunk.nodes.as_ref())
            .expect("node shader layout");
        queue.write_buffer(
            frame.nodes.as_ref().unwrap(),
            u64::from(chunk.node_offset) * BvhNode::min_size().get(),
            encoded.as_ref(),
        );
        frame.chunks.insert(chunk.triangle_offset, chunk.revision);
    }
    frame.chunks.retain(|offset, _| {
        scene
            .geometry
            .iter()
            .any(|chunk| chunk.triangle_offset == *offset)
    });
    frame.instances.set(scene.instances.clone());
    frame.tlas.set(scene.tlas_nodes.clone());
    frame.instances.write_buffer(&device, &queue);
    frame.tlas.write_buffer(&device, &queue);
    let world_from_clip = view.world_from_view.to_matrix() * view.clip_from_view.inverse();
    let shadow_key = shadow_cache_key(scene.static_revision, size, world_from_clip);
    let cache_samples = static_cache_samples(size, device.limits().max_storage_buffer_binding_size);
    let shadow_bytes = u64::from(size.x) * u64::from(size.y) * u64::from(cache_samples) * 12;
    let shadow_cache_enabled = cache_samples != 0;
    if frame.shadow_key != Some(shadow_key) {
        // Cache as many original coverage samples as the adapter binding permits.
        let bytes = if shadow_cache_enabled {
            shadow_bytes
        } else {
            12
        };
        if frame
            .shadow_masks
            .as_ref()
            .is_none_or(|buffer| buffer.size() != bytes)
        {
            frame.shadow_masks = Some(device.create_buffer(&BufferDescriptor {
                label: Some("static shadow masks"),
                size: bytes,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        ctx.command_encoder()
            .clear_buffer(frame.shadow_masks.as_ref().unwrap(), 0, None);
        frame.shadow_key = Some(shadow_key);
    }
    let bounce_bytes =
        (u64::from(size.x) * u64::from(size.y) * u64::from(cache_samples) * 4).max(4);
    ensure_buffer(
        &mut frame.bounce_cache,
        bounce_bytes,
        "static bounce hits",
        &device,
    );
    let shadow_cpu_span = gpu_timing
        .as_ref()
        .map(|timing| timing.cpu_span("shadow_projection_upload"));
    let map_size = crate::ray_shadow_maps::RESOLUTION;
    let requested = *frame.shadow_requested.get_or_insert_with(|| {
        std::env::var("BEASTIE_SHADOW_MAP_CONTROL").as_deref() != Ok("trace")
    });
    let map_pipeline = cache.get_render_pipeline(pipeline.shadow_depth);
    let selection = crate::ray_shadow_maps::select(
        &device.limits(),
        size,
        requested,
        appearance.shadows() && shaders.shadows,
        map_pipeline.is_some(),
    );
    let maps_enabled = selection.enabled;
    let (map_params, dynamic_casters) = crate::ray_shadow_maps::fit(&scene, maps_enabled);
    let allocated_size = if maps_enabled { map_size } else { 1 };
    let allocated_layers = if maps_enabled { 12 } else { 1 };
    if frame
        .shadow_maps
        .as_ref()
        .is_none_or(|m| m.0 != allocated_size)
    {
        let texture = device.create_texture(&TextureDescriptor {
            label: Some("dynamic shadow depth array"),
            size: Extent3d {
                width: allocated_size,
                height: allocated_size,
                depth_or_array_layers: allocated_layers,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Depth32Float,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&TextureViewDescriptor {
            dimension: Some(TextureViewDimension::D2Array),
            aspect: TextureAspect::DepthOnly,
            ..default()
        });
        let layers = (0..allocated_layers)
            .map(|layer| {
                texture.create_view(&TextureViewDescriptor {
                    dimension: Some(TextureViewDimension::D2),
                    aspect: TextureAspect::DepthOnly,
                    base_array_layer: layer,
                    array_layer_count: Some(1),
                    ..default()
                })
            })
            .collect();
        frame.shadow_maps = Some((allocated_size, texture, view, layers));
    }
    for (camera, matrix) in frame
        .shadow_cameras
        .iter_mut()
        .zip(map_params.clip_from_world)
    {
        camera.set(VisibilityParams {
            clip_from_world: matrix,
        });
        camera.write_buffer(&device, &queue);
    }
    frame.shadow_map_params.set(map_params);
    frame.shadow_map_params.write_buffer(&device, &queue);
    drop(shadow_cpu_span);
    if let Some(timing) = &gpu_timing {
        timing.observe_shadow_maps(
            selection.enabled,
            selection.reason,
            u64::from(allocated_size).pow(2) * u64::from(allocated_layers) * 4,
            1744,
        );
    }
    if let Some(timing) = &gpu_timing {
        let pixels = u64::from(size.x) * u64::from(size.y);
        let visibility_bytes = if primary == PrimaryVisibility::Compute {
            8
        } else {
            pixels * 4 * if primary.packed() { 8 } else { 12 }
        };
        timing.observe_framebuffers(
            pixels * 8
                + visibility_bytes
                + frame.shadow_masks.as_ref().unwrap().size()
                + frame.bounce_cache.as_ref().unwrap().size()
                + u64::from(allocated_size).pow(2) * u64::from(allocated_layers) * 4
                + 1744,
        );
    }
    frame.params.set(Params {
        world_from_clip: view.world_from_view.to_matrix() * view.clip_from_view.inverse(),
        water: water.0,
        roots: UVec4::new(
            0,
            scene.world_root,
            scene.shadow_root,
            u32::from(primary.packed()),
        ),
        shadow_roots: UVec4::new(
            scene.static_shadow_root,
            scene.dynamic_shadow_root,
            cache_samples,
            0,
        ),
        cache_roots: UVec4::new(
            scene.static_world_root,
            scene.dynamic_world_root,
            0,
            u32::from(
                scene.triangle_count < (1 << 24)
                    && scene.instances.iter().filter(|i| i.pad1 != 0).count() <= 16,
            ),
        ),
        static_instances: std::array::from_fn(|i| {
            UVec4::from_array(scene.static_instances[i * 4..i * 4 + 4].try_into().unwrap())
        }),
        size: UVec4::new(
            size.x,
            size.y,
            scene.instances.len() as u32,
            u32::from(!appearance.shadows()),
        ),
    });
    frame.params.write_buffer(&device, &queue);
    frame.visibility_params.set(VisibilityParams {
        clip_from_world: view.clip_from_view * view.world_from_view.to_matrix().inverse(),
    });
    frame.visibility_params.write_buffer(&device, &queue);
    let output = &frame.output.as_ref().unwrap().2;
    let bind = device.create_bind_group(
        "ray scene",
        &cache.get_bind_group_layout(&pipeline.layout),
        &BindGroupEntries::sequential((
            frame.triangles.as_ref().unwrap().as_entire_binding(),
            frame.nodes.as_ref().unwrap().as_entire_binding(),
            frame.instances.binding().unwrap(),
            frame.tlas.binding().unwrap(),
            frame.params.binding().unwrap(),
            output,
            frame.surfaces.as_ref().unwrap().as_entire_binding(),
            &frame.visibility.as_ref().unwrap().3,
            frame.shadow_masks.as_ref().unwrap().as_entire_binding(),
            frame.bounce_cache.as_ref().unwrap().as_entire_binding(),
            &frame.shadow_maps.as_ref().unwrap().2,
            frame.shadow_map_params.binding().unwrap(),
        )),
    );
    let transfer = device.create_bind_group(
        "ray transfer",
        &cache.get_bind_group_layout(&pipeline.blit_layout),
        &BindGroupEntries::sequential((output,)),
    );
    let recorder = ctx.diagnostic_recorder();
    let recorder = recorder.as_deref();
    let span = recorder.time_span(ctx.command_encoder(), "ray_trace");
    if let Some(timing) = &gpu_timing {
        timing.resolve_completed(ctx.command_encoder(), queue.get_timestamp_period());
    }
    let timing_slot = gpu_timing.as_ref().and_then(|timing| timing.begin(&device));
    let raster_timing_slot = visibility_pipeline
        .and(gpu_timing.as_ref())
        .and_then(|timing| timing.begin_named(&device, "raster_visibility"));
    let map_timing_slot = maps_enabled
        .then(|| {
            gpu_timing
                .as_ref()
                .and_then(|t| t.begin_named(&device, "dynamic_shadow_maps"))
        })
        .flatten();
    if maps_enabled {
        for layer in 0..12 {
            let shadow_bind = device.create_bind_group(
                "dynamic shadow scene",
                &cache.get_bind_group_layout(&pipeline.visibility_layout),
                &BindGroupEntries::sequential((
                    frame.triangles.as_ref().unwrap().as_entire_binding(),
                    frame.instances.binding().unwrap(),
                    frame.shadow_cameras[layer].binding().unwrap(),
                )),
            );
            let mut pass = ctx
                .command_encoder()
                .begin_render_pass(&RenderPassDescriptor {
                    label: Some("dynamic shadow depth"),
                    color_attachments: &[],
                    depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                        view: &frame.shadow_maps.as_ref().unwrap().3[layer],
                        depth_ops: Some(Operations {
                            load: LoadOp::Clear(0.0),
                            store: StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: map_timing_slot
                        .as_ref()
                        .filter(|_| layer == 0 || layer == 11)
                        .map(|slot| wgpu::RenderPassTimestampWrites {
                            query_set: &slot.queries,
                            beginning_of_pass_write_index: (layer == 0).then_some(0),
                            end_of_pass_write_index: (layer == 11).then_some(1),
                        }),
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
            pass.set_pipeline(map_pipeline.unwrap());
            pass.set_bind_group(0, &shadow_bind, &[]);
            for &index in &dynamic_casters {
                let instance = &scene.instances[index];
                let chunk = scene
                    .geometry
                    .iter()
                    .find(|c| c.node_offset == instance.root)
                    .unwrap();
                pass.draw(
                    chunk.triangle_offset * 3
                        ..(chunk.triangle_offset + chunk.triangles.len() as u32) * 3,
                    index as u32..index as u32 + 1,
                );
            }
        }
    }
    if let (Some(timing), Some(slot)) = (&gpu_timing, map_timing_slot) {
        timing.finish(slot, ctx.command_encoder(), queue.get_timestamp_period());
    }
    if let Some(visibility_pipeline) = visibility_pipeline {
        let visibility_bind = device.create_bind_group(
            "visibility scene",
            &cache.get_bind_group_layout(&pipeline.visibility_layout),
            &BindGroupEntries::sequential((
                frame.triangles.as_ref().unwrap().as_entire_binding(),
                frame.instances.binding().unwrap(),
                frame.visibility_params.binding().unwrap(),
            )),
        );
        let mut pass = ctx
            .command_encoder()
            .begin_render_pass(&RenderPassDescriptor {
                label: Some("raster primary visibility"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &frame.visibility.as_ref().unwrap().3,
                    resolve_target: None,
                    depth_slice: None,
                    ops: Operations {
                        load: LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                    view: &frame.visibility.as_ref().unwrap().5,
                    depth_ops: Some(Operations {
                        load: LoadOp::Clear(0.0),
                        store: StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: raster_timing_slot.as_ref().map(|slot| {
                    wgpu::RenderPassTimestampWrites {
                        query_set: &slot.queries,
                        beginning_of_pass_write_index: Some(0),
                        end_of_pass_write_index: Some(1),
                    }
                }),
                occlusion_query_set: None,
                multiview_mask: None,
            });
        pass.set_pipeline(visibility_pipeline);
        pass.set_bind_group(0, &visibility_bind, &[]);
        for (index, instance) in scene.instances.iter().enumerate() {
            if let Some(chunk) = scene
                .geometry
                .iter()
                .find(|chunk| chunk.node_offset == instance.root)
            {
                pass.draw(
                    chunk.triangle_offset * 3
                        ..(chunk.triangle_offset + chunk.triangles.len() as u32) * 3,
                    index as u32..index as u32 + 1,
                );
            }
        }
    }
    {
        let mut pass = ctx
            .command_encoder()
            .begin_compute_pass(&ComputePassDescriptor {
                label: Some("trace aquarium"),
                timestamp_writes: timing_slot.as_ref().map(|slot| {
                    wgpu::ComputePassTimestampWrites {
                        query_set: &slot.queries,
                        beginning_of_pass_write_index: Some(0),
                        end_of_pass_write_index: Some(1),
                    }
                }),
            });
        pass.set_pipeline(compute);
        pass.set_bind_group(0, &bind, &[]);
        pass.dispatch_workgroups(size.x.div_ceil(8), size.y.div_ceil(8), 1);
    }
    if let (Some(timing), Some(slot)) = (&gpu_timing, raster_timing_slot) {
        timing.finish(slot, ctx.command_encoder(), queue.get_timestamp_period());
    }
    if let (Some(timing), Some(slot)) = (&gpu_timing, timing_slot) {
        timing.finish(slot, ctx.command_encoder(), queue.get_timestamp_period());
    }
    if let Some(attachment) = target.out_texture_color_attachment(Some(LinearRgba::BLACK)) {
        let mut pass = ctx
            .command_encoder()
            .begin_render_pass(&RenderPassDescriptor {
                label: Some("present traced aquarium"),
                color_attachments: &[Some(attachment)],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        pass.set_pipeline(blit);
        pass.set_bind_group(0, &transfer, &[]);
        pass.draw(0..3, 0..1);
    }
    span.end(ctx.command_encoder());
    ready.0.store(true, Ordering::Release);
}

pub(crate) fn shadow_cache_key(
    revision: u64,
    size: UVec2,
    world_from_clip: Mat4,
) -> (u64, UVec2, [u32; 16]) {
    (
        revision,
        size,
        world_from_clip.to_cols_array().map(f32::to_bits),
    )
}

fn static_cache_samples(size: UVec2, binding_limit: u64) -> u32 {
    let one_sample_bytes = u64::from(size.x) * u64::from(size.y) * 12;
    if one_sample_bytes == 0 {
        return 0;
    }
    (binding_limit / one_sample_bytes).min(2) as u32
}

#[test]
fn static_cache_reuses_one_sample_when_two_exceed_the_binding_limit() {
    let limit = 128 * 1024 * 1024;
    assert_eq!(static_cache_samples(UVec2::new(1920, 1080), limit), 2);
    assert_eq!(static_cache_samples(UVec2::new(3840, 2160), limit), 1);
    assert_eq!(static_cache_samples(UVec2::new(7680, 4320), limit), 0);
    assert_eq!(static_cache_samples(UVec2::ZERO, limit), 0);
    assert_eq!(static_cache_samples(UVec2::ONE, 23), 1);
    assert_eq!(static_cache_samples(UVec2::ONE, 24), 2);
}

fn ensure_buffer(
    buffer: &mut Option<Buffer>,
    size: u64,
    label: &'static str,
    device: &RenderDevice,
) -> bool {
    if buffer.as_ref().is_some_and(|b| b.size() >= size) {
        return false;
    }
    assert!(
        size <= device.limits().max_storage_buffer_binding_size,
        "ray scene exceeds this GPU storage binding limit"
    );
    *buffer = Some(device.create_buffer(&BufferDescriptor {
        label: Some(label),
        size: geometry_buffer_capacity(size, device.limits().max_storage_buffer_binding_size),
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    }));
    true
}

// Geometry arenas already reserve per-mesh power-of-two slots. A second byte
// power-of-two expansion can waste another 100%, especially after separating
// 48-byte geometry from 96-byte attributes. Modest slack still amortizes growth.
fn geometry_buffer_capacity(required: u64, limit: u64) -> u64 {
    assert!(required <= limit);
    required
        .saturating_add(required / 8)
        .max(256)
        .next_multiple_of(256)
        .min(limit)
}

#[cfg(test)]
mod capacity_tests {
    use super::geometry_buffer_capacity;

    #[test]
    fn geometry_capacity_covers_uploads_without_doubling_each_attribute_arena() {
        let limit = 128 * 1024 * 1024;
        for required in [
            0,
            1,
            255,
            256,
            257,
            749_520 * 48,
            749_520 * 96,
            limit - 1,
            limit,
        ] {
            let capacity = geometry_buffer_capacity(required, limit);
            assert!(capacity >= required && capacity <= limit);
            assert_eq!(capacity % 256, 0);
            assert!(capacity <= required + required / 8 + 256);
        }
        assert!(
            geometry_buffer_capacity(749_520 * 48, limit)
                + geometry_buffer_capacity(749_520 * 96, limit)
                < 128 * 1024 * 1024
        );
    }
}

fn clear_target(target: &ViewTarget, ctx: &mut RenderContext) {
    if let Some(attachment) =
        target.out_texture_color_attachment(Some(LinearRgba::new(0.038, 0.067, 0.072, 1.0)))
    {
        ctx.command_encoder()
            .begin_render_pass(&RenderPassDescriptor {
                label: Some("ray startup clear"),
                color_attachments: &[Some(attachment)],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
    }
}
