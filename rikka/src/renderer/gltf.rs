use std::sync::Arc;

use anyhow::{Context, Result};
use gltf::Gltf;
use nalgebra::Vector4;

use rikka_gpu::{self as gpu, ash::vk, buffer::*, gpu::Gpu, image::*, sampler::*};

pub struct MaterialData {
    pub base_color_factor: Vector4<f32>,
}

type BufferHandle = Arc<Buffer>;
type ImageHandle = Arc<Image>;
type SamplerHandle = Arc<Sampler>;

pub struct MeshDraw {
    pub position_buffer: Option<BufferHandle>,
    pub index_buffer: Option<BufferHandle>,
    pub tex_coord_buffer: Option<BufferHandle>,
    pub normal_buffer: Option<BufferHandle>,
    pub tangent_buffer: Option<BufferHandle>,

    pub material_buffer: Option<BufferHandle>,
    pub material_data: MaterialData,

    pub position_offset: u32,
    pub index_offset: u32,
    pub texcoord_offset: u32,
    pub normal_offset: u32,
    pub tangent_offset: u32,

    pub count: u32,
}

pub struct GltfScene {
    pub gpu_buffers: Vec<BufferHandle>,
    pub gpu_images: Vec<ImageHandle>,
    pub gpu_samplers: Vec<SamplerHandle>,

    pub buffer_data: Vec<Vec<u8>>,
}

impl GltfScene {
    fn create_image_from_file(gpu: &mut Gpu, file_name: &str) -> Result<ImageHandle> {
        log::info!("Loading texture {}...", file_name,);

        let mut relative_uri = String::from("assets/SunTemple-glTF/");
        relative_uri.push_str(file_name);
        let texture_data = image::open(relative_uri).context("Failed to open image file")?;

        let image_desc = ImageDesc::new(texture_data.width(), texture_data.height(), 1)
            .set_format(vk::Format::R8G8B8A8_UNORM)
            .set_usage_flags(vk::ImageUsageFlags::SAMPLED);
        let texture_image = gpu.create_image(image_desc)?;

        let texture_rgba8 = texture_data.clone().into_rgba8();
        let texture_data_bytes = texture_rgba8.as_raw();
        let texture_data_size = std::mem::size_of_val(texture_data_bytes.as_slice());

        // log::info!(
        //     "Texture data size: {:?}, dimensions: {:?}",
        //     texture_data_size,
        // );

        let staging_buffer = gpu.create_buffer(
            BufferDesc::new()
                .set_device_only(false)
                .set_size(texture_data_size as _)
                .set_resource_usage(gpu::types::ResourceUsageType::Staging),
        )?;

        gpu.copy_data_to_image(&texture_image, &staging_buffer, texture_data_bytes)?;

        log::info!("Finished loading image {}", file_name);

        Ok(Arc::new(texture_image))
    }

    pub fn from_file(gpu: &mut Gpu, file_name: &str) -> Result<Self> {
        let mut gltf_file = Gltf::open(file_name)?;

        let gltf_blob = gltf_file.blob.take();

        let gltf_buffers = gltf_file.buffers();
        let mut buffer_data = Vec::with_capacity(gltf_buffers.len());
        let mut blob_index = None;

        for buffer in gltf_buffers {
            let data = match buffer.source() {
                gltf::buffer::Source::Bin => {
                    blob_index = Some(buffer.index());
                    Vec::<u8>::new()
                }
                gltf::buffer::Source::Uri(uri) => {
                    // XXX: Get relative directory...
                    let mut relative_uri = String::from("assets/SunTemple-glTF/");
                    relative_uri.push_str(uri);

                    let binary_data =
                        std::fs::read(relative_uri).context("Failed to read gltf uri")?;
                    binary_data
                }
            };

            buffer_data.push(data);
        }

        if let Some(blob_index) = blob_index {
            buffer_data[blob_index] = gltf_blob.expect("Global blob not found");
        }

        // let materials = gltf_file.materials();
        // log::info!("Number of materials: {}", materials.len());

        // for material in materials {}

        let gltf_images = gltf_file.images();
        let mut gpu_images = Vec::with_capacity(gltf_images.len());

        for image in gltf_images {
            let gpu_image = match image.source() {
                gltf::image::Source::Uri { uri, .. } => GltfScene::create_image_from_file(gpu, uri),
                gltf::image::Source::View { view, .. } => {
                    todo!()
                }
            }?;

            gpu_images.push(gpu_image);
        }

        let samplers = gltf_file.samplers();
        log::info!("Samplers length: {}", samplers.len());

        Ok(Self {
            gpu_buffers: vec![],
            gpu_images,
            gpu_samplers: vec![],

            buffer_data,
        })
    }
}
