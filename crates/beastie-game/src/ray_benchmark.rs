//! Explicit, surface-free GPU evidence. This freezes a real production scene and
//! measures all shadow/visibility/lighting passes, excluding scene updates and presentation.
use bevy::{
    asset::AssetApp,
    camera::visibility::VisibilityPlugin,
    prelude::*,
    render::render_resource::{ShaderType, encase},
};
use clap::ValueEnum;
use std::{path::Path, time::Duration};
use wgpu::util::DeviceExt;

use crate::{args::RenderProbe, ray_scene::RayScene};

fn dimensions() -> (u32, u32) {
    static SIZE: std::sync::OnceLock<(u32, u32)> = std::sync::OnceLock::new();
    *SIZE.get_or_init(|| match std::env::var("BEASTIE_BENCH_SIZE").as_deref() {
        Ok("low") => (640, 360),
        Ok("4k") => (3840, 2160),
        _ => (1920, 1080),
    })
}
fn width() -> u32 {
    dimensions().0
}
fn height() -> u32 {
    dimensions().1
}
const WARMUP: usize = 30;
const SAMPLES: usize = 180;
const BATCH_FRAMES: usize = 10;

#[derive(Clone, Copy, ShaderType)]
pub(super) struct BenchParams {
    pub world_from_clip: Mat4,
    pub size: UVec4,
    pub water: Vec4,
    pub roots: UVec4,
    pub shadow_roots: UVec4,
    pub cache_roots: UVec4,
    pub static_instances: [UVec4; 4],
}

/// Uses production scene generators, transform/visibility propagation and BLAS/TLAS
/// extraction. No substitute geometry or window/render plugin participates.
pub(super) fn scene_snapshot() -> (RayScene, BenchParams) {
    snapshot_from_app(&mut scene_app())
}

fn scene_app() -> App {
    let world = beastie_core::WorldState::new(42, "Mop");
    let plan = beastie_view::plan(&world, &beastie_view::ViewState::default()).0;
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        bevy::mesh::MeshPlugin,
        TransformPlugin,
        VisibilityPlugin,
    ))
    .init_asset::<StandardMaterial>()
    .init_resource::<crate::creature::CreatureMotion>()
    .init_resource::<crate::appearance::RenderAppearance>()
    .insert_resource(crate::renderer::SceneFrame { plan })
    .add_plugins((
        crate::renderer::RendererPlugin,
        crate::ray_scene::RayScenePlugin,
    ));
    app.finish();
    app.cleanup();
    // Startup deferred entities, then stable transforms and extraction.
    app.update();
    app.update();
    app
}

fn snapshot_from_app(app: &mut App) -> (RayScene, BenchParams) {
    let plan = &app.world().resource::<crate::renderer::SceneFrame>().plan;
    let water = Vec4::new(
        (plan.elapsed_ms.saturating_add(plan.simulation_remainder_ms) as f64 / 1000.0) as f32,
        if plan.reduced_flashes { 0.45 } else { 1.0 },
        0.0,
        0.0,
    );
    let scene = app.world().resource::<RayScene>().clone();
    assert!(!scene.instances.is_empty(), "production scene extraction");
    let mut cameras = app
        .world_mut()
        .query_filtered::<(&Projection, &GlobalTransform), With<Camera3d>>();
    let (projection, transform) = cameras.single(app.world()).expect("production tank camera");
    let mut projection = projection.clone();
    projection.update(width() as f32, height() as f32);
    let params = BenchParams {
        world_from_clip: transform.to_matrix() * projection.get_clip_from_view().inverse(),
        size: UVec4::new(width(), height(), scene.instances.len() as u32, 0),
        water,
        roots: UVec4::new(0, scene.world_root, scene.shadow_root, 1),
        shadow_roots: UVec4::new(scene.static_shadow_root, scene.dynamic_shadow_root, 2, 0),
        cache_roots: UVec4::new(scene.static_world_root, scene.dynamic_world_root, 0, 1),
        static_instances: std::array::from_fn(|i| {
            UVec4::from_array(scene.static_instances[i * 4..i * 4 + 4].try_into().unwrap())
        }),
    };
    (scene, params)
}

pub(super) fn encoded<T: ShaderType + encase::internal::WriteInto>(value: &T) -> Vec<u8> {
    let mut encoded = encase::StorageBuffer::new(Vec::new());
    encoded.write(value).expect("production shader layout");
    encoded.into_inner()
}

#[test]
fn headless_snapshot_contains_real_world_and_overlay_geometry() {
    let (scene, params) = scene_snapshot();
    assert!(
        scene
            .geometry
            .iter()
            .map(|chunk| chunk.triangles.len())
            .sum::<usize>()
            > 10_000
    );
    assert!(
        scene
            .instances
            .iter()
            .any(|instance| instance.material.w > 0.5)
    );
    assert!(
        scene
            .instances
            .iter()
            .any(|instance| instance.material.w < 0.5)
    );
    assert_ne!(scene.world_root, u32::MAX);
    assert_ne!(scene.shadow_root, u32::MAX);
    assert!(params.world_from_clip.is_finite());
    assert_eq!(encoded(&params).len(), 208);
}

#[test]
#[ignore = "explicit real GPU benchmark; requires BEASTIE_BENCH_OUTPUT, no display required"]
fn offscreen_renderer_benchmark() {
    let output = std::env::var("BEASTIE_BENCH_OUTPUT").expect("set BEASTIE_BENCH_OUTPUT JSON path");
    let probe = RenderProbe::from_str(
        &std::env::var("BEASTIE_BENCH_PROBE").unwrap_or_else(|_| "full".into()),
        false,
    )
    .expect("valid BEASTIE_BENCH_PROBE");
    let (scene, params) = scene_snapshot();
    bevy::tasks::block_on(benchmark(&scene, params, probe, Path::new(&output)));
}

async fn benchmark(
    scene: &RayScene,
    mut params: BenchParams,
    probe: RenderProbe,
    output_path: &Path,
) {
    let instance = wgpu::Instance::default();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions::default())
        .await
        .expect("GPU adapter");
    let info = adapter.get_info();
    if !adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
        std::fs::write(output_path, serde_json::to_vec_pretty(&serde_json::json!({
            "schema_version": 1, "available": false, "reason": "GPU timestamp queries unavailable",
            "adapter": info.name, "backend": format!("{:?}", info.backend), "render_probe": probe,
        })).unwrap()).unwrap();
        return;
    }
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("Beastie offscreen benchmark"),
            required_features: wgpu::Features::TIMESTAMP_QUERY,
            ..Default::default()
        })
        .await
        .expect("timestamp-enabled ordinary compute device");
    device.set_device_lost_callback(|reason, message| {
        eprintln!("shadowmap device lost: {reason:?}: {message}")
    });
    let cache_samples = crate::raytrace::static_cache_samples(
        UVec2::new(width(), height()),
        device.limits().max_storage_buffer_binding_size,
    );
    params.shadow_roots.z = cache_samples;
    let mut geometry = vec![0u8; scene.triangle_count as usize * 48];
    let mut surfaces = vec![0u8; scene.surface_count as usize * 96];
    let mut nodes = vec![0u8; scene.node_count as usize * 32];
    for chunk in &scene.geometry {
        let triangles: Vec<_> = chunk
            .triangles
            .iter()
            .zip(chunk.surface_indices.iter())
            .map(|(triangle, &index)| triangle.geometry(chunk.surface_offset + index))
            .collect();
        let attributes = chunk.surfaces.as_ref();
        let bytes = encoded(&triangles);
        let offset = chunk.triangle_offset as usize * 48;
        geometry[offset..offset + bytes.len()].copy_from_slice(&bytes);
        let bytes = encoded(attributes);
        let offset = chunk.surface_offset as usize * 96;
        surfaces[offset..offset + bytes.len()].copy_from_slice(&bytes);
        let bytes = encoded(chunk.nodes.as_ref());
        let offset = chunk.node_offset as usize * 32;
        nodes[offset..offset + bytes.len()].copy_from_slice(&bytes);
    }
    let data = [
        geometry,
        nodes,
        encoded(&scene.instances),
        encoded(&scene.tlas_nodes),
        encoded(&params),
        surfaces,
    ];
    let geometry_bytes: usize = data
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != 4)
        .map(|(_, bytes)| bytes.len())
        .sum();
    let buffers: Vec<_> = data
        .iter()
        .enumerate()
        .map(|(i, bytes)| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("production scene buffer"),
                contents: bytes,
                usage: if i == 4 {
                    wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST
                } else {
                    wgpu::BufferUsages::STORAGE
                },
            })
        })
        .collect();
    let shadow_cache_bytes =
        (u64::from(width()) * u64::from(height()) * u64::from(cache_samples) * 12).max(4);
    let shadow_cache = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("stationary visibility records"),
        size: shadow_cache_bytes,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("offscreen production radiance"),
        size: wgpu::Extent3d {
            width: width(),
            height: height(),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    let visibility_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("offscreen raster visibility"),
        size: wgpu::Extent3d {
            width: width() * 2,
            height: height() * 2,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::R32Uint,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("offscreen raster depth"),
        size: visibility_texture.size(),
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TRANSIENT,
        view_formats: &[],
    });
    let visibility_view = visibility_texture.create_view(&Default::default());
    let depth_view = depth_texture.create_view(&Default::default());
    let visibility_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("visibility"),
        source: wgpu::ShaderSource::Wgsl(include_str!("ray_visibility.wgsl").into()),
    });
    let visibility_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("raster primary visibility"),
        layout: None,
        vertex: wgpu::VertexState {
            module: &visibility_shader,
            entry_point: Some("visibility_vertex"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &visibility_shader,
            entry_point: Some("visibility_fragment_packed"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::R32Uint,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Greater),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });
    let visibility_params = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("visibility camera"),
        contents: &encoded(&params.world_from_clip.inverse()),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let visibility_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &visibility_pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: buffers[0].as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: buffers[2].as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: visibility_params.as_entire_binding(),
            },
        ],
    });
    let density =
        crate::ray_shadow_maps::density(UVec2::new(width(), height())).unwrap_or((1024, 512));
    let shadow_resolution = density.0;
    let map_selection = crate::ray_shadow_maps::select(
        &device.limits(),
        UVec2::new(width(), height()),
        std::env::var("BEASTIE_SHADOW_MAP_CONTROL").as_deref() != Ok("trace"),
        !matches!(probe, RenderProbe::NoShadows | RenderProbe::PrimaryOnly),
        true,
    );
    let shadow_map_enabled = map_selection.enabled;
    let atlas = true;
    let (mut map_params, shadow_casters) =
        crate::ray_shadow_maps::fit(scene, shadow_map_enabled, shadow_resolution);
    map_params.info.z = u32::from(atlas);
    let map_uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("dynamic shadow projections"),
        contents: &encoded(&map_params),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });
    let map_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("twelve dynamic shadow directions"),
        size: wgpu::Extent3d {
            width: if shadow_map_enabled {
                shadow_resolution * if atlas { 4 } else { 1 }
            } else {
                1
            },
            height: if shadow_map_enabled {
                shadow_resolution * if atlas { 3 } else { 1 }
            } else {
                1
            },
            depth_or_array_layers: if atlas { 1 } else { 12 },
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: crate::ray_static_maps::DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let map_view = map_texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        aspect: wgpu::TextureAspect::DepthOnly,
        ..Default::default()
    });
    let map_layer_views: Vec<_> = (0..12)
        .map(|index| {
            map_texture.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2),
                aspect: wgpu::TextureAspect::DepthOnly,
                base_array_layer: if atlas { 0 } else { index },
                array_layer_count: Some(1),
                ..Default::default()
            })
        })
        .collect();
    let indexed = shadow_map_enabled;
    let shadow_index = indexed.then(|| crate::ray_shadow_index::ShadowIndex::new(&device, scene));
    let indexed_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("indexed shadow"),
        source: wgpu::ShaderSource::Wgsl(include_str!("ray_shadow_index.wgsl").into()),
    });
    let map_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("exact dynamic caster depth"),
        layout: None,
        vertex: wgpu::VertexState {
            module: if indexed {
                &indexed_shader
            } else {
                &visibility_shader
            },
            entry_point: Some(if indexed {
                "shadow_vertex"
            } else {
                "visibility_vertex"
            }),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: None,
        primitive: wgpu::PrimitiveState {
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: crate::ray_static_maps::DEPTH_FORMAT,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Greater),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });
    let original_map_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("exact dynamic caster depth"),
        layout: None,
        vertex: wgpu::VertexState {
            module: &visibility_shader,
            entry_point: Some("visibility_vertex"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: None,
        primitive: wgpu::PrimitiveState {
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: crate::ray_static_maps::DEPTH_FORMAT,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Greater),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });
    let map_matrices: Vec<_> = map_params
        .clip_from_world
        .iter()
        .map(|matrix| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("one shadow projection"),
                contents: &encoded(matrix),
                usage: wgpu::BufferUsages::UNIFORM,
            })
        })
        .collect();
    let map_binds: Vec<_> = map_matrices
        .iter()
        .map(|matrix| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &map_pipeline.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: shadow_index
                            .as_ref()
                            .map_or(&buffers[0], |g| &g.positions)
                            .as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: buffers[2].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: matrix.as_entire_binding(),
                    },
                ],
            })
        })
        .collect();
    let static_resolution = density.1;
    let static_enabled = shadow_map_enabled;
    let static_maps =
        crate::ray_static_maps::StaticMaps::new(&device, scene, static_enabled, static_resolution);
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("production trace_frame"),
        source: wgpu::ShaderSource::Wgsl(
            crate::raytrace::probe_shader_source(probe)
                .replace(
                    "const CACHE_BOUNCE:bool=true;",
                    if std::env::var("BEASTIE_BENCH_BOUNCE_CACHE").as_deref() == Ok("off") {
                        "const CACHE_BOUNCE:bool=false;"
                    } else {
                        "const CACHE_BOUNCE:bool=true;"
                    },
                )
                .into(),
        ),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("production trace_frame"),
        layout: None,
        module: &shader,
        entry_point: Some("trace_frame"),
        compilation_options: Default::default(),
        cache: None,
    });
    let bindings = [0, 1, 2, 3, 4, 6];
    let mut entries: Vec<_> = buffers
        .iter()
        .zip(bindings)
        .map(|(buffer, binding)| wgpu::BindGroupEntry {
            binding,
            resource: buffer.as_entire_binding(),
        })
        .collect();
    entries.push(wgpu::BindGroupEntry {
        binding: 5,
        resource: wgpu::BindingResource::TextureView(&view),
    });
    entries.push(wgpu::BindGroupEntry {
        binding: 8,
        resource: shadow_cache.as_entire_binding(),
    });
    entries.push(wgpu::BindGroupEntry {
        binding: 7,
        resource: wgpu::BindingResource::TextureView(&visibility_view),
    });
    let bounce_cache = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: (u64::from(width()) * u64::from(height()) * u64::from(cache_samples) * 4).max(4),
        usage: wgpu::BufferUsages::STORAGE,
        mapped_at_creation: false,
    });
    entries.push(wgpu::BindGroupEntry {
        binding: 9,
        resource: bounce_cache.as_entire_binding(),
    });
    entries.push(wgpu::BindGroupEntry {
        binding: 10,
        resource: wgpu::BindingResource::TextureView(&map_view),
    });
    entries.push(wgpu::BindGroupEntry {
        binding: 11,
        resource: map_uniform.as_entire_binding(),
    });
    entries.push(wgpu::BindGroupEntry {
        binding: 12,
        resource: wgpu::BindingResource::TextureView(&static_maps.view),
    });
    entries.push(wgpu::BindGroupEntry {
        binding: 13,
        resource: static_maps.uniform.as_entire_binding(),
    });
    let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &entries,
    });
    let mut first_static_fill = true;
    let measurements = measure_frames(&device, &queue, |encoder, queries, query_index| {
        let filled_static = first_static_fill && static_maps.enabled;
        if filled_static {
            static_maps.record(
                &device,
                encoder,
                &original_map_pipeline,
                scene,
                &buffers[0],
                &buffers[2],
                Some((queries, query_index)),
            );
        }
        first_static_fill = false;
        if shadow_map_enabled {
            for (outer, layer_view) in
                map_layer_views
                    .iter()
                    .enumerate()
                    .take(if atlas { 1 } else { 12 })
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("dynamic shadow depth"),
                    color_attachments: &[],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: layer_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(0.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: if outer == 0 && !filled_static {
                        Some(wgpu::RenderPassTimestampWrites {
                            query_set: queries,
                            beginning_of_pass_write_index: Some(query_index),
                            end_of_pass_write_index: None,
                        })
                    } else {
                        None
                    },
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_pipeline(&map_pipeline);
                for layer in if atlas { 0..12 } else { outer..outer + 1 } {
                    if atlas {
                        let x = (layer as u32 % 4) * shadow_resolution;
                        let y = (layer as u32 / 4) * shadow_resolution;
                        pass.set_viewport(
                            x as f32,
                            y as f32,
                            shadow_resolution as f32,
                            shadow_resolution as f32,
                            0.0,
                            1.0,
                        );
                        pass.set_scissor_rect(x, y, shadow_resolution, shadow_resolution);
                    }
                    pass.set_bind_group(0, &map_binds[layer], &[]);
                    for &index in &shadow_casters {
                        let instance = scene.instances[index];
                        let chunk = scene
                            .geometry
                            .iter()
                            .find(|chunk| chunk.node_offset == instance.root)
                            .expect("shadow mesh range");
                        if let Some(geometry) = &shadow_index {
                            pass.set_index_buffer(
                                geometry.indices.slice(..),
                                wgpu::IndexFormat::Uint32,
                            );
                            pass.draw_indexed(
                                geometry.range_for(&instance),
                                0,
                                index as u32..index as u32 + 1,
                            );
                        } else {
                            pass.draw(
                                chunk.triangle_offset * 3
                                    ..(chunk.triangle_offset + chunk.triangles.len() as u32) * 3,
                                index as u32..index as u32 + 1,
                            );
                        }
                    }
                }
            }
        }
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("raster visibility"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &visibility_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: if shadow_map_enabled {
                    None
                } else {
                    Some(wgpu::RenderPassTimestampWrites {
                        query_set: queries,
                        beginning_of_pass_write_index: Some(query_index),
                        end_of_pass_write_index: None,
                    })
                },
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&visibility_pipeline);
            pass.set_bind_group(0, &visibility_bind, &[]);
            for (index, instance) in scene.instances.iter().enumerate() {
                let chunk = scene
                    .geometry
                    .iter()
                    .find(|chunk| chunk.node_offset == instance.root)
                    .expect("mesh range");
                pass.draw(
                    chunk.triangle_offset * 3
                        ..(chunk.triangle_offset + chunk.triangles.len() as u32) * 3,
                    index as u32..index as u32 + 1,
                );
            }
        }
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("offscreen trace_frame"),
            timestamp_writes: Some(wgpu::ComputePassTimestampWrites {
                query_set: queries,
                beginning_of_pass_write_index: None,
                end_of_pass_write_index: Some(query_index + 1),
            }),
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind, &[]);
        pass.dispatch_workgroups(width().div_ceil(8), height().div_ceil(8), 1);
    });
    let mut timings: Vec<_> = measurements
        .gpu_ms
        .iter()
        .skip(WARMUP)
        .flatten()
        .copied()
        .collect();
    let cold_gpu_ms = measurements.gpu_ms[0];
    timings.sort_by(f64::total_cmp);
    let summary = summarize_timings(&measurements);
    if let Ok(png_path) = std::env::var("BEASTIE_BENCH_PNG") {
        save_radiance_png(&device, &queue, &texture, Path::new(&png_path));
    }
    // Compare the warmed cache against the original combined shadow tree on the
    // exact same frozen scene and pipeline. No traversal-order/asset-order drift.
    let cached_pixels = radiance_bytes(&device, &queue, &texture);
    let mut reference_static = static_maps.params.clone();
    reference_static.info.x = 0;
    queue.write_buffer(&static_maps.uniform, 0, &encoded(&reference_static));
    let mut reference_maps = map_params.clone();
    reference_maps.info.x = 0;
    queue.write_buffer(&map_uniform, 0, &encoded(&reference_maps));
    let mut reference_params = params;
    reference_params.shadow_roots.z = 0;
    queue.write_buffer(&buffers[4], 0, &encoded(&reference_params));
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind, &[]);
        pass.dispatch_workgroups(width().div_ceil(8), height().div_ceil(8), 1);
    }
    queue.submit([encoder.finish()]);
    let reference_pixels = radiance_bytes(&device, &queue, &texture);
    let mut different_pixels = 0u64;
    let mut maximum_linear_difference = 0.0f32;
    let mut sum_linear_difference = 0.0f64;
    for (cached, reference) in cached_pixels
        .as_chunks::<8>()
        .0
        .iter()
        .zip(reference_pixels.as_chunks::<8>().0.iter())
    {
        different_pixels += u64::from(cached[..6] != reference[..6]);
        for channel in 0..3 {
            let at = channel * 2;
            let a = positive_half(u16::from_le_bytes([cached[at], cached[at + 1]]));
            let b = positive_half(u16::from_le_bytes([reference[at], reference[at + 1]]));
            let difference = (a - b).abs();
            maximum_linear_difference = maximum_linear_difference.max(difference);
            sum_linear_difference += f64::from(difference);
        }
    }
    if let Ok(path) = std::env::var("BEASTIE_BENCH_REFERENCE_PNG") {
        save_radiance_png(&device, &queue, &texture, Path::new(&path));
    }
    let report = serde_json::json!({
        "visibility_bytes": u64::from(width())*u64::from(height())*4*8, "schema_version": 4, "measurement": "Surface-free frozen production scene with post-completion timestamp resolve. Isolated pass latency is per submitted frame; batched pass latencies may overlap. Bounded batches contain at most10 frames. Sum of per-batch GPU envelopes/frame excludes inter-batch CPU/readback gaps and includes batch fill/drain. Warm completion intervals exclude all batch boundaries and warmup. Global timing_validation.gpu_envelope_ms includes CPU/readback gaps and is not GPU frame cost. All active shadow depth, primary visibility and lighting passes are included. Excludes CPU scene updates, uploads and presentation.",
        "available": summary.isolated_pass_latency.as_ref().map_or(!timings.is_empty(), |distribution| distribution.samples > 0), "adapter": info.name, "backend": format!("{:?}", info.backend),
        "render_probe": probe, "viewport_pixels": [width(), height()], "warmup_dispatches": WARMUP, "requested_samples": SAMPLES, "pass_latency_samples": timings.len(),
        "gpu_timing_summary": summary,
        "source_state": {"world_seed": 42, "creature_name": "Mop", "view": "ViewState::default (Compose)", "updates": 2, "water": params.water.to_array(), "world_from_clip": params.world_from_clip.to_cols_array()},
        "scene": {"instances": scene.instances.len(), "live_triangles": scene.geometry.iter().map(|chunk| chunk.triangles.len()).sum::<usize>(), "triangle_arena_capacity": scene.triangle_count, "blas_node_arena_capacity": scene.node_count, "tlas_nodes": scene.tlas_nodes.len(), "gpu_geometry_bytes": geometry_bytes},
        "shadow_depth_bits":crate::ray_static_maps::DEPTH_BYTES*8,
        "shadow_atlas": {"enabled":shadow_map_enabled,"columns":4,"rows":3,"passes":u32::from(shadow_map_enabled)},
        "indexed_shadows": {"enabled":indexed,"unique_vertices":shadow_index.as_ref().map(|g|g.vertices),"triangles":shadow_index.as_ref().map(|g|g.triangles),"allocated_bytes":shadow_index.as_ref().map(|g|g.bytes)},
        "persistent_static_maps":{"enabled":static_maps.enabled,"resolution":static_maps.resolution,"caster_count":static_maps.caster_count,"texture_bytes":static_maps.bytes(),"initial_fill_in_first_frame":static_maps.enabled,"receiver_mode":"dynamic-only"},
        "dynamic_shadow_maps": {"enabled":shadow_map_enabled,"reason":map_selection.reason,"resolution":shadow_resolution,"layers":1,"directions":12,"caster_instances":shadow_casters.len(),
            "depth_bytes":if shadow_map_enabled {u64::from(shadow_resolution)*u64::from(shadow_resolution)*12*crate::ray_static_maps::DEPTH_BYTES} else {crate::ray_static_maps::DEPTH_BYTES},"uniform_bytes":encoded(&map_params).len()+12*64,
            "active_depth_passes":u32::from(shadow_map_enabled),"measurement_includes_all_active_depth_passes":true,"reference":"same shader,original shadow rays,static caches disabled"},
        "gpu_pass_latency_samples_ms_sorted": timings,
        "timing_validation": measurements,
        "bounce_cache": {"allocated_bytes": bounce_cache.size()},
        "shadow_cache": {"allocated_bytes": shadow_cache_bytes,
            "cold_first_dispatch_gpu_ms": if measurements.submission_mode == "isolated" && !measurements.gpu_duration_exceeds_submission_wall_indices.contains(&0) { cold_gpu_ms } else { None },
            "cold_first_dispatch_pass_latency_ms": cold_gpu_ms,
            "records_per_pixel": cache_samples, "bytes_per_record": 12,
            "static_instances": scene.instances.iter().filter(|instance| instance.pad1 != 0).count(),
            "reference_comparison": {"different_rgb_pixels": different_pixels,
                "total_pixels": width() * height(), "maximum_linear_channel_difference": maximum_linear_difference,
                "mean_absolute_linear_channel_difference": sum_linear_difference / f64::from(width() * height() * 3)}},
    });
    std::fs::write(output_path, serde_json::to_vec_pretty(&report).unwrap())
        .expect("benchmark JSON");
    if measurements.submission_mode == "isolated" {
        eprintln!(
            "offscreen {probe:?}: isolated pass latency p50 {:?} ms, {} valid samples -> {}",
            summary
                .isolated_pass_latency
                .as_ref()
                .and_then(|d| d.p50_ms),
            summary
                .isolated_pass_latency
                .as_ref()
                .map_or(0, |distribution| distribution.samples),
            output_path.display()
        );
    } else {
        eprintln!(
            "offscreen {probe:?}: batched GPU envelope/frame {:?} ms ({} dispatched, including warmup); post-warmup completion cadence p50 {:?} ms, {} valid intervals -> {}",
            summary.batched_gpu_envelope_per_dispatched_frame_ms,
            summary.total_dispatched_frames,
            summary
                .batched_post_warmup_completion_intervals
                .as_ref()
                .and_then(|d| d.p50_ms),
            summary
                .batched_post_warmup_completion_intervals
                .as_ref()
                .map_or(0, |d| d.samples),
            output_path.display()
        );
    }
}

fn mapped_bytes(device: &wgpu::Device, buffer: &wgpu::Buffer) -> Vec<u8> {
    let (sender, receiver) = std::sync::mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
    device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(Duration::from_secs(60)),
        })
        .expect("bounded GPU completion");
    receiver
        .recv_timeout(Duration::from_secs(60))
        .expect("GPU readback callback")
        .expect("GPU mapping");
    let bytes = buffer.slice(..).get_mapped_range().to_vec();
    buffer.unmap();
    bytes
}

// The production blit transfers linear radiance to an sRGB target. Match that
// transfer for the optional evidence image; this copy is outside timed passes.
fn save_radiance_png(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    path: &Path,
) {
    let bytes = radiance_bytes(device, queue, texture);
    let mut image = image::RgbaImage::new(width(), height());
    for (pixel, source) in image.pixels_mut().zip(bytes.as_chunks::<8>().0.iter()) {
        for channel in 0..3 {
            let half = u16::from_le_bytes([source[channel * 2], source[channel * 2 + 1]]);
            let linear = positive_half(half).clamp(0.0, 1.0);
            let srgb = if linear <= 0.0031308 {
                linear * 12.92
            } else {
                1.055 * linear.powf(1.0 / 2.4) - 0.055
            };
            pixel[channel] = (srgb * 255.0).round() as u8;
        }
        pixel[3] = 255;
    }
    image.save(path).expect("offscreen radiance PNG");
}

fn radiance_bytes(device: &wgpu::Device, queue: &wgpu::Queue, texture: &wgpu::Texture) -> Vec<u8> {
    let bytes_per_row = width() * 8;
    assert_eq!(bytes_per_row % wgpu::COPY_BYTES_PER_ROW_ALIGNMENT, 0);
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("offscreen PNG readback"),
        size: u64::from(bytes_per_row) * u64::from(height()),
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(height()),
            },
        },
        texture.size(),
    );
    queue.submit([encoder.finish()]);
    mapped_bytes(device, &readback)
}

fn positive_half(bits: u16) -> f32 {
    // Production output is clamped to [0,1], so sign and nonfinite encodings
    // indicate invalid output rather than a color to silently reinterpret.
    assert_eq!(bits & 0x8000, 0, "negative output radiance");
    let exponent = (bits >> 10) & 31;
    let mantissa = bits & 1023;
    assert!(exponent < 31, "nonfinite output radiance");
    if exponent == 0 {
        f32::from(mantissa) * 2.0_f32.powi(-24)
    } else {
        (1.0 + f32::from(mantissa) / 1024.0) * 2.0_f32.powi(i32::from(exponent) - 15)
    }
}

#[test]
fn evidence_half_conversion_preserves_linear_radiance() {
    assert_eq!(positive_half(0), 0.0);
    assert_eq!(positive_half(0x3c00), 1.0);
    assert_eq!(positive_half(0x3800), 0.5);
    assert_eq!(positive_half(1), 2.0_f32.powi(-24));
}

/// Common measurement path for compute, raster+compute and cache experiments.
/// Isolated submissions prevent future-frame stage overlap from masquerading as
/// per-frame GPU cost. Batched mode exists only to diagnose overlap/throughput.
pub(super) fn measure_frames(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    mut record_frame: impl FnMut(&mut wgpu::CommandEncoder, &wgpu::QuerySet, u32),
) -> FrameTimings {
    let batched = match std::env::var("BEASTIE_BENCH_SUBMISSION").as_deref() {
        Ok("isolated") => false,
        Err(_) | Ok("batched") => true,
        Ok(other) => panic!("invalid BEASTIE_BENCH_SUBMISSION {other}; use isolated or batched"),
    };
    let frame_count = WARMUP + SAMPLES;
    let queries = device.create_query_set(&wgpu::QuerySetDescriptor {
        label: Some("offscreen frame boundaries"),
        ty: wgpu::QueryType::Timestamp,
        count: (frame_count * 2) as u32,
    });
    let bytes = (frame_count * 16) as u64;
    let resolve = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: bytes,
        usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: bytes,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut raw_pairs = Vec::with_capacity(frame_count);
    let mut submission_wall_ms = Vec::new();
    let test_start = std::time::Instant::now();
    if batched {
        // Bound in-flight depth work after the full210-frame submission produced
        // a Metal BufferAsyncError. Controls use these identical batch boundaries.

        for first in (0..frame_count).step_by(BATCH_FRAMES) {
            let count = (frame_count - first).min(BATCH_FRAMES);
            let mut encoder = device.create_command_encoder(&Default::default());
            for local in 0..count {
                record_frame(&mut encoder, &queries, (local * 2) as u32);
            }
            let start = std::time::Instant::now();
            queue.submit([encoder.finish()]);
            wait_gpu(device);
            submission_wall_ms.push(start.elapsed().as_secs_f64() * 1000.0);
            let mut encoder = device.create_command_encoder(&Default::default());
            encoder.resolve_query_set(&queries, 0..(count * 2) as u32, &resolve, 0);
            encoder.copy_buffer_to_buffer(&resolve, 0, &readback, 0, (count * 16) as u64);
            queue.submit([encoder.finish()]);
            let values = mapped_bytes(device, &readback);
            raw_pairs.extend(decode_pairs(&values[..count * 16]));
        }
    } else {
        for _ in 0..frame_count {
            let mut encoder = device.create_command_encoder(&Default::default());
            record_frame(&mut encoder, &queries, 0);
            let start = std::time::Instant::now();
            queue.submit([encoder.finish()]);
            wait_gpu(device);
            submission_wall_ms.push(start.elapsed().as_secs_f64() * 1000.0);
            let mut encoder = device.create_command_encoder(&Default::default());
            encoder.resolve_query_set(&queries, 0..2, &resolve, 0);
            encoder.copy_buffer_to_buffer(&resolve, 0, &readback, 0, 16);
            queue.submit([encoder.finish()]);
            let bytes = mapped_bytes(device, &readback);
            raw_pairs.push(decode_pairs(&bytes[..16])[0]);
        }
    }
    analyze_timestamps(
        raw_pairs,
        submission_wall_ms,
        f64::from(queue.get_timestamp_period()),
        batched,
        test_start.elapsed().as_secs_f64() * 1000.0,
    )
}

fn decode_pairs(bytes: &[u8]) -> Vec<[u64; 2]> {
    bytes
        .as_chunks::<16>()
        .0
        .iter()
        .map(|pair| {
            [
                u64::from_le_bytes(pair[..8].try_into().unwrap()),
                u64::from_le_bytes(pair[8..].try_into().unwrap()),
            ]
        })
        .collect()
}

#[derive(serde::Serialize)]
struct BatchEnvelope {
    first_frame: usize,
    frames: usize,
    warmup: bool,
    raw_gpu_envelope_ms: Option<f64>,
    completed_submission_wall_ms: Option<f64>,
    invalid_reason: Option<&'static str>,
    gpu_envelope_ms: Option<f64>,
}

#[derive(serde::Serialize)]
pub(super) struct FrameTimings {
    submission_mode: &'static str,
    timestamp_period_ns: f64,
    raw_timestamp_pairs: Vec<[u64; 2]>,
    /// In frame order, including warmup. Invalid durations are null, not zero.
    pub gpu_ms: Vec<Option<f64>>,
    invalid_samples: Vec<serde_json::Value>,
    overlapping_frame_indices: Vec<usize>,
    gpu_duration_exceeds_submission_wall_indices: Vec<usize>,
    /// Frame submission through GPU completion, excluding query readback/setup.
    submission_wall_ms: Vec<f64>,
    /// Global timestamp extent includes inter-batch CPU/readback gaps. Never frame cost.
    gpu_envelope_ms: Option<f64>,
    batch_frames: usize,
    batches: Vec<BatchEnvelope>,
    sum_valid_gpu_ms: f64,
    measurement_loop_wall_ms: f64,
}

fn analyze_timestamps(
    raw_timestamp_pairs: Vec<[u64; 2]>,
    submission_wall_ms: Vec<f64>,
    period: f64,
    batched: bool,
    measurement_loop_wall_ms: f64,
) -> FrameTimings {
    let mut invalid_samples = Vec::new();
    let mut gpu_ms = Vec::new();
    let mut exceeding = Vec::new();
    for (index, &[start, end]) in raw_timestamp_pairs.iter().enumerate() {
        let reason = if start == 0 || end == 0 {
            Some("zero_timestamp")
        } else if end < start {
            Some("end_before_start")
        } else if end == start {
            Some("zero_duration")
        } else {
            None
        };
        let duration = reason
            .is_none()
            .then(|| (end - start) as f64 * period / 1_000_000.0);
        if let Some(reason) = reason {
            invalid_samples.push(serde_json::json!({"frame_index": index, "warmup": index < WARMUP, "reason": reason, "raw_start": start, "raw_end": end}));
        }
        if !batched && duration.is_some_and(|ms| ms > submission_wall_ms[index]) {
            exceeding.push(index);
        }
        gpu_ms.push(duration);
    }
    let overlapping_frame_indices = raw_timestamp_pairs
        .windows(2)
        .enumerate()
        .filter_map(|(i, pair)| (pair[1][0] > 0 && pair[1][0] < pair[0][1]).then_some(i + 1))
        .collect();
    let minimum = raw_timestamp_pairs
        .iter()
        .map(|pair| pair[0])
        .filter(|&v| v > 0)
        .min();
    let maximum = raw_timestamp_pairs
        .iter()
        .map(|pair| pair[1])
        .filter(|&v| v > 0)
        .max();
    let gpu_envelope_ms = minimum
        .zip(maximum)
        .and_then(|(start, end)| end.checked_sub(start))
        .map(|ticks| ticks as f64 * period / 1_000_000.0);
    let batch_frames = if batched { BATCH_FRAMES } else { 1 };
    let batches = raw_timestamp_pairs
        .chunks(batch_frames)
        .enumerate()
        .map(|(index, pairs)| {
            let valid = pairs.iter().all(|p| p[0] > 0 && p[1] > p[0]);
            let envelope = valid.then(|| {
                let start = pairs.iter().map(|p| p[0]).min().unwrap();
                let end = pairs.iter().map(|p| p[1]).max().unwrap();
                (end - start) as f64 * period / 1_000_000.0
            });
            let wall = submission_wall_ms.get(index).copied();
            let invalid_reason = if !valid {
                Some("invalid_frame_timestamps")
            } else if wall.is_none_or(|ms| !ms.is_finite() || ms <= 0.0) {
                Some("missing_or_invalid_submission_wall_bound")
            } else if envelope.unwrap() > wall.unwrap() {
                Some("gpu_envelope_exceeds_submission_wall")
            } else {
                None
            };
            BatchEnvelope {
                raw_gpu_envelope_ms: envelope,
                completed_submission_wall_ms: wall,
                invalid_reason,
                first_frame: index * batch_frames,
                frames: pairs.len(),
                warmup: index * batch_frames < WARMUP,
                gpu_envelope_ms: envelope.filter(|_| invalid_reason.is_none()),
            }
        })
        .collect();
    FrameTimings {
        submission_mode: if batched { "batched" } else { "isolated" },
        timestamp_period_ns: period,
        batch_frames,
        batches,
        sum_valid_gpu_ms: gpu_ms.iter().flatten().sum(),
        raw_timestamp_pairs,
        gpu_ms,
        invalid_samples,
        overlapping_frame_indices,
        gpu_duration_exceeds_submission_wall_indices: exceeding,
        submission_wall_ms,
        gpu_envelope_ms,
        measurement_loop_wall_ms,
    }
}

#[test]
fn timestamp_audit_preserves_invalid_indices_and_detects_overlap() {
    let result = analyze_timestamps(
        vec![
            [100, 200],
            [150, 250],
            [300, 0],
            [0, 400],
            [500, 500],
            [700, 600],
        ],
        vec![0.00001; 6],
        1.0,
        false,
        1.0,
    );
    assert_eq!(result.overlapping_frame_indices, [1]);
    assert_eq!(result.invalid_samples.len(), 4);
    assert_eq!(result.invalid_samples[0]["frame_index"], 2);
    assert_eq!(result.invalid_samples[0]["reason"], "zero_timestamp");
    assert_eq!(result.gpu_duration_exceeds_submission_wall_indices, [0, 1]);
    assert_eq!(result.gpu_ms[2], None);
}

fn wait_gpu(device: &wgpu::Device) {
    device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(Duration::from_secs(60)),
        })
        .expect("measured submission completion");
}

#[path = "ray_shadow_oracle.rs"]
mod shadow_oracle;

#[derive(serde::Serialize)]
struct TimingDistribution {
    samples: usize,
    p50_ms: Option<f64>,
    p95_ms: Option<f64>,
    p99_ms: Option<f64>,
}

fn distribution(values: impl Iterator<Item = f64>) -> TimingDistribution {
    let mut values: Vec<_> = values.collect();
    values.sort_by(f64::total_cmp);
    let percentile = |p: usize| {
        values
            .get((values.len() * p).div_ceil(100).saturating_sub(1))
            .copied()
    };
    TimingDistribution {
        samples: values.len(),
        p50_ms: percentile(50),
        p95_ms: percentile(95),
        p99_ms: percentile(99),
    }
}

#[derive(serde::Serialize)]
struct TimingSummary {
    submission_mode: &'static str,
    total_dispatched_frames: usize,
    invalid_pass_timestamps: usize,
    isolated_pass_latency: Option<TimingDistribution>,
    /// Post-warmup isolated latency samples rejected for exceeding their own
    /// submission-to-completion wall bound. Raw values and indices remain above.
    isolated_latency_rejected_samples: usize,
    /// Diagnostic latency only: overlapping spans must never be summed or called frame time.
    batched_overlapping_pass_latency: Option<TimingDistribution>,
    /// Sum of individual batch GPU envelopes divided by their total frame count.
    /// Includes batch fill/drain, but excludes inter-batch CPU/readback gaps.
    batched_gpu_envelope_per_dispatched_frame_ms: Option<f64>,
    batched_warm_gpu_envelope_per_frame_ms: Option<f64>,
    batch_frames: usize,
    excluded_batch_boundary_intervals: usize,
    batched_envelope_unavailable_reason: Option<&'static str>,
    batched_post_warmup_completion_intervals: Option<TimingDistribution>,
    /// Destination frame indices for each rejected adjacent completion interval.
    rejected_completion_intervals: Vec<serde_json::Value>,
}

fn summarize_timings(timings: &FrameTimings) -> TimingSummary {
    let batched = timings.submission_mode == "batched";
    let isolated_rejected = timings
        .gpu_duration_exceeds_submission_wall_indices
        .iter()
        .filter(|&&index| index >= WARMUP)
        .count();
    let latencies = || {
        distribution(
            timings
                .gpu_ms
                .iter()
                .enumerate()
                .skip(WARMUP)
                .filter(|(index, _)| {
                    batched
                        || !timings
                            .gpu_duration_exceeds_submission_wall_indices
                            .contains(index)
                })
                .filter_map(|(_, duration)| *duration),
        )
    };
    let mut cadence = Vec::new();
    let mut rejected_completion_intervals = Vec::new();
    let mut excluded_batch_boundary_intervals = 0;
    if batched {
        for index in WARMUP + 1..timings.raw_timestamp_pairs.len() {
            if index % timings.batch_frames == 0 {
                excluded_batch_boundary_intervals += 1;
                continue;
            }
            let previous = timings.raw_timestamp_pairs[index - 1][1];
            let current = timings.raw_timestamp_pairs[index][1];
            let reason = if timings.batches[index / timings.batch_frames]
                .invalid_reason
                .is_some()
            {
                Some("invalid_batch_timing")
            } else if timings.gpu_ms[index - 1].is_none() || timings.gpu_ms[index].is_none() {
                Some("invalid_endpoint_frame_timestamp")
            } else if current <= previous {
                Some("nonincreasing_completion_timestamp")
            } else {
                None
            };
            if let Some(reason) = reason {
                rejected_completion_intervals.push(serde_json::json!({"frame_index": index, "reason": reason, "raw_previous_end": previous, "raw_end": current}));
            } else {
                cadence
                    .push((current - previous) as f64 * timings.timestamp_period_ns / 1_000_000.0);
            }
        }
    }
    let envelope_valid = !timings.raw_timestamp_pairs.is_empty()
        && timings.invalid_samples.is_empty()
        && timings
            .batches
            .iter()
            .all(|batch| batch.invalid_reason.is_none());
    TimingSummary {
        submission_mode: timings.submission_mode,
        total_dispatched_frames: timings.raw_timestamp_pairs.len(),
        invalid_pass_timestamps: timings.invalid_samples.len(),
        isolated_pass_latency: (!batched).then(latencies),
        isolated_latency_rejected_samples: if batched { 0 } else { isolated_rejected },
        batched_overlapping_pass_latency: batched.then(latencies),
        batched_gpu_envelope_per_dispatched_frame_ms: (batched && envelope_valid).then(|| {
            timings
                .batches
                .iter()
                .map(|b| b.gpu_envelope_ms.unwrap())
                .sum::<f64>()
                / timings.raw_timestamp_pairs.len() as f64
        }),
        batched_warm_gpu_envelope_per_frame_ms: (batched
            && envelope_valid
            && timings.raw_timestamp_pairs.len() > WARMUP)
            .then(|| {
                let warm: Vec<_> = timings.batches.iter().filter(|b| !b.warmup).collect();
                warm.iter().map(|b| b.gpu_envelope_ms.unwrap()).sum::<f64>()
                    / warm.iter().map(|b| b.frames).sum::<usize>() as f64
            }),
        batch_frames: timings.batch_frames,
        excluded_batch_boundary_intervals,
        batched_envelope_unavailable_reason: (batched && !envelope_valid)
            .then_some("incomplete_or_invalid_frame_or_batch_timestamps"),
        batched_post_warmup_completion_intervals: batched
            .then(|| distribution(cadence.into_iter())),
        rejected_completion_intervals,
    }
}

#[test]
fn batched_report_separates_overlapping_latency_from_completion_cadence() {
    // Every pass spans 100 ticks, but completed frames arrive every 2 ticks.
    let pairs: Vec<_> = (0..WARMUP + 4)
        .map(|i| [1 + i as u64 * 2, 101 + i as u64 * 2])
        .collect();
    let analyzed = analyze_timestamps(pairs.clone(), vec![200.0; 4], 1_000_000.0, true, 1.0);
    let summary = summarize_timings(&analyzed);
    assert!(summary.isolated_pass_latency.is_none());
    assert_eq!(
        summary.batched_overlapping_pass_latency.unwrap().p50_ms,
        Some(100.0)
    );
    let cadence = summary.batched_post_warmup_completion_intervals.unwrap();
    assert_eq!(cadence.samples, 3);
    assert_eq!(cadence.p50_ms, Some(2.0));
    assert_eq!(
        summary.batched_gpu_envelope_per_dispatched_frame_ms,
        Some((118.0 * 3.0 + 106.0) / 34.0)
    );
    assert!(summary.rejected_completion_intervals.is_empty());

    let mut invalid = pairs;
    invalid[WARMUP + 1] = [0, 0];
    let summary = summarize_timings(&analyze_timestamps(
        invalid,
        vec![200.0; 4],
        1_000_000.0,
        true,
        1.0,
    ));
    assert_eq!(summary.invalid_pass_timestamps, 1);
    assert_eq!(summary.batched_gpu_envelope_per_dispatched_frame_ms, None);
    assert_eq!(summary.rejected_completion_intervals.len(), 3);
    assert_eq!(
        summary
            .batched_post_warmup_completion_intervals
            .unwrap()
            .samples,
        0
    );
}

#[test]
fn isolated_summary_rejects_impossible_duration_but_preserves_raw_evidence() {
    let pairs: Vec<_> = (0..WARMUP + 3)
        .map(|index| {
            let start = index as u64 * 200 + 1;
            [start, start + if index == WARMUP + 1 { 100 } else { 10 }]
        })
        .collect();
    let timing = analyze_timestamps(pairs, vec![20.0; WARMUP + 3], 1_000_000.0, false, 1_000.0);
    assert_eq!(
        timing.gpu_duration_exceeds_submission_wall_indices,
        [WARMUP + 1]
    );
    assert_eq!(
        timing.gpu_ms[WARMUP + 1],
        Some(100.0),
        "preserve raw anomalous duration"
    );
    let summary = summarize_timings(&timing);
    assert_eq!(summary.isolated_latency_rejected_samples, 1);
    let valid = summary.isolated_pass_latency.unwrap();
    assert_eq!(valid.samples, 2);
    assert_eq!(valid.p95_ms, Some(10.0));
}

#[test]
fn bounded_batch_summary_excludes_cpu_gaps_and_warmup_boundaries() {
    let pairs: Vec<_> = (0..WARMUP + SAMPLES)
        .map(|i| {
            let start = 1 + (i / BATCH_FRAMES) as u64 * 10_000 + (i % BATCH_FRAMES) as u64 * 2;
            [start, start + 100]
        })
        .collect();
    let timing = analyze_timestamps(pairs, vec![200.0; 21], 1_000_000.0, true, 1.0);
    let summary = summarize_timings(&timing);
    assert_eq!(
        summary.batched_gpu_envelope_per_dispatched_frame_ms,
        Some(11.8)
    );
    assert_eq!(summary.batched_warm_gpu_envelope_per_frame_ms, Some(11.8));
    assert_eq!(summary.excluded_batch_boundary_intervals, 17);
    let cadence = summary.batched_post_warmup_completion_intervals.unwrap();
    assert_eq!(cadence.samples, 162);
    assert_eq!(cadence.p50_ms, Some(2.0));
    assert!(timing.gpu_envelope_ms.unwrap() > 200_000.0);
    assert_eq!(timing.batches.len(), 21);
}

#[test]
fn impossible_batch_envelope_preserves_raw_evidence_and_rejects_cadence() {
    let pairs: Vec<_> = (0..40)
        .map(|i| {
            let start = 1 + (i / BATCH_FRAMES) as u64 * 1000 + (i % BATCH_FRAMES) as u64 * 2;
            [start, start + 100]
        })
        .collect();
    let timing = analyze_timestamps(
        pairs,
        vec![200.0, 200.0, 200.0, 110.0],
        1_000_000.0,
        true,
        1000.0,
    );
    assert_eq!(timing.batches[3].raw_gpu_envelope_ms, Some(118.0));
    assert_eq!(
        timing.batches[3].invalid_reason,
        Some("gpu_envelope_exceeds_submission_wall")
    );
    assert_eq!(timing.batches[3].gpu_envelope_ms, None);
    let summary = summarize_timings(&timing);
    assert_eq!(summary.batched_gpu_envelope_per_dispatched_frame_ms, None);
    assert_eq!(summary.batched_warm_gpu_envelope_per_frame_ms, None);
    assert_eq!(
        summary
            .batched_post_warmup_completion_intervals
            .unwrap()
            .samples,
        0
    );
    assert_eq!(summary.rejected_completion_intervals.len(), 9);
}
