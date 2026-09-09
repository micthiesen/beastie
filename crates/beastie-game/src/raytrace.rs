//! Unified ordinary-compute rendering. The fullscreen draw only transfers computed radiance.
use crate::ray_scene::{
    BvhNode, GeometryTriangle, GpuInstance, RayScene, Triangle, TriangleSurface,
};
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
pub struct RayTracePlugin;
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
        let shader = app
            .world_mut()
            .resource_mut::<Assets<Shader>>()
            .add(Shader::from_wgsl(
                include_str!("raytrace.wgsl"),
                "embedded://beastie/raytrace.wgsl",
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
                .insert_resource(RayShaders { shader, blit })
                .add_systems(RenderStartup, setup_pipeline)
                .add_systems(RayTracing, render);
        }
    }
}
#[derive(Resource)]
struct RayShaders {
    shader: Handle<Shader>,
    blit: Handle<Shader>,
}
#[derive(Resource)]
struct RayPipeline {
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
}
#[derive(Default)]
struct FrameBuffers {
    triangles: Option<Buffer>,
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
            ),
        ),
    );
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
    let Some(compute) = cache.get_compute_pipeline(pipeline.compute) else {
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
    let triangle_bytes = u64::from(scene.triangle_count) * GeometryTriangle::min_size().get();
    let surface_bytes = u64::from(scene.triangle_count) * TriangleSurface::min_size().get();
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
        let geometry: Vec<_> = chunk.triangles.iter().map(Triangle::geometry).collect();
        let surfaces: Vec<_> = chunk.triangles.iter().map(Triangle::surface).collect();
        let mut encoded = encase::StorageBuffer::new(Vec::new());
        encoded.write(&geometry).expect("triangle shader layout");
        queue.write_buffer(
            frame.triangles.as_ref().unwrap(),
            u64::from(chunk.triangle_offset) * GeometryTriangle::min_size().get(),
            encoded.as_ref(),
        );
        let mut encoded = encase::StorageBuffer::new(Vec::new());
        encoded
            .write(&surfaces)
            .expect("triangle surface shader layout");
        queue.write_buffer(
            frame.surfaces.as_ref().unwrap(),
            u64::from(chunk.triangle_offset) * TriangleSurface::min_size().get(),
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
    frame.params.set(Params {
        world_from_clip: view.world_from_view.to_matrix() * view.clip_from_view.inverse(),
        water: water.0,
        roots: UVec4::new(0, scene.world_root, scene.shadow_root, 0),
        size: UVec4::new(
            size.x,
            size.y,
            scene.instances.len() as u32,
            u32::from(!appearance.shadows()),
        ),
    });
    frame.params.write_buffer(&device, &queue);
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
    let timing_slot = gpu_timing.as_ref().and_then(|timing| timing.begin(&device));
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
