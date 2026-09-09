//! Headless shader translation checks, not physical backend or driver tests.
use naga::{
    AddressSpace, Module, TypeInner,
    back::{hlsl, msl, spv},
    valid::{Capabilities, ValidationFlags, Validator},
};

#[test]
fn compute_tracer_translates_without_optional_gpu_capabilities() {
    validate_and_translate(include_str!("raytrace.wgsl"));
}

#[test]
fn presentation_shader_translates_without_optional_gpu_capabilities() {
    validate_and_translate(include_str!("ray_blit.wgsl"));
}

fn validate_and_translate(source: &str) {
    let module = naga::front::wgsl::parse_str(source)
        .unwrap_or_else(|error| panic!("{}", error.emit_to_string(source)));
    // Empty capabilities deliberately exclude ray queries, subgroups, f64 and
    // other optional hardware features. Ordinary compute needs none of them.
    let info = Validator::new(ValidationFlags::all(), Capabilities::empty())
        .validate(&module)
        .unwrap_or_else(|error| panic!("{}", error.emit_to_string(source)));
    let spirv = spv::write_vec(&module, &info, &spv::Options::default(), None)
        .expect("Vulkan SPIR-V translation");
    assert!(!spirv.is_empty());

    let mut hlsl_options = hlsl::Options {
        fake_missing_bindings: false,
        ..Default::default()
    };
    for (_, global) in module.global_variables.iter() {
        if let Some(binding) = &global.binding {
            hlsl_options.binding_map.insert(
                *binding,
                hlsl::BindTarget {
                    space: binding.group.try_into().expect("HLSL binding space"),
                    register: binding.binding,
                    ..Default::default()
                },
            );
        }
    }
    let mut hlsl_source = String::new();
    let hlsl_pipeline = hlsl::PipelineOptions::default();
    let reflection = hlsl::Writer::new(&mut hlsl_source, &hlsl_options, &hlsl_pipeline)
        .write(&module, &info, None)
        .expect("DirectX HLSL translation");
    assert!(!hlsl_source.is_empty());
    for entry in reflection.entry_point_names {
        entry.expect("HLSL entry-point translation");
    }

    let msl_options = metal_options(&module);
    let (msl_source, translation) = msl::write_string(
        &module,
        &info,
        &msl_options,
        &msl::PipelineOptions::default(),
    )
    .expect("Metal MSL translation");
    assert!(!msl_source.is_empty());
    for entry in translation.entry_point_names {
        entry.expect("MSL entry-point translation");
    }
}

fn metal_options(module: &Module) -> msl::Options {
    let mut options = msl::Options {
        lang_version: (2, 0),
        fake_missing_bindings: false,
        ..Default::default()
    };
    let mut resources = msl::EntryPointResources {
        // Runtime storage-array bounds checks consume this auxiliary buffer.
        sizes_buffer: Some(30),
        ..Default::default()
    };
    for (_, global) in module.global_variables.iter() {
        let Some(binding) = &global.binding else {
            continue;
        };
        let slot = binding.binding.try_into().expect("Metal resource slot");
        assert_eq!(
            binding.group, 0,
            "Allocate disjoint slots for additional groups"
        );
        let mut target = msl::BindTarget::default();
        if matches!(module.types[global.ty].inner, TypeInner::Image { .. }) {
            target.texture = Some(slot);
            target.mutable = matches!(
                module.types[global.ty].inner,
                TypeInner::Image {
                    class: naga::ImageClass::Storage { .. },
                    ..
                }
            );
        } else {
            target.buffer = Some(slot);
            target.mutable = matches!(
                global.space,
                AddressSpace::Storage { access } if access.contains(naga::StorageAccess::STORE)
            );
        }
        resources.resources.insert(*binding, target);
    }
    for entry in &module.entry_points {
        options
            .per_entry_point_map
            .insert(entry.name.clone(), resources.clone());
    }
    options
}

/// Explicit driver validation: never runs in the display/GPU-independent gate.
/// This tests the production traversal prefix, not a separately maintained shader.
#[cfg(test)]
mod gpu_traversal {
    use bevy::{
        prelude::*,
        render::render_resource::{ShaderType, encase},
    };
    use wgpu::util::DeviceExt;

    use crate::ray_scene::{BvhNode, GpuInstance, Triangle};

    #[derive(Clone, Copy, ShaderType)]
    struct TestRay {
        origin: Vec4,
        direction: Vec4,
        flags: UVec4,
    }

    #[derive(ShaderType)]
    struct TestParams {
        world_from_clip: Mat4,
        size: UVec4,
        water: Vec4,
        roots: UVec4,
    }

    #[derive(Debug, Clone, Copy)]
    struct CpuIntersection {
        t: f64,
        u: f64,
        v: f64,
    }

    #[derive(Debug)]
    struct CpuHit {
        t: f64,
        triangle: usize,
        instance: usize,
    }

    const ENTRY: &str = r#"
struct TestRay { origin:vec4<f32>, direction:vec4<f32>, flags:vec4<u32> }
struct TestResult { hit:vec4<f32>, ids:vec4<u32>, normal:vec4<f32>, albedo:vec4<f32> }
@group(0) @binding(7) var<storage,read> test_rays:array<TestRay>;
@group(0) @binding(8) var<storage,read_write> test_results:array<TestResult>;
@compute @workgroup_size(64)
fn verify_traversal(@builtin(global_invocation_id) id:vec3<u32>) {
    if(id.x>=arrayLength(&test_rays)) { return; }
    let ray=test_rays[id.x];
    let hit=trace(ray.origin.xyz,ray.direction.xyz,ray.origin.w,ray.flags.x!=0u,ray.flags.y!=0u);
    var normal=vec3(0.0); var albedo=vec3(0.0);
    if(hit.instance!=0xffffffffu) {
        normal=normal_at(hit,ray.direction.xyz);
        albedo=albedo_at(hit);
    }
    test_results[id.x]=TestResult(vec4(hit.t,hit.u,hit.v,0.0),vec4(hit.triangle,hit.instance,0u,0u),vec4(normal,0.0),vec4(albedo,0.0));
}
"#;

    #[test]
    #[ignore = "requires a native GPU; run explicitly with --ignored --nocapture"]
    fn production_gpu_traversal_matches_brute_force() {
        bevy::tasks::block_on(run());
    }

    fn validation_source() -> String {
        let production = include_str!("raytrace.wgsl");
        let (traversal, _) = production
            .split_once("fn water_hash")
            .expect("production traversal boundary");
        let (_, surface) = production
            .split_once("fn normal_at")
            .expect("production normal function");
        let (surface, _) = surface
            .split_once("struct Lighting")
            .expect("production surface function boundary");
        // Execute the actual production attribute functions and binding layout,
        // while leaving unrelated water/lighting/output texture resources unused.
        format!("{traversal}fn normal_at{surface}{ENTRY}")
    }

    #[test]
    fn surface_oracle_translates_without_optional_gpu_capabilities() {
        super::validate_and_translate(&validation_source());
    }

    async fn run() {
        let (triangles, nodes) = geometry();
        let instances = instances();
        let mut tlas = Vec::new();
        let all: Vec<_> = instances
            .iter()
            .enumerate()
            .map(|(i, instance)| {
                let mut lo = Vec3::splat(f32::INFINITY);
                let mut hi = Vec3::splat(f32::NEG_INFINITY);
                for triangle in &triangles {
                    for vertex in [triangle.a, triangle.b, triangle.c] {
                        let point = instance
                            .world_from_local
                            .transform_point3(vertex.truncate());
                        lo = lo.min(point);
                        hi = hi.max(point);
                    }
                }
                (i as u32, lo, hi)
            })
            .collect();
        assert_eq!(append_tree(&mut tlas, &all), 0);
        let world: Vec<_> = all
            .iter()
            .copied()
            .filter(|(i, _, _)| instances[*i as usize].material.w < 0.5)
            .collect();
        let shadow: Vec<_> = world
            .iter()
            .copied()
            .filter(|(i, _, _)| instances[*i as usize].material.z < 0.5)
            .collect();
        let world_root = append_tree(&mut tlas, &world);
        let shadow_root = append_tree(&mut tlas, &shadow);
        let rays = rays();
        let params = TestParams {
            world_from_clip: Mat4::IDENTITY,
            size: UVec4::new(rays.len() as u32, 1, instances.len() as u32, 0),
            water: Vec4::ZERO,
            roots: UVec4::new(0, world_root, shadow_root, 0),
        };
        let instance = wgpu::Instance::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .expect("native adapter required for this explicitly requested test");
        eprintln!("GPU traversal adapter: {:?}", adapter.get_info());
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Beastie traversal correctness"),
                required_features: wgpu::Features::empty(),
                ..Default::default()
            })
            .await
            .expect("ordinary compute device");
        let source = validation_source();
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("production traversal validation"),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
        macro_rules! storage {
            ($value:expr, $usage:expr) => {{
                let mut encoded = encase::StorageBuffer::new(Vec::new());
                encoded.write(&$value).expect("production shader layout");
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: None,
                    contents: &encoded.into_inner(),
                    usage: $usage,
                })
            }};
        }
        let geometry: Vec<_> = triangles.iter().map(Triangle::geometry).collect();
        let surfaces: Vec<_> = triangles.iter().map(Triangle::surface).collect();
        let storage_usage = wgpu::BufferUsages::STORAGE;
        let buffers = [
            storage!(geometry, storage_usage),
            storage!(nodes, storage_usage),
            storage!(instances, storage_usage),
            storage!(tlas, storage_usage),
            storage!(params, wgpu::BufferUsages::UNIFORM),
            storage!(surfaces, storage_usage),
            storage!(rays, storage_usage),
        ];
        let byte_len = rays.len() as u64 * 64;
        let output = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("validation results"),
            size: byte_len,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: byte_len,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let bindings = [0, 1, 2, 3, 4, 6, 7, 8];
        let entries: Vec<_> = bindings
            .iter()
            .map(|&binding| wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: if binding == 4 {
                        wgpu::BufferBindingType::Uniform
                    } else {
                        wgpu::BufferBindingType::Storage {
                            read_only: binding != 8,
                        }
                    },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            })
            .collect();
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &entries,
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("verify_traversal"),
            compilation_options: Default::default(),
            cache: None,
        });
        let mut entries: Vec<_> = buffers
            .iter()
            .zip(&bindings)
            .map(|(buffer, &binding)| wgpu::BindGroupEntry {
                binding,
                resource: buffer.as_entire_binding(),
            })
            .collect();
        entries.push(wgpu::BindGroupEntry {
            binding: 8,
            resource: output.as_entire_binding(),
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &layout,
            entries: &entries,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bind, &[]);
            pass.dispatch_workgroups((rays.len() as u32).div_ceil(64), 1, 1);
        }
        encoder.copy_buffer_to_buffer(&output, 0, &readback, 0, byte_len);
        queue.submit([encoder.finish()]);
        let (sender, receiver) = std::sync::mpsc::channel();
        readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                sender.send(result).unwrap()
            });
        device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(std::time::Duration::from_secs(30)),
            })
            .expect("GPU traversal completion");
        receiver
            .recv_timeout(std::time::Duration::from_secs(30))
            .unwrap()
            .expect("result mapping");
        let bytes = readback.slice(..).get_mapped_range();
        let mut hit_count = 0;
        for (index, (ray, result)) in rays.iter().zip(bytes.chunks_exact(64)).enumerate() {
            let read_f32 =
                |offset| f32::from_le_bytes(result[offset..offset + 4].try_into().unwrap());
            let read_vec3 =
                |offset| Vec3::new(read_f32(offset), read_f32(offset + 4), read_f32(offset + 8));
            let gpu_t = f64::from(read_f32(0));
            let gpu_u = f64::from(read_f32(4));
            let gpu_v = f64::from(read_f32(8));
            let gpu_triangle = u32::from_le_bytes(result[16..20].try_into().unwrap()) as usize;
            let gpu_instance = u32::from_le_bytes(result[20..24].try_into().unwrap());
            let expected = brute_force(ray, &triangles, &instances);
            assert_eq!(
                gpu_instance != u32::MAX,
                expected.is_some(),
                "hit/miss ray {index}: GPU t={gpu_t}, CPU={expected:?}"
            );
            if let Some(expected) = expected {
                hit_count += 1;
                if ray.flags.y == 0 {
                    assert!(
                        (gpu_t - expected.t).abs() < 0.0002 * expected.t.max(1.0),
                        "closest distance ray {index}: GPU={gpu_t}, CPU={expected:?}"
                    );
                }
                let candidate = CpuHit {
                    t: gpu_t,
                    triangle: gpu_triangle,
                    instance: gpu_instance as usize,
                };
                let actual = intersect(
                    ray,
                    &triangles[candidate.triangle],
                    &instances[candidate.instance],
                )
                .expect("GPU hit must identify a real eligible triangle");
                assert!(
                    (actual.t - candidate.t).abs() < 0.0002 * actual.t.max(1.0),
                    "reported primitive ray {index}: GPU={candidate:?}, CPU={actual:?}"
                );
                assert!(
                    (gpu_u - actual.u).abs() < 0.0002 && (gpu_v - actual.v).abs() < 0.0002,
                    "barycentrics ray {index}: GPU=({gpu_u},{gpu_v}), CPU={actual:?}"
                );
                let (normal, albedo) = surface_reference(
                    ray,
                    &triangles[candidate.triangle],
                    &instances[candidate.instance],
                    actual,
                );
                assert!(
                    read_vec3(32).as_dvec3().distance(normal) < 0.0003,
                    "normal ray {index}: GPU={:?}, CPU={normal:?}",
                    read_vec3(32)
                );
                assert!(
                    read_vec3(48).as_dvec3().distance(albedo) < 0.0003,
                    "albedo ray {index}: GPU={:?}, CPU={albedo:?}",
                    read_vec3(48)
                );
            }
        }
        assert!(hit_count > 100, "ensure meaningful intersection coverage");
        assert!(
            hit_count < rays.len() - 100,
            "ensure meaningful miss coverage"
        );
        eprintln!(
            "Validated {} rays ({hit_count} hits), closest/any-hit, transformed overlapping instances, category roots, slab boundaries, barycentrics and interpolated surface attributes",
            rays.len()
        );
    }

    /// Independent double-precision brute-force Moller-Trumbore reference: no BVH
    /// box tests, stack management or traversal ordering is shared with the shader.
    fn intersect(
        ray: &TestRay,
        triangle: &Triangle,
        instance: &GpuInstance,
    ) -> Option<CpuIntersection> {
        if ray.flags.x != 0
            && (instance.material.w > 0.5 || (ray.flags.y != 0 && instance.material.z > 0.5))
        {
            return None;
        }
        let o = instance
            .local_from_world
            .transform_point3(ray.origin.truncate())
            .as_dvec3();
        let d = instance
            .local_from_world
            .transform_vector3(ray.direction.truncate())
            .as_dvec3();
        let a = triangle.a.truncate().as_dvec3();
        let e1 = triangle.b.truncate().as_dvec3() - a;
        let e2 = triangle.c.truncate().as_dvec3() - a;
        let p = d.cross(e2);
        let det = e1.dot(p);
        if det.abs() < 1e-10 {
            return None;
        }
        let q0 = o - a;
        let u = q0.dot(p) / det;
        let q = q0.cross(e1);
        let v = d.dot(q) / det;
        let t = e2.dot(q) / det;
        ((-0.000001..=1.000001).contains(&u)
            && v >= -0.000001
            && u + v <= 1.000001
            && t > 0.0001
            && t < f64::from(ray.origin.w))
        .then_some(CpuIntersection { t, u, v })
    }

    fn surface_reference(
        ray: &TestRay,
        triangle: &Triangle,
        instance: &GpuInstance,
        hit: CpuIntersection,
    ) -> (bevy::math::DVec3, bevy::math::DVec3) {
        let weights = [1.0 - hit.u - hit.v, hit.u, hit.v];
        let interpolate = |values: [Vec4; 3]| {
            values
                .into_iter()
                .zip(weights)
                .map(|(value, weight)| value.truncate().as_dvec3() * weight)
                .sum::<bevy::math::DVec3>()
        };
        let normal_matrix = instance.local_from_world.as_dmat4().transpose();
        let mut normal = normal_matrix
            .transform_vector3(interpolate([triangle.n0, triangle.n1, triangle.n2]))
            .normalize();
        let a = triangle.a.truncate().as_dvec3();
        let geometric = normal_matrix.transform_vector3(
            (triangle.b.truncate().as_dvec3() - a).cross(triangle.c.truncate().as_dvec3() - a),
        );
        if geometric.dot(ray.direction.truncate().as_dvec3()) > 0.0 {
            normal = -normal;
        }
        let albedo = (interpolate([triangle.c0, triangle.c1, triangle.c2])
            * instance.tint.truncate().as_dvec3())
        .max(bevy::math::DVec3::ZERO);
        (normal, albedo)
    }

    #[test]
    fn brute_force_observes_explicit_upper_barycentric_bound() {
        let triangle = primitive(Vec3::ZERO, Vec3::X, Vec3::Y);
        let instance = instances()[0];
        let ray = TestRay {
            origin: Vec4::new(1.0000015, -0.00000075, 1.0, 20.0),
            direction: Vec3::NEG_Z.extend(0.0),
            flags: UVec4::ZERO,
        };
        assert!(intersect(&ray, &triangle, &instance).is_none());
        let inside_tolerance = TestRay {
            origin: Vec4::new(1.0000005, -0.00000075, 1.0, 20.0),
            ..ray
        };
        assert!(intersect(&inside_tolerance, &triangle, &instance).is_some());
    }

    fn brute_force(
        ray: &TestRay,
        triangles: &[Triangle],
        instances: &[GpuInstance],
    ) -> Option<CpuHit> {
        instances
            .iter()
            .enumerate()
            .flat_map(|(instance, transform)| {
                triangles
                    .iter()
                    .enumerate()
                    .filter_map(move |(triangle, primitive)| {
                        intersect(ray, primitive, transform).map(|hit| CpuHit {
                            t: hit.t,
                            triangle,
                            instance,
                        })
                    })
            })
            .min_by(|a, b| a.t.total_cmp(&b.t))
    }

    fn geometry() -> (Vec<Triangle>, Vec<BvhNode>) {
        let mut triangles = Vec::new();
        // A cube supplies coplanar shared edges, corners, parallel rays and rays
        // originating inside. A slanted triangle exercises all ray coordinates.
        for axis in 0..3 {
            for side in [-1.0, 1.0] {
                let mut corners = [Vec3::ZERO; 4];
                for (point, (a, b)) in
                    corners
                        .iter_mut()
                        .zip([(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)])
                {
                    point[axis] = side;
                    point[(axis + 1) % 3] = a;
                    point[(axis + 2) % 3] = b;
                }
                for [a, b, c] in [
                    [corners[0], corners[1], corners[2]],
                    [corners[0], corners[2], corners[3]],
                ] {
                    triangles.push(primitive(a, b, c));
                }
            }
        }
        triangles.push(primitive(
            Vec3::new(-0.7, -0.3, 1.3),
            Vec3::new(0.8, -0.2, 1.7),
            Vec3::new(0.1, 0.9, 1.2),
        ));
        let bounds: Vec<_> = triangles
            .iter()
            .enumerate()
            .map(|(i, t)| {
                (
                    i as u32,
                    t.a.truncate().min(t.b.truncate()).min(t.c.truncate()),
                    t.a.truncate().max(t.b.truncate()).max(t.c.truncate()),
                )
            })
            .collect();
        let mut nodes = Vec::new();
        append_tree(&mut nodes, &bounds);
        (triangles, nodes)
    }

    fn primitive(a: Vec3, b: Vec3, c: Vec3) -> Triangle {
        let n = (b - a).cross(c - a).normalize().extend(0.0);
        Triangle {
            a: a.extend(1.0),
            b: b.extend(1.0),
            c: c.extend(1.0),
            n0: (n.truncate() + Vec3::new(0.12, 0.17, 0.09))
                .normalize()
                .extend(0.0),
            n1: (n.truncate() + Vec3::new(-0.2, 0.07, 0.11))
                .normalize()
                .extend(0.0),
            n2: (n.truncate() + Vec3::new(0.08, -0.15, 0.05))
                .normalize()
                .extend(0.0),
            c0: Vec4::new(0.2 + a.x * 0.03, 0.35, 0.81, 1.0),
            c1: Vec4::new(0.71, 0.25 + b.y * 0.03, 0.1, 1.0),
            c2: Vec4::new(0.31, 0.81, 0.3 + c.z * 0.03, 1.0),
        }
    }

    fn instances() -> Vec<GpuInstance> {
        (0..8)
            .map(|i| {
                let world = Mat4::from_scale_rotation_translation(
                    if i == 0 {
                        Vec3::ONE
                    } else {
                        Vec3::new(
                            if i % 2 == 0 { -0.7 } else { 1.4 },
                            0.6 + i as f32 * 0.1,
                            0.8,
                        )
                    },
                    Quat::from_rotation_y(i as f32 * 0.19),
                    Vec3::new((i % 4) as f32 * 0.9, (i / 4) as f32 * 0.8, i as f32 * -0.17),
                );
                GpuInstance {
                    world_from_local: world,
                    local_from_world: world.inverse(),
                    material: Vec4::new(
                        0.5,
                        0.0,
                        if i == 6 { 1.0 } else { 0.0 },
                        if i == 7 { 1.0 } else { 0.0 },
                    ),
                    tint: Vec4::new(
                        0.5 + i as f32 * 0.06,
                        0.95 - i as f32 * 0.04,
                        0.6 + i as f32 * 0.03,
                        1.0,
                    ),
                    root: 0,
                    transmission: 0.0,
                    pad1: 0,
                    pad2: 0,
                }
            })
            .collect()
    }

    fn append_tree(nodes: &mut Vec<BvhNode>, members: &[(u32, Vec3, Vec3)]) -> u32 {
        fn fill(nodes: &mut Vec<BvhNode>, index: usize, members: &[(u32, Vec3, Vec3)]) {
            let lo = members
                .iter()
                .fold(Vec3::splat(f32::INFINITY), |v, m| v.min(m.1));
            let hi = members
                .iter()
                .fold(Vec3::splat(f32::NEG_INFINITY), |v, m| v.max(m.2));
            if members.len() == 1 {
                nodes[index] = BvhNode {
                    min: lo,
                    max: hi,
                    first: members[0].0,
                    count: 1,
                };
            } else {
                let first = nodes.len();
                nodes.extend([BvhNode::default(); 2]);
                nodes[index] = BvhNode {
                    min: lo,
                    max: hi,
                    first: first as u32,
                    count: 0,
                };
                let middle = members.len() / 2;
                fill(nodes, first, &members[..middle]);
                fill(nodes, first + 1, &members[middle..]);
            }
        }
        if members.is_empty() {
            return u32::MAX;
        }
        let root = nodes.len();
        nodes.push(BvhNode::default());
        fill(nodes, root, members);
        root as u32
    }

    fn rays() -> Vec<TestRay> {
        let mut base = Vec::new();
        for y in -12..=12 {
            for x in -12..=12 {
                base.push((
                    Vec3::new(x as f32 * 0.3, y as f32 * 0.3, 6.0),
                    Vec3::new(0.013, -0.021, -1.0),
                    20.0,
                ));
            }
        }
        for axis in 0..3 {
            for coordinate in [-1.0001, -1.0, 0.0, 1.0, 1.0001] {
                for sign in [-1.0, 1.0] {
                    let mut o = Vec3::splat(coordinate);
                    o[axis] = sign * 5.0;
                    let mut d = Vec3::ZERO;
                    d[axis] = -sign;
                    base.push((o, d, 20.0));
                    base.push((o, d, 0.01));
                    base.push((Vec3::ZERO, d, 20.0));
                }
            }
        }
        base.into_iter()
            .flat_map(|(origin, direction, limit)| {
                [(0, 0), (1, 0), (1, 1)].map(|(secondary, any_hit)| TestRay {
                    origin: origin.extend(limit),
                    direction: direction.extend(0.0),
                    flags: UVec4::new(secondary, any_hit, 0, 0),
                })
            })
            .collect()
    }
}
