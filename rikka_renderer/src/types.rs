use bitflags::bitflags;

use rikka_core::nalgebra::{Matrix4, Vector4};

#[derive(Clone, Copy)]
#[repr(C)]
pub struct MaterialData {
    pub base_color_factor: Vector4<f32>,
    pub diffuse_texture: u32,
    // Occulsion metallic roughness
    pub omr_texture: u32,
    pub normal_texture: u32,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct Vertex {
    pub positions: [f32; 3],
    // pub tex_coords: [f32; 2],
}

bitflags! {
    pub struct DrawFlags : u32 {
        const ALPHA_MASK = 0x1;
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct MeshData {
    pub model: Matrix4<f32>,
    pub inverse_model: Matrix4<f32>,

    pub texture_indices: [u32; 4],
    pub base_color_factor: Vector4<f32>,
    pub omr_factor: Vector4<f32>,
    pub alpha_cutoff: f32,

    padding: [f32; 3],

    pub flags: u32,
}
