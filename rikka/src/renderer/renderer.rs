use anyhow::Result;

use rikka_gpu::{self as gpu, buffer::*, descriptor_set::*, image::*, pipeline::*, sampler::*};

pub struct MaterialDesc {}

pub struct Material {}

pub struct Renderer {}

impl Renderer {
    pub fn create_buffer(desc: BufferDesc) -> Result<Buffer> {
        todo!()
    }
}
