//! Directional depth maps for static and dynamic casters. Dynamic shadow meshes
//! may use bounded LOD; both depth sampling and LOD approximate ray visibility.
use crate::ray_scene::RayScene;
use bevy::{prelude::*, render::render_resource::ShaderType};

#[cfg(test)]
pub const RESOLUTION: u32 = 1024;

#[derive(Clone, Default, ShaderType)]
pub struct ShadowMapParams {
    pub clip_from_world: [Mat4; 12],
    /// x=min light-space z,y=depth span; other components reserved.
    pub depth_ranges: [Vec4; 12],
    /// enabled, resolution, layout (0=array,1=4x3 atlas), caster_count.
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

pub fn density(size: UVec2) -> Option<(u32, u32)> {
    // Viewport::for_drawable fits the authored 16:9 scene uniformly. Margins on
    // wide/tall windows do not increase its shadow texel density.
    // Caps bound Depth16 dynamic atlas + static layers to 120 MiB at 4K.
    if size.x == 0 || size.y == 0 || size.x > 3840 || size.y > 2160 {
        return None;
    }
    let ratio = (size.x as f64 / 1920.0).min(size.y as f64 / 1080.0);
    let dynamic = ((1024.0 * ratio).ceil() as u32)
        .next_power_of_two()
        .clamp(512, 2048);
    Some((dynamic, dynamic / 2))
}
pub fn fit(scene: &RayScene, enabled: bool, resolution: u32) -> (ShadowMapParams, Vec<usize>) {
    fit_casters(scene, enabled, dynamic_casters(scene), resolution)
}

pub fn fit_static(
    scene: &RayScene,
    enabled: bool,
    resolution: u32,
) -> (ShadowMapParams, Vec<usize>) {
    let casters = scene
        .instances
        .iter()
        .enumerate()
        .filter_map(|(id, instance)| {
            (instance.pad1 != 0 && instance.material.w <= 0.5 && instance.material.z <= 0.5)
                .then_some(id)
        })
        .collect();
    fit_casters(scene, enabled, casters, resolution)
}

fn fit_casters(
    scene: &RayScene,
    enabled: bool,
    casters: Vec<usize>,
    resolution: u32,
) -> (ShadowMapParams, Vec<usize>) {
    let mut params = ShadowMapParams {
        clip_from_world: [Mat4::IDENTITY; 12],
        depth_ranges: [Vec4::Y; 12],
        info: UVec4::new(u32::from(enabled), resolution, 0, casters.len() as u32),
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
    for (params, casters) in [
        fit(&scene, true, RESOLUTION),
        fit_static(&scene, true, crate::ray_static_maps::RESOLUTION),
    ] {
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
    let map_density = density(size);
    let reason = if !requested {
        "diagnostic-ray-control"
    } else if !shadows {
        "shadows-disabled"
    } else if map_density.is_none() {
        "viewport-outside-density-budget"
    } else if limits.max_texture_array_layers < 12
        || map_density
            .is_some_and(|(resolution, _)| limits.max_texture_dimension_2d < resolution * 4)
    {
        "adapter-limits"
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
fn density_keeps_pixel_ratio_with_power_of_two_budget() {
    assert_eq!(density(UVec2::new(640, 360)), Some((512, 256)));
    assert_eq!(density(UVec2::new(1920, 1080)), Some((1024, 512)));
    assert_eq!(density(UVec2::new(3840, 2160)), Some((2048, 1024)));
    assert_eq!(density(UVec2::new(3840, 1080)), Some((1024, 512)));
    assert_eq!(density(UVec2::new(1920, 2160)), Some((1024, 512)));
    assert_eq!(density(UVec2::new(1080, 1920)), Some((1024, 512)));
    assert_eq!(density(UVec2::new(3841, 2160)), None);
    assert_eq!(density(UVec2::ZERO), None);
}

#[test]
fn density_policy_preserves_limits_startup_and_resize_fallback() {
    let mut limits = wgpu::Limits {
        max_texture_dimension_2d: 8192,
        ..Default::default()
    };
    for size in [
        UVec2::new(640, 360),
        UVec2::new(1920, 1080),
        UVec2::new(3840, 2160),
        UVec2::new(3840, 1080),
    ] {
        assert!(select(&limits, size, true, true, true).enabled);
        assert_eq!(
            select(&limits, size, true, true, false).reason,
            "pipeline-warming"
        );
    }
    for size in [UVec2::ZERO, UVec2::new(3841, 2160), UVec2::new(3840, 2161)] {
        assert_eq!(
            select(&limits, size, true, true, true).reason,
            "viewport-outside-density-budget"
        );
    }
    limits.max_texture_dimension_2d = 4096;
    assert!(!select(&limits, UVec2::new(3840, 2160), true, true, true).enabled);
    assert!(select(&limits, UVec2::new(1920, 1080), true, true, true).enabled);
    limits.max_texture_dimension_2d = 2048;
    assert!(select(&limits, UVec2::new(640, 360), true, true, true).enabled);
    limits.max_texture_array_layers = 1;
    let size = UVec2::new(640, 360);
    assert_eq!(
        select(&limits, size, true, true, true).reason,
        "adapter-limits"
    );
    assert_eq!(
        select(&limits, size, false, true, true).reason,
        "diagnostic-ray-control"
    );
    assert_eq!(
        select(&limits, size, true, false, true).reason,
        "shadows-disabled"
    );
}
