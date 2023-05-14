use std::sync::Arc;

use rikka_core::nalgebra::Vector4;
use rikka_gpu::{buffer::Buffer, descriptor_set::DescriptorSet, image::Image};

use crate::renderer::*;

pub const INVALID_FLOAT_VALUE: f32 = f32::MAX;

bitflags::bitflags! {
    pub struct DrawFlags : u32 {
        const NONE = 0x0;
        const ALPHA_MASK = 0x1;
        const DOUBLE_SIDED = 0x2;
        const TRANSPARENT = 0x4;
        const HAS_NORMALS = 0x8;
        const HAS_TEXCOORDS = 0x10;
        const HAS_TANGENTS = 0x20;
        const HAS_JOINTS = 0x40;
        const HAS_WEIGHTS = 0x80;
        const ALPHA_DITHER = 0x100;
    }
}

pub struct PBRMaterial {
    pub material: Arc<Material>,
    /// Also used to store Mesh data
    pub material_buffer: Handle<Buffer>,
    pub descriptor_set: Arc<DescriptorSet>,

    pub diffuse_image: Option<Handle<Image>>,
    pub metallic_roughness_image: Option<Handle<Image>>,
    pub normal_image: Option<Handle<Image>>,
    pub occlusion_image: Option<Handle<Image>>,

    pub base_color_factor: Vector4<f32>,
    pub metallic_roughness_occlusion_factor: Vector4<f32>,
    pub alpha_cutoff: f32,
    pub draw_flags: DrawFlags,
}

impl PBRMaterial {
    pub fn new(
        material: Arc<Material>,
        material_buffer: Handle<Buffer>,
        descriptor_set: Arc<DescriptorSet>,
    ) -> Self {
        Self {
            material,
            material_buffer,
            descriptor_set,
            diffuse_image: None,
            metallic_roughness_image: None,
            normal_image: None,
            occlusion_image: None,
            base_color_factor: Vector4::new(0.0, 0.0, 0.0, 0.0),
            metallic_roughness_occlusion_factor: Vector4::new(0.0, 0.0, 0.0, 0.0),
            alpha_cutoff: INVALID_FLOAT_VALUE,
            draw_flags: DrawFlags::NONE,
        }
    }
}
