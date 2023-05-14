use std::{mem::size_of, sync::Arc};

use anyhow::Result;

use rikka_core::{nalgebra::Vector4, vk};
use rikka_gpu::{buffer::*, command_buffer::CommandBuffer, descriptor_set::*};
use rikka_graph::{graph::Graph, types::*};

use crate::{renderer::*, scene_renderer::mesh::*};

pub struct SimplePbrPass {
    mesh_instances: Vec<MeshInstance>,
    zero_buffer: Handle<Buffer>,
    bindless_descriptor_set: Arc<DescriptorSet>,
}

impl SimplePbrPass {
    pub fn new(
        renderer: &Renderer,
        render_graph: &Graph,
        meshes: &[Arc<Mesh>],
        bindless_descriptor_set: Arc<DescriptorSet>,
    ) -> Result<Self> {
        let mesh_instances = meshes
            .into_iter()
            .map(|mesh| MeshInstance::new(mesh.clone(), 0))
            .collect::<Vec<_>>();

        let zero_buffer_data = Vector4::<f32>::new(0.0, 0.0, 0.0, 0.0);
        let zero_buffer = renderer.create_buffer(
            BufferDesc::new()
                .set_size(std::mem::size_of_val(zero_buffer_data.as_slice()) as _)
                .set_usage_flags(vk::BufferUsageFlags::VERTEX_BUFFER)
                .set_device_only(false),
        )?;
        zero_buffer.copy_data_to_buffer(zero_buffer_data.as_slice())?;

        Ok(Self {
            mesh_instances,
            zero_buffer,
            bindless_descriptor_set,
        })
    }

    pub fn create_render_pass(&self) -> Box<dyn RenderPass> {
        Box::new(SimplePbrRenderPass {
            mesh_instances: self.mesh_instances.clone(),
            zero_buffer: self.zero_buffer.clone(),
            bindless_descriptor_set: self.bindless_descriptor_set.clone(),
        })
    }
}

struct SimplePbrRenderPass {
    mesh_instances: Vec<MeshInstance>,
    zero_buffer: Handle<Buffer>,
    bindless_descriptor_set: Arc<DescriptorSet>,
}

impl RenderPass for SimplePbrRenderPass {
    fn render(&self, command_buffer: &CommandBuffer) -> Result<()> {
        for mesh_instance in &self.mesh_instances {
            let mesh = &mesh_instance.mesh;

            if mesh.transparent() {
                continue;
            }
            let graphics_pipeline = &mesh.pbr_material.material.render_technique.passes
                [mesh_instance.material_pass_index]
                .graphics_pipeline;

            // XXX: Do not bind pipeline ber draw, sort based on material and bind sparringly
            // XXX FIXME: The process of obtaining the pipeline from the mesh and material
            command_buffer.bind_graphics_pipeline(graphics_pipeline);
            command_buffer.bind_descriptor_set(
                &self.bindless_descriptor_set,
                graphics_pipeline.raw_layout(),
                1,
            );

            mesh.draw(command_buffer, graphics_pipeline, &self.zero_buffer);
        }

        Ok(())
    }

    fn post_render(&self, command_buffer: &CommandBuffer, graph: &Graph) -> Result<()> {
        Ok(())
    }

    fn name(&self) -> &str {
        "Simple PBR render pass"
    }
}
