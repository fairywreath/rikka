use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{anyhow, Context, Result};
use ddsfile::{Dds, DxgiFormat};
use gltf::Gltf;
use nalgebra::{Vector3, Vector4};

use rikka_gpu::{
    self as gpu, ash::vk, buffer::*, descriptor_set::*, gpu::Gpu, image::*, sampler::*,
};

pub struct MaterialData {
    pub base_color_factor: Vector4<f32>,
}

type BufferHandle = Arc<Buffer>;
type ImageHandle = Arc<Image>;
type SamplerHandle = Arc<Sampler>;
type DescriptorSetHandle = Arc<DescriptorSet>;

pub struct MeshDraw {
    pub position_buffer: Option<BufferHandle>,
    pub index_buffer: Option<BufferHandle>,
    pub tex_coords_buffer: Option<BufferHandle>,
    pub normal_buffer: Option<BufferHandle>,
    pub tangent_buffer: Option<BufferHandle>,

    pub material_buffer: Option<BufferHandle>,
    pub material_data: MaterialData,

    pub position_offset: u32,
    pub index_offset: u32,
    pub count: u32,
    pub tex_coords_offset: u32,
    pub normal_offset: u32,
    pub tangent_offset: u32,

    // XXX: Have a descriptor cache system(ideally inside rikka_gpu)
    pub descriptor_set: Option<DescriptorSet>,
}

impl Default for MeshDraw {
    fn default() -> Self {
        MeshDraw {
            position_buffer: None,
            index_buffer: None,
            tex_coords_buffer: None,
            normal_buffer: None,
            tangent_buffer: None,

            material_buffer: None,
            material_data: MaterialData {
                base_color_factor: Vector4::new(0.0, 0.0, 0.0, 0.0),
            },

            position_offset: 0,
            index_offset: 0,
            count: 0,
            tex_coords_offset: 0,
            normal_offset: 0,
            tangent_offset: 0,

            descriptor_set: None,
        }
    }
}

pub struct GltfScene {
    pub gpu_buffers: Vec<BufferHandle>,
    pub gpu_images: Vec<ImageHandle>,
    pub gpu_samplers: Vec<SamplerHandle>,

    pub buffers_data: Vec<Vec<u8>>,

    pub mesh_draws: Vec<MeshDraw>,

    // XXX: Make nice descriptor set cache system in GPU
    pub descriptor_set_layout: Arc<DescriptorSetLayout>,
}

fn dxgi_format_to_vulkan_format(dxgi_format: DxgiFormat) -> vk::Format {
    match dxgi_format {
        DxgiFormat::BC1_UNorm => vk::Format::BC1_RGBA_UNORM_BLOCK,
        DxgiFormat::BC1_UNorm_sRGB => vk::Format::BC1_RGBA_SRGB_BLOCK,
        DxgiFormat::BC3_UNorm => vk::Format::BC3_UNORM_BLOCK,
        DxgiFormat::BC3_UNorm_sRGB => vk::Format::BC3_SRGB_BLOCK,
        DxgiFormat::BC5_UNorm => vk::Format::BC5_UNORM_BLOCK,
        _ => todo!(),
    }
}

fn gltf_min_filter_to_vulkan_filter(gltf_filter: gltf::texture::MinFilter) -> vk::Filter {
    match gltf_filter {
        gltf::texture::MinFilter::Linear
        | gltf::texture::MinFilter::LinearMipmapLinear
        | gltf::texture::MinFilter::LinearMipmapNearest => vk::Filter::LINEAR,

        gltf::texture::MinFilter::Nearest
        | gltf::texture::MinFilter::NearestMipmapLinear
        | gltf::texture::MinFilter::NearestMipmapNearest => vk::Filter::NEAREST,
    }
}

fn gltf_mag_filter_to_vulkan_filter(gltf_filter: gltf::texture::MagFilter) -> vk::Filter {
    match gltf_filter {
        gltf::texture::MagFilter::Linear => vk::Filter::LINEAR,
        gltf::texture::MagFilter::Nearest => vk::Filter::NEAREST,
    }
}

impl GltfScene {
    // XXX: Mut is required here for the copy function, it does not need to be mut however(change in gpu layer)
    fn create_image_from_file(gpu: &Gpu, file_name: &str) -> Result<Image> {
        // log::info!("Loading texture {}...", file_name,);

        // XXX: Handle this nicely...
        let mut relative_uri = String::from("assets/SunTemple-glTF/");
        // let mut relative_uri = String::from("assets/Sponza/glTF/");
        relative_uri.push_str(file_name);

        let data = std::fs::read(relative_uri.clone())?;

        if let Ok(dds) = ddsfile::Dds::read(&mut std::io::Cursor::new(&data)) {
            let mut vulkan_format = vk::Format::UNDEFINED;

            if let Some(format) = dds.get_dxgi_format() {
                log::info!("Format is DXGI {:?}", format);
                vulkan_format = dxgi_format_to_vulkan_format(format);
            } else if let Some(format) = dds.get_d3d_format() {
                log::info!("Format is D3D {:?}", format);
                todo!()
            }

            let image_desc = ImageDesc::new(dds.get_width(), dds.get_height(), 1)
                .set_format(vulkan_format)
                .set_usage_flags(vk::ImageUsageFlags::SAMPLED);
            let texture_image = gpu.create_image(image_desc)?;

            // XXX: Handle mip maps and texture layers

            let texture_data_bytes = dds.get_data(0)?;
            let texture_data_size = std::mem::size_of_val(texture_data_bytes);

            let staging_buffer = gpu.create_buffer(
                BufferDesc::new()
                    .set_device_only(false)
                    .set_size(texture_data_size as _)
                    .set_resource_usage(gpu::types::ResourceUsageType::Staging),
            )?;

            gpu.copy_data_to_image(&texture_image, &staging_buffer, texture_data_bytes)?;

            log::info!("Finished loading image {}", file_name);

            Ok(texture_image)
        } else {
            let dynamic_image = image::load_from_memory(&data)?;

            let image_desc = ImageDesc::new(dynamic_image.width(), dynamic_image.height(), 1)
                .set_format(vk::Format::R8G8B8A8_UNORM)
                .set_usage_flags(vk::ImageUsageFlags::SAMPLED);
            let texture_image = gpu.create_image(image_desc)?;

            let texture_rgba8 = dynamic_image.clone().into_rgba8();
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

            // log::info!("Finished loading image {}", file_name);

            Ok(texture_image)
        }
    }

    fn load_images(gpu: &Gpu, images: gltf::iter::Images) -> Result<Vec<Image>> {
        let mut gpu_images = Vec::with_capacity(images.len());

        let image_loading_start_time = Instant::now();

        for image in images {
            let gpu_image = match image.source() {
                gltf::image::Source::Uri { uri, .. } => GltfScene::create_image_from_file(gpu, uri),
                gltf::image::Source::View { view, .. } => {
                    todo!()
                }
            }?;

            gpu_images.push(gpu_image);
        }

        let image_loading_end_time = Instant::now();
        let image_loading_dt = image_loading_end_time - image_loading_start_time;
        log::info!("Image loading total time: {:?}", image_loading_dt);

        Ok(gpu_images)
    }

    fn load_samplers(gpu: &Gpu, samplers: gltf::iter::Samplers) -> Result<Vec<SamplerHandle>> {
        let mut gpu_samplers = Vec::with_capacity(samplers.len());

        for sampler in samplers {
            let sampler_desc = SamplerDesc::new()
                .set_min_filter(gltf_min_filter_to_vulkan_filter(
                    sampler
                        .min_filter()
                        .unwrap_or(gltf::texture::MinFilter::Linear),
                ))
                .set_mag_filter(gltf_mag_filter_to_vulkan_filter(
                    sampler
                        .mag_filter()
                        .unwrap_or(gltf::texture::MagFilter::Linear),
                ));

            let gpu_sampler = Arc::new(gpu.create_sampler(sampler_desc)?);
            gpu_samplers.push(gpu_sampler);
        }

        Ok(gpu_samplers)
    }

    fn load_buffers_data(
        buffers: gltf::iter::Buffers,
        blob: Option<Vec<u8>>,
    ) -> Result<Vec<Vec<u8>>> {
        let mut buffers_data = Vec::with_capacity(buffers.len());
        let mut blob_index = None;

        log::info!("Gltf buffers length: {}", buffers.len());

        for buffer in buffers {
            let data = match buffer.source() {
                gltf::buffer::Source::Bin => {
                    blob_index = Some(buffer.index());
                    Vec::<u8>::new()
                }
                gltf::buffer::Source::Uri(uri) => {
                    // XXX: Get relative directory...
                    let mut relative_uri = String::from("assets/SunTemple-glTF/");
                    // let mut relative_uri = String::from("assets/Sponza/glTF/");
                    relative_uri.push_str(uri);

                    let binary_data =
                        std::fs::read(relative_uri).context("Failed to read gltf uri")?;
                    binary_data
                }
            };

            buffers_data.push(data);
        }

        if let Some(blob_index) = blob_index {
            buffers_data[blob_index] = blob.expect("Global blob not found");
        }

        Ok(buffers_data)
    }

    fn load_buffer_views(
        gpu: &Gpu,
        buffer_views: gltf::iter::Views,
        buffers_data: &[Vec<u8>],
    ) -> Result<Vec<BufferHandle>> {
        let mut gpu_buffers = Vec::with_capacity(buffer_views.len());

        log::info!("Buffer views length {}", buffer_views.len());

        for buffer_view in buffer_views {
            let length = buffer_view.length();
            let range_start = buffer_view.offset();
            let range_end = range_start + length;

            let data = &buffers_data[buffer_view.buffer().index()][range_start..range_end];

            // log::debug!("Buffer view {} data: {:?}", buffer_view.index(), data);

            let gpu_buffer = gpu.create_buffer(
                BufferDesc::new()
                    .set_size(length as _)
                    .set_usage_flags(
                        vk::BufferUsageFlags::VERTEX_BUFFER | vk::BufferUsageFlags::INDEX_BUFFER,
                    )
                    .set_device_only(false),
            )?;
            gpu_buffer.copy_data_to_buffer(data)?;

            gpu_buffers.push(Arc::new(gpu_buffer));
        }

        Ok(gpu_buffers)
    }

    fn load_materials() {
        todo!()
    }

    fn load_meshes(gpu: &Gpu, meshes: gltf::iter::Meshes) -> Result<Vec<MeshDraw>> {
        todo!()
    }

    // XXX: Make descriptor set management nicer
    pub fn from_file(gpu: &Gpu, file_name: &str, uniform_buffer: &Arc<Buffer>) -> Result<Self> {
        let mut gltf_file = Gltf::open(file_name)?;

        // XXX: Integrate this insde GPU?
        let default_sampler = Arc::new(gpu.create_sampler(SamplerDesc::new())?);

        let mut gpu_images = GltfScene::load_images(gpu, gltf_file.images())?;
        for image in &mut gpu_images {
            image.set_linked_sampler(default_sampler.clone());
        }

        let gpu_images = gpu_images
            .into_iter()
            .map(|image| Arc::new(image))
            .collect::<Vec<_>>();

        let gpu_samplers = GltfScene::load_samplers(gpu, gltf_file.samplers())?;

        let gltf_blob = gltf_file.blob.take();
        let buffers_data = GltfScene::load_buffers_data(gltf_file.buffers(), gltf_blob)?;

        let gpu_buffers = GltfScene::load_buffer_views(gpu, gltf_file.views(), &buffers_data)?;

        // let gltf_materials = gltf_file.materials();

        let descriptor_set_layout = gpu
            .create_descriptor_set_layout(
                DescriptorSetLayoutDesc::new()
                    .add_binding(DescriptorBinding::new(
                        vk::DescriptorType::UNIFORM_BUFFER,
                        0,
                        1,
                        vk::ShaderStageFlags::VERTEX,
                    ))
                    .add_binding(DescriptorBinding::new(
                        vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                        1,
                        1,
                        vk::ShaderStageFlags::FRAGMENT,
                    )),
            )
            .unwrap();
        let descriptor_set_layout = Arc::new(descriptor_set_layout);

        let gltf_meshes = gltf_file.meshes();
        let mut mesh_draws = Vec::with_capacity(gltf_meshes.len());

        log::info!("Meshes count: {}", gltf_meshes.len());

        for mesh in gltf_meshes {
            for primitive in mesh.primitives() {
                let mut mesh_draw = MeshDraw::default();

                if primitive.mode() != gltf::mesh::Mode::Triangles {
                    return Err(anyhow!(
                        "glTF primitive mode is not TRIANGLES, only TRIANGLES is supported"
                    ));
                }

                if let Some(positions_accessor) = primitive.get(&gltf::Semantic::Positions) {
                    let buffer_view = positions_accessor.view().unwrap();
                    mesh_draw.position_buffer = Some(gpu_buffers[buffer_view.index()].clone());
                    mesh_draw.position_offset = positions_accessor.offset() as _;
                } else {
                    return Err(anyhow!("glTF positions accessor does not exist!"));
                }

                if let Some(indices_accessor) = primitive.indices() {
                    let buffer_view = indices_accessor.view().unwrap();
                    mesh_draw.index_buffer = Some(gpu_buffers[buffer_view.index()].clone());
                    mesh_draw.index_offset = indices_accessor.offset() as _;
                    mesh_draw.count = indices_accessor.count() as _;
                } else {
                    return Err(anyhow!("glTF indices accessor does not exist!"));
                }

                if let Some(tex_coords_accessor) = primitive.get(&gltf::Semantic::TexCoords(0)) {
                    let buffer_view = tex_coords_accessor.view().unwrap();
                    mesh_draw.tex_coords_buffer = Some(gpu_buffers[buffer_view.index()].clone());
                    mesh_draw.tex_coords_offset = tex_coords_accessor.offset() as _;
                } else {
                    return Err(anyhow!(
                        "glTF texture coordinates 0 accessor does not exist!"
                    ));
                }

                if let Some(normals_accessor) = primitive.get(&gltf::Semantic::Normals) {
                    let buffer_view = normals_accessor.view().unwrap();
                    mesh_draw.normal_buffer = Some(gpu_buffers[buffer_view.index()].clone());
                    mesh_draw.normal_offset = normals_accessor.offset() as _;
                } else {
                    return Err(anyhow!("glTF normals accessor does not exist!"));
                }

                if let Some(tangents_accessor) = primitive.get(&gltf::Semantic::Tangents) {
                    let buffer_view = tangents_accessor.view().unwrap();
                    mesh_draw.tangent_buffer = Some(gpu_buffers[buffer_view.index()].clone());
                    mesh_draw.tangent_offset = tangents_accessor.offset() as _;
                } else {
                    // return Err(anyhow!(r#"glTF tangents accessor does not exist!"#));
                }

                let material = primitive.material();
                let pbr_material = material.pbr_metallic_roughness();

                let mut diffuse_image = None;

                if let Some(diffuse_info) = pbr_material.base_color_texture() {
                    let diffuse_texture = diffuse_info.texture();
                    diffuse_image = Some(gpu_images[diffuse_texture.source().index()].clone());

                    // XXX: Handle samplers properly
                    // let diffuse_sampler = gpu_samplers[diffuse_texture.sampler().index()].clone();
                } else {
                    todo!()
                }

                let binding_resources = vec![
                    DescriptorSetBindingResource::buffer(uniform_buffer.clone(), 0),
                    DescriptorSetBindingResource::image(diffuse_image.unwrap(), 1),
                ];

                mesh_draw.descriptor_set = Some(
                    gpu.create_descriptor_set(
                        DescriptorSetDesc::new(descriptor_set_layout.clone())
                            .set_binding_resources(binding_resources),
                    )?,
                );

                mesh_draws.push(mesh_draw);
            }
        }

        Ok(Self {
            gpu_buffers,
            gpu_images,
            gpu_samplers,

            buffers_data,
            mesh_draws,

            descriptor_set_layout,
        })
    }

    // XXX: Have function to create pipeline vertex input, etc. This should be handled by the shader reflection system no?
}
