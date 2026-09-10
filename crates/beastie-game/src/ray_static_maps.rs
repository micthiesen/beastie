//! Persistent directional depth maps for static casters shading moving receivers.
use bevy::render::render_resource::encase;
#[cfg(test)]
pub const RESOLUTION: u32 = 512;
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth16Unorm;
pub const DEPTH_BYTES: u64 = 2;

pub(crate) fn encoded<
    T: bevy::render::render_resource::ShaderType + encase::internal::WriteInto,
>(
    value: &T,
) -> Vec<u8> {
    let mut encoded = encase::StorageBuffer::new(Vec::new());
    encoded.write(value).expect("production shader layout");
    encoded.into_inner()
}
#[cfg(test)]
use crate::ray_shadow_maps::ShadowMapParams;
use crate::{ray_scene::RayScene, ray_shadow_maps};
use wgpu::util::DeviceExt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StaticMapKey {
    revision: u64,
    enabled: bool,
    resolution: u32,
}
impl StaticMapKey {
    pub fn new(revision: u64, enabled: bool, resolution: u32) -> Self {
        Self {
            revision,
            enabled,
            resolution: if enabled { resolution } else { 1 },
        }
    }
}

pub struct StaticMaps {
    pub key: StaticMapKey,
    #[cfg(test)]
    pub params: ShadowMapParams,
    pub uniform: wgpu::Buffer,
    pub view: wgpu::TextureView,
    #[cfg(test)]
    pub revision: u64,
    pub resolution: u32,
    pub enabled: bool,
    #[cfg(test)]
    pub caster_count: usize,
    _texture: wgpu::Texture,
    layers: Vec<wgpu::TextureView>,
    matrices: Vec<wgpu::Buffer>,
}
impl StaticMaps {
    pub fn new(device: &wgpu::Device, scene: &RayScene, enabled: bool, resolution: u32) -> Self {
        let key = StaticMapKey::new(scene.static_revision, enabled, resolution);
        let resolution = key.resolution;
        let (params, _casters) = ray_shadow_maps::fit_static(scene, enabled, resolution);
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("persistent static shadow layers"),
            size: wgpu::Extent3d {
                width: resolution,
                height: resolution,
                depth_or_array_layers: if enabled { 12 } else { 1 },
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            aspect: wgpu::TextureAspect::DepthOnly,
            ..Default::default()
        });
        let layers = (0..if enabled { 12 } else { 1 })
            .map(|i| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    aspect: wgpu::TextureAspect::DepthOnly,
                    base_array_layer: i,
                    array_layer_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("static shadow projections"),
            contents: &encoded(&params),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let matrices = params
            .clip_from_world
            .iter()
            .map(|m| {
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: None,
                    contents: &encoded(m),
                    usage: wgpu::BufferUsages::UNIFORM,
                })
            })
            .collect();
        Self {
            key,
            #[cfg(test)]
            params,
            uniform,
            view,
            #[cfg(test)]
            revision: scene.static_revision,
            resolution,
            enabled,
            #[cfg(test)]
            caster_count: _casters.len(),
            _texture: texture,
            layers,
            matrices,
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn record(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        pipeline: &wgpu::RenderPipeline,
        scene: &RayScene,
        geometry: &wgpu::Buffer,
        instances: &wgpu::Buffer,
        timestamp: Option<(&wgpu::QuerySet, u32)>,
    ) {
        if !self.enabled {
            return;
        }
        for layer in 0..12 {
            let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &pipeline.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: geometry.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: instances.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: self.matrices[layer].as_entire_binding(),
                    },
                ],
            });
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("persistent static shadow fill"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.layers[layer],
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: timestamp.filter(|_| layer == 0).map(|(queries, index)| {
                    wgpu::RenderPassTimestampWrites {
                        query_set: queries,
                        beginning_of_pass_write_index: Some(index),
                        end_of_pass_write_index: None,
                    }
                }),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &bind, &[]);
            for (index, instance) in scene
                .instances
                .iter()
                .enumerate()
                .filter(|(_, i)| i.pad1 != 0 && i.material.w <= 0.5 && i.material.z <= 0.5)
            {
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
    pub fn bytes(&self) -> u64 {
        u64::from(self.resolution).pow(2) * if self.enabled { 12 } else { 1 } * DEPTH_BYTES
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn static_map_key_invalidates_scene_mode_and_density_but_normalizes_dummy() {
        let key = StaticMapKey::new(7, true, 512);
        assert_eq!(key, StaticMapKey::new(7, true, 512));
        assert_ne!(key, StaticMapKey::new(8, true, 512));
        assert_ne!(key, StaticMapKey::new(7, false, 512));
        assert_ne!(key, StaticMapKey::new(7, true, 1024));
        assert_eq!(
            StaticMapKey::new(7, false, 512),
            StaticMapKey::new(7, false, 1024)
        );
    }
}
