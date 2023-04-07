use anyhow::Result;
use crossbeam_channel::{Receiver, Sender};

use rikka_gpu::{escape::Handle, image::Image, transfer::ImageUploadRequest};

struct ImageFileLoadRequest {
    file_name: String,
    image: Handle<Image>,
}

pub struct AsynchronousLoader {
    image_file_load_requests: Vec<ImageFileLoadRequest>,
    image_file_load_complete_sender: Sender<ImageUploadRequest>,
}

fn load_image_data(file_name: &str) -> Result<Vec<u8>> {
    let data = std::fs::read(file_name)?;

    if let Ok(dds) = ddsfile::Dds::read(&mut std::io::Cursor::new(&data)) {
        Ok(dds.get_data(0)?.to_vec())
    } else {
        let dynamic_image = image::load_from_memory(&data)?;
        // XXX: How expensive/slow is this? Maybe this conversion should not be done at all
        let texture_rgba8 = dynamic_image.clone().into_rgba8();

        // log::info!(
        //     "Loaded image {} with size {}",
        //     file_name,
        //     texture_rgba8.as_raw().len()
        // );

        Ok(texture_rgba8.as_raw().clone())
    }
}

impl AsynchronousLoader {
    pub fn new(image_file_load_complete_sender: Sender<ImageUploadRequest>) -> Self {
        AsynchronousLoader {
            image_file_load_requests: Vec::new(),
            image_file_load_complete_sender,
        }
    }

    // XXX: Use a channel to request
    pub fn request_image_file_load(&mut self, file_name: &str, image: Handle<Image>) {
        self.image_file_load_requests.push(ImageFileLoadRequest {
            file_name: file_name.to_string(),
            image,
        })
    }

    /// Called periodically
    pub fn update(&mut self) -> Result<()> {
        if let Some(image_request) = self.image_file_load_requests.pop() {
            let image_data = load_image_data(image_request.file_name.as_str())?;
            self.image_file_load_complete_sender
                .send(ImageUploadRequest {
                    image: image_request.image,
                    data: image_data,
                })?;
        }

        Ok(())
    }
}
