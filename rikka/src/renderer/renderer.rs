use std::sync::Arc;

use anyhow::Result;
use crossbeam_channel::{Receiver, Sender};

use rikka_core::vk;
use rikka_gpu::{
    buffer::*, command_buffer::*, descriptor_set::*, gpu::Gpu, image::*, pipeline::*, sampler::*,
};

pub struct RenderTechniqueDesc {
    graphics_pipelines: Vec<GraphicsPipelineDesc>,
}

impl RenderTechniqueDesc {
    pub fn new() -> Self {
        RenderTechniqueDesc {
            graphics_pipelines: Vec::new(),
        }
    }

    pub fn add_graphics_pipeline(mut self, graphics_pipeline: GraphicsPipelineDesc) -> Self {
        self.graphics_pipelines.push(graphics_pipeline);
        self
    }
}

pub struct RenderPass {
    // XXX: Remove `pub` here
    pub graphics_pipeline: Arc<GraphicsPipeline>,
}

pub struct RenderTechnique {
    // XXX: Remove `pub` here
    pub passes: Vec<RenderPass>,
}

pub struct MaterialDesc {
    render_index: u32,
    render_technique: Arc<RenderTechnique>,
}

impl MaterialDesc {
    pub fn new(render_index: u32, render_technique: Arc<RenderTechnique>) -> Self {
        MaterialDesc {
            render_index,
            render_technique,
        }
    }
}

pub struct Material {
    render_index: u32,
    render_technique: Arc<RenderTechnique>,
}

pub struct Renderer {
    gpu: Gpu,
}

impl Renderer {
    pub fn new(gpu: Gpu) -> Self {
        Renderer { gpu }
    }

    // XXX: Remove these eventually
    pub fn gpu(&self) -> &Gpu {
        &self.gpu
    }
    pub fn gpu_mut(&mut self) -> &mut Gpu {
        &mut self.gpu
    }

    pub fn begin_frame(&mut self) -> Result<()> {
        // XXX: Handle swapchain recreation
        self.gpu.new_frame()?;
        self.gpu.swapchain_acquire_next_image()?;
        Ok(())
    }

    pub fn end_frame(&mut self) -> Result<()> {
        // XXX: Handle swapchain recreation
        // XXX: Submit queued command buffers
        self.gpu.submit_queued_graphics_command_buffers()?;
        self.gpu.present()?;
        Ok(())
    }

    pub fn set_present_mode(&mut self, present_mode: vk::PresentModeKHR) -> Result<()> {
        self.gpu.set_present_mode(present_mode)
    }

    pub fn aspect_ratio(&self) -> f32 {
        let swapchain_extent = self.gpu.swapchain_extent();
        swapchain_extent.width as f32 / swapchain_extent.height as f32
    }

    pub fn extent(&self) -> vk::Extent2D {
        self.gpu.swapchain_extent()
    }

    pub fn create_buffer(&self, desc: BufferDesc) -> Result<Arc<Buffer>> {
        let buffer = self.gpu.create_buffer(desc)?;
        Ok(Arc::new(buffer))
    }

    pub fn create_image(&mut self, desc: ImageDesc) -> Result<Arc<Image>> {
        let image = self.gpu.create_image(desc)?;
        Ok(Arc::new(image))
    }

    pub fn create_sampler(&self, desc: SamplerDesc) -> Result<Arc<Sampler>> {
        let sampler = self.gpu.create_sampler(desc)?;
        Ok(Arc::new(sampler))
    }

    pub fn create_technique(&self, desc: RenderTechniqueDesc) -> Result<Arc<RenderTechnique>> {
        let graphics_pipelines = desc
            .graphics_pipelines
            .into_iter()
            .map(|graphics_pipeline_desc| {
                let graphics_pipeline =
                    self.gpu.create_graphics_pipeline(graphics_pipeline_desc)?;
                Ok(Arc::new(graphics_pipeline))
            })
            .collect::<Result<Vec<_>>>()?;

        let passes = graphics_pipelines
            .into_iter()
            .map(|graphics_pipeline| RenderPass { graphics_pipeline })
            .collect::<Vec<_>>();

        Ok(Arc::new(RenderTechnique { passes }))
    }

    pub fn create_material(&self, desc: MaterialDesc) -> Result<Arc<Material>> {
        Ok(Arc::new(Material {
            render_index: desc.render_index,
            render_technique: desc.render_technique,
        }))
    }

    pub fn get_material_pipeline(material: &Material, pass_index: u32) -> Arc<GraphicsPipeline> {
        material.render_technique.passes[pass_index as usize]
            .graphics_pipeline
            .clone()
    }

    pub fn create_descriptor_set(
        &self,
        material: &Material,
        desc: DescriptorSetDesc,
    ) -> Result<Arc<DescriptorSet>> {
        todo!()
    }

    pub fn command_buffer(&mut self, thread_index: u32) -> Result<Arc<CommandBuffer>> {
        self.gpu.current_command_buffer(thread_index)
    }

    pub fn queue_command_buffer(&mut self, command_buffer: Arc<CommandBuffer>) {
        self.gpu.queue_graphics_command_buffer(command_buffer);
    }

    /// XXX: Resource OBRM/RAII is not completely "safe" as they can be destroyed when used.
    ///      Need a resource system tracker in the GPU for this, or at least have a simple sender/receiver to delay
    ///      object destruction until the end of the current frame
    ///
    ///      Currently we just wait idle before dropping any resources but this needs to be removed
    pub fn wait_idle(&self) {
        self.gpu.wait_idle();
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        self.gpu.wait_idle();
    }
}
