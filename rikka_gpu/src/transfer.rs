use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use crossbeam_channel::{Receiver, Sender};

use anyhow::Result;
use rikka_core::vk;

use crate::{
    barriers::*, buffer::*, command_buffer::*, constants, escape::*, factory::*, image::Image,
    queue::*, synchronization::*,
};

pub struct ImageUploadRequest {
    pub image: Handle<Image>,
    pub data: Vec<u8>,
    // XXX: Have a mechanism to signal upon completion?
}

pub struct TransferManager {
    _device: DeviceGuard,
    command_pools: Vec<CommandPool>,
    command_buffers: Vec<CommandBuffer>,

    transfer_queue: Queue,
    graphics_queue: Queue,

    submission_semaphore: Semaphore,
    /// Timeline semaphore wait value
    submission_index: u64,

    // XXX: This needs to be a persistently mapped buffer
    staging_buffer: Escape<Buffer>,
    staging_buffer_offset: AtomicUsize,

    image_upload_requests: Vec<ImageUploadRequest>,
    completed_images: Vec<Handle<Image>>,

    image_upload_request_sender: Sender<ImageUploadRequest>,
    image_upload_request_receiver: Receiver<ImageUploadRequest>,

    image_upload_complete_sender: Sender<Handle<Image>>,
}

const STAGING_BUFFER_SIZE: u32 = 64 * 1024 * 1024;

impl TransferManager {
    pub fn new(
        device: DeviceGuard,
        factory: &Factory,
        transfer_queue: Queue,
        graphics_queue: Queue,
        image_upload_complete_sender: Sender<Handle<Image>>,
    ) -> Result<Self> {
        let staging_buffer = factory.create_buffer(
            BufferDesc::new()
                .set_size(STAGING_BUFFER_SIZE)
                .set_device_only(false),
        )?;
        let staging_buffer_offset = AtomicUsize::new(0);

        let mut command_pools = Vec::with_capacity(constants::MAX_FRAMES as usize);
        let mut command_buffers = Vec::with_capacity(constants::MAX_FRAMES as usize);

        for i in 0..constants::MAX_FRAMES {
            let command_pool = CommandPool::new(device.clone(), transfer_queue.family_index())?;

            let metadata = CommandBufferMetaData {
                array_index: i,
                frame_index: i,
                thread_index: 0,
            };

            let command_buffer =
                command_pool.allocate_command_buffer(vk::CommandBufferLevel::PRIMARY)?;
            let command_buffer =
                CommandBuffer::new(device.clone(), command_buffer, metadata, false);

            command_pools.push(command_pool);
            command_buffers.push(command_buffer);
        }

        let submission_semaphore = Semaphore::new(device.clone(), SemaphoreType::Timeline)?;
        let submission_index = 0;

        let (image_upload_request_sender, image_upload_request_receiver) =
            crossbeam_channel::unbounded();

        Ok(Self {
            _device: device,
            command_pools,
            command_buffers,
            transfer_queue,
            graphics_queue,
            submission_semaphore,
            submission_index,
            staging_buffer,
            staging_buffer_offset,
            image_upload_requests: Vec::new(),
            completed_images: Vec::new(),

            image_upload_request_sender,
            image_upload_request_receiver,
            image_upload_complete_sender,
        })
    }

    /// Function to be run periodically to perform asynchronous transfers
    pub fn perform_transfers(&mut self) -> Result<()> {
        // XXX: Technically we can have two in flight transfer_queue submissions running at once
        //      Implement that one day...
        if !self.completed_images.is_empty()
            || !self.image_upload_requests.is_empty() && (self.submission_index > 0)
        {
            // log::info!("Waiting for transfer submission semaphore....");

            self.submission_semaphore
                .wait_for_value(self.submission_index)?;

            for image in self.completed_images.drain(..) {
                self.image_upload_complete_sender.send(image)?;
            }
        }

        // XXX: Make this as parallel as possible
        let current_frame = 0;
        self.command_pools[current_frame].reset();

        self.receive_image_upload_requests();

        if !self.image_upload_requests.is_empty() {
            // XXX: Handle multiple image uploads
            let image_request = self.image_upload_requests.pop().unwrap();

            let command_buffer = &self.command_buffers[current_frame];
            command_buffer.begin()?;

            // XXX: Query proper number of channels from image format.
            let num_channels = 4;
            // XXX: Handle proper alignment when number of channels is not guaranteed to be multiple of 4.
            // let image_alignment = 4;
            // let aligned_image_size =
            //     image_request.image.width() * image_request.image.height() * num_channels;
            // let current_offset = self
            //     .staging_buffer_offset
            //     .fetch_add(aligned_image_size as usize, Ordering::Relaxed);

            self.staging_buffer
                .copy_data_to_buffer(&image_request.data)?;

            let barriers = Barriers::new().add_image(
                &image_request.image,
                ResourceState::UNDEFINED,
                ResourceState::COPY_DESTINATION,
            );
            command_buffer.pipeline_barrier(barriers);

            command_buffer.copy_buffer_to_image(&self.staging_buffer, &image_request.image, 0);

            // log::info!(
            //     "Transfer index {}, graphics index {}",
            //     self.transfer_queue.family_index(),
            //     self.graphics_queue.family_index()
            // );

            let barriers = Barriers::new().add_image_with_queue_transfer(
                &image_request.image,
                ResourceState::COPY_DESTINATION,
                ResourceState::COPY_DESTINATION,
                &self.transfer_queue,
                &self.graphics_queue,
            );
            command_buffer.pipeline_barrier(barriers);

            command_buffer.end()?;

            let signal_semaphores = SemaphoreSubmitInfo {
                semaphore: &self.submission_semaphore,
                stage_mask: vk::PipelineStageFlags2::TRANSFER,
                value: Some(self.submission_index + 1),
            };
            self.transfer_queue
                .submit(&[command_buffer], &[], &[signal_semaphores])?;
            self.submission_index += 1;

            self.completed_images.push(image_request.image);

            // log::info!(
            //     "Submitted transfer commands for submission index {}",
            //     self.submission_index
            // );
        }

        Ok(())
    }

    pub fn new_image_upload_request_sender(&self) -> Sender<ImageUploadRequest> {
        self.image_upload_request_sender.clone()
    }

    /// Receives image upload requests from the channel
    fn receive_image_upload_requests(&mut self) {
        while !self.image_upload_request_receiver.is_empty() {
            self.image_upload_requests
                .push(self.image_upload_request_receiver.recv().unwrap());
        }
    }
}

impl Drop for TransferManager {
    fn drop(&mut self) {
        log::info!("Transfer Manager dropped!");
    }
}
