use std::{collections::HashMap, rc::Rc};

use anyhow::{Context, Result};
use serde_derive::{Deserialize, Serialize};

use rikka_core::vk;
use rikka_gpu::{
    buffer::Buffer, command_buffer::CommandBuffer, escape::Handle, image::Image, types::*,
};

// XXX: Use a better typestate system

#[derive(Copy, Clone, Serialize, Deserialize, Debug, PartialEq)]
pub enum ResourceType {
    Buffer,
    Texture,
    Attachment,
    Reference,
}

#[derive(Clone, Copy)]
pub struct ResourceHandle {
    pub index: usize,
}

impl ResourceHandle {
    pub fn new(index: usize) -> Self {
        Self { index }
    }

    pub fn invalid() -> Self {
        Self { index: usize::MAX }
    }

    pub fn is_invalid(&self) -> bool {
        self.index == usize::MAX
    }
}

pub type NodeHandle = ResourceHandle;

#[derive(Clone)]
pub struct BufferInfo {
    pub buffer: Option<Handle<Buffer>>,

    // XXX: Do we need these(already stored inside `Buffer`)?
    pub size: u32,
    pub usage_flags: vk::BufferUsageFlags,
}

#[derive(Clone)]
pub struct ImageInfo {
    pub image: Option<Handle<Image>>,
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub format: vk::Format,
    pub usage_flags: vk::ImageUsageFlags,
    pub load_op: RenderPassOperation,
}

#[derive(Clone)]
pub struct ResourceInfo {
    pub buffer: Option<BufferInfo>,
    pub image: Option<ImageInfo>,
    pub external: bool,
}

impl Default for ResourceInfo {
    fn default() -> Self {
        Self {
            buffer: None,
            image: None,
            external: false,
        }
    }
}

#[derive(Clone)]
pub struct Resource {
    pub resource_type: ResourceType,
    pub info: ResourceInfo,
    pub producer: NodeHandle,
    pub output: ResourceHandle,
    pub ref_count: i32,
    pub name: String,
}

impl Resource {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_type(&mut self, resource_type: ResourceType) -> &mut Self {
        self.resource_type = resource_type;
        self
    }

    pub fn set_name(&mut self, name: String) -> &mut Self {
        self.name = name;
        self
    }

    pub fn set_info(&mut self, info: ResourceInfo) -> &mut Self {
        self.info = info;
        self
    }

    pub fn set_output(&mut self, output: ResourceHandle) -> &mut Self {
        self.output = output;
        self
    }

    pub fn set_producer(&mut self, producer: NodeHandle) -> &mut Self {
        self.producer = producer;
        self
    }

    pub fn set_ref_count(&mut self, value: i32) -> &mut Self {
        self.ref_count = value;
        self
    }

    pub fn add_ref_count(&mut self, value: i32) {
        self.ref_count += value;
    }

    pub fn remove_ref_count(&mut self, value: i32) {
        self.ref_count -= value;
    }

    pub fn gpu_image(&self) -> Result<Handle<Image>> {
        self.info
            .image
            .as_ref()
            .context("Resource is not an image")?
            .image
            .clone()
            .context("Image resource does not contain a GPU image handle")
    }

    pub fn gpu_image_bindless_index(&self) -> Result<u32> {
        Ok(self.gpu_image()?.bindless_index())
    }
}

impl Default for Resource {
    fn default() -> Self {
        Self {
            resource_type: ResourceType::Attachment,
            info: ResourceInfo {
                buffer: None,
                image: None,
                external: false,
            },
            producer: NodeHandle::invalid(),
            output: ResourceHandle::invalid(),
            name: String::new(),
            ref_count: 0,
        }
    }
}

pub struct InputDesc {
    pub resource_type: ResourceType,
    /// Name of the output resource this input originates from
    pub name: String,
}

pub struct OutputDesc {
    pub resource_type: ResourceType,
    pub name: String,
    pub info: ResourceInfo,
}

pub struct NodeDesc {
    pub inputs: Vec<InputDesc>,
    pub outputs: Vec<OutputDesc>,
    pub enabled: bool,
    pub name: String,
}

pub trait RenderPass {
    // XXX: These might have to be mut :)
    // fn pre_render(&self, command_buffer: &CommandBuffer) -> Result<()>;
    fn render(&self, command_buffer: &CommandBuffer) -> Result<()>;
    // fn resize(&self, width: u32, height: u32) -> Result<()>;
    fn name(&self) -> &str;
}

pub struct Node {
    pub rendering_state: Option<RenderingState>,
    pub inputs: Vec<ResourceHandle>,
    pub outputs: Vec<ResourceHandle>,
    pub edges: Vec<NodeHandle>,
    pub enabled: bool,
    pub name: String,
    pub render_pass: Option<Box<dyn RenderPass>>,
}

impl Node {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_rendering_state(&mut self, rendering_state: RenderingState) -> &mut Self {
        self.rendering_state = Some(rendering_state);
        self
    }

    pub fn add_inputs(&mut self, inputs: &[ResourceHandle]) -> &mut Self {
        self.inputs.extend_from_slice(inputs);
        self
    }

    pub fn add_outputs(&mut self, outputs: &[ResourceHandle]) -> &mut Self {
        self.outputs.extend_from_slice(outputs);
        self
    }

    pub fn add_edges(&mut self, edges: &[NodeHandle]) -> &mut Self {
        self.edges.extend_from_slice(edges);
        self
    }

    pub fn set_enable(&mut self, enable: bool) -> &mut Self {
        self.enabled = enable;
        self
    }

    pub fn set_name(&mut self, name: String) -> &mut Self {
        self.name = name;
        self
    }
}

impl Default for Node {
    fn default() -> Self {
        Self {
            rendering_state: None,
            inputs: Vec::new(),
            outputs: Vec::new(),
            edges: Vec::new(),
            enabled: true,
            name: String::new(),
            render_pass: None,
        }
    }
}

pub(crate) struct ResourcePool<T: Default> {
    array: Vec<T>,
    returned_indices: Vec<usize>,
}

impl<T: Default> ResourcePool<T> {
    pub fn new() -> Self {
        Self {
            array: Vec::new(),
            returned_indices: Vec::new(),
        }
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        self.array.get(index)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.array.get_mut(index)
    }

    pub fn free_resource(&mut self, index: usize) {
        self.returned_indices.push(index);
    }

    pub fn new_index(&mut self) -> usize {
        if !self.returned_indices.is_empty() {
            return self.returned_indices.pop().unwrap();
        }

        self.array.push(Default::default());
        self.array.len() - 1
    }

    pub fn get_new(&mut self) -> (&mut T, usize) {
        let index = self.new_index();
        (self.get_mut(index).unwrap(), index)
    }

    pub fn push(&mut self, value: T) -> usize {
        let index;
        if !self.returned_indices.is_empty() {
            index = self.returned_indices.pop().unwrap();
            self.array[index] = value;
        } else {
            self.array.push(value);
            index = self.array.len()
        }

        index
    }
}

pub(crate) struct RenderPassCache {
    // XXX: Change String key
    pub(crate) render_pass_map: HashMap<String, Box<dyn RenderPass>>,
}

pub(crate) struct ResourceCache {
    pub(crate) resources: ResourcePool<Resource>,
    // XXX: Change String key
    pub(crate) resource_map: HashMap<String, usize>,
}

pub(crate) struct NodeCache {
    pub(crate) nodes: ResourcePool<Node>,
    // XXX: Change String key
    pub(crate) node_map: HashMap<String, usize>,
}
