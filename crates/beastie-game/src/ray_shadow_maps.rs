//! Directional depth maps for dynamic casters. The source geometry
//! remains exact; depth-map visibility is a sampled approximation to ray queries.
use crate::ray_scene::RayScene;
use bevy::{prelude::*, render::render_resource::ShaderType};

pub const RESOLUTION: u32 = 1024;
/// Depth32Float sampling and attachments require no optional adapter features.
/// A one-layer dummy texture keeps the shared layout valid on the ray fallback.
pub fn supported(limits: &wgpu::Limits) -> bool {
    limits.max_texture_dimension_2d >= RESOLUTION && limits.max_texture_array_layers >= 12
}

#[derive(Clone, Default, ShaderType)]
pub struct ShadowMapParams {
    pub clip_from_world: [Mat4; 12],
    /// x=min light-space z,y=depth span; other components reserved.
    pub depth_ranges: [Vec4; 12],
    /// enabled,resolution,layers,caster_count.
    pub info: UVec4,
}

pub fn directions() -> [Vec3; 12] {
    static DIRECTIONS: std::sync::OnceLock<[Vec3; 12]> = std::sync::OnceLock::new();
    *DIRECTIONS.get_or_init(|| {
        let source = include_str!("raytrace.wgsl");
        let (_, array) = source
            .split_once("const SHADOW_LIGHTS=array<vec3<f32>,12>(")
            .expect("original light table");
        let (array, _) = array
            .split_once("const CACHE_BOUNCE")
            .expect("end light table");
        let values: Vec<Vec3> = array
            .split("vec3(")
            .skip(1)
            .map(|entry| {
                let (xyz, _) = entry.split_once(')').unwrap();
                let values: Vec<f32> = xyz.split(',').map(|v| v.trim().parse().unwrap()).collect();
                Vec3::new(values[0], values[1], values[2])
            })
            .collect();
        values.try_into().expect("twelve shader light directions")
    })
}

pub fn dynamic_casters(scene: &RayScene) -> Vec<usize> {
    scene
        .instances
        .iter()
        .enumerate()
        .filter_map(|(id, instance)| {
            (instance.pad1 == 0 && instance.material.w <= 0.5 && instance.material.z <= 0.5)
                .then_some(id)
        })
        .collect()
}

pub fn fit(scene: &RayScene, enabled: bool) -> (ShadowMapParams, Vec<usize>) {
    let casters = dynamic_casters(scene);
    let mut params = ShadowMapParams {
        clip_from_world: [Mat4::IDENTITY; 12],
        depth_ranges: [Vec4::Y; 12],
        info: UVec4::new(u32::from(enabled), RESOLUTION, 12, casters.len() as u32),
    };
    if !enabled {
        return (params, casters);
    }
    let points: Vec<Vec3> = casters
        .iter()
        .flat_map(|&id| {
            let instance = scene.instances[id];
            let chunk = scene
                .geometry
                .iter()
                .find(|chunk| chunk.node_offset == instance.root)
                .unwrap();
            let bounds = chunk.nodes[0];
            (0..8).map(move |corner| {
                instance.world_from_local.transform_point3(Vec3::new(
                    if corner & 1 == 0 {
                        bounds.min.x
                    } else {
                        bounds.max.x
                    },
                    if corner & 2 == 0 {
                        bounds.min.y
                    } else {
                        bounds.max.y
                    },
                    if corner & 4 == 0 {
                        bounds.min.z
                    } else {
                        bounds.max.z
                    },
                ))
            })
        })
        .collect();
    if points.is_empty() {
        return (params, casters);
    }
    for (index, light) in directions().into_iter().enumerate() {
        let u = Vec3::Y.cross(light).normalize();
        let v = light.cross(u).normalize();
        // Use the table vector itself as the depth coordinate. Its squared length
        // is retained to convert projected depth differences back to ray t.
        let project = |p: Vec3| Vec3::new(p.dot(u), p.dot(v), p.dot(light));
        let lo = points
            .iter()
            .fold(Vec3::splat(f32::INFINITY), |a, &p| a.min(project(p)))
            - Vec3::splat(0.001);
        let hi = points
            .iter()
            .fold(Vec3::splat(f32::NEG_INFINITY), |a, &p| a.max(project(p)))
            + Vec3::splat(0.001);
        let span = hi - lo;
        let x = (u * (2.0 / span.x)).extend(-(hi.x + lo.x) / span.x);
        let y = (v * (2.0 / span.y)).extend(-(hi.y + lo.y) / span.y);
        let z = (light / span.z).extend(-lo.z / span.z);
        params.clip_from_world[index] = Mat4::from_cols(x, y, z, Vec4::W).transpose();
        params.depth_ranges[index] = Vec4::new(lo.z, span.z, light.length_squared(), 0.0);
    }
    (params, casters)
}

#[test]
fn light_table_and_fit_are_finite_and_conservative() {
    let (scene, _) = crate::ray_benchmark::scene_snapshot();
    let (params, casters) = fit(&scene, true);
    assert!(!casters.is_empty());
    for (index, direction) in directions().into_iter().enumerate() {
        assert!((direction.length_squared() - 1.0).abs() < 1e-6);
        assert!(params.clip_from_world[index].is_finite());
        assert!(params.depth_ranges[index].y > 0.0);
        for &id in &casters {
            let instance = scene.instances[id];
            let chunk = scene
                .geometry
                .iter()
                .find(|chunk| chunk.node_offset == instance.root)
                .unwrap();
            for triangle in chunk.triangles.iter() {
                for vertex in [triangle.a, triangle.b, triangle.c] {
                    let clip = params.clip_from_world[index]
                        * instance.world_from_local
                        * vertex.truncate().extend(1.0);
                    assert!(
                        clip.x.abs() <= 1.00001
                            && clip.y.abs() <= 1.00001
                            && clip.z >= -0.00001
                            && clip.z <= 1.00001
                    );
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    pub enabled: bool,
    pub reason: &'static str,
}

/// Fixed maps are accepted only within the viewport range reviewed in native
/// motion. Larger viewports preserve shadow density with original ray traversal.
pub fn select(
    limits: &wgpu::Limits,
    size: UVec2,
    requested: bool,
    shadows: bool,
    pipeline_ready: bool,
) -> Selection {
    let reason = if !requested {
        "diagnostic-ray-control"
    } else if !shadows {
        "shadows-disabled"
    } else if !supported(limits) {
        "adapter-limits"
    } else if size.x == 0 || size.y == 0 || size.x > 1920 || size.y > 1080 {
        "viewport-outside-validated-range"
    } else if !pipeline_ready {
        "pipeline-warming"
    } else {
        "validated-viewport"
    };
    Selection {
        enabled: reason == "validated-viewport",
        reason,
    }
}

#[test]
fn policy_preserves_ray_fallback_for_large_views_limits_and_startup() {
    let limits = wgpu::Limits::default();
    for size in [
        UVec2::new(1280, 720),
        UVec2::new(1920, 1080),
        UVec2::new(1440, 1080),
    ] {
        assert!(select(&limits, size, true, true, true).enabled);
        assert_eq!(
            select(&limits, size, true, true, false).reason,
            "pipeline-warming"
        );
    }
    for size in [
        UVec2::ZERO,
        UVec2::new(1921, 1080),
        UVec2::new(1920, 1081),
        UVec2::new(1080, 1920),
        UVec2::new(3840, 2160),
    ] {
        assert!(!select(&limits, size, true, true, true).enabled);
    }
    let size = UVec2::new(1920, 1080);
    let mut limited = limits.clone();
    limited.max_texture_array_layers = 1;
    assert_eq!(
        select(&limited, size, true, true, true).reason,
        "adapter-limits"
    );
    limited = limits;
    limited.max_texture_dimension_2d = 512;
    assert_eq!(
        select(&limited, size, true, true, true).reason,
        "adapter-limits"
    );
    assert_eq!(
        select(&limited, size, false, true, true).reason,
        "diagnostic-ray-control"
    );
    assert_eq!(
        select(&limited, size, true, false, true).reason,
        "shadows-disabled"
    );
}
