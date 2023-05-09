use std::sync::Arc;

use rikka_core::{
    nalgebra::{Matrix4, Vector4},
    vk,
};
use rikka_gpu::{
    buffer::Buffer, command_buffer::CommandBuffer, constants::INVALID_BINDLESS_TEXTURE_INDEX,
    descriptor_set::DescriptorSet, image::Image,
};

use crate::{renderer::*, scene};

pub const INVALID_FLOAT_VALUE: f32 = f32::MAX;

bitflags::bitflags! {
    pub struct DrawFlags : u32 {
        const NONE = 0x1;
        const ALPHA_MASK = 0x1;
        const DOUBLE_SIDED = 0x2;
        const TRANSPARENT = 0x3;
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

pub struct Mesh {
    pub pbr_material: PBRMaterial,

    pub position_buffer: Option<Handle<Buffer>>,
    pub tex_coords_buffer: Option<Handle<Buffer>>,
    pub normal_buffer: Option<Handle<Buffer>>,
    pub tangent_buffer: Option<Handle<Buffer>>,
    pub index_buffer: Option<Handle<Buffer>>,

    pub primitive_count: u32,

    pub position_offset: u32,
    pub tex_coords_offset: u32,
    pub normal_offset: u32,
    pub tangent_offset: u32,

    pub index_offset: u32,
    pub index_type: vk::IndexType,

    pub transparent: bool,

    pub scene_graph_node_index: usize,
}

impl Mesh {
    pub fn new_with_pbr_material(pbr_material: PBRMaterial) -> Self {
        Self {
            pbr_material,
            position_buffer: None,
            tex_coords_buffer: None,
            normal_buffer: None,
            tangent_buffer: None,
            index_buffer: None,
            primitive_count: 0,
            position_offset: 0,
            tex_coords_offset: 0,
            normal_offset: 0,
            tangent_offset: 0,
            index_offset: 0,
            index_type: vk::IndexType::UINT16,
            transparent: false,
            scene_graph_node_index: scene::INVALID_INDEX,
        }
    }

    fn get_texture_index(image_handle: &Option<Handle<Image>>) -> u32 {
        if let Some(image) = image_handle {
            image.bindless_index()
        } else {
            INVALID_BINDLESS_TEXTURE_INDEX
        }
    }

    pub fn create_gpu_data(&self) -> GPUMeshData {
        GPUMeshData {
            global_model: Matrix4::identity(),
            global_inverse_model: Matrix4::identity(),
            diffuse_texture_index: Self::get_texture_index(&self.pbr_material.diffuse_image),
            metallic_roughness_texture_index: Self::get_texture_index(
                &self.pbr_material.metallic_roughness_image,
            ),
            normal_texture_index: Self::get_texture_index(&self.pbr_material.normal_image),
            occlusion_texture_index: Self::get_texture_index(&self.pbr_material.occlusion_image),
            base_color_factor: self.pbr_material.base_color_factor,
            metallic_roughness_occlusion_factor: self
                .pbr_material
                .metallic_roughness_occlusion_factor,
            alpha_cutoff: self.pbr_material.alpha_cutoff,
            flags: self.pbr_material.draw_flags.bits(),
        }
    }

    pub fn draw(&self, command_buffer: &CommandBuffer) {
        command_buffer.bind_vertex_buffer(
            self.position_buffer.as_ref().unwrap(),
            0,
            self.position_offset as _,
        );
        command_buffer.bind_vertex_buffer(
            self.tex_coords_buffer.as_ref().unwrap(),
            1,
            self.tex_coords_offset as _,
        );
        command_buffer.bind_vertex_buffer(
            self.normal_buffer.as_ref().unwrap(),
            2,
            self.normal_offset as _,
        );

        // XXX: From where should we access the zero buffer?
        // if let Some(tangent_buffer) = &self.tangent_buffer {
        //     command_buffer.bind_vertex_buffer(tangent_buffer, 3, self.tangent_offset as _);
        // } else {
        //     command_buffer.bind_vertex_buffer(&self.zero_buffer, 3, 0);
        // }

        command_buffer
            .bind_index_buffer(self.index_buffer.as_ref().unwrap(), self.index_offset as _);

        // XXX: From where should we access the graphics pipeline layout?
        // command_buffer.bind_descriptor_set(
        //     self.descriptor_set.as_ref().unwrap(),
        //     graphics_pipeline.raw_layout(),
        //     0,
        // );

        command_buffer.draw_indexed(self.primitive_count, 1, 0, 0, 0);
    }

    pub fn transparent(&self) -> bool {
        self.transparent
    }
}

pub struct MeshInstance {
    pub mesh: Arc<Mesh>,
    pub material_pass_index: usize,
}

impl MeshInstance {
    pub fn new(mesh: Arc<Mesh>, material_pass_index: usize) -> Self {
        Self {
            mesh,
            material_pass_index,
        }
    }
}

#[derive(Clone, Copy)]
pub struct GPUMeshData {
    pub global_model: Matrix4<f32>,
    pub global_inverse_model: Matrix4<f32>,

    pub diffuse_texture_index: u32,
    pub metallic_roughness_texture_index: u32,
    pub normal_texture_index: u32,
    pub occlusion_texture_index: u32,

    pub base_color_factor: Vector4<f32>,
    pub metallic_roughness_occlusion_factor: Vector4<f32>,
    pub alpha_cutoff: f32,
    pub flags: u32,
}

impl GPUMeshData {
    pub fn set_matrices_from_scene_graph(&mut self, mesh: &Mesh, scene_graph: &scene::Graph) {
        self.global_model = scene_graph.global_matrices[mesh.scene_graph_node_index];
        self.global_inverse_model = self.global_model.try_inverse().unwrap();
    }
}
