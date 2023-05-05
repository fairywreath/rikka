use anyhow::Result;

use rikka_core::vk;
use rikka_gpu::{
    barriers::{Barriers, ResourceState},
    command_buffer::CommandBuffer,
    gpu::Gpu,
    image::*,
    types::*,
};

use crate::{builder::*, types::*};

pub struct Graph {
    // pub(crate) builder: Builder,
    // pub(crate) nodes: Vec<NodeHandle>,
    pub builder: Builder,
    pub nodes: Vec<NodeHandle>,
}

impl Graph {
    pub fn new(builder: Builder, nodes: Vec<NodeHandle>) -> Self {
        Self { builder, nodes }
    }

    pub fn reset(&mut self) {
        todo!()
    }

    /// Enable render pass node
    pub fn enable_render_pass(&mut self, name: &str) -> Result<()> {
        self.builder.access_node_mut_by_name(name)?.set_enable(true);
        Ok(())
    }

    /// Disable render pass node
    pub fn disable_render_pass(&mut self, name: &str) -> Result<()> {
        self.builder
            .access_node_mut_by_name(name)?
            .set_enable(false);
        Ok(())
    }

    pub fn compile(&mut self, gpu: &mut Gpu) -> Result<()> {
        // Clear all node edges
        for node_handle in &self.nodes {
            self.builder
                .access_node_mut_by_handle(node_handle)?
                .edges
                .clear();
        }

        // Compute edges for all nodes
        for node_handle in &self.nodes {
            let enabled = self.builder.access_node_by_handle(&node_handle)?.enabled;
            if enabled {
                let node_inputs = self
                    .builder
                    .access_node_by_handle(&node_handle)
                    .unwrap()
                    .inputs
                    .clone();

                for input in &node_inputs {
                    let resource_name = self.builder.access_resource_by_handle(input)?.name.clone();

                    // `output_resource` -> `input_resource`
                    // XXX: name String is also cloned here, have a work around so this can be avoided?
                    let output_resource = self
                        .builder
                        .access_resource_by_name(&resource_name)?
                        .clone();

                    if let Ok(input_resource) = self.builder.access_resource_mut_by_handle(input) {
                        input_resource.producer = output_resource.producer;
                        input_resource.info = output_resource.info;
                        input_resource.output = output_resource.output;
                    }

                    if let Ok(parent_node) = self
                        .builder
                        .access_node_mut_by_handle(&output_resource.producer)
                    {
                        parent_node.edges.push(*node_handle);
                    }
                }
            }
        }

        let mut sorted_nodes = Vec::new();
        let mut node_stack = Vec::new();

        // 1: Visited
        // 2: Added
        // XXX: size this appropriately
        let mut visited = vec![0 as u8; 1024];

        // Topological sort
        for node_handle in &self.nodes {
            let enabled = self.builder.access_node_by_handle(&node_handle)?.enabled;
            if enabled {
                node_stack.push(*node_handle);

                while !node_stack.is_empty() {
                    let current_node = node_stack.last().unwrap().clone();
                    if visited[current_node.index] == 2 {
                        node_stack.pop();
                    } else if visited[current_node.index] == 1 {
                        visited[current_node.index] = 2;
                        node_stack.pop();
                        sorted_nodes.push(current_node);
                    } else {
                        visited[current_node.index] = 1;

                        let node = self.builder.access_node_mut_by_handle(&current_node)?;
                        if !node.edges.is_empty() {
                            for edge_node in &node.edges {
                                if visited[edge_node.index] == 0 {
                                    node_stack.push(*edge_node);
                                }
                            }
                        }
                    }
                }
            }
        }

        assert!(self.nodes.len() == sorted_nodes.len());
        sorted_nodes.reverse();
        self.nodes = sorted_nodes;

        // Calculate ref counts of output->input resources
        for node_handle in &self.nodes {
            let enabled = self.builder.access_node_by_handle(&node_handle)?.enabled;
            if enabled {
                let inputs = self
                    .builder
                    .access_node_by_handle(&node_handle)
                    .unwrap()
                    .inputs
                    .clone();
                for input_handle in &inputs {
                    let output_handle = self
                        .builder
                        .access_resource_by_handle(&input_handle)?
                        .output;

                    // Increase ref count of outputs that are used as inputs
                    self.builder
                        .access_resource_mut_by_handle(&output_handle)?
                        .ref_count += 1;
                }
            }
        }

        // Image aliasing free list
        let mut image_free_list = Vec::<rikka_gpu::escape::Handle<Image>>::new();

        for node_handle in &self.nodes {
            if !self.builder.access_node_by_handle(&node_handle)?.enabled {
                continue;
            }

            let (outputs, inputs) = {
                let node = self.builder.access_node_by_handle(&node_handle)?;
                (node.outputs.clone(), node.inputs.clone())
            };

            for output_handle in outputs {
                let (resource_info, resource_type, resource_name) = {
                    let resource = self.builder.access_resource_by_handle(&output_handle)?;
                    (
                        resource.info.clone(),
                        resource.resource_type,
                        resource.name.as_str(),
                    )
                };

                if !resource_info.external {
                    if resource_type == ResourceType::Attachment {
                        let image_info = &resource_info.image.unwrap();
                        if !image_free_list.is_empty() {
                            // XXX: Reuse free images
                            todo!()
                        } else {
                            let image_desc = ImageDesc::new(
                                image_info.width,
                                image_info.height,
                                image_info.depth,
                            )
                            .set_format(image_info.format)
                            .set_image_type(vk::ImageType::TYPE_2D)
                            .set_usage_flags(image_info.usage_flags);

                            log::trace!("Creating GPU image for node output {}", resource_name);
                            let image = gpu.create_image(image_desc)?;

                            self.builder
                                .access_resource_mut_by_handle(&output_handle)?
                                .info
                                .image
                                .as_mut()
                                .unwrap()
                                .image = Some(image);
                        }
                    }
                }
            }

            // Reuse images if it is not referenced anymore/image aliasing
            for node_handle in inputs {
                // Resource handle this input originates from
                let origin_resource_handle =
                    self.builder.access_resource_by_handle(&node_handle)?.output;

                // XXX: Re-design to work around the borrow checker nicely
                let (resource_info, resource_type, ref_count) = {
                    let resource = self
                        .builder
                        .access_resource_mut_by_handle(&origin_resource_handle)?;

                    resource.ref_count -= 1;
                    assert!(resource.ref_count >= 0);

                    (
                        resource.info.clone(),
                        resource.resource_type,
                        resource.ref_count,
                    )
                };

                if !resource_info.external && ref_count == 0 {
                    if resource_type == ResourceType::Attachment
                        || resource_type == ResourceType::Texture
                    {
                        // XXX: Reuse free image
                        // image_free_list.push(resource_info.image.unwrap().image.unwrap().clone());
                    }
                }
            }
        }

        // Create dynamic rendering states/renderpasses + framebuffers
        for node_handle in &self.nodes {
            let mut rendering_state = None;
            {
                let node = self.builder.access_node_by_handle(&node_handle)?;
                if !node.enabled {
                    continue;
                }

                if node.rendering_state.is_none() {
                    rendering_state = Some(self.create_rendering_state(node)?);
                }
            }

            if rendering_state.is_some() {
                self.builder
                    .access_node_mut_by_handle(&node_handle)?
                    .rendering_state = rendering_state;
            }
        }

        Ok(())
    }

    fn create_rendering_state(&self, node: &Node) -> Result<RenderingState> {
        let mut rendering_state = RenderingState::new_dimensionless();
        let mut width = 0;
        let mut height = 0;

        for output in &node.outputs {
            let resource = self.builder.access_resource_by_handle(&output)?;
            if resource.resource_type == ResourceType::Attachment {
                log::trace!(
                    "Creating (vulkan dynamic) rendering state for node {} output {}",
                    &node.name,
                    &resource.name
                );
                let image_info = resource.info.image.as_ref().unwrap();

                if format_has_depth(image_info.format) {
                    rendering_state = rendering_state.set_depth_attachment(
                        RenderDepthStencilAttachment::new()
                            .set_format(image_info.format)
                            .set_clear_value(vk::ClearDepthStencilValue {
                                depth: 1.0,
                                stencil: 0,
                            })
                            .set_depth_operation(RenderPassOperation::Clear)
                            .set_image_view(image_info.image.as_ref().unwrap().raw_view()),
                    );
                } else {
                    rendering_state = rendering_state.add_color_attachment(
                        RenderColorAttachment::new()
                            .set_format(image_info.format)
                            .set_clear_value(vk::ClearColorValue {
                                float32: [1.0, 1.0, 1.0, 1.0],
                            })
                            .set_operation(image_info.load_op)
                            .set_image_view(image_info.image.as_ref().unwrap().raw_view())
                            .set_image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL),
                    );
                }

                if width != 0 {
                    assert_eq!(image_info.width, width);
                }
                if height != 0 {
                    assert_eq!(image_info.height, height);
                }

                width = image_info.width;
                height = image_info.height;
            }
        }

        rendering_state = rendering_state.set_width(width).set_height(height);

        Ok(rendering_state)
    }

    pub fn render(&self, command_buffer: &CommandBuffer) -> Result<()> {
        for node_handle in &self.nodes {
            let node = self.builder.access_node_by_handle(&node_handle)?;
            if !node.enabled {
                continue;
            }

            let mut barriers = Barriers::new();

            // Transition image barriers
            for input_handle in &node.inputs {
                let input_resource = self.builder.access_resource_by_handle(&input_handle)?;
                match input_resource.resource_type {
                    ResourceType::Texture => {
                        let image = input_resource
                            .info
                            .image
                            .as_ref()
                            .unwrap()
                            .image
                            .as_ref()
                            .unwrap();

                        barriers = barriers.add_image(
                            image,
                            ResourceState::RENDER_TARGET,
                            ResourceState::SHADER_RESOURCE,
                        );
                    }
                    _ => {}
                }
            }

            for output_handle in &node.outputs {
                let output_resource = self.builder.access_resource_by_handle(&output_handle)?;
                match output_resource.resource_type {
                    ResourceType::Attachment => {
                        let image_info = output_resource.info.image.as_ref().unwrap();

                        if format_has_depth(image_info.format) {
                            barriers = barriers.add_image(
                                image_info.image.as_ref().unwrap(),
                                ResourceState::UNDEFINED,
                                ResourceState::DEPTH_WRITE,
                            );
                        } else {
                            barriers = barriers.add_image(
                                image_info.image.as_ref().unwrap(),
                                ResourceState::UNDEFINED,
                                ResourceState::RENDER_TARGET,
                            );
                        }
                    }
                    _ => {}
                }
            }

            command_buffer.begin()?;
            command_buffer.pipeline_barrier(barriers);

            // XXX: set viewport

            if let Some(render_pass) = &node.render_pass {
                render_pass.pre_render(command_buffer)?;
                command_buffer.begin_rendering(node.rendering_state.as_ref().unwrap().clone());
                render_pass.render(command_buffer)?;
                command_buffer.end_rendering();
            }

            command_buffer.end()?;
        }

        Ok(())
    }

    pub fn on_resize(&mut self, width: u32, height: u32) {
        todo!()
    }

    pub fn access_resource(&self, handle: ResourceHandle) -> &Resource {
        todo!()
    }

    pub fn access_node(&self, handle: NodeHandle) -> &Node {
        todo!()
    }

    pub fn access_node_by_name(&self, name: &str) -> Result<&Node> {
        self.builder.access_node_by_name(name)
    }

    pub fn add_node(&mut self, desc: NodeDesc) {
        todo!()
    }
}
