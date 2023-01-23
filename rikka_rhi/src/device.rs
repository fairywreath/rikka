use ash::vk;

use crate::deletion_queue::DeferredDeletionQueue;

pub struct Device {
    deletion_queue: DeferredDeletionQueue,
}

impl Device {
    pub(crate) fn get_deletion_queue(&mut self) -> &mut DeferredDeletionQueue {
        &mut self.deletion_queue
    }
}
