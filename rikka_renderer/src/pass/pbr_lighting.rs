use std::{mem::size_of, sync::Arc};

use anyhow::Result;

use rikka_core::vk;
use rikka_gpu::{buffer::*, command_buffer::CommandBuffer, descriptor_set::*};
use rikka_graph::{graph::Graph, types::*};

use crate::{renderer::*, scene_renderer::types::*, types::*};

pub struct PBRLightingPass {
    /// Fullscreen mesh
    mesh: Mesh,
}

impl PBRLightingPass {
    pub fn new(
        renderer: &Renderer,
        render_graph: &Graph,
        scene_uniform_buffer: Handle<Buffer>,
        // XXX: This can be stored/created internally?
        render_technique: &Arc<RenderTechnique>,
    ) -> Result<Self> {
        let material_desc =
            MaterialDesc::new(0, render_technique.clone(), String::from("pbr_lighting"));
        let material = renderer.create_material(material_desc)?;

        // XXX: Use dynamic uniform buffer
        let material_buffer_desc = BufferDesc::new()
            .set_usage_flags(vk::BufferUsageFlags::UNIFORM_TEXEL_BUFFER)
            .set_size(size_of::<GPUMeshData> as u32)
            .set_device_only(false);
        let material_buffer = renderer.create_buffer(material_buffer_desc)?;

        // XXX: Use accessprs fpr a lot of the structs instead of public mbembers
        let descriptor_set_layout = render_technique.passes[0]
            .graphics_pipeline
            .descriptor_set_layouts()[0]
            .clone();
        let descriptor_set_desc = DescriptorSetDesc::new(descriptor_set_layout)
            .add_buffer_resource(material_buffer.clone(), 0)
            .add_buffer_resource(scene_uniform_buffer, 0);
        let descriptor_set = renderer.create_descriptor_set(descriptor_set_desc)?;

        let mut mesh = Mesh::new_with_pbr_material(PBRMaterial::new(
            material,
            material_buffer,
            descriptor_set,
        ));

        // XXX: Set mesh position buffer?

        let node = render_graph.access_node_by_name("pbr_lighting_pass")?;

        let diffuse_texture_resource = render_graph.access_resource_by_handle(node.inputs[0])?;
        let normal_texture_resource = render_graph.access_resource_by_handle(node.inputs[1])?;
        let roughness_texture_resource = render_graph.access_resource_by_handle(node.inputs[2])?;
        let position_texture_resource = render_graph.access_resource_by_handle(node.inputs[3])?;

        mesh.pbr_material.diffuse_texture_index =
            diffuse_texture_resource.gpu_image_bindless_index()?;
        mesh.pbr_material.normal_texture_index =
            normal_texture_resource.gpu_image_bindless_index()?;
        mesh.pbr_material.roughness_texture_index =
            roughness_texture_resource.gpu_image_bindless_index()?;
        mesh.pbr_material.position_texture_index =
            position_texture_resource.gpu_image_bindless_index()?;

        Ok(Self { mesh })
    }

    /// Copies mesh data to the GPU buffer
    pub fn upload_data_to_gpu(&self) -> Result<()> {
        self.mesh
            .pbr_material
            .material_buffer
            .copy_data_to_buffer(&[self.mesh.create_gpu_data()])
    }
}

impl RenderPass for PBRLightingPass {
    fn pre_render(&self, command_buffer: &CommandBuffer) -> Result<()> {
        todo!()
    }

    fn render(&self, command_buffer: &CommandBuffer) -> Result<()> {
        let material_pass_index = 0;
        let graphics_pipeline = &self.mesh.pbr_material.material.render_technique.passes
            [material_pass_index]
            .graphics_pipeline;

        command_buffer.bind_graphics_pipeline(&graphics_pipeline);
        command_buffer.bind_vertex_buffer(self.mesh.position_buffer.as_ref().unwrap(), 0, 0);
        command_buffer.bind_descriptor_set(
            &self.mesh.pbr_material.descriptor_set,
            graphics_pipeline.raw_layout(),
            0,
        );
        command_buffer.draw(3, 1, 0, 0);

        Ok(())
    }

    fn resize(&self, width: u32, height: u32) -> Result<()> {
        todo!()
    }

    fn name(&self) -> &str {
        "PBRLightingPass"
    }
}
