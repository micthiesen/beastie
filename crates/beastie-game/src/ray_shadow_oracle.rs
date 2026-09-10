//! Explicit moving-scene cache oracle. This uses real renderer updates and compares
//! cached lighting with uncached lighting on each identical extracted frame.
use super::*;

#[test]
#[ignore = "requires a real GPU; no window or display required"]
fn moving_shadow_cache_matches_uncached_frames() {
    bevy::tasks::block_on(run(2, false));
}

#[test]
#[ignore = "requires a real GPU; exercises the smaller-buffer adapter path"]
fn one_sample_cache_matches_uncached_frames() {
    bevy::tasks::block_on(run(1, false));
}

#[test]
#[ignore = "requires a real GPU; compares sampled shadow maps with exact ray shadows"]
fn moving_shadow_maps_preserve_bounded_image_error() {
    bevy::tasks::block_on(run(2, true));
}

async fn run(cache_samples: u32, shadow_map_enabled: bool) {
    let mut app = scene_app();
    let fixed = {
        let mut query = app.world_mut().query_filtered::<(Entity, &Mesh3d, &MeshMaterial3d<StandardMaterial>), With<crate::ray_scene::RayStatic>>();
        query
            .iter(app.world())
            .find(|(_, _, material)| {
                !app.world()
                    .resource::<Assets<StandardMaterial>>()
                    .get(&material.0)
                    .unwrap()
                    .unlit
            })
            .map(|(entity, mesh, _)| (entity, mesh.0.clone()))
            .unwrap()
    };
    let instance = wgpu::Instance::default();
    let adapter = instance.request_adapter(&Default::default()).await.unwrap();
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor::default())
        .await
        .unwrap();
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("production cached shadow oracle"),
        source: wgpu::ShaderSource::Wgsl(crate::raytrace::probe_shader_source(RenderProbe::Full)),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: None,
        module: &shader,
        entry_point: Some("trace_frame"),
        compilation_options: Default::default(),
        cache: None,
    });
    let cache = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("persistent real shadow cache"),
        size: u64::from(WIDTH * HEIGHT) * 24,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: WIDTH,
            height: HEIGHT,
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
    let bounce_cache = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(WIDTH) * u64::from(HEIGHT) * 8,
        usage: wgpu::BufferUsages::STORAGE,
        mapped_at_creation: false,
    });
    let mut previous_key = None;
    let labels = [
        "cold",
        "warm",
        "creature-left-low",
        "creature-right-low",
        "carried-ball",
        "released-ball-left",
        "ball-right-plant-sway",
        "settings",
        "title",
        "continue",
        "static-transform",
        "static-normals",
        "static-hidden",
        "static-visible",
        "static-removed",
    ];
    let mut reports = Vec::new();
    for (step, label) in labels.iter().enumerate() {
        if step > 0 {
            {
                let mut frame = app
                    .world_mut()
                    .resource_mut::<crate::renderer::SceneFrame>();
                frame.plan.elapsed_ms += 500;
                if step == 2 || step == 3 {
                    frame.plan.creature.position = beastie_core::NormalizedPosition::new(
                        if step == 2 { 2500 } else { 7500 },
                        1800,
                    );
                }
                if (4..=6).contains(&step) {
                    let ball = frame
                        .plan
                        .objects
                        .iter_mut()
                        .find(|object| {
                            object.kind == beastie_view::ObjectKind::Toy(beastie_core::ToyId::Ball)
                        })
                        .unwrap();
                    ball.carried = step == 4;
                    ball.position = beastie_core::NormalizedPosition::new(
                        if step == 5 { 2400 } else { 7800 },
                        1300,
                    );
                }
            }
            if (7..=9).contains(&step) {
                let world = beastie_core::WorldState::new(42, "Mop");
                let view = beastie_view::ViewState {
                    mode: match step {
                        7 => beastie_view::UiMode::Settings,
                        8 => beastie_view::UiMode::Title,
                        _ => beastie_view::UiMode::Compose,
                    },
                    ..Default::default()
                };
                app.world_mut()
                    .resource_mut::<crate::renderer::SceneFrame>()
                    .plan = beastie_view::plan(&world, &view).0;
            }
            match step {
                10 => {
                    app.world_mut()
                        .entity_mut(fixed.0)
                        .get_mut::<Transform>()
                        .unwrap()
                        .translation
                        .x += 0.2;
                }
                11 => {
                    {
                        let mut meshes = app.world_mut().resource_mut::<Assets<Mesh>>();
                        let mut mesh = meshes.get_mut(&fixed.1).unwrap();
                        let count = mesh.count_vertices();
                        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.6, 0.8]; count]);
                    }
                    app.world_mut()
                        .write_message(AssetEvent::Modified { id: fixed.1.id() });
                }
                12 => {
                    app.world_mut()
                        .entity_mut(fixed.0)
                        .insert(Visibility::Hidden);
                }
                13 => {
                    app.world_mut()
                        .entity_mut(fixed.0)
                        .insert(Visibility::Visible);
                }
                14 => {
                    app.world_mut().despawn(fixed.0);
                }
                _ => {}
            }
            app.update();
            app.update();
        }
        let (scene, mut params) = snapshot_from_app(&mut app);
        params.shadow_roots.z = cache_samples;
        let key = crate::raytrace::shadow_cache_key(
            scene.static_revision,
            UVec2::new(WIDTH, HEIGHT),
            params.world_from_clip,
        );
        let clear = previous_key != Some(key);
        if (1..=6).contains(&step) {
            assert!(
                !clear,
                "dynamic motion must retain stationary cache: {label}"
            );
        }
        if step >= 8 {
            assert!(clear, "camera/static edits must invalidate: {label}");
        }
        previous_key = Some(key);
        let mut geometry = vec![0u8; scene.triangle_count as usize * 48];
        let mut surfaces = vec![0u8; scene.surface_count as usize * 96];
        let mut nodes = vec![0u8; scene.node_count as usize * 32];
        for chunk in &scene.geometry {
            let values: Vec<_> = chunk
                .triangles
                .iter()
                .zip(chunk.surface_indices.iter())
                .map(|(triangle, &index)| triangle.geometry(chunk.surface_offset + index))
                .collect();
            let bytes = encoded(&values);
            let offset = chunk.triangle_offset as usize * 48;
            geometry[offset..offset + bytes.len()].copy_from_slice(&bytes);
            let bytes = encoded(chunk.surfaces.as_ref());
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
        let buffers: Vec<_> = data
            .iter()
            .enumerate()
            .map(|(i, bytes)| {
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: None,
                    contents: bytes,
                    usage: if i == 4 {
                        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST
                    } else {
                        wgpu::BufferUsages::STORAGE
                    },
                })
            })
            .collect();
        let visibility_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("offscreen raster visibility"),
            size: wgpu::Extent3d {
                width: WIDTH * 2,
                height: HEIGHT * 2,
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
        let shadow_resolution = crate::ray_shadow_maps::RESOLUTION;
        let (map_params, shadow_casters) = crate::ray_shadow_maps::fit(&scene, shadow_map_enabled);
        let map_uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("dynamic shadow projections"),
            contents: &encoded(&map_params),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let map_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("twelve dynamic shadow directions"),
            size: wgpu::Extent3d {
                width: shadow_resolution,
                height: shadow_resolution,
                depth_or_array_layers: 12,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
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
                    base_array_layer: index,
                    array_layer_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();
        let map_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
                            resource: buffers[0].as_entire_binding(),
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

        let mut entries: Vec<_> = buffers
            .iter()
            .zip([0, 1, 2, 3, 4, 6])
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
            resource: cache.as_entire_binding(),
        });
        entries.push(wgpu::BindGroupEntry {
            binding: 7,
            resource: wgpu::BindingResource::TextureView(&visibility_view),
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
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &entries,
        });
        let dispatch = |clear_cache: bool| {
            let mut encoder = device.create_command_encoder(&Default::default());
            if clear_cache {
                encoder.clear_buffer(&cache, 0, None);
            }
            if shadow_map_enabled {
                for layer in 0..12 {
                    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("dynamic shadow depth"),
                        color_attachments: &[],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: &map_layer_views[layer],
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Clear(0.0),
                                store: wgpu::StoreOp::Store,
                            }),
                            stencil_ops: None,
                        }),
                        timestamp_writes: None,
                        occlusion_query_set: None,
                        multiview_mask: None,
                    });
                    pass.set_pipeline(&map_pipeline);
                    pass.set_bind_group(0, &map_binds[layer], &[]);
                    for &index in &shadow_casters {
                        let instance = scene.instances[index];
                        let chunk = scene
                            .geometry
                            .iter()
                            .find(|chunk| chunk.node_offset == instance.root)
                            .expect("shadow mesh range");
                        pass.draw(
                            chunk.triangle_offset * 3
                                ..(chunk.triangle_offset + chunk.triangles.len() as u32) * 3,
                            index as u32..index as u32 + 1,
                        );
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
                    timestamp_writes: None,
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

            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
                pass.set_pipeline(&pipeline);
                pass.set_bind_group(0, &bind, &[]);
                pass.dispatch_workgroups(WIDTH.div_ceil(8), HEIGHT.div_ceil(8), 1);
            }
            queue.submit([encoder.finish()]);
            radiance_bytes(&device, &queue, &texture)
        };
        let cached = dispatch(clear);
        if let Ok(dir) = std::env::var("BEASTIE_MAP_ORACLE_CAPTURE_DIR") {
            std::fs::create_dir_all(&dir).unwrap();
            save_radiance_png(
                &device,
                &queue,
                &texture,
                &Path::new(&dir).join(format!("{step:02}-{label}-maps.png")),
            );
        }
        let mut reference_maps = map_params.clone();
        reference_maps.info.x = 0;
        queue.write_buffer(&map_uniform, 0, &encoded(&reference_maps));
        params.shadow_roots.z = 0;
        queue.write_buffer(&buffers[4], 0, &encoded(&params));
        let reference = dispatch(false);
        if let Ok(dir) = std::env::var("BEASTIE_MAP_ORACLE_CAPTURE_DIR") {
            save_radiance_png(
                &device,
                &queue,
                &texture,
                &Path::new(&dir).join(format!("{step:02}-{label}-trace.png")),
            );
        }
        let mut max_difference = 0.0_f64;
        let mut sum_difference = 0.0_f64;
        for (a, b) in cached.chunks_exact(8).zip(reference.chunks_exact(8)) {
            for channel in 0..3 {
                let at = channel * 2;
                let delta = f64::from(
                    (positive_half(u16::from_le_bytes([a[at], a[at + 1]]))
                        - positive_half(u16::from_le_bytes([b[at], b[at + 1]])))
                    .abs(),
                );
                max_difference = max_difference.max(delta);
                sum_difference += delta;
            }
        }
        let different = cached
            .chunks_exact(8)
            .zip(reference.chunks_exact(8))
            .filter(|(a, b)| a[..6] != b[..6])
            .count();
        eprintln!(
            "cache oracle {label}: {different} differing RGB pixels, clear={clear}, revision={}",
            scene.static_revision
        );
        reports.push(serde_json::json!({"frame": label, "different_rgb_pixels": different, "max_linear_rgb_difference":max_difference,"mean_linear_rgb_difference":sum_difference/f64::from(WIDTH*HEIGHT*3),"resolution":shadow_resolution,"cache_cleared": clear, "static_revision": scene.static_revision}));
        if shadow_map_enabled {
            // Broad regression bounds, not a claim of exactness or visual acceptance.
            // Native review covers the sparse contact-edge differences separately.
            assert!(
                different < (WIDTH * HEIGHT) as usize / 200,
                "shadow-map coverage error on {label}"
            );
            assert!(max_difference < 0.6, "shadow-map contrast error on {label}");
            assert!(
                sum_difference / f64::from(WIDTH * HEIGHT * 3) < 0.0002,
                "shadow-map image error on {label}"
            );
        } else {
            assert_eq!(different, 0, "cached shadow mismatch on {label}");
        }
    }
    if let Ok(path) = std::env::var("BEASTIE_CACHE_ORACLE_OUTPUT") {
        std::fs::write(path, serde_json::to_vec_pretty(&reports).unwrap()).unwrap();
    }
}

#[test]
#[ignore = "requires a real GPU; no window or display required"]
fn static_bounce_ties_preserve_full_world_order_after_dynamic_rearrangement() {
    bevy::tasks::block_on(async {
        use crate::ray_scene::{BvhNode, GpuInstance, Triangle};
        let instance = wgpu::Instance::default();
        let adapter = instance.request_adapter(&Default::default()).await.unwrap();
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .unwrap();
        let production = include_str!("raytrace.wgsl");
        let traversal = production.split_once("fn water_hash").unwrap().0;
        let attrs = production
            .split_once("fn normal_at")
            .unwrap()
            .1
            .split_once("struct Lighting")
            .unwrap()
            .0;
        let entry = r#"
@group(0) @binding(12) var<storage,read_write> result:array<vec4<u32>>;
@compute @workgroup_size(1)
fn verify_bounce_tie() {
    let origin=vec3(0.0,0.0,1.0); let direction=vec3(0.0,0.0,-1.0);
    var mask=0u;
    if(params.cache_roots.z!=0u) { mask=result[0].w; }
    let cold=cached_bounce(origin,direction,0u,mask);
    let warm=cached_bounce(origin,direction,0u,cold.mask);
    let reference=trace(origin,direction,2.5,true,false);
    result[0]=vec4(cold.hit.instance,warm.hit.instance,reference.instance,cold.mask);
    result[1]=bitcast<vec4<u32>>(vec4(albedo_at(cold.hit),0.0));
    result[2]=bitcast<vec4<u32>>(vec4(albedo_at(reference),0.0));
}
"#;
        let source = format!("{traversal}fn normal_at{attrs}{entry}");
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None,
            layout: None,
            module: &shader,
            entry_point: Some("verify_bounce_tie"),
            compilation_options: Default::default(),
            cache: None,
        });
        let triangles: Vec<_> = (0..4)
            .map(|i| {
                let center = match i {
                    2 => -3.75,
                    3 => 3.75,
                    _ => 0.0,
                };
                let width = if i < 2 { 0.8 } else { 0.1 };
                let color = if i == 0 {
                    Vec4::new(1.0, 0.0, 0.0, 1.0)
                } else {
                    Vec4::new(0.0, 1.0, 0.0, 1.0)
                };
                Triangle {
                    a: Vec4::new(center - width, -0.5, 0.0, 0.0),
                    b: Vec4::new(center + width, -0.5, 0.0, 0.0),
                    c: Vec4::new(center, 0.5, 0.0, 0.0),
                    n0: Vec4::Z,
                    n1: Vec4::Z,
                    n2: Vec4::Z,
                    c0: color,
                    c1: color,
                    c2: color,
                }
            })
            .collect();
        let bounds = [
            (Vec3::new(-3.0, -1.0, 0.0), Vec3::new(1.0, 1.0, 0.0)),
            (Vec3::new(-1.0, -1.0, -1.0), Vec3::new(3.0, 1.0, 0.5)),
            (Vec3::new(-4.0, -1.0, 0.0), Vec3::new(-3.5, 1.0, 0.9)),
            (Vec3::new(3.5, -1.0, 0.0), Vec3::new(4.0, 1.0, 0.9)),
        ];
        let leaf = |i: usize| BvhNode {
            min: bounds[i].0,
            max: bounds[i].1,
            first: i as u32,
            count: 1,
        };
        let parent = |a: BvhNode, b: BvhNode, first: u32| BvhNode {
            min: a.min.min(b.min),
            max: a.max.max(b.max),
            first,
            count: 0,
        };
        let nodes: Vec<_> = (0..4).map(leaf).collect();
        let mut tlas = Vec::new();
        for alternate in [false, true] {
            let offset = tlas.len() as u32;
            let mut x = leaf(2);
            // Dynamic nodes do not hit the ray; their changing bounds alter the
            // full-tree visit order while both stationary triangles remain fixed.
            if alternate {
                x.max.z = 0.0;
            }
            let left = parent(x, leaf(0), offset + 3);
            let right = parent(leaf(1), leaf(3), offset + 5);
            tlas.extend([
                parent(left, right, offset + 1),
                left,
                right,
                x,
                leaf(0),
                leaf(1),
                leaf(3),
            ]);
        }
        tlas.extend([parent(leaf(0), leaf(1), 15), leaf(0), leaf(1)]);
        tlas.extend([parent(leaf(2), leaf(3), 18), leaf(2), leaf(3)]);
        let instances: Vec<_> = (0..4)
            .map(|i| GpuInstance {
                world_from_local: Mat4::IDENTITY,
                local_from_world: Mat4::IDENTITY,
                material: Vec4::ZERO,
                tint: Vec4::ONE,
                root: i,
                transmission: 0.0,
                pad1: if i < 2 { i + 1 } else { 0 },
                pad2: i,
            })
            .collect();
        let geometry: Vec<_> = triangles
            .iter()
            .enumerate()
            .map(|(i, t)| t.geometry(i as u32))
            .collect();
        let surfaces: Vec<_> = triangles.iter().map(Triangle::surface).collect();
        let mut params = BenchParams {
            world_from_clip: Mat4::IDENTITY,
            size: UVec4::new(1, 1, 4, 0),
            water: Vec4::ZERO,
            roots: UVec4::new(0, 0, 0, 0),
            shadow_roots: UVec4::ZERO,
            cache_roots: UVec4::new(14, 17, 0, 1),
            static_instances: [
                UVec4::new(0, 1, u32::MAX, u32::MAX),
                UVec4::splat(u32::MAX),
                UVec4::splat(u32::MAX),
                UVec4::splat(u32::MAX),
            ],
        };
        let raw = [
            encoded(&geometry),
            encoded(&nodes),
            encoded(&instances),
            encoded(&tlas),
            encoded(&params),
            encoded(&surfaces),
            vec![0; 4],
            vec![0; 48],
        ];
        let bindings = [0, 1, 2, 3, 4, 6, 9, 12];
        let buffers: Vec<_> = raw
            .iter()
            .enumerate()
            .map(|(i, bytes)| {
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: None,
                    contents: bytes,
                    usage: if i == 4 {
                        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST
                    } else {
                        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC
                    },
                })
            })
            .collect();
        let entries: Vec<_> = buffers
            .iter()
            .zip(bindings)
            .map(|(b, binding)| wgpu::BindGroupEntry {
                binding,
                resource: b.as_entire_binding(),
            })
            .collect();
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &entries,
        });
        for phase in 0..2 {
            params.roots.y = phase * 7;
            params.cache_roots.z = phase;
            queue.write_buffer(&buffers[4], 0, &encoded(&params));
            let readback = device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: 48,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            let mut encoder = device.create_command_encoder(&Default::default());
            {
                let mut pass = encoder.begin_compute_pass(&Default::default());
                pass.set_pipeline(&pipeline);
                pass.set_bind_group(0, &bind, &[]);
                pass.dispatch_workgroups(1, 1, 1);
            }
            encoder.copy_buffer_to_buffer(&buffers[7], 0, &readback, 0, 48);
            queue.submit([encoder.finish()]);
            let bytes = mapped_bytes(&device, &readback);
            let word = |i: usize| u32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap());
            assert_eq!(
                word(2),
                phase,
                "fixture must change the full-world static tie winner"
            );
            assert_eq!(
                word(0),
                word(2),
                "cold/cache lookup must preserve full-world winner"
            );
            assert_eq!(
                word(1),
                word(2),
                "warm cache must preserve full-world winner"
            );
            assert_ne!(
                word(3) & (1 << 25),
                0,
                "static tie classification must persist"
            );
            assert_eq!(
                &bytes[16..28],
                &bytes[32..44],
                "distinct material colors must match"
            );
        }
    });
}

#[test]
#[ignore = "requires a real GPU; protects finite shadow-ray distance semantics"]
fn shadow_map_far_blocker_keeps_nearer_finite_ray_occlusion() {
    bevy::tasks::block_on(async {
        use crate::ray_scene::{BvhNode, GpuInstance, Triangle};
        let instance = wgpu::Instance::default();
        let adapter = instance.request_adapter(&Default::default()).await.unwrap();
        let (device, queue) = adapter.request_device(&Default::default()).await.unwrap();
        let production = include_str!("raytrace.wgsl");
        let traversal = production.split_once("fn water_hash").unwrap().0;
        let dynamic = production
            .split_once("fn dynamic_shadow_blocked")
            .unwrap()
            .1
            .split_once("fn lighting")
            .unwrap()
            .0;
        let entry = r#"
@group(0) @binding(12) var<storage,read_write> result:array<u32>;
@compute @workgroup_size(1)
fn verify_finite_shadow() {
    let origin=vec3(0.0); let light=vec3(0.0,0.0,1.0);
    result[0]=u32(dynamic_shadow_blocked(origin,light,0u));
    result[1]=u32(trace_root(origin,light,35.0,true,true,params.shadow_roots.y).instance!=0xffffffffu);
}
"#;
        let source = format!("{traversal}fn dynamic_shadow_blocked{dynamic}{entry}");
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None,
            layout: None,
            module: &shader,
            entry_point: Some("verify_finite_shadow"),
            compilation_options: Default::default(),
            cache: None,
        });
        let geometry: Vec<_> = [10.0, 50.0]
            .into_iter()
            .map(|z| {
                Triangle {
                    a: Vec4::new(-1.0, -1.0, z, 0.0),
                    b: Vec4::new(1.0, -1.0, z, 0.0),
                    c: Vec4::new(0.0, 1.0, z, 0.0),
                    n0: Vec4::Z,
                    n1: Vec4::Z,
                    n2: Vec4::Z,
                    c0: Vec4::ONE,
                    c1: Vec4::ONE,
                    c2: Vec4::ONE,
                }
                .geometry(0)
            })
            .collect();
        let mut node = BvhNode {
            min: Vec3::new(-1.0, -1.0, 10.0),
            max: Vec3::new(1.0, 1.0, 50.0),
            first: 0,
            count: 2,
        };
        let tlas = BvhNode { count: 1, ..node };
        let object = GpuInstance {
            world_from_local: Mat4::IDENTITY,
            local_from_world: Mat4::IDENTITY,
            material: Vec4::ZERO,
            tint: Vec4::ONE,
            root: 0,
            transmission: 0.0,
            pad1: 0,
            pad2: 0,
        };
        let params = BenchParams {
            world_from_clip: Mat4::IDENTITY,
            size: UVec4::new(1, 1, 1, 0),
            water: Vec4::ZERO,
            roots: UVec4::ZERO,
            shadow_roots: UVec4::ZERO,
            cache_roots: UVec4::ZERO,
            static_instances: [UVec4::splat(u32::MAX); 4],
        };
        let map_params = crate::ray_shadow_maps::ShadowMapParams {
            clip_from_world: [Mat4::from_scale(Vec3::new(1.0, 1.0, 0.01)); 12],
            depth_ranges: [Vec4::new(0.0, 100.0, 1.0, 0.0); 12],
            info: UVec4::new(1, 1, 1, 1),
        };
        let raw = [
            encoded(&geometry),
            encoded(&vec![node]),
            encoded(&vec![object]),
            encoded(&vec![tlas]),
            encoded(&params),
            encoded(&map_params),
            vec![0; 8],
        ];
        let bindings = [0, 1, 2, 3, 4, 11, 12];
        let buffers: Vec<_> = raw
            .iter()
            .enumerate()
            .map(|(i, bytes)| {
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: None,
                    contents: bytes,
                    usage: if i == 4 || i == 5 {
                        wgpu::BufferUsages::UNIFORM
                    } else {
                        wgpu::BufferUsages::STORAGE
                            | wgpu::BufferUsages::COPY_DST
                            | wgpu::BufferUsages::COPY_SRC
                    },
                })
            })
            .collect();
        let depth = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = depth.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let attachment = depth.create_view(&Default::default());
        let mut entries: Vec<_> = buffers
            .iter()
            .zip(bindings)
            .map(|(b, binding)| wgpu::BindGroupEntry {
                binding,
                resource: b.as_entire_binding(),
            })
            .collect();
        entries.push(wgpu::BindGroupEntry {
            binding: 10,
            resource: wgpu::BindingResource::TextureView(&view),
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &entries,
        });
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 8,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        for nearer_blocker in [true, false] {
            node.first = u32::from(!nearer_blocker);
            node.count = if nearer_blocker { 2 } else { 1 };
            queue.write_buffer(&buffers[1], 0, &encoded(&vec![node]));
            let mut encoder = device.create_command_encoder(&Default::default());
            {
                // The layer keeps the light-facing blocker at t=50. It hides a
                // nearer blocker at t=10, inside the original t<35 shadow ray.
                let _clear = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: None,
                    color_attachments: &[],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &attachment,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(0.5),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
            }
            {
                let mut pass = encoder.begin_compute_pass(&Default::default());
                pass.set_pipeline(&pipeline);
                pass.set_bind_group(0, &bind, &[]);
                pass.dispatch_workgroups(1, 1, 1);
            }
            encoder.copy_buffer_to_buffer(&buffers[6], 0, &readback, 0, 8);
            queue.submit([encoder.finish()]);
            let bytes = mapped_bytes(&device, &readback);
            assert_eq!(
                u32::from_le_bytes(bytes[..4].try_into().unwrap()),
                u32::from(nearer_blocker)
            );
            assert_eq!(
                &bytes[..4],
                &bytes[4..],
                "map fallback must match the finite original shadow ray"
            );
        }
    });
}
