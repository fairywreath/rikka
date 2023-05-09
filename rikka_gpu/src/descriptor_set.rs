use std::sync::Arc;

use anyhow::{Context, Result};

use rikka_core::vk;
pub use rikka_shader::types::DescriptorBinding;

use crate::{buffer::Buffer, constants, escape::*, factory::DeviceGuard, image::Image};

pub struct DescriptorPoolDesc {
    pub pool_sizes: Vec<vk::DescriptorPoolSize>,
    pub flags: vk::DescriptorPoolCreateFlags,
    pub max_sets: u32,
}

impl DescriptorPoolDesc {
    pub fn new() -> Self {
        Self {
            pool_sizes: vec![],
            flags: vk::DescriptorPoolCreateFlags::empty(),
            max_sets: 0,
        }
    }

    pub fn add_pool_size(mut self, descriptor_type: vk::DescriptorType, count: u32) -> Self {
        self.pool_sizes.push(
            vk::DescriptorPoolSize::builder()
                .ty(descriptor_type)
                .descriptor_count(count)
                .build(),
        );
        self
    }

    pub fn set_flags(mut self, flags: vk::DescriptorPoolCreateFlags) -> Self {
        self.flags = flags;
        self
    }

    pub fn set_max_sets(mut self, max_sets: u32) -> Self {
        self.max_sets = max_sets;
        self
    }
}

pub struct DescriptorPool {
    device: DeviceGuard,
    raw: vk::DescriptorPool,
}

impl DescriptorPool {
    pub(crate) unsafe fn create(device: DeviceGuard, desc: DescriptorPoolDesc) -> Result<Self> {
        let create_info = vk::DescriptorPoolCreateInfo::builder()
            .flags(desc.flags)
            .max_sets(desc.max_sets)
            .pool_sizes(&desc.pool_sizes);

        let raw = device
            .raw()
            .create_descriptor_pool(&create_info, None)
            .context("Failed to create vulkan descriptor pool!")?;

        Ok(Self { device, raw })
    }

    pub(crate) unsafe fn destroy(self) {
        self.device.raw().destroy_descriptor_pool(self.raw, None);
    }

    pub fn raw(&self) -> vk::DescriptorPool {
        self.raw
    }
}

#[derive(Debug)]
pub struct DescriptorSetLayoutDesc {
    pub bindings: Vec<DescriptorBinding>,
    pub bindless: bool,
    pub dynamic: bool,
    pub flags: vk::DescriptorSetLayoutCreateFlags,
}

impl DescriptorSetLayoutDesc {
    pub fn new() -> Self {
        Self {
            bindings: vec![],
            bindless: false,
            dynamic: false,
            flags: vk::DescriptorSetLayoutCreateFlags::empty(),
        }
    }

    pub fn add_binding(mut self, binding: DescriptorBinding) -> Self {
        self.bindings.push(binding);
        self
    }

    pub fn set_bindings(mut self, bindings: Vec<DescriptorBinding>) -> Self {
        self.bindings = bindings;
        self
    }

    pub fn set_bindless(mut self, bindless: bool) -> Self {
        self.bindless = bindless;
        self
    }

    pub fn set_dynamic(mut self, dynamic: bool) -> Self {
        self.dynamic = dynamic;
        self
    }

    pub fn set_flags(mut self, flags: vk::DescriptorSetLayoutCreateFlags) -> Self {
        self.flags = flags;
        self
    }
}

pub struct DescriptorSetLayout {
    device: DeviceGuard,
    raw: vk::DescriptorSetLayout,
    bindings: Vec<DescriptorBinding>,
    binding_index_to_array_index: [usize; constants::MAX_SHADER_BINDING_INDEX as usize],
    bindless: bool,
    dynamic: bool,
}

fn can_descriptor_type_be_bindless(descriptor_type: vk::DescriptorType) -> bool {
    match descriptor_type {
        vk::DescriptorType::COMBINED_IMAGE_SAMPLER | vk::DescriptorType::STORAGE_IMAGE => true,
        _ => false,
    }
}

impl DescriptorSetLayout {
    pub(crate) unsafe fn create(
        device: DeviceGuard,
        desc: DescriptorSetLayoutDesc,
    ) -> Result<Self> {
        let max_shader_binding_index = desc
            .bindings
            .iter()
            .max_by_key(|binding| binding.index)
            .map_or_else(
                || constants::MAX_SHADER_BINDING_INDEX + 1,
                |binding| binding.index,
            );
        if max_shader_binding_index > constants::MAX_SHADER_BINDING_INDEX {
            return Err(anyhow::anyhow!("Maximum shader binding index is invalid"));
        }

        let mut binding_index_to_array_index =
            [usize::MAX; constants::MAX_SHADER_BINDING_INDEX as usize];
        let mut vulkan_bindings =
            Vec::<vk::DescriptorSetLayoutBinding>::with_capacity(desc.bindings.len() as usize);

        for (array_index, binding) in desc.bindings.iter().enumerate() {
            binding_index_to_array_index[binding.index as usize] = array_index as usize;

            // if desc.bindless && can_descriptor_type_be_bindless(binding.descriptor_type) {
            //     // XXX: Handle this nicer...
            //     vulkan_bindings.push(vk::DescriptorSetLayoutBinding::default());
            //     continue;
            // }

            let vulkan_binding = {
                let vulkan_binding = vk::DescriptorSetLayoutBinding::builder()
                    .binding(binding.index)
                    .descriptor_type(binding.descriptor_type)
                    .descriptor_count(binding.count)
                    .stage_flags(binding.shader_stage_flags);

                // XXX: Properly support dynamically bound descriptors.
                // if desc.dynamic && (binding.descriptor_type == vk::DescriptorType::UNIFORM_BUFFER) {
                //     vulkan_binding =
                //         vulkan_binding.descriptor_type(vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC);
                // }

                vulkan_binding.build()
            };
            vulkan_bindings.push(vulkan_binding);
        }

        let mut create_info = vk::DescriptorSetLayoutCreateInfo::builder()
            .bindings(&vulkan_bindings)
            .flags(desc.flags);

        let raw = {
            if desc.bindless {
                let binding_flags = vec![
                    vk::DescriptorBindingFlags::PARTIALLY_BOUND
                        // | vk::DescriptorBindingFlags::VARIABLE_DESCRIPTOR_COUNT
                        | vk::DescriptorBindingFlags::UPDATE_AFTER_BIND;
                    vulkan_bindings.len()
                ];

                let mut binding_flags_info =
                    vk::DescriptorSetLayoutBindingFlagsCreateInfo::builder()
                        .binding_flags(&binding_flags);

                create_info = create_info
                    .push_next(&mut binding_flags_info)
                    .flags(vk::DescriptorSetLayoutCreateFlags::UPDATE_AFTER_BIND_POOL);

                device
                    .raw()
                    .create_descriptor_set_layout(&create_info, None)
                    .context("Failed to create vulkan descriptor set layout")?
            } else {
                device
                    .raw()
                    .create_descriptor_set_layout(&create_info, None)
                    .context("Failed to create vulkan descriptor set layout")?
            }
        };

        Ok(Self {
            device,
            raw,
            bindings: desc.bindings,
            binding_index_to_array_index,
            bindless: desc.bindless,
            dynamic: desc.dynamic,
        })
    }

    pub(crate) unsafe fn destroy(self) {
        self.device
            .raw()
            .destroy_descriptor_set_layout(self.raw, None);
    }

    pub fn raw(&self) -> vk::DescriptorSetLayout {
        self.raw
    }

    pub fn num_bidings(&self) -> u32 {
        self.bindings.len() as u32
    }

    pub fn bindings(&self) -> &[DescriptorBinding] {
        &self.bindings
    }

    pub fn binding_for_shader_binding_index(
        &self,
        shader_binding_index: u32,
    ) -> &DescriptorBinding {
        assert!((shader_binding_index as usize) < self.binding_index_to_array_index.len());
        let binding_data_index = self.binding_index_to_array_index[shader_binding_index as usize];
        &self.bindings[binding_data_index]
    }

    pub fn is_bindless(&self) -> bool {
        self.bindless
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum DescriptorSetBindingResourceType {
    Buffer,
    ImageSampler,
    // ImageArray,
}

pub struct DescriptorSetBindingResource {
    resource_type: DescriptorSetBindingResourceType,

    // XXX: Need strong references for these?
    pub buffer: Option<Handle<Buffer>>,
    pub image: Option<Handle<Image>>,

    pub count: u32,
    pub binding_index: u32,
}

impl DescriptorSetBindingResource {
    pub fn buffer(buffer: Handle<Buffer>, binding_index: u32) -> Self {
        Self {
            resource_type: DescriptorSetBindingResourceType::Buffer,
            buffer: Some(buffer),
            image: None,
            count: 1,
            binding_index,
        }
    }

    pub fn image(image: Handle<Image>, binding_index: u32) -> Self {
        Self {
            resource_type: DescriptorSetBindingResourceType::ImageSampler,
            buffer: None,
            image: Some(image),
            count: 1,
            binding_index,
        }
    }

    pub fn resource_type(&self) -> DescriptorSetBindingResourceType {
        self.resource_type
    }
}

pub struct DescriptorSetDesc {
    // pub set_index: u32,
    pub binding_resources: Vec<DescriptorSetBindingResource>,

    // XXX: Need strong reference always?
    pub pool: Option<Handle<DescriptorPool>>,
    pub layout: Handle<DescriptorSetLayout>,
    // XXX: Properly support bindless images
    // pub bindless: false,
}

impl DescriptorSetDesc {
    pub fn new(layout: Handle<DescriptorSetLayout>) -> Self {
        Self {
            layout,
            pool: None,
            binding_resources: vec![],
        }
    }

    pub fn set_binding_resources(
        mut self,
        binding_resources: Vec<DescriptorSetBindingResource>,
    ) -> Self {
        self.binding_resources = binding_resources;
        self
    }

    pub fn add_binding_resource(mut self, binding_resource: DescriptorSetBindingResource) -> Self {
        self.binding_resources.push(binding_resource);
        self
    }

    pub fn add_buffer_resource(mut self, buffer: Handle<Buffer>, binding_index: u32) -> Self {
        self.binding_resources
            .push(DescriptorSetBindingResource::buffer(buffer, binding_index));
        self
    }

    pub fn add_image_resource(mut self, image: Handle<Image>, binding_index: u32) -> Self {
        self.binding_resources
            .push(DescriptorSetBindingResource::image(image, binding_index));
        self
    }

    pub fn set_pool(mut self, pool: Handle<DescriptorPool>) -> Self {
        self.pool = Some(pool);
        self
    }
}

pub struct DescriptorSet {
    raw: vk::DescriptorSet,
    // XXX: Might be better/easier to have separate arrays for each resource type?
    //      Probably do not need to hold a strong reference to bindinh resources
    // binding_resources: Vec<DescriptorSetBindingResource>,
    device: DeviceGuard,

    // XXX: Need strong references for these?
    // XXX: These need to be no-guard
    pool: Handle<DescriptorPool>,
    layout: Handle<DescriptorSetLayout>,
    // XXX: Add support for multiple descriptor sets?
    // set_index: u32,
}

impl DescriptorSet {
    pub fn new(device: DeviceGuard, desc: DescriptorSetDesc) -> Result<Self> {
        let pool = desc.pool.clone().unwrap();

        let set_layouts = [desc.layout.raw()];
        let mut allocate_info = vk::DescriptorSetAllocateInfo::builder()
            .descriptor_pool(pool.raw())
            .set_layouts(&set_layouts);

        let raws = {
            if desc.layout.is_bindless() {
                let max_bindless_binding = [constants::MAX_NUM_BINDLESS_RESOURCECS - 1];
                let mut count_info =
                    vk::DescriptorSetVariableDescriptorCountAllocateInfo::builder()
                        .descriptor_counts(&max_bindless_binding);
                allocate_info = allocate_info.push_next(&mut count_info);

                unsafe {
                    device
                        .raw()
                        .allocate_descriptor_sets(&allocate_info)
                        .context("Failed to create vulkan descriptor set")?
                }
            } else {
                unsafe {
                    device
                        .raw()
                        .allocate_descriptor_sets(&allocate_info)
                        .context("Failed to create vulkan descriptor set")?
                }
            }
        };

        let mut set = Self {
            device,
            raw: raws[0],
            // binding_resources: vec![],
            pool,
            layout: desc.layout,
        };

        set.update(&desc.binding_resources)
            .context("Failed to update descriptor set")?;

        Ok(set)
    }

    pub fn raw(&self) -> vk::DescriptorSet {
        self.raw
    }

    pub fn raw_layout(&self) -> vk::DescriptorSetLayout {
        self.layout.raw()
    }

    // XXX: Do we need to cache `binding_resources`? If not pass as value/move.
    pub fn update(&mut self, binding_resources: &[DescriptorSetBindingResource]) -> Result<()> {
        let mut vulkan_write_descriptors =
            Vec::<vk::WriteDescriptorSet>::with_capacity(binding_resources.len());

        // Image/buffer descriptor write infos need to be valid when calling vkUpdateDescriptorSets
        let mut descriptor_buffer_infos = Vec::<vk::DescriptorBufferInfo>::new();
        let mut descriptor_image_infos = Vec::<vk::DescriptorImageInfo>::new();

        for resource in binding_resources {
            let binding = self
                .layout
                .binding_for_shader_binding_index(resource.binding_index);
            // XXX: Check that reource type corresponds to binding type etc.

            // XXX: These should be equal; We actually do not require any info from the layout bindings array other than to verify)
            assert!(resource.binding_index == binding.index);

            if self.layout.is_bindless() && can_descriptor_type_be_bindless(binding.descriptor_type)
            {
                continue;
            }

            vulkan_write_descriptors.push(Self::create_vulkan_write_descriptor_set(
                self.raw,
                &binding,
                &resource,
                &mut descriptor_buffer_infos,
                &mut descriptor_image_infos,
            ));
        }

        descriptor_image_infos.clear();

        unsafe {
            self.device
                .raw()
                .update_descriptor_sets(&vulkan_write_descriptors, &[]);
        }

        Ok(())
    }

    fn create_vulkan_write_descriptor_set(
        descriptor_set: vk::DescriptorSet,
        binding: &DescriptorBinding,
        resource: &DescriptorSetBindingResource,
        buffer_descriptors: &mut Vec<vk::DescriptorBufferInfo>,
        image_descriptors: &mut Vec<vk::DescriptorImageInfo>,
    ) -> vk::WriteDescriptorSet {
        let mut write_descriptor = vk::WriteDescriptorSet::builder()
            .dst_set(descriptor_set)
            .dst_binding(resource.binding_index)
            .dst_array_element(0)
            .descriptor_type(binding.descriptor_type);
        // XXX: ash bug or intentional?
        write_descriptor.descriptor_count = binding.count;

        match binding.descriptor_type {
            vk::DescriptorType::COMBINED_IMAGE_SAMPLER => {
                if binding.count == 1 {
                    // XXX: Need clone here since reource passed as ref. Maybe pass as value if `binding_resources`(see `update`) does not need to be cahced?
                    let image = resource.image.clone().unwrap();
                    let sampler = image.linked_sampler().unwrap();
                    let image_descriptor = vk::DescriptorImageInfo::builder()
                        .image_view(image.raw_view())
                        .sampler(sampler.raw())
                        .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                        .build();

                    image_descriptors.push(image_descriptor);
                    write_descriptor = write_descriptor
                        .image_info(std::slice::from_ref(image_descriptors.last().unwrap()))
                } else {
                    todo!("Image array descriptors not yet implemented")
                }
            }
            vk::DescriptorType::STORAGE_IMAGE => {
                let image = resource.image.clone().unwrap();
                let image_descriptor = vk::DescriptorImageInfo::builder()
                    .image_view(image.raw_view())
                    .image_layout(vk::ImageLayout::GENERAL)
                    .build();

                image_descriptors.push(image_descriptor);
                write_descriptor = write_descriptor
                    .image_info(std::slice::from_ref(image_descriptors.last().unwrap()));
            }
            vk::DescriptorType::UNIFORM_BUFFER => {
                let buffer = resource.buffer.clone().unwrap();

                // XXX: Implement proper dynamic buffers with "parent buffers"
                // if buffer.resource_usage_type() == ResourceUsageType::Dynamic {
                //     write_descriptor = write_descriptor
                //         .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC);
                // }

                let buffer_descriptor = vk::DescriptorBufferInfo::builder()
                    .offset(0)
                    .range(buffer.size() as u64)
                    .buffer(buffer.raw())
                    .build();
                buffer_descriptors.push(buffer_descriptor);
                write_descriptor = write_descriptor
                    .buffer_info(std::slice::from_ref(buffer_descriptors.last().unwrap()));
            }
            vk::DescriptorType::STORAGE_BUFFER => {
                let buffer = resource.buffer.clone().unwrap();
                let buffer_descriptor = vk::DescriptorBufferInfo::builder()
                    .offset(0)
                    .range(buffer.size() as u64)
                    .buffer(buffer.raw())
                    .build();
                buffer_descriptors.push(buffer_descriptor);
                write_descriptor = write_descriptor
                    .buffer_info(std::slice::from_ref(buffer_descriptors.last().unwrap()));
            }
            _ => todo!(
                "Vulkan write descriptor for type {:?} not yet supported",
                binding.descriptor_type
            ),
        }

        write_descriptor.build()
    }
}
