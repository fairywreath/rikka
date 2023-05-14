use std::sync::Arc;

use rikka_core::{
    nalgebra::{Matrix4, Vector4},
    vk,
};
use rikka_gpu::{
    buffer::Buffer, command_buffer::CommandBuffer, constants::INVALID_BINDLESS_TEXTURE_INDEX,
    image::Image, pipeline::GraphicsPipeline,
};

use crate::{renderer::*, scene, scene_renderer::material::*};

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

    pub meshlet_offset: u32,
    pub meshlet_count: u32,
    pub gpu_mesh_index: u32,

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
            meshlet_offset: u32::MAX,
            meshlet_count: u32::MAX,
            gpu_mesh_index: u32::MAX,
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

    pub fn create_gpu_data(&self) -> GpuMeshData {
        GpuMeshData {
            global_model: Matrix4::identity(),
            global_inverse_model: Matrix4::identity(),
            base_color_factor: self.pbr_material.base_color_factor,
            diffuse_texture_index: Self::get_texture_index(&self.pbr_material.diffuse_image),
            metallic_roughness_texture_index: Self::get_texture_index(
                &self.pbr_material.metallic_roughness_image,
            ),
            normal_texture_index: Self::get_texture_index(&self.pbr_material.normal_image),
            occlusion_texture_index: Self::get_texture_index(&self.pbr_material.occlusion_image),
            // metallic_roughness_occlusion_factor: self
            //     .pbr_material
            //     .metallic_roughness_occlusion_factor,
            // alpha_cutoff: self.pbr_material.alpha_cutoff,
            // flags: self.pbr_material.draw_flags.bits(),
        }
    }

    pub fn draw(
        &self,
        command_buffer: &CommandBuffer,
        graphics_pipeline: &GraphicsPipeline,
        zero_buffer: &Buffer,
    ) {
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
        if let Some(tangent_buffer) = &self.tangent_buffer {
            command_buffer.bind_vertex_buffer(tangent_buffer, 3, self.tangent_offset as _);
        } else {
            command_buffer.bind_vertex_buffer(&zero_buffer, 3, 0);
        }

        command_buffer
            .bind_index_buffer(self.index_buffer.as_ref().unwrap(), self.index_offset as _);

        // XXX: From where should we access the graphics pipeline layout?
        command_buffer.bind_descriptor_set(
            &self.pbr_material.descriptor_set,
            graphics_pipeline.raw_layout(),
            0,
        );

        command_buffer.draw_indexed(self.primitive_count, 1, 0, 0, 0);
    }

    pub fn transparent(&self) -> bool {
        self.pbr_material
            .draw_flags
            .contains(DrawFlags::TRANSPARENT)
    }
}

#[derive(Clone)]
pub struct MeshInstance {
    pub mesh: Arc<Mesh>,
    pub material_pass_index: usize,
    pub gpu_mesh_instance_index: usize,
    pub scene_graph_node_index: usize,
}

impl MeshInstance {
    pub fn new(mesh: Arc<Mesh>, material_pass_index: usize) -> Self {
        Self {
            mesh,
            material_pass_index,
            gpu_mesh_instance_index: usize::MAX,
            scene_graph_node_index: usize::MAX,
        }
    }

    pub fn new_with_indices(
        mesh: Arc<Mesh>,
        material_pass_index: usize,
        gpu_mesh_instance_index: usize,
        scene_graph_node_index: usize,
    ) -> Self {
        Self {
            mesh,
            material_pass_index,
            gpu_mesh_instance_index,
            scene_graph_node_index,
        }
    }
}

#[derive(Clone)]
pub struct MeshInstanceDraw {
    pub mesh_instance: Arc<MeshInstance>,
    // pub material_pass_index: usize,
}

impl MeshInstanceDraw {
    pub fn new(mesh_instance: Arc<MeshInstance>, material_pass_index: usize) -> Self {
        Self {
            mesh_instance,
            // material_pass_index,
        }
    }
}

/// Mesh material data
#[derive(Clone, Copy)]
#[repr(C)]
pub struct GpuMeshData {
    pub global_model: Matrix4<f32>,
    pub global_inverse_model: Matrix4<f32>,

    pub base_color_factor: Vector4<f32>,

    pub diffuse_texture_index: u32,
    pub metallic_roughness_texture_index: u32,
    pub normal_texture_index: u32,
    pub occlusion_texture_index: u32,
    // pub metallic_roughness_occlusion_factor: Vector4<f32>,
    // pub alpha_cutoff: f32,
    // pub flags: u32,
}

impl GpuMeshData {
    pub fn set_matrices_from_scene_graph(&mut self, mesh: &Mesh, scene_graph: &scene::Graph) {
        self.global_model = scene_graph.global_matrices[mesh.scene_graph_node_index];
        self.global_inverse_model = self.global_model.try_inverse().unwrap();
    }
}
