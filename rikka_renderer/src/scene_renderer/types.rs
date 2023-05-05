use std::sync::Arc;

use rikka_core::{
    nalgebra::{Matrix4, Vector4},
    vk,
};
use rikka_gpu::{buffer::Buffer, command_buffer::CommandBuffer, descriptor_set::DescriptorSet};

use crate::{renderer::*, scene};

pub struct PBRMaterial {
    pub material: Arc<Material>,
    pub material_buffer: Handle<Buffer>,
    pub descriptor_set: Handle<DescriptorSet>,

    pub diffuse_texture_index: u32,
    pub roughness_texture_index: u32,
    pub normal_texture_index: u32,
    pub occlusion_texture_index: u32,

    pub base_color_factor: Vector4<f32>,
    pub metallic_roughness_occlusion_factor: Vector4<f32>,
    pub alpha_cutoff: f32,
    pub flags: u32,
}

pub struct Mesh {
    pub position_buffer: Option<Handle<Buffer>>,
    pub index_buffer: Option<Handle<Buffer>>,
    pub tex_coords_buffer: Option<Handle<Buffer>>,
    pub normal_buffer: Option<Handle<Buffer>>,
    pub tangent_buffer: Option<Handle<Buffer>>,

    pub pbr_material: PBRMaterial,

    pub primitive_count: u32,

    pub position_offset: u32,
    pub tex_coords_offset: u32,
    pub normal_offset: u32,
    pub tangent_offset: u32,

    pub index_offset: u32,
    pub index_type: vk::IndexType,

    pub scene_graph_node_index: usize,
}

impl Mesh {
    pub fn create_gpu_data(&self) -> GPUMeshData {
        GPUMeshData {
            global_model: Matrix4::identity(),
            global_inverse_model: Matrix4::identity(),
            diffuse_texture_index: self.pbr_material.diffuse_texture_index,
            roughness_texture_index: self.pbr_material.roughness_texture_index,
            normal_texture_index: self.pbr_material.normal_texture_index,
            occlusion_texture_index: self.pbr_material.occlusion_texture_index,
            base_color_factor: self.pbr_material.base_color_factor,
            metallic_roughness_occlusion_factor: self
                .pbr_material
                .metallic_roughness_occlusion_factor,
            alpha_cutoff: self.pbr_material.alpha_cutoff,
            flags: self.pbr_material.flags,
        }
    }

    pub fn draw(&self, command_buffer: &CommandBuffer) {
        todo!()
    }
}

pub struct MeshInstance {
    pub mesh: Arc<Mesh>,
    pub material_pass_index: usize,
}

pub struct GPUMeshData {
    pub global_model: Matrix4<f32>,
    pub global_inverse_model: Matrix4<f32>,

    pub diffuse_texture_index: u32,
    pub roughness_texture_index: u32,
    pub normal_texture_index: u32,
    pub occlusion_texture_index: u32,

    pub base_color_factor: Vector4<f32>,
    pub metallic_roughness_occlusion_factor: Vector4<f32>,
    pub alpha_cutoff: f32,
    pub flags: u32,
}

impl GPUMeshData {
    pub fn set_matrices(&mut self, mesh: &Mesh, scene_graph: &scene::Graph) {
        self.global_model = scene_graph.global_matrices[mesh.scene_graph_node_index];
        self.global_inverse_model = self.global_model.try_inverse().unwrap();
    }
}
