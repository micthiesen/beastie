//! Presentation-only diagnostics and authored surface materials.
use bevy::prelude::*;
use clap::ValueEnum;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub enum SurfaceTreatment {
    Sharp,
    #[default]
    Beveled,
    Separated,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub enum LightingStudy {
    #[default]
    Finished,
    Unlit,
    NoShadows,
    Clay,
}

#[derive(Clone, Copy)]
pub enum SurfaceMaterial {
    Skin,
    Eye,
    Stone,
    Plant,
    Rubber,
    Cloth,
    Brass,
    Food,
}

#[derive(
    Resource, Debug, Clone, Copy, Default, bevy::render::extract_resource::ExtractResource,
)]
pub struct RenderAppearance {
    pub treatment: SurfaceTreatment,
    pub study: LightingStudy,
}

impl RenderAppearance {
    pub fn surface(self, kind: SurfaceMaterial) -> StandardMaterial {
        let (roughness, metallic) = match kind {
            SurfaceMaterial::Skin => (0.58, 0.0),
            SurfaceMaterial::Eye => (0.24, 0.0),
            SurfaceMaterial::Stone => (0.94, 0.0),
            SurfaceMaterial::Plant => (0.66, 0.0),
            SurfaceMaterial::Rubber => (0.68, 0.0),
            SurfaceMaterial::Cloth => (0.98, 0.0),
            SurfaceMaterial::Brass => (0.36, 0.72),
            SurfaceMaterial::Food => (0.60, 0.0),
        };
        let mut material = self.material(roughness, metallic);
        if matches!(kind, SurfaceMaterial::Skin) && self.study != LightingStudy::Clay {
            // Soft diffuse fill keeps tiny surface occluders from reading as dark skin seams.
            material.diffuse_transmission = 0.4;
        }
        material
    }
    pub fn style(self) -> crate::voxel::SurfaceStyle {
        match self.treatment {
            SurfaceTreatment::Sharp => crate::voxel::SurfaceStyle::Sharp,
            SurfaceTreatment::Beveled => crate::voxel::SurfaceStyle::Beveled,
            SurfaceTreatment::Separated => crate::voxel::SurfaceStyle::Separated,
        }
    }

    pub fn shadows(self) -> bool {
        !matches!(self.study, LightingStudy::Unlit | LightingStudy::NoShadows)
    }

    pub fn material(self, roughness: f32, metallic: f32) -> StandardMaterial {
        StandardMaterial {
            perceptual_roughness: if self.study == LightingStudy::Clay {
                0.8
            } else {
                roughness
            },
            metallic: if self.study == LightingStudy::Clay {
                0.0
            } else {
                metallic
            },
            unlit: self.study == LightingStudy::Unlit,
            ..default()
        }
    }

    pub fn mesh(self, mut mesh: Mesh) -> Mesh {
        if self.study == LightingStudy::Clay {
            let color = Color::srgb(0.72, 0.72, 0.72).to_linear().to_f32_array();
            let colors = vec![color; mesh.count_vertices()];
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
        }
        mesh
    }
}
