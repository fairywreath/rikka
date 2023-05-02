use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use crossbeam_channel::Sender;

use anyhow::{anyhow, Context, Result};
use ddsfile::{Dds, DxgiFormat};
use gltf::Gltf;

use rikka_core::{
    nalgebra::{Matrix4, Vector3, Vector4},
    vk,
};
use rikka_gpu::{
    self as gpu, buffer::*, constants::INVALID_BINDLESS_TEXTURE_INDEX, descriptor_set::*,
    escape::Handle, gpu::Gpu, image::*, sampler::*,
};

use crate::renderer::{loader::*, scene, MaterialData};

pub struct MeshDraw {
    pub position_buffer: Option<Handle<Buffer>>,
    pub index_buffer: Option<Handle<Buffer>>,
    pub tex_coords_buffer: Option<Handle<Buffer>>,
    pub normal_buffer: Option<Handle<Buffer>>,
    pub tangent_buffer: Option<Handle<Buffer>>,

    pub material_buffer: Option<Handle<Buffer>>,
    pub material_data: MaterialData,

    // Material data
    pub diffuse_texture: u32,
    pub omr_texture: u32,
    pub normal_texture: u32,

    pub base_color_factor: Vector4<f32>,
    pub omr_factor: Vector4<f32>,
    pub scale: Vector3<f32>,

    pub alpha_cutoff: f32,
    pub flags: u32,

    // XXX: Remove this
    pub textures_incomplete: bool,

    pub position_offset: u32,
    pub index_offset: u32,
    pub count: u32,
    pub tex_coords_offset: u32,
    pub normal_offset: u32,
    pub tangent_offset: u32,

    // XXX: Have a descriptor cache system(ideally inside rikka_gpu)
    pub descriptor_set: Option<Arc<DescriptorSet>>,

    pub scene_graph_node_index: usize,
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
                diffuse_texture: INVALID_BINDLESS_TEXTURE_INDEX,
                omr_texture: INVALID_BINDLESS_TEXTURE_INDEX,
                normal_texture: INVALID_BINDLESS_TEXTURE_INDEX,
            },

            position_offset: 0,
            index_offset: 0,
            count: 0,
            tex_coords_offset: 0,
            normal_offset: 0,
            tangent_offset: 0,

            descriptor_set: None,
            textures_incomplete: false,

            diffuse_texture: 0,
            omr_texture: 0,
            normal_texture: 0,

            base_color_factor: Vector4::default(),
            omr_factor: Vector4::default(),
            scale: Vector3::default(),

            alpha_cutoff: 0.0,
            flags: 0,

            scene_graph_node_index: usize::MAX,
        }
    }
}

pub struct GltfScene {
    pub mesh_draws: Vec<MeshDraw>,
    pub scene_graph: scene::Graph,
    pub _gpu_images: Vec<Handle<Image>>,
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
    fn create_image_from_file(gpu: &mut Gpu, file_name: &str) -> Result<Handle<Image>> {
        let data = std::fs::read(file_name)?;

        if let Ok(dds) = ddsfile::Dds::read(&mut std::io::Cursor::new(&data)) {
            let mut vulkan_format = vk::Format::UNDEFINED;

            if let Some(format) = dds.get_dxgi_format() {
                vulkan_format = dxgi_format_to_vulkan_format(format);
            } else if let Some(format) = dds.get_d3d_format() {
                todo!()
            }

            let image_desc = ImageDesc::new(dds.get_width(), dds.get_height(), 1)
                .set_format(vulkan_format)
                .set_usage_flags(vk::ImageUsageFlags::SAMPLED);
            let texture_image: Handle<Image> = gpu.create_image(image_desc)?.into();

            // XXX: Handle mip maps and texture layers

            let texture_data_bytes = dds.get_data(0)?;
            let texture_data_size = std::mem::size_of_val(texture_data_bytes);

            let staging_buffer = gpu.create_buffer(
                BufferDesc::new()
                    .set_device_only(false)
                    .set_size(texture_data_size as _)
                    .set_resource_usage(gpu::types::ResourceUsageType::Staging),
            )?;

            gpu.copy_data_to_image(texture_image.clone(), &staging_buffer, texture_data_bytes)?;

            Ok(texture_image)
        } else {
            let dynamic_image = image::load_from_memory(&data)?;

            let image_desc = ImageDesc::new(dynamic_image.width(), dynamic_image.height(), 1)
                .set_format(vk::Format::R8G8B8A8_UNORM)
                .set_usage_flags(vk::ImageUsageFlags::SAMPLED);
            let texture_image: Handle<Image> = gpu.create_image(image_desc)?.into();

            let texture_rgba8 = dynamic_image.clone().into_rgba8();
            let texture_data_bytes = texture_rgba8.as_raw();
            let texture_data_size = std::mem::size_of_val(texture_data_bytes.as_slice());

            let staging_buffer = gpu.create_buffer(
                BufferDesc::new()
                    .set_device_only(false)
                    .set_size(texture_data_size as _)
                    .set_resource_usage(gpu::types::ResourceUsageType::Staging),
            )?;

            gpu.copy_data_to_image(texture_image.clone(), &staging_buffer, texture_data_bytes)?;

            Ok(texture_image)
        }
    }

    fn create_image(
        gpu: &mut Gpu,
        file_name: &str,
        // XXX: Use a channel for this
        async_loader: &mut AsynchronousLoader,
    ) -> Result<Handle<Image>> {
        let data = std::fs::read(file_name)?;
        let mut data = std::io::Cursor::new(&data);

        // XXX: How slow is this read?
        if let Ok(dds) = ddsfile::Dds::read(&mut data) {
            let mut vulkan_format = vk::Format::UNDEFINED;

            if let Some(format) = dds.get_dxgi_format() {
                vulkan_format = dxgi_format_to_vulkan_format(format);
            } else if let Some(format) = dds.get_d3d_format() {
                todo!()
            }

            let image_desc = ImageDesc::new(dds.get_width(), dds.get_height(), 1)
                .set_format(vulkan_format)
                .set_usage_flags(vk::ImageUsageFlags::SAMPLED);
            let texture_image = Handle::from(gpu.create_image(image_desc)?);

            // XXX: Do this internally in the GPU
            gpu.add_bindless_image_update(rikka_gpu::types::ImageResourceUpdate {
                frame: 0,
                image: Some(texture_image.clone()),
                sampler: None,
            });

            async_loader.request_image_file_load(file_name, texture_image.clone());
            Ok(texture_image)
        } else {
            // log::info!("Attempting to read file {}", file_name);

            // let reader = image::io::Reader::new(data);
            let reader = image::io::Reader::open(file_name)?;

            // XXX: Use proper format instead of always converting to R8G8B8A_UNORM?
            // let format = reader.format()?;
            let format = vk::Format::R8G8B8A8_UNORM;

            let (width, height) = reader.into_dimensions()?;

            let image_desc = ImageDesc::new(width, height, 1)
                .set_format(format)
                .set_usage_flags(vk::ImageUsageFlags::SAMPLED);
            let texture_image = Handle::from(gpu.create_image(image_desc)?);

            // XXX: Do this internally in the GPU
            gpu.add_bindless_image_update(rikka_gpu::types::ImageResourceUpdate {
                frame: 0,
                image: Some(texture_image.clone()),
                sampler: None,
            });

            // log::info!("Finished (soft) reading file {}", file_name);
            async_loader.request_image_file_load(file_name, texture_image.clone());
            Ok(texture_image)
        }
    }

    fn load_images(
        gpu: &mut Gpu,
        root_path_buf: &PathBuf,
        images: gltf::iter::Images,
        // XXX: Use a channel for this
        async_loader: &mut AsynchronousLoader,
    ) -> Result<Vec<Handle<Image>>> {
        let mut gpu_images = Vec::with_capacity(images.len());

        let image_loading_start_time = Instant::now();

        for image in images {
            let gpu_image = match image.source() {
                gltf::image::Source::Uri { uri, .. } => {
                    let mut uri_path = root_path_buf.clone();
                    uri_path.push(uri);
                    // GltfScene::create_image_from_file(gpu, uri_path.to_str().unwrap())
                    GltfScene::create_image(gpu, uri_path.to_str().unwrap(), async_loader)
                }
                gltf::image::Source::View { view, .. } => {
                    panic!("glTF image loading from view not implemented!");
                }
            }?;

            gpu_images.push(gpu_image);
        }

        let image_loading_end_time = Instant::now();
        let image_loading_dt = image_loading_end_time - image_loading_start_time;
        log::info!("Image loading total time: {:?}", image_loading_dt);

        Ok(gpu_images)
    }

    fn load_samplers(gpu: &Gpu, samplers: gltf::iter::Samplers) -> Result<Vec<Handle<Sampler>>> {
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

            let gpu_sampler = Handle::from(gpu.create_sampler(sampler_desc)?);
            gpu_samplers.push(gpu_sampler);
        }

        Ok(gpu_samplers)
    }

    fn load_buffers_data(
        root_path_buf: &PathBuf,
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
                    let mut uri_path = root_path_buf.clone();
                    uri_path.push(uri);

                    let binary_data = std::fs::read(uri_path).context("Failed to read gltf uri")?;
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
        gpu: &mut Gpu,
        buffer_views: gltf::iter::Views,
        buffers_data: &[Vec<u8>],
    ) -> Result<Vec<Handle<Buffer>>> {
        let mut gpu_buffers = Vec::with_capacity(buffer_views.len());

        log::info!("Buffer views length {}", buffer_views.len());

        for buffer_view in buffer_views {
            let length = buffer_view.length();
            let range_start = buffer_view.offset();
            let range_end = range_start + length;

            let data = &buffers_data[buffer_view.buffer().index()][range_start..range_end];

            let staging_buffer = gpu.create_buffer(
                BufferDesc::new()
                    .set_size(length as _)
                    // .set_usage_flags(
                    //     vk::BufferUsageFlags::VERTEX_BUFFER | vk::BufferUsageFlags::INDEX_BUFFER,
                    // )
                    .set_device_only(false),
            )?;
            staging_buffer.copy_data_to_buffer(data)?;

            let gpu_buffer = gpu.create_buffer(
                BufferDesc::new()
                    .set_size(length as _)
                    .set_usage_flags(
                        vk::BufferUsageFlags::VERTEX_BUFFER | vk::BufferUsageFlags::INDEX_BUFFER,
                    )
                    .set_device_only(true),
            )?;
            gpu.copy_buffer(&staging_buffer, &gpu_buffer)?;

            gpu_buffers.push(Handle::from(gpu_buffer));
        }

        Ok(gpu_buffers)
    }

    pub fn from_file(
        gpu: &mut Gpu,
        file_name: &str,
        uniform_buffer: &Handle<Buffer>,
        descriptor_set_layout: &Handle<DescriptorSetLayout>,
        // XXX: Use a channel for this
        async_loader: &mut AsynchronousLoader,
    ) -> Result<Self> {
        let mut root_path_buf = PathBuf::from(file_name);
        root_path_buf.pop();

        let mut gltf_file = Gltf::open(file_name)?;

        let gpu_images =
            GltfScene::load_images(gpu, &root_path_buf, gltf_file.images(), async_loader)?;

        let gpu_samplers = GltfScene::load_samplers(gpu, gltf_file.samplers())?;

        let gltf_blob = gltf_file.blob.take();
        let buffers_data =
            GltfScene::load_buffers_data(&root_path_buf, gltf_file.buffers(), gltf_blob)?;

        log::info!("Buffers data length {}", buffers_data[0].len());

        let gpu_buffers = GltfScene::load_buffer_views(gpu, gltf_file.views(), &buffers_data)?;

        let gltf_meshes = gltf_file.meshes();
        let mut mesh_draws = Vec::with_capacity(gltf_meshes.len());

        log::info!("Meshes count: {}", gltf_meshes.len());

        // gLTF model can have multiple scene, but right now only 1 scene (the default one) is used
        // let gltf_scenes = gltf_file.scenes().collect::<Vec<_>>();

        // XXX: Do topological sort approach?
        let gltf_nodes = gltf_file.nodes();
        log::debug!("gLTF number of nodes: {}", gltf_nodes.len());

        let mut scene_graph = scene::Graph::with_num_nodes(gltf_nodes.len());

        // Set scene graph hierarchy with level order traversal/BFS
        let root_scene = gltf_file.default_scene().unwrap();
        log::debug!("gLTF default scene: {}", root_scene.index());

        let mut nodes_to_visit = VecDeque::new();
        for node in root_scene.nodes() {
            scene_graph.set_hierarchy(node.index(), scene::INVALID_INDEX, 0);
            nodes_to_visit.push_back(node);
        }

        let mut current_level = 0;
        while !nodes_to_visit.is_empty() {
            let node = nodes_to_visit.pop_front().unwrap();

            // Find to set this now as we will be traversing all of the nodes
            scene_graph.set_local_matrix(node.index(), Matrix4::from(node.transform().matrix()));

            current_level += 1;
            for child in node.children() {
                scene_graph.set_hierarchy(child.index(), node.index(), current_level);
                nodes_to_visit.push_back(child);
            }

            let mesh = node.mesh().unwrap();
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
                    // log::info!(
                    //     "Mesh index {} indices count {}",
                    //     primitive.index(),
                    //     mesh_draw.count
                    // );
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

                    // log::info!("Contains tangents!");
                }
                // else {
                // log::info!("Does not contain tangents! index {}", primitive.index());
                // return Err(anyhow!(r#"glTF tangents accessor does not exist!"#));
                // }

                let material = primitive.material();
                let pbr_material = material.pbr_metallic_roughness();

                let mut diffuse_image = None;

                if let Some(diffuse_info) = pbr_material.base_color_texture() {
                    let diffuse_texture = diffuse_info.texture();
                    diffuse_image = Some(gpu_images[diffuse_texture.source().index()].clone());

                    // XXX: Handle samplers properly
                    // let diffuse_sampler = gpu_samplers[diffuse_texture.sampler().index()].clone();
                } else {
                    log::info!(
                        "Does not contain base color texture! primitive index {}, material index {}",
                        primitive.index(), material.index().unwrap(),
                    );

                    // XXX: Use a default texture or use a different shader pipeline
                    // mesh_draw.textures_incomplete = true;
                    // mesh_draws.push(mesh_draw);
                    continue;
                }

                let mut omr_image = None;
                if let Some(omr_info) = pbr_material.metallic_roughness_texture() {
                    let omr_texture = omr_info.texture();
                    omr_image = Some(gpu_images[omr_texture.source().index()].clone());
                } else {
                    log::info!(
                        "Does not contain metallic roughness texture! primitive index {}",
                        primitive.index()
                    );

                    // XXX: Use a default texture or use a different shader pipeline
                    // mesh_draw.textures_incomplete = true;
                    // mesh_draws.push(mesh_draw);
                    continue;
                }

                let mut normal_image = None;
                if let Some(normal_info) = material.normal_texture() {
                    let normal_texture = normal_info.texture();
                    normal_image = Some(gpu_images[normal_texture.source().index()].clone());
                } else {
                    log::info!(
                        "Does not contain normal texture! index {}",
                        primitive.index()
                    );

                    // XXX: Use a default texture or use a different shader pipeline
                    // mesh_draw.textures_incomplete = true;
                    // mesh_draws.push(mesh_draw);
                    // continue;
                }

                mesh_draw.material_data = MaterialData {
                    base_color_factor: pbr_material.base_color_factor().into(),
                    diffuse_texture: diffuse_image.clone().unwrap().bindless_index(),
                    omr_texture: omr_image.clone().unwrap().bindless_index(),
                    normal_texture: normal_image.clone().unwrap().bindless_index(),
                };
                let material_buffer = gpu.create_buffer(
                    BufferDesc::new()
                        .set_size(std::mem::size_of::<MaterialData>() as _)
                        .set_usage_flags(vk::BufferUsageFlags::UNIFORM_BUFFER)
                        .set_device_only(false),
                )?;
                material_buffer
                    .copy_data_to_buffer(std::slice::from_ref(&mesh_draw.material_data))?;
                mesh_draw.material_buffer = Some(Handle::from(material_buffer));
                // log::info!(
                //     "Primitive diffuse texture index: {}",
                //     mesh_draw.material_data.diffuse_texture,
                // );

                let binding_resources = vec![
                    DescriptorSetBindingResource::buffer(uniform_buffer.clone(), 0),
                    DescriptorSetBindingResource::buffer(
                        mesh_draw.material_buffer.clone().unwrap(),
                        4,
                    ),
                ];

                mesh_draw.descriptor_set = Some(Arc::new(
                    gpu.create_descriptor_set(
                        DescriptorSetDesc::new(descriptor_set_layout.clone())
                            .set_binding_resources(binding_resources),
                    )?,
                ));

                mesh_draw.scene_graph_node_index = node.index();

                mesh_draws.push(mesh_draw);
            }
        }

        Ok(Self {
            mesh_draws,
            scene_graph,
            _gpu_images: gpu_images,
        })
    }
}
