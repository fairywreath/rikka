use std::sync::Arc;

use anyhow::Result;

use rikka_core::vk;
use rikka_gpu::{
    buffer::*, command_buffer::*, descriptor_set::*, gpu::Gpu, image::*, pipeline::*, sampler::*,
};

pub use rikka_gpu::escape::Handle;

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

pub struct RenderTechniquePass {
    graphics_pipeline: Handle<GraphicsPipeline>,
}

pub struct RenderTechnique {
    passes: Vec<RenderTechniquePass>,
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
        self.gpu.new_frame()?;
        if let Err(_) = self.gpu.swapchain_acquire_next_image() {
            self.gpu.recreate_swapchain()?;
            self.gpu.advance_frame_counters();
        }

        Ok(())
    }

    pub fn end_frame(&mut self) -> Result<()> {
        self.gpu.submit_queued_graphics_command_buffers()?;

        self.gpu.present().unwrap_or_else(|_| {
            self.gpu.wait_idle();
            false
        });

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

    pub fn create_buffer(&self, desc: BufferDesc) -> Result<Handle<Buffer>> {
        Ok(self.gpu.create_buffer(desc)?)
    }

    pub fn create_image(&mut self, desc: ImageDesc) -> Result<Handle<Image>> {
        Ok(self.gpu.create_image(desc)?)
    }

    pub fn create_sampler(&self, desc: SamplerDesc) -> Result<Handle<Sampler>> {
        Ok(self.gpu.create_sampler(desc)?)
    }

    pub fn create_technique(&self, desc: RenderTechniqueDesc) -> Result<Arc<RenderTechnique>> {
        let graphics_pipelines = desc
            .graphics_pipelines
            .into_iter()
            .map(|graphics_pipeline_desc| {
                Ok(Handle::from(
                    self.gpu.create_graphics_pipeline(graphics_pipeline_desc)?,
                ))
            })
            .collect::<Result<Vec<_>>>()?;

        let passes = graphics_pipelines
            .into_iter()
            .map(|graphics_pipeline| RenderTechniquePass { graphics_pipeline })
            .collect::<Vec<_>>();

        Ok(Arc::new(RenderTechnique { passes }))
    }

    pub fn create_material(&self, desc: MaterialDesc) -> Result<Arc<Material>> {
        Ok(Arc::new(Material {
            render_index: desc.render_index,
            render_technique: desc.render_technique,
        }))
    }

    pub fn get_material_pipeline(material: &Material, pass_index: u32) -> Handle<GraphicsPipeline> {
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
        log::info!("Renderer dropped");
    }
}
